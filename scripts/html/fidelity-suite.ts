import fs from "node:fs";
import path from "node:path";
import {
  assertCatalogComplete,
  type FeatureCaseResult,
  loadFeatureCatalog,
} from "./feature-catalog";
import { type FidelityPhase, fidelityThresholdForPhase } from "./fidelity-phase";
import {
  assertExactlyOneReportedInputSha,
  assertExpectedInputSha,
  assertInputUnchanged,
  type CaptureBrowser,
  type CaptureOptions,
  createCaptureBrowser,
  runWithCaptureBrowser,
  type ScoreReport,
  sha256File,
} from "./reference-capture";
import { assertScrollCoverage, measureScrollCoverage } from "./scroll-coverage";
import {
  compareTocCaptures,
  runTocProbe,
  type TocEntry,
  tocBinaryFromBuildReport,
  tocProbePath,
  validateTocEntries,
} from "./toc-fidelity";

const VIEWPORTS = [
  { width: 1280, height: 900 },
  { width: 960, height: 720 },
  { width: 620, height: 900 },
] as const;
const BOTTOM_SCROLL_REQUEST = 1_000_000_000;
const SCROLLS = [
  { name: "0", requested: 0 },
  { name: "600", requested: 600 },
  { name: "1800", requested: 1800 },
  { name: "bottom", requested: BOTTOM_SCROLL_REQUEST },
] as const;

export interface SuiteOptions {
  tocBuildReport?: string;
  binary: string;
  input: string | null;
  inputs: string;
  output: string;
  minScore: number;
  phase?: FidelityPhase;
}

interface CaseResult {
  fixture: string;
  width: number;
  height: number;
  requestedScrollY: number;
  krrScrollY: number;
  chromeScrollY: number;
  score: ScoreReport;
  passed: boolean;
  outputDir: string;
  error?: string;
}

interface ProbeResult {
  scrolls: Map<string, number>;
  htmlSha256: string;
}

function scrollLabel(scroll: number): string {
  return scroll === BOTTOM_SCROLL_REQUEST ? "bottom" : String(scroll);
}

export function parseSuiteOptions(argv: string[]): SuiteOptions {
  const values = suiteArgumentValues(argv);
  const binary = values.get("--binary");
  if (!binary) throw new Error("Missing required option: --binary");
  const input = values.get("--input") ?? null;
  const inputs = path.resolve(values.get("--inputs") ?? "tests/fixtures/html");
  if (input && values.has("--inputs")) throw new Error("Use only one of --input and --inputs");
  const phase = (values.get("--phase") ?? "final") as FidelityPhase;
  const minScore = Number(values.get("--min-score") ?? (phase === "interim" ? "90" : "99"));
  if (!Number.isFinite(minScore) || minScore < 0 || minScore > 100) {
    throw new Error("--min-score must be between 0 and 100");
  }
  fidelityThresholdForPhase(phase, minScore);
  return {
    phase,
    ...(values.get("--toc-build-report")
      ? { tocBuildReport: path.resolve(values.get("--toc-build-report") ?? "") }
      : {}),
    binary: path.resolve(binary),
    input: input ? path.resolve(input) : null,
    inputs,
    output: path.resolve(values.get("--output") ?? "tmp/html-chrome-fidelity-suite"),
    minScore,
  };
}

function suiteArgumentValues(argv: string[]): Map<string, string> {
  const values = new Map<string, string>();
  const allowed = new Set([
    "--binary",
    "--input",
    "--inputs",
    "--output",
    "--min-score",
    "--toc-build-report",
    "--phase",
  ]);
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv.at(index);
    const value = argv.at(index + 1);
    if (!name || !allowed.has(name) || value === undefined || value.startsWith("--")) {
      throw new Error(`Invalid suite argument near index ${index}`);
    }
    values.set(name, value);
  }
  return values;
}

export function scrollPositionsMatch(expected: number, actual: number): boolean {
  return Math.abs(actual - expected) < 0.01;
}

