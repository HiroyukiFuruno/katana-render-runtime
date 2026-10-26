import fs from "node:fs";
import path from "node:path";
import { FixtureRepository } from "./diagram_update_fixtures";
import type { LanguageName, ManagedFullCompareRequest } from "./managed-full-compare-input";
import { type FixtureSourcePin, validateLanguageInventory } from "./managed-full-compare-input";
import {
  type ManagedCommandResult,
  type ManagedToolPin,
  parseChildJson,
  requireCommandSuccess,
  requireScoreProcessOutcome,
  runManagedCommand,
} from "./managed-full-compare-process";
import {
  type PaintProjectedReferenceEvidence,
  parsePaintProjectedReferenceEvidence,
  validateLanguageScoreEvidence,
} from "./managed-full-compare-results";
import { fileSha256 } from "./mermaid-pair-manifest";
import {
  nativeFrameBuildProofArgv,
  requireNativeFrameBuildFacts,
} from "./native-frame-build-proof";
import { readNativeFrameCaptureConfig } from "./native-frame-capture-context";
import {
  captureNativeFrameEvidence,
  scoreNativeFramePaintProjection,
  verifyNativeFrameEvidence,
} from "./native-frame-evidence";
import {
  type NativeProductDimensionFailure,
  type NativeProductPaintReference,
  scoreNativeProductRows,
} from "./native-product-score";
import { requireNativeRenderInventory } from "./native-render-provenance";
import { MagickOps } from "./reference_image_ops";
import type { ReferencePair } from "./reference_pair_repository";
import { isNeutralGeometryTargetSlug } from "./reference_svg_geometry";

export interface ManagedStep {
  label: string;
  argv: string[];
  cwd: string;
  stdoutPath: string;
  stderrPath: string;
  exitPath: string;
  actualExitCode: number | null;
  signal: NodeJS.Signals | null;
  spawnError: string | null;
}

export interface LanguageAcceptance {
  language: LanguageName;
  fixtureCount: number;
  minimumScore: number;
  accepted: boolean;
  failedScores: Array<{ slug: string; score: number; minScore: number }>;
  failures: Array<{
    slug: string;
    reason: string;
    official: { width: number; height: number };
    actual: { width: number; height: number };
  }>;
  fixtureSources: FixtureSourcePin[];
  buildReceipt: string;
  nativeBindings: FilePin;
  rasterReceipt: FilePin;
  rasterProcessEvidence: FilePin;
  comparison: FilePin;
  bindingConfig: FilePin;
  nativeFrameEvidence?: FilePin;
  nativeFrameComparison?: FilePin;
  nativeProductComparison?: FilePin;
  nativeProductFailures?: FilePin;
  nativeFrameBuildProof: FilePin;
}

export interface FilePin {
  path: string;
  sha256: string;
  size: number;
}

