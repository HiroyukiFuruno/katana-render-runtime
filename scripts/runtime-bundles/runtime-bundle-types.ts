import { z } from "zod";
import type { RuntimeBundleDefinition } from "#shared/runtime_bundle";

export type RuntimeBundleMode = "write" | "check";

export interface ClassicGenerationSource {
  readonly code: string;
  readonly sha256: string;
}
export interface ClassicGenerationMappedSource extends ClassicGenerationSource {
  readonly map: string;
}
export interface ClassicRollupGeneration {
  readonly name: string;
  readonly normalizedProvenance: string;
  readonly normalized: ClassicGenerationSource;
  readonly inner: ClassicGenerationMappedSource;
  readonly wrapper: ClassicGenerationSource;
  readonly rollup: ClassicGenerationMappedSource;
  readonly final: ClassicGenerationMappedSource;
}
export interface ClassicGenerationEvidence {
  readonly generation: ClassicRollupGeneration;
  readonly header: string;
  readonly contentSha256: string;
  readonly physicalMap: string;
  readonly inputPins?: string | undefined;
}
export type ClassicGenerationObserver = (generation: ClassicRollupGeneration) => void;

export interface GeneratedBundle {
  readonly definition: RuntimeBundleDefinition;
  readonly outputPath: string;
  readonly content: string;
  readonly checksum: string;
  readonly classicGeneration?: ClassicGenerationEvidence;
}

const sourceSchema = z
  .object({ code: z.string(), sha256: z.string().regex(/^[a-f0-9]{64}$/) })
  .strict();
const mappedSchema = sourceSchema.extend({ map: z.string() }).strict();
export const classicRollupGenerationSchema = z
  .object({
    name: z.enum(["mermaid", "drawio", "zenuml"]),
    normalizedProvenance: z.string(),
    normalized: sourceSchema,
    inner: mappedSchema,
    wrapper: sourceSchema,
    rollup: mappedSchema,
    final: mappedSchema,
  })
  .strict();
export const classicGenerationEvidenceSchema = z
  .object({
    generation: classicRollupGenerationSchema,
    header: z.string(),
    contentSha256: z.string(),
    physicalMap: z.string(),
    inputPins: z.string().optional(),
  })
  .strict();

export const classicCompletedGenerationEvidenceSchema = classicGenerationEvidenceSchema
  .extend({ inputPins: z.string().min(1) })
  .strict();

export const CLASSIC_GENERATION_CONTROL_INPUTS = [
  "package.json",
  "bun.lock",
  "tsconfig.json",
  "biome.json",
  ...[
    "bundle-runtime",
    "runtime-bundle-cli",
    "runtime-rollup-bundler",
    "runtime-bundle-command",
    "runtime-bundle-types",
    "runtime-classic-generation",
    "runtime-fragment-loader",
    "runtime-entry-names",
    "runtime-bundle-checks",
    "runtime-bundle-checksum",
    "runtime-bundle-definitions",
    "runtime-unused-origin",
    "runtime-unused-source-map",
    "runtime-script-normalizer",
    "runtime-bundle-paths",
  ].map((name) => `scripts/runtime-bundles/${name}.ts`),
  "scripts/runtime-bundles/runtime-css-variables.js",
  "scripts/mermaid/native-build-receipt.ts",
  ...[
    "native-cli-managed-build",
    "native-cli-managed-build-schema",
    "native-cli-managed-input-schema",
    "native-cli-managed-inputs",
    "native-cli-managed-process",
    "native-cli-managed-cargo",
    "native-cli-managed-json",
    "native-cli-managed-raw-fd",
  ].map((name) => `scripts/mermaid/${name}.ts`),
  "scripts/mermaid/native-cli-managed-control-inputs.json",
  "crates/katana-render-runtime/src/markdown/diagram_runtime/source/shared/runtime_bundle.ts",
  ...["mermaid", "drawio", "zenuml", "mathjax"].map(
    (name) =>
      `crates/katana-render-runtime/src/markdown/diagram_runtime/source/${name}/runtime_definition.ts`,
  ),
  ...[
    "rollup",
    "terser",
    "css-tree",
    "magic-string",
    "@swc/core",
    "@rollup/plugin-swc",
    "@rollup/plugin-commonjs",
    "@rollup/plugin-node-resolve",
    "acorn",
    "source-map-js",
    "zod",
    "acorn-loose",
    "acorn-walk",
  ].map((name) => `node_modules/${name}/package.json`),
] as const;
