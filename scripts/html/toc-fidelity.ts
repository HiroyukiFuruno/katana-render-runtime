import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import type { JsonValue } from "./dynamic-state";
import {
  assertExpectedInputSha,
  compareImages,
  type ScoreReport,
  sha256File,
} from "./reference-capture";

export interface TocEntry {
  destination: string;
  hash: string;
  scrollY: number;
  capturePath: string;
  targetVisible: boolean;
}

export interface TocComparison {
  destination: string;
  krrScrollY: number;
  chromeScrollY: number;
  score: ScoreReport;
  passed: boolean;
  outputDir: string;
  error?: string;
}

export function tocProbePath(binary: string): string {
  return path.join(
    path.dirname(binary),
    `html_toc_probe${process.platform === "win32" ? ".exe" : ""}`,
  );
}

export function validateTocEntries(value: JsonValue | TocEntry[]): TocEntry[] {
  if (!Array.isArray(value)) throw new Error("TOC report entries must be an array");
  const destinations = new Set<string>();
  return value.map((item) => {
    if (typeof item !== "object" || item === null) throw new Error("Invalid TOC entry");
    const entry = item as Record<string, JsonValue>;
    if (
      typeof entry.destination !== "string" ||
      entry.destination.length === 0 ||
      typeof entry.hash !== "string" ||
      entry.hash !== `#${entry.destination}` ||
      typeof entry.scrollY !== "number" ||
      !Number.isFinite(entry.scrollY) ||
      entry.scrollY < 0 ||
      typeof entry.capturePath !== "string" ||
      entry.capturePath.length === 0 ||
      entry.targetVisible !== true
    )
      throw new Error("Invalid or invisible TOC destination");
    if (destinations.has(entry.destination)) throw new Error("Duplicate TOC destination");
    destinations.add(entry.destination);
    return {
      destination: entry.destination,
      hash: entry.hash,
      scrollY: entry.scrollY,
      capturePath: entry.capturePath,
      targetVisible: true,
    };
  });
}

export function matchTocEntries(chrome: TocEntry[], krr: TocEntry[]): void {
  if (
    chrome.length !== krr.length ||
    chrome.some(
      (entry, index) =>
        entry.destination !== krr[index]?.destination || entry.hash !== krr[index]?.hash,
    )
  ) {
    throw new Error("KRR and Chrome TOC destination coverage differs");
  }
}

export function compareTocCaptures(
  chrome: TocEntry[],
  krr: TocEntry[],
  minScore: number,
): TocComparison[] {
  matchTocEntries(chrome, krr);
  return chrome.map((reference, index) => {
    const actual = krr[index];
    if (!actual) throw new Error("Missing KRR TOC capture");
    const score = compareImages(reference.capturePath, actual.capturePath, minScore);
    const scrollMatches = Math.abs(actual.scrollY - reference.scrollY) < 0.01;
    const passed = score.passed && scrollMatches;
    return {
      destination: reference.destination,
      krrScrollY: actual.scrollY,
      chromeScrollY: reference.scrollY,
      score,
      passed,
      outputDir: path.dirname(actual.capturePath),
      ...(passed
        ? {}
        : {
            error: `TOC ${reference.destination}: score=${score.score}, scroll=${actual.scrollY}/${reference.scrollY}`,
          }),
    };
  });
}

export function runTocProbe(
  binary: string,
  input: string,
  output: string,
  viewport: { width: number; height: number },
  expectedHtmlSha256: string,
  expectedBinarySha256: string,
): TocEntry[] {
  if (sha256File(binary) !== expectedBinarySha256)
    throw new Error("TOC probe binary changed during capture");
  const nonce = randomUUID();
  const result = Bun.spawnSync(
    [binary, "writes_fixture_toc_diagnostic_when_requested", "--nocapture"],
    {
      env: {
        ...process.env,
        KRR_HTML_TOC_DIAGNOSTIC_INPUT: input,
        KRR_HTML_TOC_DIAGNOSTIC_OUTPUT: output,
        KRR_HTML_TOC_DIAGNOSTIC_WIDTH: String(viewport.width),
        KRR_HTML_TOC_DIAGNOSTIC_HEIGHT: String(viewport.height),
        KRR_HTML_TOC_DIAGNOSTIC_SCALE: "1",
        KRR_HTML_TOC_DIAGNOSTIC_NONCE: nonce,
      },
    },
  );
  if (result.exitCode !== 0)
    throw new Error(`TOC probe failed: ${new TextDecoder().decode(result.stderr)}`);
  if (sha256File(binary) !== expectedBinarySha256)
    throw new Error("TOC probe binary changed during capture");
  assertExpectedInputSha(expectedHtmlSha256, sha256File(input));
  const value: JsonValue = JSON.parse(
    fs.readFileSync(path.join(output, "toc-report.json"), "utf8"),
  );
  if (typeof value !== "object" || value === null) throw new Error("Invalid TOC report");
  const report = value as Record<string, JsonValue>;
  if (report.executionNonce !== nonce) throw new Error("Stale TOC execution report");
  if (!new TextDecoder().decode(result.stdout).includes("test result: ok. 1 passed")) {
    throw new Error("TOC runtime diagnostic did not execute exactly one test");
  }
  if (typeof report.htmlSha256 !== "string") throw new Error("Missing TOC source hash");
  assertExpectedInputSha(expectedHtmlSha256, report.htmlSha256);
  const reportedViewport = report.viewport;
  if (typeof reportedViewport !== "object" || reportedViewport === null)
    throw new Error("Missing TOC viewport");
  const dimensions = reportedViewport as Record<string, JsonValue>;
  if (
    dimensions.width !== viewport.width ||
    dimensions.height !== viewport.height ||
    dimensions.scale !== 1
  ) {
    throw new Error("TOC capture viewport differs");
  }
  if (report.entries === undefined) throw new Error("Missing TOC entries");
  return validateTocEntries(report.entries);
}

function runtimeTestExecutable(value: JsonValue): string | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const report = value as Record<string, JsonValue>;
  if (report.reason !== "compiler-artifact" || typeof report.executable !== "string")
    return undefined;
  if (typeof report.target !== "object" || report.target === null) return undefined;
  if (typeof report.profile !== "object" || report.profile === null) return undefined;
  const target = report.target as Record<string, JsonValue>;
  const profile = report.profile as Record<string, JsonValue>;
  if (target.name !== "katana_render_runtime" || profile.test !== true) return undefined;
  if (!Array.isArray(target.kind) || !target.kind.includes("lib")) return undefined;
  return report.executable;
}

function buildReportExecutables(reportPath: string): string[] {
  return fs
    .readFileSync(reportPath, "utf8")
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => runtimeTestExecutable(JSON.parse(line) as JsonValue))
    .filter((candidate): candidate is string => candidate !== undefined);
}

function onlyBuildExecutable(candidates: string[]): string {
  if (candidates.length !== 1 || !candidates[0])
    throw new Error("Expected one KRR runtime test executable");
  return candidates[0];
}

export function tocBinaryPathFromBuildReport(reportPath: string): string {
  const executable = onlyBuildExecutable(buildReportExecutables(reportPath));
  if (!path.posix.isAbsolute(executable) && !path.win32.isAbsolute(executable))
    throw new Error("Expected an absolute KRR runtime test executable");
  return executable;
}

export function tocBinaryFromBuildReport(reportPath: string): string {
  return onlyBuildExecutable(
    buildReportExecutables(reportPath)
      .map((candidate) => path.resolve(candidate))
      .filter((candidate) => fs.existsSync(candidate) && fs.statSync(candidate).isFile()),
  );
}