export async function runManagedLanguage(
  request: ManagedFullCompareRequest,
  language: LanguageName,
  buildReceiptPath: string,
  outputRoot: string,
  steps: ManagedStep[],
  expectedFixtureSources: FixtureSourcePin[],
): Promise<LanguageAcceptance> {
  const input = request.languages[language],
    workspace = path.join(outputRoot, language),
    svgDirectory = path.join(workspace, "candidate-svg"),
    candidateFixtures = path.join(workspace, "canonical-nonempty-fixtures"),
    rasterAttempt = path.join(workspace, "candidate-raster-attempt"),
    rasterPairs = path.join(workspace, "candidate-raster-pairs"),
    sessionConfig = path.join(workspace, "candidate-raster-session.json"),
    rasterEvidence = path.join(rasterAttempt, "raster-process-evidence.json"),
    rasterReceipt = path.join(rasterAttempt, "raster-production-receipt.json"),
    bindingConfig = path.join(workspace, "fresh-raster-comparison-binding.json"),
    comparisonDirectory = path.join(workspace, "comparison");
  fs.mkdirSync(workspace, { mode: 0o700 });
  createEmptyDirectory(svgDirectory);
  createEmptyDirectory(rasterAttempt);
  createEmptyDirectory(rasterPairs);
  const inventory = validateLanguageInventory(request, language, expectedFixtureSources);
  const slugs = inventory.slugs;
  let nativeFrameEvidence: FilePin | undefined;
  let nativeEvidenceValue: ReturnType<typeof verifyNativeFrameEvidence> | undefined;
  let nativeFrameComparison: FilePin | undefined;
  let nativeProductComparison: FilePin | undefined;
  let nativeProductFailures: FilePin | undefined;
  let nativeProductScore: ReturnType<typeof validateLanguageScoreEvidence> | undefined;
  let nativeFrameAccepted = true;
  assertNativeFrameInputMode(input);
  const capturePins = input.nativeFrameCapture;
  if (capturePins === undefined) throw new Error("Native frame managed capture pins are missing");
  const buildProof = runStep(
    request,
    outputRoot,
    steps,
    `${language}-native-frame-build-proof`,
    nativeFrameBuildProofArgv(request.tools.python.path, request.repositoryRoot, capturePins),
  );
  requireCommandSuccess(buildProof);
  requireNativeFrameBuildFacts(
    parseChildJson<unknown>(buildProof, "native frame build proof"),
    capturePins,
    request.tools.cargo.sha256,
  );
  const suppliedNativeEvidence = loadNativeFrameEvidence(request, input, language, slugs);
  nativeEvidenceValue = suppliedNativeEvidence?.evidence;
  nativeFrameEvidence = suppliedNativeEvidence?.pin;
  copyNonemptyFixtures(input.fixtures, candidateFixtures, inventory.sources, slugs);
  createEmptyDirectory(comparisonDirectory);

  const session = runStep(request, outputRoot, steps, `${language}-candidate-session`, [
    request.tools.bun.path,
    "run",
    "scripts/mermaid/raster-production-session.ts",
    "--template",
    input.officialTemplate.path,
    "--template-sha256",
    input.officialTemplate.sha256,
    "--fixtures",
    candidateFixtures,
    "--attempt",
    rasterAttempt,
    "--output",
    rasterPairs,
    "--config",
    sessionConfig,
    "--role",
    "candidate",
  ]);
  const sessionResponse = parseChildJson<WriterResponse>(session, "candidate session writer");
  assertWriterResponse(sessionResponse, sessionConfig, slugs, "candidate");
  const candidateFixtureArguments = candidateFixtureArgumentPairs(candidateFixtures);

  runStep(request, outputRoot, steps, `${language}-native-render`, [
    request.tools.bun.path,
    "run",
    "scripts/mermaid/native-render-provenance.ts",
    ...candidateFixtureArguments.nativeRender,
    "--fixture-glob",
    "*.md",
    "--output-dir",
    svgDirectory,
    "--binary",
    request.binary,
    "--runtime",
    request.runtime.path,
    "--build-receipt",
    buildReceiptPath,
  ]);
  const fixtures = new FixtureRepository(candidateFixtures).list();
  const nativeBindings = requireNativeRenderInventory(
    path.join(svgDirectory, "render-bindings.json"),
    fixtures,
    svgDirectory,
  );
  if (nativeBindings.size !== slugs.length || slugs.some((slug) => !nativeBindings.has(slug))) {
    throw new Error(`${language} native render receipt differs from the fixed inventory`);
  }
  runStep(request, outputRoot, steps, `${language}-raster-process`, [
    request.tools.python.path,
    "scripts/mermaid/produce_raster_process_evidence.py",
    "--config",
    sessionConfig,
    "--config-sha256",
    sessionResponse.sha256,
    "--role",
    "candidate",
    "--bun",
    request.tools.bun.path,
    ...candidateFixtureArguments.rasterProcess,
    "--input",
    svgDirectory,
    "--output",
    rasterPairs,
    "--fixture-glob",
    "*.md",
    "--mermaid-js",
    request.tools.mermaidJs.path,
    "--mermaid-zenuml-js",
    request.tools.mermaidZenUmlJs.path,
  ]);
  if (!fs.existsSync(rasterEvidence) || !fs.existsSync(rasterReceipt)) {
    throw new Error(`${language} raster producer omitted its process evidence or receipt`);
  }
  if (nativeEvidenceValue !== undefined) {
    assertNativeFrameInputs(nativeEvidenceValue, svgDirectory, rasterPairs, slugs);
  }
  if (input.nativeFrameCapture !== undefined) {
    const captured = captureNativeFrame(
      request,
      language,
      input.nativeFrameCapture,
      svgDirectory,
      rasterPairs,
      workspace,
      slugs,
    );
    nativeEvidenceValue = captured.evidence;
    nativeFrameEvidence = captured.pin;
  }

  const binding = runStep(request, outputRoot, steps, `${language}-comparison-binding`, [
    request.tools.bun.path,
    "run",
    "scripts/mermaid/raster-comparison-binding.ts",
    "--trusted-repository-root",
    request.repositoryRoot,
    "--fixtures",
    candidateFixtures,
    "--config",
    bindingConfig,
    "--official-receipt",
    input.officialReceipt.path,
    "--official-receipt-sha256",
    input.officialReceipt.sha256,
    "--official-process-evidence",
    input.officialProcessEvidence.path,
    "--official-process-sha256",
    input.officialProcessEvidence.sha256,
    "--official-pair-directory",
    input.officialPairDirectory,
    "--candidate-receipt",
    rasterReceipt,
    "--candidate-receipt-sha256",
    fileSha256(rasterReceipt),
    "--candidate-process-evidence",
    rasterEvidence,
    "--candidate-process-sha256",
    fileSha256(rasterEvidence),
    "--candidate-pair-directory",
    rasterPairs,
  ]);
  const bindingResponse = parseChildJson<BindingResponse>(
    binding,
    "raster comparison binding writer",
  );
  assertWriterResponse(bindingResponse, bindingConfig, slugs, "binding");

  const scoring = runScoringStep(request, outputRoot, steps, `${language}-strict-score`, [
    request.tools.bun.path,
    "run",
    "scripts/mermaid/reference-compare.ts",
    "--official",
    input.officialPairDirectory,
    "--katana",
    rasterPairs,
    "--katana-svg-dir",
    svgDirectory,
    "--fixtures-dir",
    candidateFixtures,
    "--fixture-glob",
    "*.md",
    "--output",
    comparisonDirectory,
    "--theme",
    "dark",
    "--min-score",
    "99",
    "--reference-mode",
    "strictVendorOriginal",
    "--reference-scope",
    "formal",
    "--raster-binding-config",
    bindingConfig,
    "--raster-binding-config-sha",
    bindingResponse.sha256,
  ]);
  const contextPath = path.join(comparisonDirectory, "reference-context.json"),
    projectedEvidencePath = path.join(
      comparisonDirectory,
      "paint-projected-reference-evidence.json",
    ),
    contextValue = readJson(contextPath),
    projectedEvidence =
      expectedGeometrySlugs(slugs).length > 0
        ? parsePaintProjectedReferenceEvidence(readJson(projectedEvidencePath))
        : undefined;
  if (projectedEvidence !== undefined) {
    assertProjectedEvidenceEnvelope(contextValue, projectedEvidencePath);
    assertPaintProjectedReferenceFilePins(
      projectedEvidence,
      comparisonDirectory,
      input.officialPairDirectory,
      rasterPairs,
      svgDirectory,
    );
  }
  const score = validateLanguageScoreEvidence(
    language,
    slugs,
    readJson(path.join(comparisonDirectory, "scores.json")),
    readJson(path.join(comparisonDirectory, "fixture-inventory.json")),
    contextValue,
    projectedEvidence,
  );
  requireScoreProcessOutcome(scoring, score.accepted);
  if (nativeEvidenceValue !== undefined) {
    const nativePairs = slugs.map((slug) => ({
      slug,
      officialPath: path.join(input.officialPairDirectory, `${slug}.png`),
      katanaPath: path.join(rasterPairs, `${slug}.png`),
      officialSvgPath: path.join(input.officialPairDirectory, `${slug}.svg`),
      katanaSvgPath: path.join(svgDirectory, `${slug}.svg`),
    }));
    const nativeProjectionDirectory = path.join(
      comparisonDirectory,
      "native-frame-paint-projection",
    );
    createEmptyDirectory(nativeProjectionDirectory);
    const nativeComparison = await scoreNativeFramePaintProjection(
      nativePairs,
      nativeEvidenceValue,
      nativeProjectionDirectory,
      "dark",
      new MagickOps(path.join(comparisonDirectory, "native-frame-paint-magick"), null, "dark"),
    );
    nativeFrameAccepted = [...nativeComparison.scores.values()].every(
      (row) => row.score >= 99 && row.structuralIntegrityAccepted,
    );
    nativeProductScore = writeNativeProductScores(
      language,
      slugs,
      nativePairs,
      nativeEvidenceValue.rows,
      nativeComparison,
      comparisonDirectory,
      readJson(path.join(comparisonDirectory, "fixture-inventory.json")),
    );
    nativeProductComparison = filePin(
      path.join(comparisonDirectory, "native-product-comparison", "scores.json"),
    );
    nativeProductFailures = filePin(
      path.join(comparisonDirectory, "native-product-comparison", "failures.json"),
    );
    nativeFrameComparison = filePin(nativeComparison.evidencePath);
  }
  const requiredNativeProductScore = requireNativeProductScoreEvidence(nativeProductScore);
  return {
    language,
    fixtureCount: score.fixtureCount,
    minimumScore: Math.min(score.minimumScore, requiredNativeProductScore.minimumScore),
    accepted: score.accepted && nativeFrameAccepted && requiredNativeProductScore.accepted,
    failedScores: [...score.scores, ...requiredNativeProductScore.scores]
      .filter((row) => !row.passed || row.score < 99)
      .map(({ slug, score, minScore }) => ({ slug, score, minScore })),
    failures: requiredNativeProductScore.failures,
    fixtureSources: inventory.sources,
    buildReceipt: buildReceiptPath,
    nativeBindings: filePin(path.join(svgDirectory, "render-bindings.json")),
    rasterReceipt: filePin(rasterReceipt),
    rasterProcessEvidence: filePin(rasterEvidence),
    comparison: filePin(path.join(comparisonDirectory, "scores.json")),
    bindingConfig: filePin(bindingConfig),
    nativeFrameBuildProof: filePin(buildProof.stdoutPath),
    ...(nativeFrameEvidence === undefined ? {} : { nativeFrameEvidence }),
    ...(nativeFrameComparison === undefined ? {} : { nativeFrameComparison }),
    ...(nativeProductComparison === undefined ? {} : { nativeProductComparison }),
    ...(nativeProductFailures === undefined ? {} : { nativeProductFailures }),
  };
}

