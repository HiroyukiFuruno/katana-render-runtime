import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import type { DiagramThemeName } from "./diagram_theme";
import { fileSha256 } from "./mermaid-pair-manifest";
import {
  type PaintProjectedReferenceInput,
  type PaintProjectedReferenceRecord,
  rasterizePaintProjectedReferenceBatch,
} from "./neutral-svg-browser-rasterizer";
import type { MagickOps, PaintProjectedImageComparison } from "./reference_image_ops";
import type { ReferencePair } from "./reference_pair_repository";
import {
  assertCompletePaintProjectedTargetSlugs,
  compareSvgGeometryForNeutralRaster,
  isPaintProjectedReferenceTargetSlug,
  PAINT_PROJECTED_REFERENCE_POLICY,
  prepareNeutralSvg,
} from "./reference_svg_geometry";

const EVIDENCE_SCHEMA = "krr-mermaid-paint-projected-reference-evidence-v1";
const STRUCTURAL_POLICY = "ordered-svg-tree-and-nonpaint-plus-computed-paint-visibility-v2";
const RENDERER = "pinned-playwright-chromium-svg-capture-v1";

export interface PaintProjectedReferenceScore {
  policy: typeof PAINT_PROJECTED_REFERENCE_POLICY;
  score: number;
  structuralIntegrityAccepted: boolean;
  reasons: string[];
  maxTransformDelta: number;
  fullCanvasRmseScore: number;
  fullCanvasMaeScore: number;
  contentRmseScore: number;
  contentMaeScore: number;
  width: number;
  height: number;
  roi: PaintProjectedImageComparison["roi"];
  evidenceSha256: string;
}

export interface PaintProjectedReferenceRun {
  scores: Map<string, PaintProjectedReferenceScore>;
  evidencePath: string;
  evidenceSha256: string;
  targetCount: number;
}

interface ScoredPaintProjectedPair {
  slug: string;
  structuralIntegrity: {
    accepted: boolean;
    reasons: string[];
    maxTransformDelta: number;
    signatureSha256: string;
  };
  official: { svgPath: string; svgSha256: string; sourcePngPath: string; sourcePngSha256: string };
  candidate: { svgPath: string; svgSha256: string; sourcePngPath: string; sourcePngSha256: string };
  projectedReference: {
    svgPath: string;
    svgSha256: string;
    pngPath: string;
    pngSha256: string;
    width: number;
    height: number;
    graphicsElementCount: number;
    paintVisibilitySha256: string;
  };
  projectedMetrics: PaintProjectedImageComparison;
}

export async function scorePaintProjectedReferenceTargets(
  pairs: ReferencePair[],
  comparisonDirectory: string,
  theme: DiagramThemeName,
  magick: MagickOps,
): Promise<PaintProjectedReferenceRun> {
  const targets = pairs.filter((pair) => isPaintProjectedReferenceTargetSlug(pair.slug));
  assertCompleteLanguageTargetSet(pairs, targets);
  const outputDirectory = path.join(comparisonDirectory, "paint-projected-reference");
  const prepared = prepareInputs(targets, outputDirectory);
  const rendered = await rasterizePaintProjectedReferenceBatch(
    prepared.inputs,
    outputDirectory,
    theme,
  );
  const evidencePairs = compareTargetPairs(
    targets,
    prepared.sources,
    rendered.records,
    magick,
    outputDirectory,
  );
  assertPairSourcePins(evidencePairs);
  const evidence = buildEvidence(rendered, evidencePairs);
  const evidencePath = path.join(comparisonDirectory, "paint-projected-reference-evidence.json"),
    bytes = Buffer.from(`${JSON.stringify(evidence, null, 2)}\n`);
  fs.writeFileSync(evidencePath, bytes, { flag: "wx", mode: 0o600 });
  assertPairSourcePins(evidencePairs);
  const evidenceSha256 = digest(bytes);
  return {
    scores: new Map(
      evidencePairs.map((pair) => [pair.slug, scoreFromEvidence(pair, evidenceSha256)]),
    ),
    evidencePath,
    evidenceSha256,
    targetCount: targets.length,
  };
}

