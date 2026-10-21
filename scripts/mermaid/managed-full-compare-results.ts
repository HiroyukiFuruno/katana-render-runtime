import { z } from "zod";
import { ReferenceScorePolicy } from "./reference_score_policy";
import { isNeutralGeometryTargetSlug } from "./reference_svg_geometry";

const projectedRasterBoundsFields = {
  roi: z
    .object({
      x: z.number().int().nonnegative(),
      y: z.number().int().nonnegative(),
      width: z.number().int().positive(),
      height: z.number().int().positive(),
    })
    .strict(),
  width: z.number().int().positive(),
  height: z.number().int().positive(),
};

const scoreRowSchema = z
  .object({
    slug: z.string().min(1),
    score: z.number().finite().min(0).max(100),
    rgbScore: z.number().finite().min(0).max(100),
    scoreSemantics: z.enum([
      "rgb-v1",
      "paint-projected-reference-v1",
      "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1",
    ]),
    canvasRmseScore: z.number().finite().min(0).max(100),
    canvasMaeScore: z.number().finite().min(0).max(100),
    contentRmseScore: z.number().finite().min(0).max(100),
    contentMaeScore: z.number().finite().min(0).max(100),
    minScore: z.number().finite().min(99).max(100),
    exceptionReason: z.string(),
    passed: z.boolean(),
    paintProjectedReference: z
      .object({
        policy: z.literal("raw-svg-geometry-candidate-computed-paint-v1"),
        score: z.number().finite().min(0).max(100),
        structuralIntegrityAccepted: z.boolean(),
        reasons: z.array(z.string()),
        maxTransformDelta: z.number().finite().min(0),
        fullCanvasRmseScore: z.number().finite().min(0).max(100),
        fullCanvasMaeScore: z.number().finite().min(0).max(100),
        contentRmseScore: z.number().finite().min(0).max(100),
        contentMaeScore: z.number().finite().min(0).max(100),
        evidenceSha256: z.string().regex(/^[a-f0-9]{64}$/),
        ...projectedRasterBoundsFields,
      })
      .strict()
      .optional(),
    actualPngShape: z
      .object({
        policy: z.literal("rgb-channel-edge-support-v1"),
        score: z.number().finite().min(0).max(100),
        fullCanvasScore: z.number().finite().min(0).max(100),
        foregroundUnionScore: z.number().finite().min(0).max(100),
        officialPngSha256: z.string().regex(/^[a-f0-9]{64}$/),
        actualPngSha256: z.string().regex(/^[a-f0-9]{64}$/),
        ...projectedRasterBoundsFields,
      })
      .strict()
      .optional(),
  })
  .strict();

const sourceRasterRecordSchema = z
  .object({
    svgPath: z.string().min(1),
    svgSha256: z.string().regex(/^[a-f0-9]{64}$/),
    sourcePngPath: z.string().min(1),
    sourcePngSha256: z.string().regex(/^[a-f0-9]{64}$/),
  })
  .strict();

const projectedReferenceSchema = z
  .object({
    svgPath: z.string().min(1),
    svgSha256: z.string().regex(/^[a-f0-9]{64}$/),
    pngPath: z.string().min(1),
    pngSha256: z.string().regex(/^[a-f0-9]{64}$/),
    width: z.number().int().positive(),
    height: z.number().int().positive(),
    graphicsElementCount: z.number().int().positive(),
    paintVisibilitySha256: z.string().regex(/^[a-f0-9]{64}$/),
  })
  .strict();