export function candidateFixtureArgumentPairs(candidateFixtures: string): {
  nativeRender: ["--fixtures-dir", string];
  rasterProcess: ["--fixtures", string];
} {
  return {
    nativeRender: ["--fixtures-dir", candidateFixtures],
    rasterProcess: ["--fixtures", candidateFixtures],
  };
}

function expectedGeometrySlugs(slugs: string[]): string[] {
  return slugs.filter(isNeutralGeometryTargetSlug);
}

function assertPaintProjectedReferenceFilePins(
  evidence: PaintProjectedReferenceEvidence,
  comparisonDirectory: string,
  officialDirectory: string,
  candidatePngDirectory: string,
  candidateSvgDirectory: string,
  projectedDirectory = comparisonDirectory,
  candidatePngBySlug?: ReadonlyMap<string, { path: string; sha256: string }>,
) {
  const derivativeDirectory = path.join(projectedDirectory, "paint-projected-reference");
  for (const pair of evidence.pairs) {
    const stem = pair.slug;
    assertPinnedFile(
      pair.official.svgPath,
      path.join(officialDirectory, `${stem}.svg`),
      pair.official.svgSha256,
    );
    assertPinnedFile(
      pair.official.sourcePngPath,
      path.join(officialDirectory, `${stem}.png`),
      pair.official.sourcePngSha256,
    );
    assertPinnedFile(
      pair.candidate.svgPath,
      path.join(candidateSvgDirectory, `${stem}.svg`),
      pair.candidate.svgSha256,
    );
    const candidatePng = candidatePngBySlug?.get(stem);
    assertPinnedFile(
      pair.candidate.sourcePngPath,
      candidatePng?.path ?? path.join(candidatePngDirectory, `${stem}.png`),
      candidatePng?.sha256 ?? pair.candidate.sourcePngSha256,
    );
    assertPinnedFile(
      pair.projectedReference.svgPath,
      path.join(derivativeDirectory, `${stem}-reference.svg`),
      pair.projectedReference.svgSha256,
    );
    assertPinnedFile(
      pair.projectedReference.pngPath,
      path.join(derivativeDirectory, `${stem}-reference.png`),
      pair.projectedReference.pngSha256,
    );
  }
  assertPinnedFile(
    evidence.browser.executablePath,
    evidence.browser.executablePath,
    evidence.browser.executableSha256,
  );
}

