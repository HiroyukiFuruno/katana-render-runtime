import fs from "node:fs";
import { z } from "zod";

export const SOURCE_COMMIT = "c01f0165059e472f45472cccddc573d32c13548a";
export const SOURCE_REPOSITORY = "https://github.com/HiroyukiFuruno/katana-resvg";
export const UPSTREAM_REPOSITORY = "https://github.com/linebender/resvg";
export const FEATURES = [
  "memmap-fonts",
  "raster-images",
  "svgz",
  "system-fonts",
  "text",
  "writer",
] as const;
export const EXCLUDED = ["crates/usvg/src/main.rs", "crates/resvg/src/main.rs"] as const;
export const METADATA = ["PROVENANCE.json", "README.md", "LICENSE-APACHE", "LICENSE-MIT"] as const;
export const SOURCE_FILE_COUNT = 48;
export const UPSTREAM_TEST_COUNT = 2;
export const LICENSE_FILE_COUNT = 2;

const digest = z.string().regex(/^[0-9a-f]{64}$/);
const adaptationSchema = z.strictObject({
  cratePaths: z.int().nonnegative().optional(),
  enabledFeaturePredicates: z.record(z.string(), z.int().nonnegative()).optional(),
  harfrustAliasPaths: z.int().nonnegative().optional(),
  internalUsvgAlias: z.int().nonnegative().optional(),
  usvgPaths: z.int().nonnegative().optional(),
});
const fileSchema = z.strictObject({
  source: z.string().min(1),
  destination: z.string().min(1),
  sourceSha256: digest,
  transplantedSha256: digest,
  sourceBytes: z.int().nonnegative(),
  transplantedBytes: z.int().nonnegative(),
  adaptations: adaptationSchema,
});
const licenseSchema = z.strictObject({
  source: z.string().min(1),
  destination: z.enum(["LICENSE-APACHE", "LICENSE-MIT"]),
  sourceSha256: digest,
  transplantedSha256: digest,
});
const adaptationPolicySchema = z.strictObject({
  allowAttributes: z.string().min(1),
  cratePaths: z.string().min(1),
  enabledFeaturePredicates: z.string().min(1),
  harfrustAlias: z.string().min(1),
  resvgUsvgPaths: z.string().min(1),
  sourceSemantics: z.string().min(1),
});
const provenanceSchema = z.strictObject({
  schema: z.literal("krr-svg-engine-source-provenance-v1"),
  sourceRepository: z.literal(SOURCE_REPOSITORY),
  upstreamRepository: z.literal(UPSTREAM_REPOSITORY),
  sourceCommit: z.literal(SOURCE_COMMIT),
  licenseExpression: z.literal("Apache-2.0 OR MIT"),
  excludedSourcePaths: z.array(z.string().min(1)).length(EXCLUDED.length),
  enabledFeatures: z.array(z.enum(FEATURES)).length(FEATURES.length),
  sourceFileCount: z.literal(SOURCE_FILE_COUNT),
  upstreamTestCount: z.literal(UPSTREAM_TEST_COUNT),
  adaptationPolicy: adaptationPolicySchema,
  files: z.array(fileSchema).length(SOURCE_FILE_COUNT),
  licenses: z
    .array(licenseSchema)
    .length(LICENSE_FILE_COUNT)
    .superRefine((licenses, context) => {
      const destinations = new Set(licenses.map((license) => license.destination));
      if (destinations.size !== LICENSE_FILE_COUNT) {
        context.addIssue({ code: "custom", message: "license destinations must be unique" });
      }
    }),
  updateProcedure: z.string().min(1),
});

export type Provenance = z.infer<typeof provenanceSchema>;

export function parseProvenance(value: unknown): Provenance {
  return provenanceSchema.parse(value);
}

export function readProvenance(file: string): Provenance {
  const parsed: unknown = JSON.parse(fs.readFileSync(file, "utf8"));
  return parseProvenance(parsed);
}