export const paintProjectedReferenceEvidenceSchema = z
  .object({
    schema: z.literal("krr-mermaid-paint-projected-reference-evidence-v1"),
    policy: z.literal("raw-svg-geometry-candidate-computed-paint-v1"),
    structuralPolicy: z.literal("ordered-svg-tree-and-nonpaint-plus-computed-paint-visibility-v2"),
    renderer: z.literal("pinned-playwright-chromium-svg-capture-v1"),
    host: z.object({ platform: z.string().min(1), arch: z.string().min(1) }).strict(),
    browser: z
      .object({
        version: z.string().min(1),
        executablePath: z.string().min(1),
        executableSha256: z.string().regex(/^[a-f0-9]{64}$/),
        viewport: z
          .object({
            width: z.literal(1520),
            height: z.literal(845),
            deviceScaleFactor: z.literal(1),
          })
          .strict(),
      })
      .strict(),
    pairs: z
      .array(
        z
          .object({
            slug: z.string().min(1),
            structuralIntegrity: z
              .object({
                accepted: z.boolean(),
                reasons: z.array(z.string()),
                maxTransformDelta: z.number().finite().min(0),
                signatureSha256: z.string().regex(/^[a-f0-9]{64}$/),
              })
              .strict(),
            official: sourceRasterRecordSchema,
            candidate: sourceRasterRecordSchema,
            projectedReference: projectedReferenceSchema,
            projectedMetrics: z
              .object({
                fullCanvasRmseScore: z.number().finite().min(0).max(100),
                fullCanvasMaeScore: z.number().finite().min(0).max(100),
                contentRmseScore: z.number().finite().min(0).max(100),
                contentMaeScore: z.number().finite().min(0).max(100),
                score: z.number().finite().min(0).max(100),
                ...projectedRasterBoundsFields,
              })
              .strict(),
          })
          .strict(),
      )
      .min(1),
  })
  .strict();

const inventorySchema = z
  .object({
    contract: z.literal("explicit-fixture-directory-and-glob"),
    expected: z.array(z.string()),
    scored: z.array(z.string()),
    skippedEmpty: z.array(z.object({ slug: z.string(), reason: z.string() }).strict()),
  })
  .strict();

const contextSchema = z
  .object({
    mode: z.literal("strictVendorOriginal"),
    scope: z.literal("formal"),
    expectedSlugCount: z.number().int().positive(),
    emptySlugCount: z.literal(0),
    scoreThreshold: z.literal(99),
    formalAccepted: z.literal(false),
    independentGold: z.literal(false),
    comparisonRole: z.enum(["reference", "native-product"]).optional(),
    scoreSemantics: z.enum([
      "rgb-v1",
      "rgb-v1+paint-projected-reference-v1",
      "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1",
    ]),
    paintProjectedReferenceEvidence: z
      .object({
        path: z.literal("paint-projected-reference-evidence.json"),
        sha256: z.string().regex(/^[a-f0-9]{64}$/),
      })
      .strict()
      .optional(),
  })
  .strict();

export interface LanguageScoreAcceptance {
  fixtureCount: number;
  minimumScore: number;
  accepted: boolean;
  scores: Array<z.infer<typeof scoreRowSchema>>;
}

export type PaintProjectedReferenceEvidence = z.infer<typeof paintProjectedReferenceEvidenceSchema>;

export function parsePaintProjectedReferenceEvidence(
  value: unknown,
): PaintProjectedReferenceEvidence {
  return paintProjectedReferenceEvidenceSchema.parse(value);
}

export interface FullScoreAcceptance {
  language: "en" | "ja";
  fixtureCount: number;
  minimumScore: number;
  accepted: boolean;
}

export function requireFullScoreAcceptance(languages: FullScoreAcceptance[]): void {
  const expected = [
    { language: "en", fixtureCount: 59 },
    { language: "ja", fixtureCount: 58 },
  ];
  if (
    languages.length !== expected.length ||
    expected.some((row, index) => {
      const actual = languages[index];
      return (
        actual?.language !== row.language ||
        actual.fixtureCount !== row.fixtureCount ||
        !Number.isFinite(actual.minimumScore) ||
        actual.minimumScore < 99 ||
        actual.accepted !== true
      );
    })
  ) {
    throw new Error("Full Mermaid comparison requires EN59 and JA58 with every score at least 99");
  }
}