function assertProjectedEvidenceEnvelope(contextValue: unknown, evidencePath: string) {
  if (typeof contextValue !== "object" || contextValue === null) {
    throw new Error("Paint-projected reference report context is invalid");
  }
  const evidenceRef = Reflect.get(contextValue, "paintProjectedReferenceEvidence");
  if (
    typeof evidenceRef !== "object" ||
    evidenceRef === null ||
    Reflect.get(evidenceRef, "path") !== path.basename(evidencePath) ||
    Reflect.get(evidenceRef, "sha256") !== fileSha256(evidencePath)
  ) {
    throw new Error("Paint-projected reference evidence differs from the score context pin");
  }
}

function assertPinnedFile(actualPath: string, expectedPath: string, expectedSha256: string) {
  const resolved = path.resolve(actualPath);
  if (resolved !== path.resolve(expectedPath))
    throw new Error(`Paint-projected reference path differs: ${actualPath}`);
  const stat = fs.lstatSync(resolved);
  if (!stat.isFile() || stat.isSymbolicLink() || fileSha256(resolved) !== expectedSha256) {
    throw new Error(`Paint-projected reference file pin differs: ${resolved}`);
  }
}

function runScoringStep(
  request: ManagedFullCompareRequest,
  outputRoot: string,
  steps: ManagedStep[],
  label: string,
  argv: string[],
) {
  const result = runManagedCommand(
    outputRoot,
    label,
    argv,
    request.repositoryRoot,
    commandTool(request, argv),
  );
  recordManagedStep(steps, result);
  if (
    result.spawnError !== null ||
    result.signal !== null ||
    ![0, 1].includes(result.exitCode ?? -1)
  ) {
    requireCommandSuccess(result);
  }
  return result;
}