function inputFiles(options: SuiteOptions): string[] {
  if (options.input) return [options.input];
  if (!fs.statSync(options.inputs).isDirectory())
    throw new Error(`HTML input directory is missing: ${options.inputs}`);
  return collectHtmlFiles(options.inputs).sort();
}

export function collectHtmlFiles(directory: string): string[] {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (entry.name.startsWith(".")) return [];
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) return collectHtmlFiles(entryPath);
    return entry.isFile() && entry.name.endsWith(".html") ? [entryPath] : [];
  });
}

export function runProbe(
  options: SuiteOptions,
  input: string,
  output: string,
  viewport: (typeof VIEWPORTS)[number],
  scrolls: ReadonlyArray<{ name: string; requested: number }> = SCROLLS,
): ProbeResult {
  const htmlSha256Before = sha256File(input);
  const binarySha256Before = sha256File(options.binary);
  const result = Bun.spawnSync([
    options.binary,
    "--input",
    input,
    "--output",
    output,
    "--width",
    String(viewport.width),
    "--height",
    String(viewport.height),
    "--scale",
    "1",
    ...scrolls.flatMap(({ requested }) => ["--scroll-y", String(requested)]),
  ]);
  if (result.exitCode !== 0)
    throw new Error(`KRR probe failed: ${new TextDecoder().decode(result.stderr)}`);
  assertBinaryUnchanged(binarySha256Before, sha256File(options.binary));
  const htmlSha256After = sha256File(input);
  assertInputUnchanged(htmlSha256Before, htmlSha256After);
  const actual = new Map<string, number>();
  const reportedHtmlSha256 = assertExactlyOneReportedInputSha(
    new TextDecoder().decode(result.stdout),
    htmlSha256After,
  );
  for (const line of new TextDecoder().decode(result.stdout).split(/\r?\n/)) {
    const match = line.match(/png=(\S+).*scroll_y=([-+]?\d+(?:\.\d+)?)/);
    if (!match?.[1] || !match[2]) continue;
    const requested = scrolls.find(({ requested }) =>
      match[1]?.includes(`scroll-${requested}.png`),
    );
    if (requested) actual.set(requested.name, Number(match[2]));
  }
  if (actual.size !== scrolls.length)
    throw new Error(`KRR probe did not report all scroll samples for ${input}`);
  return { scrolls: actual, htmlSha256: reportedHtmlSha256 };
}

async function runCase(
  options: SuiteOptions,
  input: string,
  viewport: (typeof VIEWPORTS)[number],
  scroll: number,
  krrScrollY: number,
  expectedHtmlSha256: string,
  captureBrowser: CaptureBrowser,
): Promise<CaseResult> {
  const fixture = path.basename(input, path.extname(input));
  const outputDir = path.join(
    options.output,
    `${fixture}-${viewport.width}x${viewport.height}-scroll-${scrollLabel(scroll)}`,
  );
  const krrPath = path.join(
    options.output,
    "krr",
    fixture,
    `${viewport.width}x${viewport.height}-scale-1-scroll-${scroll}.png`,
  );
  const captureOptions: CaptureOptions = {
    htmlPath: input,
    krrPath,
    outputDir,
    width: viewport.width,
    height: viewport.height,
    scale: 1,
    mode: "scroll",
    scrollY: scroll,
    minScore: options.minScore,
    expectedHtmlSha256,
  };
  const score = await runWithCaptureBrowser(captureOptions, captureBrowser);
  const metadata = JSON.parse(fs.readFileSync(path.join(outputDir, "scores.json"), "utf8")) as {
    browser: { actualScrollY: number };
    input: { htmlSha256: string };
  };
  assertExpectedInputSha(expectedHtmlSha256, metadata.input.htmlSha256);
  const scrollMatches = scrollPositionsMatch(krrScrollY, metadata.browser.actualScrollY);
  const result: CaseResult = {
    fixture,
    width: viewport.width,
    height: viewport.height,
    requestedScrollY: scroll,
    krrScrollY,
    chromeScrollY: metadata.browser.actualScrollY,
    score,
    passed: score.passed && scrollMatches,
    outputDir,
  };
  const errors: string[] = [];
  if (!score.passed)
    errors.push(`score ${score.score.toFixed(2)} below ${score.minScore.toFixed(2)}`);
  if (!scrollMatches) {
    errors.push(`scroll mismatch: KRR ${krrScrollY}, Chrome ${metadata.browser.actualScrollY}`);
  }
  if (errors.length > 0) result.error = errors.join("; ");
  return result;
}