export function validateLanguageScoreEvidence(
  language: "en" | "ja",
  expectedSlugs: string[],
  scoresValue: unknown,
  inventoryValue: unknown,
  contextValue: unknown,
  paintProjectedReferenceEvidenceValue?: unknown,
  expectation: ScoreEvidenceExpectation = { role: "reference" },
): LanguageScoreAcceptance {
  const expectedRole = expectation.role,
    expectedNativePngSha256 =
      expectation.role === "native-product" ? expectation.pngSha256BySlug : undefined;
  const expectedCount = language === "en" ? 59 : 58;
  validateExpectedSlugSet(language, expectedSlugs, expectedCount);
  const scores = z.array(scoreRowSchema).parse(scoresValue);
  const inventory = inventorySchema.parse(inventoryValue);
  const context = contextSchema.parse(contextValue);
  const geometrySlugs = expectedSlugs.filter(isNeutralGeometryTargetSlug);
  validateScoreContext(language, context, geometrySlugs, expectedRole, expectedNativePngSha256);
  const projectedEvidence =
    geometrySlugs.length > 0
      ? parsePaintProjectedReferenceEvidence(paintProjectedReferenceEvidenceValue)
      : null;
  if (projectedEvidence !== null) validateProjectedManifest(projectedEvidence, geometrySlugs);
  if (
    !isAcceptedScoreSet(
      scores,
      inventory,
      expectedSlugs,
      expectedCount,
      projectedEvidence,
      context,
      expectedRole,
      expectedNativePngSha256,
    )
  ) {
    throw new Error(`${language} comparison did not score the exact strict inventory`);
  }
  return {
    fixtureCount: scores.length,
    minimumScore: Math.min(...scores.map((row) => row.score)),
    accepted: scores.every((row) => row.score >= 99 && row.passed),
    scores,
  };
}

function validateExpectedSlugSet(
  language: "en" | "ja",
  expectedSlugs: string[],
  expectedCount: number,
): void {
  if (expectedSlugs.length !== expectedCount || new Set(expectedSlugs).size !== expectedCount) {
    throw new Error(`${language} expected fixture inventory is not the fixed complete inventory`);
  }
}

function validateScoreContext(
  language: "en" | "ja",
  context: z.infer<typeof contextSchema>,
  geometrySlugs: string[],
  expectedRole: "reference" | "native-product",
  expectedNativePngSha256: ReadonlyMap<string, string> | undefined,
): void {
  if (
    (expectedRole === "native-product" && context.comparisonRole !== "native-product") ||
    (expectedRole === "reference" && context.comparisonRole === "native-product")
  ) {
    throw new Error(`${language} comparison role does not match its trusted caller`);
  }
  if (
    expectedRole === "native-product" &&
    geometrySlugs.some((slug) => expectedNativePngSha256?.has(slug) !== true)
  ) {
    throw new Error(`${language} native PNG pins are incomplete for geometry acceptance`);
  }
  const expectedSemantics =
    geometrySlugs.length === 0
      ? "rgb-v1"
      : expectedRole === "native-product"
        ? "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1"
        : "rgb-v1+paint-projected-reference-v1";
  if (
    context.scoreSemantics !== expectedSemantics ||
    (geometrySlugs.length === 0) === (context.paintProjectedReferenceEvidence !== undefined)
  ) {
    throw new Error(`${language} comparison score semantics are incomplete`);
  }
}

function isAcceptedScoreSet(
  scores: ScoreRow[],
  inventory: z.infer<typeof inventorySchema>,
  expectedSlugs: string[],
  expectedCount: number,
  projectedEvidence: z.infer<typeof paintProjectedReferenceEvidenceSchema> | null,
  context: z.infer<typeof contextSchema>,
  expectedRole: "reference" | "native-product",
  expectedNativePngSha256: ReadonlyMap<string, string> | undefined,
): boolean {
  if (
    !sameStrings(inventory.expected, expectedSlugs) ||
    !sameStrings(inventory.scored, expectedSlugs) ||
    inventory.skippedEmpty.length !== 0 ||
    context.expectedSlugCount !== expectedCount ||
    scores.length !== expectedCount ||
    !sameStrings(
      scores.map((row) => row.slug),
      expectedSlugs,
    )
  ) {
    return false;
  }
  return !scores.some((row) =>
    scoreRowIsInvalid(row, projectedEvidence, context, expectedRole, expectedNativePngSha256),
  );
}

