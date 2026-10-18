import fs from "node:fs";
import path from "node:path";
import { type JsonValue, jsonRecords } from "./dynamic-state";
import { compareImages, sha256File } from "./reference-capture";

interface Options {
  krrRoot: string;
  chromeRoot: string;
  binarySha256: string;
  binaryPath: string;
  manifestPath: string;
  output: string;
  krrPrefix: string;
  chromePrefix: string;
}

function options(argv: string[]): Options {
  const value = (name: string): string => {
    const index = argv.indexOf(name);
    const result = argv.at(index + 1);
    if (index < 0 || result === undefined || result.startsWith("--"))
      throw new Error(`missing ${name}`);
    return result;
  };
  return {
    krrRoot: path.resolve(value("--krr-root")),
    chromeRoot: path.resolve(value("--chrome-root")),
    binarySha256: value("--binary-sha256"),
    binaryPath: path.resolve(value("--binary-path")),
    manifestPath: path.resolve(value("--manifest")),
    output: path.resolve(value("--output")),
    krrPrefix: argv.includes("--krr-prefix") ? value("--krr-prefix") : "current-toc",
    chromePrefix: argv.includes("--chrome-prefix") ? value("--chrome-prefix") : "full-png-current",
  };
}

function read(file: string): { [key: string]: JsonValue } {
  const value = JSON.parse(fs.readFileSync(file, "utf8")) as JsonValue;
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`invalid report: ${file}`);
  return value as { [key: string]: JsonValue };
}

const REQUIRED_DESTINATIONS = new Set([
  "ch1",
  "s11",
  "s12",
  "s13",
  "s14",
  "s15",
  "ch2",
  "s21",
  "s22",
  "s23",
  "s24",
  "s25",
  "s26",
  "s27",
  "s28",
  "changelog",
]);

function main(): void {
  const config = options(Bun.argv);
  if (!fs.existsSync(config.binaryPath) || fs.lstatSync(config.binaryPath).isSymbolicLink())
    throw new Error(`missing or symlinked binary: ${config.binaryPath}`);
  if (sha256File(config.binaryPath) !== config.binarySha256) throw new Error("binary SHA mismatch");
  const manifest = validateManifest(config);
  const reports = [1280, 960, 620].map((width) => compareViewport(width, config));
  const rows = reports.flatMap((report) => report.rows);
  const failures = reports.flatMap((report) => report.failures);
  const sourceSha256s = [...new Set(rows.map((row) => row.sourceSha256))];
  if (sourceSha256s.length !== 1) throw new Error("source SHA differs across viewports");
  const report = {
    kind: "known-red-regression",
    acceptance: false,
    performanceVerified: false,
    krrBinarySha256: config.binarySha256,
    actualBinarySha256: sha256File(config.binaryPath),
    manifestPath: config.manifestPath,
    manifestSha256: sha256File(config.manifestPath),
    binaryPath: config.binaryPath,
    binaryShaVerified: true,
    sourceSha256s,
    sourceShaVerified: true,
    tocInventoryComplete: rows.length === 48,
    manifestBound: manifest.caseCount === 3,
    count: rows.length,
    failures,
    results: rows,
  };
  fs.writeFileSync(config.output, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    JSON.stringify({ count: rows.length, failures: failures.length, output: config.output }),
  );
}

function validateManifest(config: Options): { caseCount: number } {
  const manifest = read(config.manifestPath);
  if (manifest.kind !== "toc_pointer_diagnostic") throw new Error("invalid TOC manifest kind");
  if (text(manifest.source_html_sha256) !== text(readReportSource(config, 1280).htmlSha256))
    throw new Error("TOC manifest source SHA mismatch");
  const runtime = record(manifest.runtime_binary, "runtime binary");
  if (
    text(runtime.sha256) !== config.binarySha256 ||
    path.resolve(text(runtime.path)) !== config.binaryPath
  )
    throw new Error("TOC manifest binary mismatch");
  if (sha256File(path.resolve(text(runtime.path))) !== text(runtime.sha256))
    throw new Error("TOC manifest binary SHA mismatch");
  const cases = jsonRecords(manifest.cases, "TOC manifest cases");
  if (cases.length !== 3) throw new Error("TOC manifest viewport inventory mismatch");
  const requiredReports = new Set(
    [1280, 960, 620].map((width) =>
      path.join(config.krrRoot, `${config.krrPrefix}-${width}`, "toc-report.json"),
    ),
  );
  for (const entry of cases) {
    const reportPath = path.resolve(text(entry.report));
    if (!requiredReports.delete(reportPath))
      throw new Error(`unexpected or duplicate TOC report: ${reportPath}`);
    if (sha256File(reportPath) !== text(entry.report_sha256))
      throw new Error(`TOC report SHA mismatch: ${reportPath}`);
    if (entry.entries !== 16) throw new Error("TOC manifest entry count mismatch");
  }
  return { caseCount: cases.length };
}