export async function runSuite(options: SuiteOptions): Promise<CaseResult[]> {
  const phase = options.phase ?? "final";
  const threshold = fidelityThresholdForPhase(phase, options.minScore);
  const results: CaseResult[] = [];
  const binarySha256 = sha256File(options.binary);
  const tocBinary = options.tocBuildReport
    ? tocBinaryFromBuildReport(options.tocBuildReport)
    : tocProbePath(options.binary);
  const tocBinarySha256 = fs.existsSync(tocBinary) ? sha256File(tocBinary) : null;
  fs.mkdirSync(path.join(options.output, "krr"), { recursive: true });
  for (const input of inputFiles(options)) {
    for (const viewport of VIEWPORTS) {
      assertBinaryUnchanged(binarySha256, sha256File(options.binary));
      results.push(...(await runViewport(options, input, viewport, tocBinarySha256)));
      assertBinaryUnchanged(binarySha256, sha256File(options.binary));
    }
  }
  fs.writeFileSync(
    path.join(options.output, "capture-build.json"),
    `${JSON.stringify({ binarySha256, tocBinarySha256 }, null, 2)}\n`,
  );
  fs.writeFileSync(
    path.join(options.output, phase === "interim" ? "interim-scores.json" : "scores.json"),
    `${JSON.stringify(phase === "interim" ? { phase, threshold, results } : results, null, 2)}\n`,
  );
  if (!options.input) writeVisualCatalogResults(options.output, results, phase);
  const passed = results.length > 0 && results.every((result) => result.passed);
  console.log(`cases: ${results.length}`);
  console.log(
    `minimum score: ${Math.min(...results.map((result) => result.score.score)).toFixed(2)}`,
  );
  console.log(`passed: ${passed}`);
  return results;
}

function writeVisualCatalogResults(
  output: string,
  results: CaseResult[],
  phase: FidelityPhase,
): void {
  const catalog = loadFeatureCatalog();
  assertCatalogComplete(catalog);
  const visualResults: FeatureCaseResult[] = catalog.visualAspects.map((entry) => {
    const fixture = path.basename(entry.fixture, path.extname(entry.fixture));
    const captures = results.filter(
      (result) => result.fixture === fixture || result.fixture.startsWith(`${fixture}-toc-`),
    );
    if (captures.length === 0) throw new Error(`unrun visual feature case: ${entry.id}`);
    const passed = captures.every((result) => result.passed);
    return {
      caseId: entry.id,
      status: passed ? "passed" : "failed",
      features: Object.fromEntries(
        entry.features.map((feature) => [feature, passed ? "passed" : "failed"]),
      ),
    };
  });
  fs.writeFileSync(
    path.join(
      output,
      phase === "interim" ? "interim-visual-feature-results.json" : "visual-feature-results.json",
    ),
    `${JSON.stringify(phase === "interim" ? { phase, threshold: 90, results: visualResults } : visualResults, null, 2)}\n`,
  );
  if (visualResults.some((result) => result.status !== "passed"))
    throw new Error("visual feature catalog score gate failed");
}

export function assertBinaryUnchanged(expected: string, actual: string): void {
  if (expected !== actual) throw new Error("KRR probe binary changed during capture");
}