function assertCompleteLanguageTargetSet(pairs: ReferencePair[], targets: ReferencePair[]) {
  const slugs = targets.map((pair) => pair.slug);
  assertCompletePaintProjectedTargetSlugs(pairs.length, slugs);
}

function prepareInputs(pairs: ReferencePair[], outputDirectory: string) {
  if (pairs.length === 0) throw new Error("Paint projection target inventory is empty");
  const inputs: PaintProjectedReferenceInput[] = [],
    sources = new Map<string, { official: string; candidate: string }>();
  for (const pair of pairs) {
    const official = readPinnedSvg(pair.officialSvgPath),
      candidate = readPinnedSvg(pair.katanaSvgPath);
    sources.set(pair.slug, { official: official.svg, candidate: candidate.svg });
    inputs.push(paintInput(pair, official, candidate, outputDirectory));
  }
  return { inputs, sources };
}

function readPinnedSvg(filePath: string) {
  if (fs.lstatSync(filePath).isSymbolicLink())
    throw new Error(`SVG source is a symlink: ${filePath}`);
  const bytes = fs.readFileSync(filePath);
  return { svg: bytes.toString("utf8"), sha256: digest(bytes) };
}

function paintInput(
  pair: ReferencePair,
  official: ReturnType<typeof readPinnedSvg>,
  candidate: ReturnType<typeof readPinnedSvg>,
  outputDirectory: string,
): PaintProjectedReferenceInput {
  const officialPngSha256 = fileSha256(pair.officialPath),
    candidatePngSha256 = fileSha256(pair.katanaPath);
  return {
    slug: pair.slug,
    officialSvgPath: pair.officialSvgPath,
    officialSvg: official.svg,
    officialSvgSha256: official.sha256,
    officialPngPath: pair.officialPath,
    officialPngSha256,
    candidateSvgPath: pair.katanaSvgPath,
    candidateSvg: candidate.svg,
    candidateSvgSha256: candidate.sha256,
    candidatePngPath: pair.katanaPath,
    candidatePngSha256,
    outputSvgPath: path.join(outputDirectory, `${pair.slug}-reference.svg`),
    outputPngPath: path.join(outputDirectory, `${pair.slug}-reference.png`),
  };
}

function compareTargetPairs(
  pairs: ReferencePair[],
  sources: Map<string, { official: string; candidate: string }>,
  records: PaintProjectedReferenceRecord[],
  magick: MagickOps,
  outputDirectory: string,
): ScoredPaintProjectedPair[] {
  return pairs.map((pair) => compareTargetPair(pair, sources, records, magick, outputDirectory));
}

function compareTargetPair(
  pair: ReferencePair,
  sources: Map<string, { official: string; candidate: string }>,
  records: PaintProjectedReferenceRecord[],
  magick: MagickOps,
  outputDirectory: string,
): ScoredPaintProjectedPair {
  const source = sources.get(pair.slug),
    rendered = recordFor(records, pair.slug);
  if (source === undefined) throw new Error(`Paint projection source is missing: ${pair.slug}`);
  const structuralIntegrity = compareSvgGeometryForNeutralRaster(
    prepareNeutralSvg(source.official),
    prepareNeutralSvg(source.candidate),
  );
  if (rendered.officialPaintVisibilitySha256 !== rendered.paintVisibilitySha256) {
    structuralIntegrity.accepted = false;
    structuralIntegrity.reasons.push("computed-paint-visibility");
  }
  const projectedMetrics = magick.comparePaintProjectedImages(
    rendered.projectedReferencePngPath,
    rendered.candidatePngPath,
    outputDirectory,
    pair.slug,
  );
  return pairEvidence(pair, rendered, structuralIntegrity, projectedMetrics);
}