type ScoreRow = z.infer<typeof scoreRowSchema>;
type ProjectedPair = z.infer<typeof paintProjectedReferenceEvidenceSchema>["pairs"][number];
type ProjectedScore = NonNullable<ScoreRow["paintProjectedReference"]>;
type ScoreEvidenceExpectation =
  | { role: "reference" }
  | { role: "native-product"; pngSha256BySlug: ReadonlyMap<string, string> };

function scoreRowIsInvalid(
  row: ScoreRow,
  projectedEvidence: z.infer<typeof paintProjectedReferenceEvidenceSchema> | null,
  context: z.infer<typeof contextSchema>,
  expectedRole: "reference" | "native-product",
  expectedNativePngSha256: ReadonlyMap<string, string> | undefined,
): boolean {
  const threshold = new ReferenceScorePolicy(99).thresholdFor(row.slug),
    geometry = isNeutralGeometryTargetSlug(row.slug),
    projected = row.paintProjectedReference,
    shape = row.actualPngShape,
    projectedScore = projected === undefined ? undefined : expectedProjectedScore(projected),
    shapeScore =
      shape === undefined ? undefined : Math.min(shape.fullCanvasScore, shape.foregroundUnionScore),
    rgbScore = Math.min(
      row.canvasRmseScore,
      row.canvasMaeScore,
      row.contentRmseScore,
      row.contentMaeScore,
    ),
    pair = projectedEvidence?.pairs.find((item) => item.slug === row.slug),
    semanticsValid = geometry
      ? expectedRole === "native-product"
        ? isNativeGeometryScoreValid(
            row,
            projected,
            shape,
            pair,
            projectedScore,
            shapeScore,
            context,
            expectedNativePngSha256,
          )
        : isReferenceGeometryScoreValid(row, projected, shape, pair, projectedScore, context)
      : row.scoreSemantics === "rgb-v1" && projected === undefined && shape === undefined;
  return (
    row.minScore !== threshold.minScore ||
    row.exceptionReason !== threshold.reason ||
    row.rgbScore !== rgbScore ||
    !semanticsValid ||
    row.score !==
      (geometry
        ? expectedRole === "native-product"
          ? Math.min(projectedScore ?? 0, shapeScore ?? 0)
          : projectedScore
        : rgbScore) ||
    row.passed !==
      (row.score >= row.minScore &&
        (projected?.structuralIntegrityAccepted ?? true) &&
        (shape === undefined || shape.score >= row.minScore))
  );
}

function isNativeGeometryScoreValid(
  row: ScoreRow,
  projected: ProjectedScore | undefined,
  shape: NonNullable<ScoreRow["actualPngShape"]> | undefined,
  pair: ProjectedPair | undefined,
  projectedScore: number | undefined,
  shapeScore: number | undefined,
  context: z.infer<typeof contextSchema>,
  expectedNativePngSha256: ReadonlyMap<string, string> | undefined,
): boolean {
  return (
    row.scoreSemantics === "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1" &&
    projected !== undefined &&
    shape !== undefined &&
    pair !== undefined &&
    context.paintProjectedReferenceEvidence?.sha256 === projected.evidenceSha256 &&
    projected.score === projectedScore &&
    shape.policy === "rgb-channel-edge-support-v1" &&
    shape.score === shapeScore &&
    shape.officialPngSha256 === pair.official.sourcePngSha256 &&
    expectedNativePngSha256?.get(row.slug) === shape.actualPngSha256 &&
    shape.width === pair.projectedMetrics.width &&
    shape.height === pair.projectedMetrics.height &&
    isRasterBoundsInsideCanvas(shape) &&
    projectedMatchesEvidence(projected, pair, projectedScore)
  );
}

function isRasterBoundsInsideCanvas(raster: NonNullable<ScoreRow["actualPngShape"]>): boolean {
  return (
    raster.roi.x + raster.roi.width <= raster.width &&
    raster.roi.y + raster.roi.height <= raster.height
  );
}