function readReportSource(config: Options, width: number): { [key: string]: JsonValue } {
  return read(
    path.join(config.chromeRoot, `${config.chromePrefix}-${width}`, "scroll-coverage.json"),
  );
}

function record(value: JsonValue | undefined, name: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`invalid ${name}`);
  return value;
}

function compareViewport(
  width: number,
  config: Options,
): { rows: ReturnType<typeof compareEntry>[]; failures: string[] } {
  const krr = read(path.join(config.krrRoot, `${config.krrPrefix}-${width}`, "toc-report.json"));
  const chrome = read(
    path.join(config.chromeRoot, `${config.chromePrefix}-${width}`, "scroll-coverage.json"),
  );
  const { viewport, krrEntries, chromeEntries } = validateViewport(width, krr, chrome);
  const rows: ReturnType<typeof compareEntry>[] = [];
  const failures: string[] = [];
  for (const entry of krrEntries.values()) {
    const destination = text(entry.destination);
    const reference = chromeEntries.get(destination);
    if (!reference) throw new Error(`missing Chrome entry: ${width}/${destination}`);
    const row = compareEntry(width, viewport, krr, entry, reference, destination);
    rows.push(row);
    failures.push(...entryFailures(row, destination));
  }
  return { rows, failures };
}

function validateViewport(
  width: number,
  krr: { [key: string]: JsonValue },
  chrome: { [key: string]: JsonValue },
): {
  viewport: { width: number; height: number; scale: number };
  krrEntries: Map<string, { [key: string]: JsonValue }>;
  chromeEntries: Map<string, { [key: string]: JsonValue }>;
} {
  if (krr.htmlSha256 !== chrome.htmlSha256) throw new Error(`source fixture mismatch: ${width}`);
  const viewport = viewportValue(krr.viewport);
  const chromeViewport = viewportValue(chrome.viewport);
  if (viewport.width !== chromeViewport.width || viewport.height !== chromeViewport.height)
    throw new Error(`viewport mismatch: ${width}`);
  if (
    !Number.isFinite(viewport.scale) ||
    viewport.scale <= 0 ||
    viewport.scale !== chromeViewport.scale
  )
    throw new Error(`viewport scale mismatch: ${width}`);
  const krrEntries = uniqueEntries(jsonRecords(krr.entries, "KRR entries"), width, "KRR");
  const chromeEntries = uniqueEntries(
    jsonRecords(chrome.toc, "Chrome TOC entries"),
    width,
    "Chrome",
  );
  if (krrEntries.size !== chromeEntries.size || krrEntries.size !== 16)
    throw new Error(`TOC inventory mismatch: ${width}`);
  if (!sameDestinations(krrEntries, chromeEntries))
    throw new Error(`TOC destinations mismatch: ${width}`);
  const sourcePath = strictFile(chrome.htmlPath, "source HTML");
  if (
    sha256File(sourcePath) !== text(chrome.htmlSha256) ||
    sha256File(sourcePath) !== text(krr.htmlSha256)
  )
    throw new Error(`source HTML SHA mismatch: ${width}`);
  return { viewport, krrEntries, chromeEntries };
}

function entryFailures(row: ReturnType<typeof compareEntry>, destination: string): string[] {
  const failures: string[] = [];
  if (!row.scrollExact) failures.push(`${row.viewport}/${destination}:scroll`);
  if (!row.targetTopVerified) failures.push(`${row.viewport}/${destination}:targetTop`);
  if (!row.targetVisible) failures.push(`${row.viewport}/${destination}:visibility`);
  if (!row.pointerValid) failures.push(`${row.viewport}/${destination}:pointer`);
  if (row.score.rmseScore < 90 || row.score.maeScore < 90)
    failures.push(`${row.viewport}/${destination}:score90`);
  return failures;
}