async function runViewport(
  options: SuiteOptions,
  input: string,
  viewport: (typeof VIEWPORTS)[number],
  tocBinarySha256: string | null,
): Promise<CaseResult[]> {
  const fixture = path.basename(input, path.extname(input));
  const coverageDir = path.join(
    options.output,
    `${fixture}-${viewport.width}x${viewport.height}-coverage`,
  );
  const coverage = await measureScrollCoverage({
    htmlPath: input,
    viewport,
    outputDir: coverageDir,
  });
  const probe = runProbe(
    options,
    input,
    path.join(options.output, "krr", fixture),
    viewport,
    coverage.scrolls,
  );
  assertExpectedInputSha(coverage.htmlSha256, probe.htmlSha256);
  const results: CaseResult[] = [];
  const captureBrowser = createCaptureBrowser();
  try {
    for (const { name, requested } of coverage.scrolls) {
      const krrScrollY = probe.scrolls.get(name);
      if (krrScrollY === undefined) throw new Error(`Missing KRR scroll sample: ${name}`);
      try {
        results.push(
          await runCase(
            options,
            input,
            viewport,
            requested,
            krrScrollY,
            probe.htmlSha256,
            captureBrowser,
          ),
        );
      } catch (error) {
        console.error(error instanceof Error ? error.message : error);
        results.push(
          failedCase(
            options,
            fixture,
            viewport,
            requested,
            krrScrollY,
            error instanceof Error ? error.message : String(error),
          ),
        );
      }
    }
  } finally {
    await captureBrowser.close();
  }
  assertScrollCoverage(
    results.map((result) => result.chromeScrollY),
    coverage.maxScrollY,
    viewport.height,
  );
  assertScrollCoverage(
    results.map((result) => result.krrScrollY),
    coverage.maxScrollY,
    viewport.height,
  );
  results.push(
    ...runTocCases(
      options,
      input,
      viewport,
      fixture,
      coverage.entries,
      probe.htmlSha256,
      tocBinarySha256,
    ),
  );
  return results;
}

function runTocCases(
  options: SuiteOptions,
  input: string,
  viewport: (typeof VIEWPORTS)[number],
  fixture: string,
  chromeEntries: TocEntry[],
  htmlSha256: string,
  binarySha256: string | null,
): CaseResult[] {
  const reference = validateTocEntries(chromeEntries);
  if (reference.length === 0) return [];
  if (binarySha256 === null) throw new Error("Missing KRR TOC probe binary");
  const output = path.join(
    options.output,
    "krr-toc",
    `${fixture}-${viewport.width}x${viewport.height}`,
  );
  const entries = runTocProbe(
    options.tocBuildReport
      ? tocBinaryFromBuildReport(options.tocBuildReport)
      : tocProbePath(options.binary),
    input,
    output,
    viewport,
    htmlSha256,
    binarySha256,
  );
  return compareTocCaptures(reference, entries, options.minScore).map((result) => ({
    fixture: `${fixture}-toc-${result.destination}`,
    width: viewport.width,
    height: viewport.height,
    requestedScrollY: result.chromeScrollY,
    krrScrollY: result.krrScrollY,
    chromeScrollY: result.chromeScrollY,
    score: result.score,
    passed: result.passed,
    outputDir: result.outputDir,
    ...(result.error ? { error: result.error } : {}),
  }));
}

function failedCase(
  options: SuiteOptions,
  fixture: string,
  viewport: (typeof VIEWPORTS)[number],
  scroll: number,
  krrScrollY: number,
  error: string,
): CaseResult {
  return {
    fixture,
    width: viewport.width,
    height: viewport.height,
    requestedScrollY: scroll,
    krrScrollY,
    chromeScrollY: -1,
    score: {
      score: 0,
      rmseScore: 0,
      maeScore: 0,
      minScore: options.minScore,
      passed: false,
      dimensions: { width: 0, height: 0 },
    },
    passed: false,
    outputDir: path.join(
      options.output,
      `${fixture}-${viewport.width}x${viewport.height}-scroll-${scrollLabel(scroll)}`,
    ),
    error,
  };
}

if (import.meta.main) {
  runSuite(parseSuiteOptions(process.argv.slice(2)))
    .then((results) => {
      process.exitCode = results.length > 0 && results.every((result) => result.passed) ? 0 : 1;
    })
    .catch((error) => {
      console.error(error instanceof Error ? error.message : error);
      process.exitCode = 1;
    });
}