function copyNonemptyFixtures(
  sourceDirectory: string,
  destinationDirectory: string,
  sources: FixtureSourcePin[],
  expectedSlugs: string[],
): void {
  createEmptyDirectory(destinationDirectory);
  const selected = sources.filter((source) => !source.empty);
  if (
    JSON.stringify(selected.map((source) => source.slug).sort()) !== JSON.stringify(expectedSlugs)
  ) {
    throw new Error("Canonical fixture copy selection differs from the pinned nonempty inventory");
  }
  for (const source of selected) {
    const sourcePath = path.join(sourceDirectory, `${source.slug}.md`);
    const destinationPath = path.join(destinationDirectory, `${source.slug}.md`);
    if (source.path !== sourcePath || fileSha256(sourcePath) !== source.markdownSha256) {
      throw new Error(`Canonical fixture changed before raster session creation: ${source.slug}`);
    }
    fs.copyFileSync(sourcePath, destinationPath, fs.constants.COPYFILE_EXCL);
    if (fileSha256(destinationPath) !== source.markdownSha256) {
      throw new Error(`Canonical fixture copy differs from its source pin: ${source.slug}`);
    }
  }
  const copiedNames = fs.readdirSync(destinationDirectory).sort();
  if (
    copiedNames.length !== expectedSlugs.length ||
    copiedNames.some((name, index) => name !== `${expectedSlugs[index]}.md`)
  ) {
    throw new Error("Canonical fixture copy does not contain the exact nonempty inventory");
  }
}

function runStep(
  request: ManagedFullCompareRequest,
  outputRoot: string,
  steps: ManagedStep[],
  label: string,
  argv: string[],
) {
  const result = runManagedCommand(
    outputRoot,
    label,
    argv,
    request.repositoryRoot,
    commandTool(request, argv),
  );
  recordManagedStep(steps, result);
  requireCommandSuccess(result);
  return result;
}