function isReferenceGeometryScoreValid(
  row: ScoreRow,
  projected: ProjectedScore | undefined,
  shape: NonNullable<ScoreRow["actualPngShape"]> | undefined,
  pair: ProjectedPair | undefined,
  projectedScore: number | undefined,
  context: z.infer<typeof contextSchema>,
): boolean {
  return (
    row.scoreSemantics === "paint-projected-reference-v1" &&
    projected !== undefined &&
    shape === undefined &&
    pair !== undefined &&
    context.paintProjectedReferenceEvidence?.sha256 === projected.evidenceSha256 &&
    projectedMatchesEvidence(projected, pair, projectedScore)
  );
}

function projectedMatchesEvidence(
  projected: ProjectedScore,
  pair: ProjectedPair,
  projectedScore: number | undefined,
): boolean {
  const metrics = pair.projectedMetrics,
    integrity = pair.structuralIntegrity;
  return (
    projected.score === projectedScore &&
    projected.structuralIntegrityAccepted === integrity.accepted &&
    sameStrings(projected.reasons, integrity.reasons) &&
    projected.maxTransformDelta === integrity.maxTransformDelta &&
    projected.fullCanvasRmseScore === metrics.fullCanvasRmseScore &&
    projected.fullCanvasMaeScore === metrics.fullCanvasMaeScore &&
    projected.contentRmseScore === metrics.contentRmseScore &&
    projected.contentMaeScore === metrics.contentMaeScore &&
    projected.width === metrics.width &&
    projected.height === metrics.height &&
    projected.roi.x === metrics.roi.x &&
    projected.roi.y === metrics.roi.y &&
    projected.roi.width === metrics.roi.width &&
    projected.roi.height === metrics.roi.height &&
    metrics.score === projectedScore
  );
}

function validateProjectedManifest(
  evidence: z.infer<typeof paintProjectedReferenceEvidenceSchema>,
  expectedSlugs: string[],
) {
  const actual = evidence.pairs.map((pair) => pair.slug);
  if (!sameStrings(actual, expectedSlugs))
    throw new Error("Paint-projected reference manifest inventory differs");
  for (const pair of evidence.pairs) {
    if (
      (pair.structuralIntegrity.accepted &&
        pair.projectedReference.paintVisibilitySha256.length !== 64) ||
      pair.projectedReference.graphicsElementCount < 1 ||
      pair.projectedReference.width !== pair.projectedMetrics.width ||
      pair.projectedReference.height !== pair.projectedMetrics.height ||
      pair.projectedMetrics.score !==
        Math.min(
          pair.projectedMetrics.fullCanvasRmseScore,
          pair.projectedMetrics.fullCanvasMaeScore,
          pair.projectedMetrics.contentRmseScore,
          pair.projectedMetrics.contentMaeScore,
        ) ||
      pair.projectedMetrics.roi.x + pair.projectedMetrics.roi.width >
        pair.projectedReference.width ||
      pair.projectedMetrics.roi.y + pair.projectedMetrics.roi.height >
        pair.projectedReference.height ||
      (pair.structuralIntegrity.accepted &&
        (pair.structuralIntegrity.reasons.length !== 0 ||
          pair.structuralIntegrity.maxTransformDelta > 0.00001)) ||
      (!pair.structuralIntegrity.accepted &&
        pair.structuralIntegrity.reasons.length === 0 &&
        pair.structuralIntegrity.maxTransformDelta <= 0.00001)
    ) {
      throw new Error(`Paint-projected reference manifest row is inconsistent: ${pair.slug}`);
    }
  }
}

function expectedProjectedScore(
  projected: NonNullable<z.infer<typeof scoreRowSchema>["paintProjectedReference"]>,
): number {
  return Math.min(
    projected.fullCanvasRmseScore,
    projected.fullCanvasMaeScore,
    projected.contentRmseScore,
    projected.contentMaeScore,
  );
}

function sameStrings(actual: string[], expected: string[]): boolean {
  return (
    actual.length === expected.length && actual.every((value, index) => value === expected[index])
  );
}