function pairEvidence(
  pair: ReferencePair,
  rendered: PaintProjectedReferenceRecord,
  structuralIntegrity: ScoredPaintProjectedPair["structuralIntegrity"],
  projectedMetrics: PaintProjectedImageComparison,
): ScoredPaintProjectedPair {
  return {
    slug: pair.slug,
    structuralIntegrity,
    official: {
      svgPath: rendered.officialSvgPath,
      svgSha256: rendered.officialSvgSha256,
      sourcePngPath: rendered.officialPngPath,
      sourcePngSha256: rendered.officialPngSha256,
    },
    candidate: {
      svgPath: rendered.candidateSvgPath,
      svgSha256: rendered.candidateSvgSha256,
      sourcePngPath: rendered.candidatePngPath,
      sourcePngSha256: rendered.candidatePngSha256,
    },
    projectedReference: {
      svgPath: rendered.projectedReferenceSvgPath,
      svgSha256: rendered.projectedReferenceSvgSha256,
      pngPath: rendered.projectedReferencePngPath,
      pngSha256: rendered.projectedReferencePngSha256,
      width: rendered.width,
      height: rendered.height,
      graphicsElementCount: rendered.graphicsElementCount,
      paintVisibilitySha256: rendered.paintVisibilitySha256,
    },
    projectedMetrics,
  };
}

function recordFor(
  records: PaintProjectedReferenceRecord[],
  slug: string,
): PaintProjectedReferenceRecord {
  const matches = records.filter((record) => record.slug === slug);
  if (matches.length !== 1 || matches[0] === undefined) {
    throw new Error(`Paint projected reference record is missing or duplicated: ${slug}`);
  }
  return matches[0];
}

function buildEvidence(
  rendered: Awaited<ReturnType<typeof rasterizePaintProjectedReferenceBatch>>,
  pairs: ScoredPaintProjectedPair[],
) {
  return {
    schema: EVIDENCE_SCHEMA,
    policy: PAINT_PROJECTED_REFERENCE_POLICY,
    structuralPolicy: STRUCTURAL_POLICY,
    renderer: RENDERER,
    host: { platform: process.platform, arch: process.arch },
    browser: {
      version: rendered.browserVersion,
      executablePath: rendered.browserExecutablePath,
      executableSha256: rendered.browserExecutableSha256,
      viewport: rendered.viewport,
    },
    pairs,
  };
}

function scoreFromEvidence(
  pair: ScoredPaintProjectedPair,
  evidenceSha256: string,
): PaintProjectedReferenceScore {
  return {
    policy: PAINT_PROJECTED_REFERENCE_POLICY,
    score: pair.projectedMetrics.score,
    structuralIntegrityAccepted: pair.structuralIntegrity.accepted,
    reasons: pair.structuralIntegrity.reasons,
    maxTransformDelta: pair.structuralIntegrity.maxTransformDelta,
    fullCanvasRmseScore: pair.projectedMetrics.fullCanvasRmseScore,
    fullCanvasMaeScore: pair.projectedMetrics.fullCanvasMaeScore,
    contentRmseScore: pair.projectedMetrics.contentRmseScore,
    contentMaeScore: pair.projectedMetrics.contentMaeScore,
    width: pair.projectedMetrics.width,
    height: pair.projectedMetrics.height,
    roi: pair.projectedMetrics.roi,
    evidenceSha256,
  };
}

function assertPairSourcePins(pairs: ScoredPaintProjectedPair[]) {
  for (const pair of pairs) {
    for (const source of [pair.official, pair.candidate]) {
      if (
        fileSha256(source.svgPath) !== source.svgSha256 ||
        fileSha256(source.sourcePngPath) !== source.sourcePngSha256
      ) {
        throw new Error(`Paint projection source changed during comparison: ${pair.slug}`);
      }
    }
    if (
      fileSha256(pair.projectedReference.svgPath) !== pair.projectedReference.svgSha256 ||
      fileSha256(pair.projectedReference.pngPath) !== pair.projectedReference.pngSha256
    ) {
      throw new Error(`Paint projected reference changed during comparison: ${pair.slug}`);
    }
  }
}

function digest(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}
