import path from "node:path";
import { z } from "zod";

const formats = ["rust", "typescript", "javascript", "python"] as const;
const sourceKinds = [
  "data",
  "drawio-libs",
  "drawio-shapes",
  "drawio-stencils",
  "drawio-webapp",
  "icon-cache1",
  "ti",
  "wikipedia",
] as const;
const resourcesRoot =
  "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources";
const hash = z.string().regex(/^[a-f0-9]{64}$/);
const relativePath = z
  .string()
  .min(1)
  .refine((value) => {
    const normalized = value.replaceAll("\\", "/");
    return (
      !path.isAbsolute(value) &&
      !path.win32.isAbsolute(value) &&
      !normalized.split("/").includes("..") &&
      path.posix.normalize(normalized) === normalized
    );
  });
const location = z.strictObject({
  line: z.int().positive(),
  column: z.int().nonnegative(),
  position: z.int().nonnegative().optional(),
});
const exceptionSide = z.strictObject({
  path: relativePath,
  sha256: hash,
  start: location,
  end: location,
});
const exception = z.strictObject({
  first: exceptionSide,
  second: exceptionSide,
  fragmentSha256: hash,
  reason: z.string().refine((value) => value.trim().length > 0),
});
const pinnedStrings = (expected: readonly string[]) =>
  z
    .array(z.string())
    .refine(
      (value) =>
        value.length === expected.length && value.every((item, index) => item === expected[index]),
      { error: "policy does not match the pinned duplication gate contract" },
    );

export const policySchema = z.strictObject({
  version: z.literal(1),
  engine: z.literal("jscpd@5.4.1"),
  minLines: z.literal(8),
  minTokens: z.literal(75),
  mode: z.literal("mild"),
  formats: z
    .array(z.enum(formats))
    .refine(
      (value) =>
        value.length === formats.length && value.every((item, index) => item === formats[index]),
    ),
  roots: pinnedStrings(["crates", "scripts", "tests"]),
  excludes: pinnedStrings(["generated", "vendor", "target", "node_modules"]),
  assetManifest: z.literal(`${resourcesRoot}/drawio-resource-manifest.json`),
  assetResourcesRoot: z.literal(resourcesRoot),
  assetSourceKinds: pinnedStrings(["drawio-shapes", "drawio-webapp"]),
  assetBoundaryReason: z.literal(
    "Exclude only unchanged Draw.io JavaScript assets explicitly owned by the upstream resource manifest; repository-owned helpers and tests remain in scope.",
  ),
  exceptions: z.array(exception),
});

const assetEntry = z.strictObject({
  logicalPath: relativePath.refine((value) => !value.includes("\\")),
  sourceKind: z.enum(sourceKinds, {
    error: "unknown sourceKind or invalid Draw.io resource manifest entry",
  }),
  byteSize: z.int().nonnegative(),
  sha256: hash,
});
export const assetManifestSchema = z.strictObject({
  version: z.literal(1),
  resourcesRoot: z.string(),
  resources: z.array(assetEntry),
});

const chunk = z
  .strictObject({
    name: z.string(),
    start: z.int().positive(),
    end: z.int().positive(),
    startLoc: location,
    endLoc: location,
  })
  .refine(
    (value) =>
      value.end >= value.start &&
      value.startLoc.line === value.start &&
      value.endLoc.line === value.end,
    { error: "invalid jscpd chunk range" },
  );
const duplicate = z.strictObject({
  firstFile: chunk,
  secondFile: chunk,
  fragment: z.string(),
  format: z.enum(formats),
  lines: z.int().positive(),
  tokens: z.int().positive(),
  kind: z.enum(["exact", "renamed", "similar", "gap", "ast", "semantic"]).optional(),
  isNew: z.boolean().optional(),
});
const sourceCount = z
  .int({ error: "jscpd source count does not match scanned roots" })
  .nonnegative({ error: "jscpd source count does not match scanned roots" });
const cloneCount = z
  .int({ error: "jscpd clone count does not match duplicate records" })
  .nonnegative({ error: "jscpd clone count does not match duplicate records" });
const count = z.int().nonnegative().optional();
const percentage = z.number().nonnegative().optional();
const statistics = z.strictObject({
  sources: sourceCount,
  clones: cloneCount,
  lines: count,
  tokens: count,
  duplicatedLines: count,
  duplicatedTokens: count,
  newClones: count,
  newDuplicatedLines: count,
  percentage,
  percentageTokens: percentage,
});
export const reportSchema = z.strictObject({
  duplicates: z.array(duplicate),
  statistics: z.strictObject({
    total: statistics,
    formats: z.strictObject({
      rust: statistics,
      typescript: statistics,
      javascript: statistics,
      python: statistics,
    }),
    detectionDate: z.string().optional(),
  }),
});

export type Policy = z.infer<typeof policySchema>;
export type Exception = z.infer<typeof exception>;
export type ExceptionSide = z.infer<typeof exceptionSide>;
export type AssetEntry = z.infer<typeof assetEntry>;
export type AssetManifest = z.infer<typeof assetManifestSchema>;
export type Chunk = z.infer<typeof chunk>;
export type Report = z.infer<typeof reportSchema>;
export type Duplicate = z.infer<typeof duplicate>;
export type Format = z.infer<typeof duplicate>["format"];