function commandTool(request: ManagedFullCompareRequest, argv: string[]): ManagedToolPin {
  if (argv[0] === request.tools.bun.path) return { kind: "bun", ...request.tools.bun };
  if (argv[0] === request.tools.python.path) return { kind: "python3", ...request.tools.python };
  throw new Error("Managed command executable does not match its pinned Bun/Python tool");
}

function assertNativeFrameInputMode(input: ManagedFullCompareRequest["languages"]["en"]): void {
  if (input.nativeFrameEvidence === undefined && input.nativeFrameCapture === undefined) {
    throw new Error("Formal Mermaid comparison requires a native frame sidecar or capture pin");
  }
  if (input.nativeFrameEvidence !== undefined && input.nativeFrameCapture === undefined) {
    throw new Error("Native frame evidence requires the managed probe/config/font pins");
  }
}

function requireNativeProductScoreEvidence(
  score: ReturnType<typeof validateLanguageScoreEvidence> | undefined,
): ReturnType<typeof validateLanguageScoreEvidence> {
  if (score === undefined) {
    throw new Error("Formal Mermaid comparison requires native product score evidence");
  }
  return score;
}

function assertNativeFramePins(
  evidence: ReturnType<typeof verifyNativeFrameEvidence>,
  capture:
    | NonNullable<ManagedFullCompareRequest["languages"]["en"]["nativeFrameCapture"]>
    | undefined,
): void {
  if (capture === undefined) throw new Error("Native frame managed pins are missing");
  for (const [actual, expected, label] of [
    [evidence.probeBinary, capture.probeBinary, "probe"],
    [evidence.config, capture.config, "config"],
    [evidence.fontContext, capture.fontContext, "font context"],
  ] as const) {
    if (actual.path !== expected.path || actual.sha256 !== expected.sha256) {
      throw new Error(`Native frame ${label} pin differs from the managed request`);
    }
  }
}

function loadNativeFrameEvidence(
  request: ManagedFullCompareRequest,
  input: ManagedFullCompareRequest["languages"]["en"],
  language: LanguageName,
  slugs: readonly string[],
): { evidence: ReturnType<typeof verifyNativeFrameEvidence>; pin: FilePin } | undefined {
  if (input.nativeFrameEvidence === undefined) return undefined;
  const evidence = verifyNativeFrameEvidence(
    readJson(input.nativeFrameEvidence.path),
    language,
    slugs,
  );
  if (
    evidence.sourceManifest.path !== request.sourceManifest.path ||
    evidence.sourceManifest.sha256 !== request.sourceManifest.sha256
  ) {
    throw new Error(`${language} native frame evidence has a different source manifest`);
  }
  assertNativeFramePins(evidence, input.nativeFrameCapture);
  return { evidence, pin: filePin(input.nativeFrameEvidence.path) };
}

function writeNativeProductScores(
  language: LanguageName,
  slugs: readonly string[],
  pairs: ReferencePair[],
  frames: ReturnType<typeof verifyNativeFrameEvidence>["rows"],
  projected: Awaited<ReturnType<typeof scoreNativeFramePaintProjection>>,
  comparisonDirectory: string,
  inventory: unknown,
): ReturnType<typeof validateLanguageScoreEvidence> {
  const output = buildNativeProductComparison(
    pairs,
    frames,
    projected,
    comparisonDirectory,
    slugs.length,
    inventory,
  );
  return validateLanguageScoreEvidence(
    language,
    [...slugs],
    output.scores,
    inventory,
    output.context,
    parsePaintProjectedReferenceEvidence(readJson(output.evidencePath)),
    {
      role: "native-product",
      pngSha256BySlug: new Map(frames.map((frame) => [frame.slug, frame.png.sha256])),
    },
    output.failures,
  );
}