function compareEntry(
  width: number,
  viewport: { width: number; height: number; scale: number },
  krr: { [key: string]: JsonValue },
  entry: { [key: string]: JsonValue },
  reference: { [key: string]: JsonValue },
  destination: string,
): {
  viewport: string;
  destination: string;
  nonce: string;
  sourceSha256: string;
  chromePngSha256: string;
  krrPngSha256: string;
  chromeScrollY: number;
  chromeRequestedScrollY: number;
  krrScrollY: number;
  scrollExact: boolean;
  chromeTargetTop: JsonValue;
  krrTargetTop: JsonValue;
  targetTopVerified: boolean;
  targetVisible: boolean;
  pointer: JsonValue;
  pointerValid: boolean;
  score: ReturnType<typeof compareImages>;
} {
  const krrPath = file(entry.capturePath);
  const chromePath = file(reference.capturePath);
  const chromeScrollY = finiteNumber(reference.actualScrollY);
  const chromeRequestedScrollY = finiteNumber(reference.requestedScrollY);
  const krrScrollY = finiteNumber(entry.scrollY);
  if (typeof entry.targetTop !== "number" || !Number.isFinite(entry.targetTop))
    throw new Error(`missing target top: ${destination}`);
  if (!targetVisibility(entry.targetVisible, entry.targetTop, entry.targetHeight, viewport.height))
    throw new Error(`invalid target visibility: ${destination}`);
  if (!pointerValid(entry.pointer, viewport.width, viewport.height))
    throw new Error(`invalid pointer: ${destination}`);
  const score = compareImages(chromePath, krrPath, 99);
  if (
    score.dimensions.width !== viewport.width * viewport.scale ||
    score.dimensions.height !== viewport.height * viewport.scale
  )
    throw new Error(`PNG dimensions mismatch: ${width}/${destination}`);
  return {
    viewport: `${width}x${viewport.height}@${viewport.scale}`,
    destination,
    nonce: text(krr.executionNonce),
    sourceSha256: text(krr.htmlSha256),
    chromePngSha256: sha256File(chromePath),
    krrPngSha256: sha256File(krrPath),
    chromeScrollY,
    chromeRequestedScrollY,
    krrScrollY,
    scrollExact: chromeScrollY === krrScrollY,
    chromeTargetTop: finiteNumber(reference.targetTop),
    krrTargetTop: entry.targetTop,
    targetTopVerified:
      typeof entry.targetTop === "number" &&
      typeof reference.targetTop === "number" &&
      entry.targetTop === reference.targetTop,
    targetVisible: targetVisibility(
      entry.targetVisible,
      entry.targetTop,
      entry.targetHeight,
      viewport.height,
    ),
    pointer: entry.pointer ?? null,
    pointerValid: pointerValid(entry.pointer, viewport.width, viewport.height),
    score,
  };
}

function sameDestinations(
  krrEntries: Map<string, { [key: string]: JsonValue }>,
  chromeEntries: Map<string, { [key: string]: JsonValue }>,
): boolean {
  if (krrEntries.size !== REQUIRED_DESTINATIONS.size) return false;
  for (const destination of REQUIRED_DESTINATIONS) {
    if (!krrEntries.has(destination) || !chromeEntries.has(destination)) return false;
  }
  return true;
}

function targetVisibility(
  declared: JsonValue | undefined,
  targetTop: JsonValue | undefined,
  targetHeight: JsonValue | undefined,
  viewportHeight: number,
): boolean {
  if (
    declared !== true ||
    typeof targetTop !== "number" ||
    !Number.isFinite(targetTop) ||
    typeof targetHeight !== "number" ||
    !Number.isFinite(targetHeight) ||
    targetHeight <= 0
  )
    return false;
  return targetTop < viewportHeight && targetTop + targetHeight > 0;
}

function uniqueEntries(
  entries: Array<{ [key: string]: JsonValue }>,
  width: number,
  kind: string,
): Map<string, { [key: string]: JsonValue }> {
  const result = new Map<string, { [key: string]: JsonValue }>();
  for (const entry of entries) {
    const destination = entryDestination(entry, kind);
    if (result.has(destination))
      throw new Error(`duplicate ${kind} destination: ${width}/${destination}`);
    result.set(destination, entry);
  }
  return result;
}

function entryDestination(entry: { [key: string]: JsonValue }, kind: string): string {
  if (typeof entry.destination === "string" && entry.destination.length > 0)
    return entry.destination;
  if (kind === "Chrome" && typeof entry.href === "string" && entry.href.startsWith("#"))
    return entry.href.slice(1);
  throw new Error(`invalid ${kind} destination`);
}

function text(value: JsonValue | undefined): string {
  if (typeof value !== "string" || value.length === 0) throw new Error("invalid text");
  return value;
}

function viewportValue(value: JsonValue | undefined): {
  width: number;
  height: number;
  scale: number;
} {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    if (
      typeof value.width !== "number" ||
      typeof value.height !== "number" ||
      typeof value.scale !== "number"
    )
      throw new Error("invalid viewport");
    return value as { width: number; height: number; scale: number };
  }
  throw new Error("invalid viewport");
}

function finiteNumber(value: JsonValue | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error("invalid number");
  return value;
}

function file(value: JsonValue | undefined): string {
  const result = path.resolve(text(value));
  if (!fs.existsSync(result) || fs.lstatSync(result).isSymbolicLink())
    throw new Error(`missing or symlinked PNG: ${result}`);
  return result;
}

function strictFile(value: JsonValue | undefined, kind: string): string {
  const result = path.resolve(text(value));
  if (!fs.existsSync(result) || fs.lstatSync(result).isSymbolicLink())
    throw new Error(`missing or symlinked ${kind}: ${result}`);
  if (!fs.statSync(result).isFile()) throw new Error(`${kind} is not a file: ${result}`);
  return result;
}

function pointerValid(value: JsonValue | undefined, width: number, height: number): boolean {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  return (
    typeof value.x === "number" &&
    Number.isFinite(value.x) &&
    typeof value.y === "number" &&
    Number.isFinite(value.y) &&
    value.x >= 0 &&
    value.x < width &&
    value.y >= 0 &&
    value.y < height
  );
}

if (import.meta.main) main();