function buildNativeProductComparison(
  pairs: ReferencePair[],
  frames: ReturnType<typeof verifyNativeFrameEvidence>["rows"],
  projected: Awaited<ReturnType<typeof scoreNativeFramePaintProjection>>,
  comparisonDirectory: string,
  expectedSlugCount: number,
  inventory: unknown,
) {
  const directory = path.join(comparisonDirectory, "native-product-comparison");
  fs.mkdirSync(directory, { mode: 0o700 });
  if (fileSha256(projected.evidencePath) !== projected.evidenceSha256) {
    throw new Error("Native projected reference evidence differs from its score pin");
  }
  const projectedEvidence = parsePaintProjectedReferenceEvidence(readJson(projected.evidencePath));
  const nativePngBySlug = new Map(frames.map((frame) => [frame.slug, frame.png]));
  assertPaintProjectedReferenceFilePins(
    projectedEvidence,
    directory,
    path.dirname(pairs[0]?.officialPath ?? directory),
    path.dirname(pairs[0]?.katanaPath ?? directory),
    path.dirname(pairs[0]?.katanaSvgPath ?? directory),
    path.dirname(projected.evidencePath),
    nativePngBySlug,
  );
  const projectedReferences = buildNativePaintReferences(
    projectedEvidence,
    frames,
    projected.evidenceSha256,
  );
  const magickDirectory = path.join(directory, "magick"),
    scoreBatch = scoreNativeProductRows(
      pairs,
      frames,
      projected.scores,
      projectedReferences,
      new MagickOps(magickDirectory, null, "dark"),
      magickDirectory,
    ),
    evidencePath = path.join(directory, "paint-projected-reference-evidence.json"),
    context = nativeProductContext(expectedSlugCount, evidencePath, projected.evidenceSha256);
  fs.copyFileSync(projected.evidencePath, evidencePath, fs.constants.COPYFILE_EXCL);
  writeNativeProductFiles(directory, scoreBatch.scores, scoreBatch.failures, inventory, context);
  return { scores: scoreBatch.scores, failures: scoreBatch.failures, evidencePath, context };
}

function buildNativePaintReferences(
  evidence: PaintProjectedReferenceEvidence,
  frames: ReturnType<typeof verifyNativeFrameEvidence>["rows"],
  evidenceSha256: string,
): ReadonlyMap<string, NativeProductPaintReference> {
  const framesBySlug = new Map(frames.map((frame) => [frame.slug, frame]));
  return new Map(
    evidence.pairs.map((pair) => {
      const frame = framesBySlug.get(pair.slug);
      if (frame === undefined) {
        throw new Error(`Missing native frame for projected reference: ${pair.slug}`);
      }
      if (
        path.resolve(pair.candidate.sourcePngPath) !== path.resolve(frame.png.path) ||
        pair.candidate.sourcePngSha256 !== frame.png.sha256
      ) {
        throw new Error(`Projected candidate PNG is not the native frame: ${pair.slug}`);
      }
      return [
        pair.slug,
        {
          projectedPng: {
            path: pair.projectedReference.pngPath,
            sha256: pair.projectedReference.pngSha256,
          },
          officialPngSha256: pair.official.sourcePngSha256,
          actualPngSha256: frame.png.sha256,
          evidenceSha256,
        },
      ] as const;
    }),
  );
}

function nativeProductContext(
  expectedSlugCount: number,
  evidencePath: string,
  evidenceSha256: string,
) {
  return {
    mode: "strictVendorOriginal" as const,
    scope: "formal" as const,
    expectedSlugCount,
    emptySlugCount: 0 as const,
    scoreThreshold: 99 as const,
    formalAccepted: false as const,
    independentGold: false as const,
    comparisonRole: "native-product" as const,
    scoreSemantics:
      "actual-png-paint-aligned-rgb-channel-edge-support-v2+paint-projected-reference-v1" as const,
    paintProjectedReferenceEvidence: { path: path.basename(evidencePath), sha256: evidenceSha256 },
  };
}

function writeNativeProductFiles(
  directory: string,
  scores: ReturnType<typeof scoreNativeProductRows>["scores"],
  failures: NativeProductDimensionFailure[],
  inventory: unknown,
  context: ReturnType<typeof nativeProductContext>,
): void {
  for (const [name, value] of [
    ["scores.json", scores],
    ["failures.json", failures],
    ["fixture-inventory.json", inventory],
    ["reference-context.json", context],
  ] as const) {
    fs.writeFileSync(path.join(directory, name), `${JSON.stringify(value, null, 2)}\n`, {
      flag: "wx",
      mode: 0o600,
    });
  }
}

function captureNativeFrame(
  request: ManagedFullCompareRequest,
  language: LanguageName,
  capture: NonNullable<ManagedFullCompareRequest["languages"]["en"]["nativeFrameCapture"]>,
  svgDirectory: string,
  rasterDirectory: string,
  workspace: string,
  slugs: readonly string[],
): { evidence: ReturnType<typeof verifyNativeFrameEvidence>; pin: FilePin } {
  const config = readNativeFrameCaptureConfig(capture.config),
    captureDirectory = path.join(workspace, "native-frame"),
    evidence = captureNativeFrameEvidence({
      language,
      sourceSvgDirectory: svgDirectory,
      sourcePngDirectory: rasterDirectory,
      outputDirectory: captureDirectory,
      probeBinary: capture.probeBinary,
      runtime: request.runtime,
      sourceManifest: request.sourceManifest,
      config: capture.config,
      fontContext: capture.fontContext,
      root: request.repositoryRoot,
      scale: config.scale,
      scrollY: config.scrollY,
      slugs,
    });
  return { evidence, pin: filePin(path.join(captureDirectory, "native-frame-evidence.json")) };
}

function assertNativeFrameInputs(
  evidence: ReturnType<typeof verifyNativeFrameEvidence>,
  svgDirectory: string,
  rasterDirectory: string,
  slugs: readonly string[],
): void {
  if (evidence.rows.length !== slugs.length)
    throw new Error("Native frame input inventory differs");
  for (const slug of slugs) {
    const row = evidence.rows.find((value) => value.slug === slug);
    if (row === undefined) throw new Error(`Missing native frame input: ${slug}`);
    const expectedSvg = path.resolve(svgDirectory, `${slug}.svg`),
      expectedPng = path.resolve(rasterDirectory, `${slug}.png`);
    if (row.inputSvg.path !== expectedSvg || fileSha256(expectedSvg) !== row.inputSvg.sha256) {
      throw new Error(`Native frame input differs from current candidate SVG: ${slug}`);
    }
    if (row.sourcePng.path !== expectedPng || fileSha256(expectedPng) !== row.sourcePng.sha256) {
      throw new Error(`Native frame source differs from current raster candidate: ${slug}`);
    }
  }
}

function recordManagedStep(steps: ManagedStep[], result: ManagedCommandResult): void {
  steps.push({
    label: result.label,
    argv: result.argv,
    cwd: result.cwd,
    stdoutPath: result.stdoutPath,
    stderrPath: result.stderrPath,
    exitPath: result.exitPath,
    actualExitCode: result.exitCode,
    signal: result.signal,
    spawnError: result.spawnError,
  });
}

function assertWriterResponse(
  response: WriterResponse | BindingResponse,
  expectedPath: string,
  expectedInventory: string[],
  role: "candidate" | "binding",
): void {
  if (response.path !== expectedPath || !/^[a-f0-9]{64}$/.test(response.sha256)) {
    throw new Error(`${role} writer returned an unexpected config path or SHA`);
  }
  if (JSON.stringify(response.inventory) !== JSON.stringify(expectedInventory)) {
    throw new Error(`${role} writer returned a different fixture inventory`);
  }
  if (role === "candidate" && "role" in response && response.role !== "candidate") {
    throw new Error("Candidate session writer returned a different role");
  }
}

function createEmptyDirectory(directory: string): void {
  fs.mkdirSync(directory, { mode: 0o700 });
  const stat = fs.lstatSync(directory);
  if (stat.isSymbolicLink() || !stat.isDirectory() || fs.realpathSync(directory) !== directory) {
    throw new Error(`Managed output directory is not canonical: ${directory}`);
  }
}

function readJson(filePath: string): unknown {
  return JSON.parse(fs.readFileSync(filePath, "utf8")) as unknown;
}

function filePin(filePath: string): FilePin {
  const stat = fs.statSync(filePath);
  return { path: filePath, sha256: fileSha256(filePath), size: stat.size };
}

interface WriterResponse {
  path: string;
  sha256: string;
  inventory: string[];
  role: "candidate";
}

interface BindingResponse {
  path: string;
  sha256: string;
  inventory: string[];
}
