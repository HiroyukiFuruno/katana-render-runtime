import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { parseSync, version as swcVersion } from "@swc/core";
import { z } from "zod";
import { resolveRepositoryCode } from "../html/harness-contract-cli";
import {
  biomeBootstrapDispatchVerified,
  CLASSIC_FACTS_TEST,
  type ContractPolicy,
  executionGraph,
  guardedFactsRecipeVerified,
  hostEvidenceSourceVerified,
  justRecipes,
  moonStaticTaskProjection,
  FACTS_CARGO_GUARD_SOURCE as NATIVE_WIDTH_CARGO_GUARD_SOURCE,
  normalizeClassicFactsTestCommands,
  policyContract,
  RUNTIME_UNUSED_FINAL_VERIFY_COMMAND,
  RUNTIME_UNUSED_LEAF_IDS,
  RUNTIME_UNUSED_REGRESSION_TESTS,
  shellWords,
  sourceCode,
} from "../html/harness-contract-graph";
import {
  generatedRuntimeRustAssetInputPaths,
  generatedRuntimeRustAssetPaths,
} from "../runtime-assets/runtime-package-asset-compressor";
import { verifySvgEngineProvenance } from "../runtime-assets/svg-engine-provenance";
import { OFFICIAL_BROWSER_CONSUMER_PINS } from "../runtime-bundles/official-browser-consumer-contract";
import {
  astObject,
  astSpan,
  browserAstNodes,
  pageIdentifier,
} from "../runtime-bundles/runtime-unused-origin";
import { verifiedThirdPartyJavascript } from "./duplication-check";
import { type Format, policySchema } from "./duplication-schema";
import {
  inspectMathJaxDeclarations,
  MATHJAX_ABI,
  MATHJAX_DECL_PROFILE,
  MATHJAX_PROVENANCE,
  MATHJAX_RUNTIME,
  MATHJAX_TYPE_PROFILE,
} from "./mathjax-declaration-contract";
import {
  collectOwnedSourcePaths,
  OWNED_SOURCE_FORMATS,
  readOwnedRepositoryIndex,
} from "./owned-source-inventory";
import {
  queryTypeProject,
  safeFile,
  safeRelative,
  type typeConfiguration,
  typeProjectNames,
  typeScriptToolInspectionScope,
} from "./owned-type-projects";

const strings = z.array(z.string());
const TOMLI_ROOT = "scripts/release/_tomli";
const TOMLI_RUFF_CONFIG = `${TOMLI_ROOT}/ruff.toml`;
const TOMLI_PROVENANCE = `${TOMLI_ROOT}/upstream-provenance.json`;
const MOON_CACHE_FIXTURE = "tests/fixtures/quality-cache/moon-cache-subject.test.ts";
const MOON_CACHE_OWNER = "scripts/quality/moon_cache_contract_test.py";
const MOON_CACHE_OWNER_AST = "scripts/quality/owned_python_owner_contract.py";
const MOON_CACHE_RECIPE = "runtime-asset-script-test";
const MOON_CACHE_OWNER_COMMAND = `python3 ${MOON_CACHE_OWNER} --moon bun run moon`;
const NATIVE_PURE_TYPE_PROFILE = "scripts/quality/tsconfig-owned-native-pure-utilities.json";
const BROWSER_REFERENCE_DIRECTORY =
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime";
const BROWSER_REFERENCE_HELPERS = new Set([
  `${BROWSER_REFERENCE_DIRECTORY}/mermaid_zenuml_bridge.js`,
]);
const BROWSER_REFERENCE_HOST = `${BROWSER_REFERENCE_DIRECTORY}/browser_reference_host.d.ts`;
const BROWSER_REFERENCE_LEAF = "scripts/mermaid/official-renderer-browser-globals.d.ts";
const BROWSER_REFERENCE_PROFILE = "scripts/quality/tsconfig-owned-browser-reference.json";
// 順序を含む実consumerとhost ABI providerのレビュー後にだけ更新する。
const BROWSER_REFERENCE_CONSUMERS = {
  ...OFFICIAL_BROWSER_CONSUMER_PINS,
  [`${BROWSER_REFERENCE_DIRECTORY}/dom_text_metrics.js`]:
    "771ded4e5ee2122cffa15516f5eaad6922200af39aead30ff0b75c3d61cedd1b",
};
const NATIVE_WIDTH_DIRECTORY = "crates/katana-render-runtime/src/markdown/mermaid_renderer";
const NATIVE_WIDTH_TRACE = `${NATIVE_WIDTH_DIRECTORY}/js_runtime_native_width_trace.js`;
const NATIVE_WIDTH_BBOX_TRACE = `${NATIVE_WIDTH_DIRECTORY}/js_runtime_native_width_bbox_trace.js`;
const NATIVE_WIDTH_HOST = `${NATIVE_WIDTH_DIRECTORY}/js_runtime_native_width_trace_host.d.ts`;
const NATIVE_WIDTH_TYPE_PROFILE = "scripts/quality/tsconfig-owned-native-width-trace.json";
// 更新には実cfg(test)→helper→include/result-scriptのcontextレビューが必要。
const NATIVE_WIDTH_TEST_CONSUMERS = {
  [`${NATIVE_WIDTH_DIRECTORY}/js_runtime.rs`]:
    "184b6738bd8d38d9e8e02f64cc42aa561bc4349282caa341bc171431a234c265",
  [`${NATIVE_WIDTH_DIRECTORY}/js_runtime_native_width_tests.rs`]:
    "9eb7b0ea3f92aab02ffa0367b3c06ca4d9e36bb6ce406e8dcee2131914d73729",
  [`${NATIVE_WIDTH_DIRECTORY}/js_runtime_native_width_c4_tests.rs`]:
    "23f0a46c985e135c195b2826f06e7ab944565ba5041790df2832b711ad8af318",
  [`${NATIVE_WIDTH_DIRECTORY}/js_runtime_native_width_ishikawa_tests.rs`]:
    "8d818595edc6750bafbead131fef9beea1987a9995a0a2613cabf1b2de3d5a9f",
  [`${NATIVE_WIDTH_DIRECTORY}/js_runtime_native_width_helpers.rs`]:
    "a21a8b260625067348d8531512c8a489d3c71ec651e3b6710c1c69c4bb4da45b",
  [NATIVE_WIDTH_BBOX_TRACE]: "36751923cccf5e76c16e4cdcbb48b9bed74354e5ea208c793c6415a85661b16b",
  [`${NATIVE_WIDTH_DIRECTORY}/js_runtime_native_width_oracle_boundary_tests.rs`]:
    "f4375e4ca17e4d1931efbf21020b7b48bf06079ed9ae00de3ea160d31683731f",
};
const NATIVE_TYPED_UTILITIES = new Set([
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_transform_bounds.js",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_bounds_attrs.js",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_group_utils.js",
  "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/drawio_svg_shape_colors.js",
]);
const PERFORMANCE_HARNESS = "scripts/html/performance-harness";
const PERFORMANCE_MANIFEST = `${PERFORMANCE_HARNESS}/Cargo.toml`;
const PERFORMANCE_ARMS = ["baseline", "candidate"] as const;
const PERFORMANCE_COVERAGE_RUN_RECIPE = "performance-harness-coverage-run";
const PERFORMANCE_COVERAGE_WRAPPER =
  "CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/hooks/run_locked_command.py --performance-coverage";
const PERFORMANCE_RECIPES = {
  format: "performance-harness-fmt-check",
  lint: "performance-harness-lint",
  coverage: "performance-harness-coverage",
} as const;
const PERFORMANCE_CLIPPY_DENY = [
  "warnings",
  "clippy::unwrap_used",
  "clippy::expect_used",
  "clippy::todo",
  "clippy::unimplemented",
  "clippy::dbg_macro",
  "clippy::panic",
  "clippy::wildcard_imports",
  "clippy::too_many_lines",
  "clippy::cognitive_complexity",
];
const PARSER_BODY_DIRECTORY = "crates/katana-render-runtime/src/renderer/backends/html_runtime";
const PARSER_BODY_OUTPUT = `${PARSER_BODY_DIRECTORY}/parser_body_onload.js`;
const PARSER_BODY_CANONICAL = `${PARSER_BODY_DIRECTORY}/parser_body_onload.ts`;
const PARSER_BODY_HOST = `${PARSER_BODY_DIRECTORY}/host_parser_onload.d.ts`;
const PARSER_BODY_PRODUCER = "scripts/runtime-bundles/parser-body-onload-generation.ts";
const PARSER_BODY_TEST = "scripts/runtime-bundles/parser-body-onload-generation.test.ts";
const PARSER_BODY_TYPE_PROFILES = Object.freeze([
  "scripts/quality/tsconfig-owned-native-html-parser.json",
  "scripts/quality/tsconfig-owned-native-html-parser-tool.json",
] as const);
const PARSER_BODY_CHECK_RECIPE = "runtime-bundle-check";
const PARSER_BODY_TEST_RECIPE = "runtime-asset-script-test";
const PARSER_CHECK_RECIPE_SHA256 =
  "eda84e7b11aa1f40bba22269fd62f33e8acb1f631467d00004c38c47296bbacd";
const PARSER_TEST_RECIPE_SHA256 =
  "0e955697c278ac8c2a56c8347cd05c1488cfd50ecfcc21c3177d208d5e8457ed";
const HTML_FEATURE_CATALOG_TEST = "scripts/html/feature-catalog_test.ts";
const PLAYWRIGHT_GEOMETRY_TEST = "scripts/mermaid/reference_svg_geometry_browser_test.ts";
const PLAYWRIGHT_MACOS_PRODUCER_TEST =
  "scripts/mermaid/official_raw_producer_macos_browser_test.ts";
const PLAYWRIGHT_TESTS = [
  HTML_FEATURE_CATALOG_TEST,
  "scripts/html/chrome_badge_metrics_test.ts",
  "scripts/html/reference-capture_browser_test.ts",
  PLAYWRIGHT_GEOMETRY_TEST,
];
const PLAYWRIGHT_CONFIG = "scripts/html/playwright.config.ts";
const PLAYWRIGHT_CONFIG_HELPER = "scripts/html/playwright-config-base.ts";
const PLAYWRIGHT_MACOS_CONFIG = "scripts/html/playwright.macos.config.ts";
const PLAYWRIGHT_RECIPE = "browser-contract-test";
const PLAYWRIGHT_COMMAND = `node node_modules/playwright/cli.js test --config ${PLAYWRIGHT_CONFIG}`;
const PLAYWRIGHT_MACOS_RECIPE = "macos-browser-contract-test";
const PLAYWRIGHT_MACOS_COMMAND = `node node_modules/playwright/cli.js test --config ${PLAYWRIGHT_MACOS_CONFIG}`;
const PLAYWRIGHT_MACOS_RUN_DEFINITION = `MACOS_BROWSER_CONTRACT_RUN := if os() == "macos" { "${PLAYWRIGHT_MACOS_COMMAND}" } else { "true" }`;
const PLAYWRIGHT_CONFIG_HELPER_SOURCE = `export const browserContractDefaults = {
  timeout: 5000,
  retries: 0,
  workers: 1,
  fullyParallel: false,
  forbidOnly: true,
  use: {
    channel: "chrome",
    headless: true,
    viewport: { width: 1280, height: 900 },
  },
} as const;`;
const PLAYWRIGHT_CONFIG_HELPER_AST = strictInputAst(
  parseSync(PLAYWRIGHT_CONFIG_HELPER_SOURCE, { syntax: "typescript" }),
);
const PLAYWRIGHT_MACOS_CONFIG_SOURCE = `import { defineConfig } from "playwright/test";
import { browserContractDefaults } from "./playwright-config-base";

// WHY: macOSの実プロセス識別情報を観測するproducer用の設定。
export default defineConfig({
  testDir: "..",
  testMatch: ["mermaid/official_raw_producer_macos_browser_test.ts"],
  ...browserContractDefaults,
  outputDir: "../../tmp/quality/macos-browser-contract/artifacts",
});`;
const PLAYWRIGHT_MACOS_CONFIG_AST = strictInputAst(
  parseSync(PLAYWRIGHT_MACOS_CONFIG_SOURCE, { syntax: "typescript" }),
);
const PLAYWRIGHT_CONFIG_SOURCE = `import { defineConfig } from "playwright/test";
import { browserContractDefaults } from "./playwright-config-base";
// WHY: ブラウザ起動の管理は既存fixtureへ任せ、テスト本体の5秒制限を維持する。
export default defineConfig({
  testDir: "..",
  testMatch: [
    "html/feature-catalog_test.ts",
    "html/chrome_badge_metrics_test.ts",
    "html/reference-capture_browser_test.ts",
    "mermaid/reference_svg_geometry_browser_test.ts",
  ],
  ...browserContractDefaults,
  outputDir: "../../tmp/quality/browser-contract/artifacts",
});`;
const PLAYWRIGHT_CONFIG_AST = strictInputAst(
  parseSync(PLAYWRIGHT_CONFIG_SOURCE, { syntax: "typescript" }),
);
const NATIVE_SOURCE_MANIFEST_TEST = "scripts/mermaid/native-source-manifest_test.ts";
const MANAGED_FULL_COMPARE_TEST = "scripts/mermaid/managed_full_compare_test.ts";
const RUNTIME_BUNDLE_TOOL_FINGERPRINT_TEST =
  "scripts/runtime-bundles/runtime-bundle-tool-fingerprint_test.ts";
const MATHJAX_UNUSED_AUDIT_SOURCE = "scripts/runtime-bundles/runtime-unused-check.ts";
const MATHJAX_UNUSED_AUDIT_TEST = "scripts/runtime-bundles/runtime-unused-mathjax_test.ts";
const MATHJAX_UNUSED_AUDIT_RECIPE = "runtime-asset-script-test";
const MATHJAX_UNUSED_AUDIT_COMMAND = `bun ${MATHJAX_UNUSED_AUDIT_SOURCE} --mermaid-js {{quote(MERMAID_JS)}} --mermaid-zenuml-js {{quote(MERMAID_ZENUML_JS)}}`;
const MATHJAX_UNUSED_STANDARD_GATE_COMMAND = `bun ${MATHJAX_UNUSED_AUDIT_SOURCE} --gate`;
const CLASSIC_GENERATION_EVIDENCE_OUTPUT =
  '{{quote(REPO_ROOT + "/tmp/quality/classic-generation-evidence/current.json")}}';
const CLASSIC_GENERATION_EVIDENCE_PRODUCER =
  "bun scripts/runtime-bundles/bundle-runtime.ts --check --classic-generation-evidence";
const CLASSIC_GENERATION_EVIDENCE_RECIPE = "runtime-classic-generation-evidence";
const RUNTIME_ASSET_PREFLIGHT_RECIPE = "runtime-asset-script-preflight-run";
const RUNTIME_ASSET_OTHER_RECIPE = "runtime-asset-script-other-run";
const SVG_ENGINE_PROVENANCE_TEST_COMMAND =
  "bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-assets/svg-engine-provenance_test.ts";
const SVG_ENGINE_VENDOR_ROOT = "crates/katana-render-runtime/vendor/svg-engine";
const RUNTIME_ASSET_AGGREGATE_RECIPE = "runtime-asset-script-aggregate-run";
const RUNTIME_UNUSED_REGRESSION = "runtime-unused-script-regression";
const RUNTIME_UNUSED_REGRESSION_RECIPE = "runtime-unused-script-regression-run";
const RUNTIME_UNUSED_PRODUCER_RECIPE = "runtime-unused-script-producer-run";
const RUNTIME_UNUSED_VERIFY_RECIPE = "runtime-unused-script-verify-run";
const RUNTIME_UNUSED_CACHE_KEY_RECIPE = "runtime-unused-script-cache-key";
const RUNTIME_UNUSED_FINAL_VERIFY_RECIPE = "runtime-unused-script-final-verify-run";
const CLASSIC_GENERATION_EVIDENCE_TEST_COMMAND =
  "bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-unused-classic-capture_test.ts ./scripts/runtime-bundles/runtime-unused-classic-binding_test.ts ./scripts/runtime-bundles/runtime-classic-generation_test.ts ./scripts/runtime-bundles/runtime-unused-semantic-adapter_test.ts";
const CLASSIC_GENERATION_EVIDENCE_CONSUMER = `KRR_CLASSIC_GENERATION_EVIDENCE_PATH=${CLASSIC_GENERATION_EVIDENCE_OUTPUT} bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-unused-classic-capture_test.ts ./scripts/runtime-bundles/runtime-unused-classic-binding_test.ts ./scripts/runtime-bundles/runtime-classic-generation_test.ts ./scripts/runtime-bundles/runtime-unused-semantic-adapter_test.ts`;
const MATHJAX_UNUSED_FACT_RECIPE = "mathjax-consumer-facts";
const HOST_EVIDENCE_INPUTS = [
  "scripts/runtime-bundles/runtime-rust-facts-host.ts",
  "scripts/runtime-bundles/runtime-rust-facts-host-inputs.ts",
  "scripts/runtime-bundles/runtime-rust-facts-host-protocol.ts",
  "scripts/runtime-bundles/runtime-rust-facts-host_test.ts",
  "scripts/runtime-bundles/runtime-rust-facts-host-freshness_test.ts",
  "scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts",
  "scripts/runtime-bundles/runtime-mathjax-audit-evidence-inputs.ts",
  "scripts/runtime-bundles/runtime-mathjax-audit-evidence-producer.ts",
  "scripts/runtime-bundles/runtime-mathjax-audit-evidence_test.ts",
] as const;
const HOST_EVIDENCE_PRECONDITIONS = [
  'bun scripts/runtime-bundles/runtime-rust-facts-host.ts verify {{ quote(REPO_ROOT) }} {{ quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json") }}',
  "NODE_ENV=test bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts verify",
] as const;
const HOST_EVIDENCE_TEST_COMMAND =
  "bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-rust-facts-host_test.ts ./scripts/runtime-bundles/runtime-rust-facts-host-freshness_test.ts ./scripts/runtime-bundles/runtime-mathjax-audit-evidence_test.ts";
const RUNTIME_ASSET_TEST_MARKER = "./scripts/runtime-assets/runtime-asset-common_test.ts";
const OWNED_VALIDATION_COMMAND =
  "bun test --path-ignore-patterns 'tmp/**' ./scripts/quality/owned-source-inventory_test.ts ./scripts/quality/owned-validation-contract_test.ts";
function runtimeAssetRecipeBody(context: ContractContext): string {
  if (context.recipes.has(RUNTIME_ASSET_OTHER_RECIPE)) {
    if (
      context.visible.has(MATHJAX_UNUSED_AUDIT_SOURCE) &&
      context.recipes.get("runtime-asset-script-test")?.trim() !==
        "bun run moon exec krr:runtime-asset-script-test"
    )
      return "";
    return context.recipes.get(RUNTIME_ASSET_OTHER_RECIPE) ?? "";
  }
  return context.recipes.get("runtime-asset-script-test") ?? "";
}
function runtimeUnusedRecipeBody(context: ContractContext): string {
  if (context.visible.has(MATHJAX_UNUSED_AUDIT_SOURCE))
    return context.recipes.get(RUNTIME_UNUSED_REGRESSION_RECIPE) ?? "";
  return (
    context.recipes.get(RUNTIME_UNUSED_REGRESSION_RECIPE) ??
    context.recipes.get("runtime-asset-script-test") ??
    ""
  );
}
function runtimeAggregateRecipeBody(context: ContractContext): string {
  return context.recipes.get(RUNTIME_ASSET_AGGREGATE_RECIPE) ?? "";
}
function runtimePreflightRecipeBody(context: ContractContext): string {
  return context.recipes.get(RUNTIME_ASSET_PREFLIGHT_RECIPE) ?? "";
}
function runtimeUnusedProducerRecipeBody(context: ContractContext): string {
  return context.recipes.get(RUNTIME_UNUSED_PRODUCER_RECIPE) ?? "";
}
function runtimeUnusedVerifyRecipeBody(context: ContractContext): string {
  return context.recipes.get(RUNTIME_UNUSED_VERIFY_RECIPE) ?? "";
}
const NATIVE_WIDTH_HASH_SOURCE = "scripts/runtime-bundles/runtime-unused-native-width-hash.ts";
const NATIVE_WIDTH_HASH_TEST = "scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts";
const NATIVE_WIDTH_HASH_RECIPE = "native-width-hash-facts";
export const NATIVE_WIDTH_HASH_INPUTS = Object.freeze([
  MATHJAX_UNUSED_AUDIT_SOURCE,
  "scripts/runtime-bundles/runtime-unused-check_test.ts",
  "scripts/runtime-bundles/runtime-unused-contexts.ts",
  "scripts/runtime-bundles/runtime-unused-origin.ts",
  NATIVE_WIDTH_HASH_SOURCE,
  NATIVE_WIDTH_HASH_TEST,
  "crates/kdr-linter/Cargo.toml",
  "crates/kdr-linter/src/bin/mathjax_consumer_facts.rs",
  "crates/kdr-linter/src/bin/mathjax_consumer_facts/native_width_hash.rs",
  "crates/kdr-linter/src/bin/mathjax_consumer_facts/native_width_hash_ast.rs",
  "crates/kdr-linter/src/bin/mathjax_consumer_facts/native_width_hash_routes.rs",
  "crates/kdr-linter/src/bin/mathjax_consumer_facts/ast_linter_tests.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/mod.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
  "Justfile",
] as const);
export const MATHJAX_UNUSED_INPUTS = [
  ...HOST_EVIDENCE_INPUTS,
  "scripts/runtime-bundles/runtime-unused-semantic-adapter.ts",
  "scripts/runtime-bundles/runtime-unused-semantic-node.ts",
  "scripts/runtime-bundles/runtime-unused-semantic-adapter_test.ts",
  "scripts/runtime-bundles/runtime-unused-entrypoint-owner.ts",
  MATHJAX_UNUSED_AUDIT_SOURCE,
  MATHJAX_UNUSED_AUDIT_TEST,
  "scripts/runtime-bundles/runtime-unused-mathjax.ts",
  "scripts/runtime-bundles/runtime-unused-contexts.ts",
  "scripts/runtime-bundles/runtime-unused-origin.ts",
  "scripts/runtime-bundles/runtime-unused-check_test.ts",
  "scripts/mermaid/official-renderer-asset-inputs.ts",
  "scripts/mermaid/diagram-update.ts",
  "scripts/mermaid/official-renderer.ts",
  "scripts/mermaid/rasterize-svg-dir.ts",
  "scripts/mermaid/raster-capture-observation.ts",
  "scripts/mermaid/strict-svg-external-images.ts",
  "scripts/mermaid/official-renderer-types.ts",
  "scripts/runtime-bundles/runtime-unused-biome.ts",
  "scripts/runtime-bundles/runtime-bundle-command.ts",
  "scripts/runtime-bundles/runtime-rollup-bundler.ts",
  "scripts/runtime-bundles/runtime-bundle-definitions.ts",
  "crates/kdr-linter/src/bin/mathjax_consumer_facts.rs",
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime/render_mathjax.js",
  "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition.ts",
  "Justfile",
] as const;
const TOMLI_WHEEL = {
  filename: "tomli-2.5.0-py3-none-any.whl",
  url: "https://files.pythonhosted.org/packages/60/3f/3e3f8fd0919249b0200c80fbc4f9a1e70be19f9883da71dfb7f8b9ab8aca/tomli-2.5.0-py3-none-any.whl",
  sha256: "32a7b79ac57a2e83670ce329ccf675798bc5a2094783a63676866b70503f2e2b",
} as const;
const TOMLI_FILES = {
  [`${TOMLI_ROOT}/__init__.py`]: "7182927b251660b7d649eec79e0dcaf03a59973793400990a693d9f6f8b73eca",
  [`${TOMLI_ROOT}/_parser.py`]: "3e782f1bc85639ecef630ae686df3d849f2bedd2e9c987d8f5109a5a154df131",
  [`${TOMLI_ROOT}/_re.py`]: "a12359fe294523a72112e434d58452a14c9d050affa2417f9927474e4166bfdd",
  [`${TOMLI_ROOT}/_types.py`]: "f864c6d9552a929c7032ace654ee05ef26ca75d21b027b801d77e65907138b74",
  [`${TOMLI_ROOT}/py.typed`]: "f0f8f2675695a10a5156fb7bd66bafbaae6a13e8d315990af862c792175e6e67",
  [`${TOMLI_ROOT}/LICENSE`]: "b80816b0d530b8accb4c2211783790984a6e3b61922c2b5ee92f3372ab2742fe",
} as const;
const tomliRuffConfiguration = z.strictObject({
  "target-version": z.literal("py38"),
  "required-version": z.literal("==0.16.10"),
});
const tomliProvenance = z.strictObject({
  distribution: z.literal("tomli"),
  version: z.literal("2.5.0"),
  requiresPython: z.literal(">=3.8"),
  source: z.literal("https://pypi.org/pypi/tomli/2.5.0/json"),
  wheel: z.strictObject({
    filename: z.literal(TOMLI_WHEEL.filename),
    url: z.literal(TOMLI_WHEEL.url),
    sha256: z.literal(TOMLI_WHEEL.sha256),
  }),
  files: z.record(z.string(), z.string()),
  licenseSHA256: z.literal(TOMLI_FILES[`${TOMLI_ROOT}/LICENSE`]),
  preservation: z.literal(
    "Unmodified official package bytes; import package directory named _tomli uses upstream relative imports.",
  ),
});
const moonOwnerAstResult = z.strictObject({
  schema: z.literal(1),
  contract: z.literal("moon-owner-host-evidence-barrier-v3"),
  valid: z.boolean(),
  limitations: z.tuple([z.string(), z.string()]),
  schedulerTasks: z
    .record(
      z.string(),
      z.strictObject({
        deps: strings,
        leaf: z.string().nullable(),
        options: z.record(z.string(), z.union([z.string(), z.boolean()])),
      }),
    )
    .nullable(),
});
const packageContract = z.object({
  scripts: z.record(z.string(), z.string()),
  devDependencies: z.record(z.string(), z.string()).default({}),
});
const biomeContract = z.object({
  files: z.object({ includes: strings }),
  linter: z.object({ enabled: z.boolean().optional() }).optional(),
  overrides: z
    .array(
      z.object({
        includes: strings,
        linter: z.object({ enabled: z.boolean().optional() }).optional(),
      }),
    )
    .default([]),
});
const pythonRecords = z.array(
  z.strictObject({ path: z.string(), kind: z.enum(["unittest", "none", "unknown"]) }),
);

export interface OwnedValidationIssue {
  code: string;
  path: string;
  detail: string;
}
export interface OwnedValidationReport {
  schema: 1;
  staticContractOnly: true;
  sourceCount: number;
  sourceDigest: string;
  languageCounts: Record<Format, number>;
  units: { bun: string[]; python: string[]; playwright: string[] };
  javascriptEnvironments: Record<string, number>;
  javascriptTypeCoverage: {
    scope: "validation-tests-and-established-typed-product-boundaries";
    verification: "not-executed";
    wholeJavaScriptStrict: "not-claimed";
    mandatoryPaths: string[];
    profileMatchedPaths: string[];
    generatedBoundPaths: string[];
    deferredProductPaths: string[];
  };
  browserReferenceHelpers: {
    presentPaths: string[];
    consumerSources: { path: string; sha256: string | null }[];
    consumerVerified: boolean;
    typeProfile: string;
    typeProfileVerified: boolean;
    compiledOwnedSources: string[];
    compilerLibraryCount: number;
    closureVerified: boolean;
    inputDigest: string | null;
  };
  nativeWidthTrace: {
    path: string;
    present: boolean;
    consumerSources: { path: string; sha256: string | null }[];
    consumerVerified: boolean;
    typeProfile: string;
    typeProfileVerified: boolean;
    hostDeclaration: string;
  };
  verifiedAssets: number;
  generatedRustAssets: {
    paths: string[];
    inputCount: number;
    inputDigest: string;
    byteVerification: "not-executed";
    requiredCheck: "runtime-package-asset-check";
    localCheckRegistered: boolean;
    ciCheckRegistered: boolean;
  };
  moonCacheFixture: {
    path: typeof MOON_CACHE_FIXTURE;
    owner: typeof MOON_CACHE_OWNER;
    ownerAstHelper: typeof MOON_CACHE_OWNER_AST;
    ownerAstContract: "moon-owner-host-evidence-barrier-v3";
    fixtureSha256: string;
    ownerSha256: string;
    ownerAstHelperSha256: string;
    rootConfig: string;
    rootConfigSha256: string;
    fixtureVerified: boolean;
    ownerVerified: boolean;
    ownerAstVerified: boolean;
    rootDagVerified: boolean;
    localCheckRegistered: boolean;
    ciCheckRegistered: boolean;
  };
  parserBodyOnload: {
    path: typeof PARSER_BODY_OUTPUT;
    canonical: typeof PARSER_BODY_CANONICAL;
    hostDeclaration: typeof PARSER_BODY_HOST;
    producer: typeof PARSER_BODY_PRODUCER;
    unitTest: typeof PARSER_BODY_TEST;
    inputPaths: string[];
    inputDigest: string;
    outputSha256: string;
    producerVerified: boolean;
    biomeIncluded: boolean;
    canonicalTypeScope: boolean;
    producerTypeScope: boolean;
    outputTypeBoundary: "generated-from-strict-typescript";
    localCheckRegistered: boolean;
    ciCheckRegistered: boolean;
    localTestRegistered: boolean;
    ciTestRegistered: boolean;
    byteVerification: "not-executed";
  };
  mathJaxUnusedAudit: {
    sourcePath: typeof MATHJAX_UNUSED_AUDIT_SOURCE;
    testPath: typeof MATHJAX_UNUSED_AUDIT_TEST;
    auditRecipe: typeof MATHJAX_UNUSED_AUDIT_RECIPE;
    auditCommand: typeof MATHJAX_UNUSED_AUDIT_COMMAND;
    standardCommand: typeof MATHJAX_UNUSED_STANDARD_GATE_COMMAND;
    factRecipe: typeof MATHJAX_UNUSED_FACT_RECIPE;
    hostEvidenceExecutionVerified: false;
    inputPaths: string[];
    inputDigest: string;
    sourceSha256: string;
    localCheckRegistered: boolean;
    ciCheckRegistered: boolean;
    commandVerified: boolean;
    inputSelectionVerified: boolean;
    standardCommandVerified: boolean;
    standardLocalCheckRegistered: boolean;
    standardCiCheckRegistered: boolean;
  };
  nativeWidthHashFacts: {
    sourcePath: typeof NATIVE_WIDTH_HASH_SOURCE;
    testPath: typeof NATIVE_WIDTH_HASH_TEST;
    factRecipe: typeof NATIVE_WIDTH_HASH_RECIPE;
    auditRecipe: typeof MATHJAX_UNUSED_AUDIT_RECIPE;
    inputPaths: string[];
    inputDigest: string;
    factRecipeVerified: boolean;
    testCommandVerified: boolean;
    localCheckRegistered: boolean;
    ciCheckRegistered: boolean;
    nativeExecutionVerified: false;
    performanceVerified: false;
  };
  standalonePerformanceHarness?: {
    inputPaths: string[];
    inputDigest: string;
    localCoverageRegistered: boolean;
    ciCoverageRegistered: boolean;
    verification: "not-executed";
  };
  issues: OwnedValidationIssue[];
  limitations: string[];
}
interface ContractContext {
  root: string;
  index: { tracked: string[]; untracked: string[] };
  tracked: Set<string>;
  visible: Set<string>;
  policy: ContractPolicy;
  recipes: Map<string, string>;
  localRecipes: Set<string>;
  ciRecipes: Set<string>;
  local: Set<string>;
  ci: Set<string>;
  rtkProxyCommandVerified: boolean;
  issues: OwnedValidationIssue[];
  moonCacheFixtureBinding?: OwnedValidationReport["moonCacheFixture"];
  parserBodyOnloadBinding?: OwnedValidationReport["parserBodyOnload"];
  mathJaxInputDigest?: string;
  read: (name: string) => string;
}
interface TypeScope {
  name: string;
  files: Set<string>;
  options: z.infer<typeof typeConfiguration>["compilerOptions"];
}

function add(context: ContractContext, code: string, name: string, detail = ""): void {
  context.issues.push({ code, path: name, detail });
}
function relative(root: string, absolute: string): string {
  return path.relative(root, absolute).replaceAll(path.sep, "/");
}
function read(root: string, name: string): string {
  return fs.readFileSync(safeFile(root, name), "utf8");
}
export function createContractRead(root: string): (name: string) => string {
  const cache = new Map<string, string>();
  return (name) => {
    const file = safeFile(root, name);
    const key = `${file}\0utf8`;
    const cached = cache.get(key);
    if (cached !== undefined) return cached;
    const source = fs.readFileSync(file, "utf8");
    cache.set(key, source);
    return source;
  };
}
function json(root: string, name: string): unknown {
  return JSON.parse(read(root, name));
}
function matches(name: string, pattern: string): boolean {
  return new Bun.Glob(pattern).match(name) || new Bun.Glob(`${pattern}/**`).match(name);
}
function included(name: string, patterns: string[]): boolean {
  return (
    patterns.some((pattern) => !pattern.startsWith("!") && matches(name, pattern)) &&
    !patterns.some((pattern) => pattern.startsWith("!") && matches(name, pattern.slice(1)))
  );
}
function registered(context: ContractContext, recipe: string): boolean {
  return context.local.has(recipe) && context.ci.has(recipe);
}

function metadata(
  root: string,
  index: { tracked: string[]; untracked: string[] },
  tracked: Set<string>,
  visible: Set<string>,
): {
  context: ContractContext;
  pkg: z.infer<typeof packageContract>;
  biome: z.infer<typeof biomeContract>;
  verifyExternalTools: () => void;
} {
  const policy = policyContract.parse(json(root, "scripts/html/harness-contract-policy.json"));
  const pkg = packageContract.parse(json(root, "package.json"));
  const readMemoized = createContractRead(root);
  const readOptional = (name: string) =>
    fs.existsSync(path.join(root, name)) ? readMemoized(name) : undefined;
  const recipes = justRecipes(readMemoized("Justfile"));
  const externalToolScope = typeScriptToolInspectionScope(root);
  const graph = executionGraph(
    policy,
    readOptional,
    pkg.scripts,
    new Set(Object.keys(pkg.devDependencies)),
    [],
    (reference, importer) => resolveRepositoryCode(root, policy.codeRoots, reference, importer),
    externalToolScope.resolve,
  );
  const context: ContractContext = {
    root,
    index,
    tracked,
    visible,
    policy,
    recipes,
    localRecipes: graph.localRecipes ?? new Set(),
    ciRecipes: graph.ciRecipes ?? new Set(),
    local: graph.localRecipes ?? new Set(),
    ci: graph.ciRecipes ?? new Set(),
    rtkProxyCommandVerified: rtkProxyCommandVerified(readMemoized("Justfile")),
    issues: [],
    read: readMemoized,
  };
  const authorities = [
    "scripts/quality/duplication-policy.json",
    "scripts/html/harness-contract-policy.json",
    "package.json",
    "Justfile",
    "biome.json",
    "tsconfig.json",
    "Cargo.toml",
    policy.ciWorkflow,
    policy.localBridge.source,
    ...(policy.moon ? [policy.moon.workspace, policy.moon.config] : []),
    ...(visible.has(TOMLI_RUFF_CONFIG) ||
    [...visible].some((name) => name.startsWith(`${TOMLI_ROOT}/`))
      ? [TOMLI_RUFF_CONFIG, TOMLI_PROVENANCE]
      : []),
  ];
  for (const name of authorities) if (!tracked.has(name)) add(context, "untracked-authority", name);
  for (const detail of new Set(graph.errors)) {
    // Historical contract fixtures intentionally do not materialize the newer
    // MathJax audit source.  The real repository still gets the normal graph
    // resolution because this exception is limited to that exact missing leaf.
    if (
      detail === `missing code: ${MATHJAX_UNUSED_AUDIT_SOURCE}` &&
      !visible.has(MATHJAX_UNUSED_AUDIT_SOURCE)
    )
      continue;
    add(context, "execution-graph-contract", "scripts/html/harness-contract-policy.json", detail);
  }
  return {
    context,
    pkg,
    biome: biomeContract.parse(json(root, "biome.json")),
    verifyExternalTools: externalToolScope.verify,
  };
}

function sourceSafety(context: ContractContext, names: string[]): boolean {
  let valid = true;
  for (const name of names) {
    try {
      safeFile(context.root, name);
    } catch (error) {
      valid = false;
      add(
        context,
        "unsafe-or-missing-source",
        name,
        error instanceof Error ? error.message : String(error),
      );
    }
  }
  return valid;
}
function checksumAsset(context: ContractContext, name: string): boolean {
  const manifest = name.includes("/diagram_runtime/generated/")
    ? path.posix.join(path.posix.dirname(name), "runtime-bundles.sha256")
    : `${name}.sha256`;
  if (!fs.existsSync(path.join(context.root, manifest))) return false;
  const records = context
    .read(manifest)
    .trim()
    .split("\n")
    .map((line) => /^(\w{64})\s+\*?(.+)$/.exec(line));
  const selected = records.filter((record) => record?.[2] === path.posix.basename(name));
  const expected = selected[0]?.[1];
  if (
    selected.length !== 1 ||
    !expected ||
    !/^[a-f0-9]{64}$/.test(expected) ||
    sha(fs.readFileSync(safeFile(context.root, name))) !== expected
  )
    throw new Error(`asset checksum mismatch: ${name}`);
  if (!context.tracked.has(manifest)) add(context, "untracked-authority", manifest);
  return true;
}
function sha(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

function staleIncludes(
  context: ContractContext,
  name: string,
  patterns: string[],
  base = "",
): void {
  const visible = context.index;
  const candidates = [...visible.tracked, ...visible.untracked];
  for (const pattern of patterns) {
    if (pattern.startsWith("!")) continue;
    const resolved = path.posix.join(base, pattern);
    if (
      /^(scripts|crates|tests)\//.test(resolved) &&
      !candidates.some((file) => matches(file, resolved))
    )
      add(context, "stale-static-include", name, resolved);
  }
}

function typeSkippingIssues(
  context: ContractContext,
  name: string,
  options: z.infer<typeof typeConfiguration>["compilerOptions"],
): void {
  if (options.skipLibCheck === true && name !== MATHJAX_TYPE_PROFILE)
    add(context, "type-project-declaration-skipping", name);
  if (options.skipDefaultLibCheck === true || options.noCheck === true)
    add(context, "type-project-check-disabled", name);
}
function verifyTypecheckDispatch(context: ContractContext): void {
  if (!registered(context, "typecheck")) add(context, "static-recipe-unregistered", "typecheck");
  if (shellWords(context.recipes.get("typecheck") ?? "").join("\0") !== "bun\0run\0typecheck")
    add(context, "typecheck-dispatch-unverified", "Justfile");
}
function typeScope(context: ContractContext, name: string): TypeScope {
  const parsed = queryTypeProject(context.root, name);
  for (const authority of parsed.authorities)
    if (!context.tracked.has(authority)) add(context, "untracked-authority", authority);
  staleIncludes(
    context,
    name,
    parsed.include,
    path.posix.dirname(name) === "." ? "" : path.posix.dirname(name),
  );
  if (
    parsed.options.strict !== true ||
    parsed.options.noImplicitAny === false ||
    parsed.options.strictNullChecks === false
  )
    add(context, "type-project-not-strict", name);
  for (const option of [
    "noUnusedLocals",
    "noUnusedParameters",
    "useUnknownInCatchVariables",
  ] as const)
    if (parsed.options[option] !== true)
      add(context, "type-project-strict-check-missing", name, option);
  typeSkippingIssues(context, name, parsed.options);
  return {
    name,
    files: parsed.files,
    options: parsed.options,
  };
}
function typeScopes(context: ContractContext, pkg: z.infer<typeof packageContract>): TypeScope[] {
  verifyTypecheckDispatch(context);
  const projects = typeProjectNames(pkg).map((name) => typeScope(context, name));
  if (
    context.visible.has(MATHJAX_RUNTIME) ||
    context.visible.has(MATHJAX_ABI) ||
    projects.some((project) => [MATHJAX_TYPE_PROFILE, MATHJAX_DECL_PROFILE].includes(project.name))
  ) {
    if (!context.tracked.has(MATHJAX_PROVENANCE))
      add(context, "untracked-authority", MATHJAX_PROVENANCE);
    const boundary = inspectMathJaxDeclarations(context.root, projects, (name) =>
      context.read(name),
    );
    for (const issue of boundary.issues)
      add(context, "mathjax-declaration-boundary-invalid", MATHJAX_TYPE_PROFILE, issue);
    context.mathJaxInputDigest = boundary.inputDigest;
  }
  return projects;
}
function parserBodyOnloadTypeScopes(
  context: ContractContext,
  pkg: z.infer<typeof packageContract>,
): TypeScope[] {
  verifyTypecheckDispatch(context);
  const parserProfiles = new Set<string>(PARSER_BODY_TYPE_PROFILES);
  return typeProjectNames(pkg)
    .filter((name) => parserProfiles.has(name))
    .map((name) => typeScope(context, name));
}

function isolatedStrictJsProfile(
  scope: TypeScope | undefined,
  files: ReadonlySet<string>,
): boolean {
  if (!scope) return false;
  const options = scope.options;
  return (
    scope.files.size === files.size &&
    [...files].every((name) => scope.files.has(name)) &&
    options.lib?.join("\0") === "es2024" &&
    options.types?.length === 0 &&
    [
      options.strict,
      options.noImplicitAny,
      options.strictNullChecks,
      options.noUncheckedIndexedAccess,
      options.exactOptionalPropertyTypes,
      options.useUnknownInCatchVariables,
      options.noUnusedLocals,
      options.noUnusedParameters,
      options.allowJs,
      options.checkJs,
      options.noEmit,
    ].every((value) => value === true) &&
    options.skipLibCheck !== true &&
    options.skipDefaultLibCheck !== true &&
    options.noCheck !== true
  );
}

function nativeWidthTypeProfile(scope: TypeScope | undefined): boolean {
  return isolatedStrictJsProfile(
    scope,
    new Set([NATIVE_WIDTH_TRACE, NATIVE_WIDTH_BBOX_TRACE, NATIVE_WIDTH_HOST]),
  );
}

function nativeWidthTraceBinding(
  context: ContractContext,
  types: TypeScope[],
): OwnedValidationReport["nativeWidthTrace"] {
  const present = context.visible.has(NATIVE_WIDTH_TRACE);
  const consumers = Object.entries(NATIVE_WIDTH_TEST_CONSUMERS);
  const selected = types.find((scope) => scope.name === NATIVE_WIDTH_TYPE_PROFILE);
  const active = present || consumers.some(([name]) => context.visible.has(name)) || !!selected;
  const consumerSources = active
    ? consumers.map(([name]) => ({
        path: name,
        sha256: context.visible.has(name) ? sha(context.read(name)) : null,
      }))
    : [];
  const consumerVerified =
    active &&
    consumers.every(
      ([name, expected]) =>
        context.tracked.has(name) &&
        consumerSources.some((row) => row.path === name && row.sha256 === expected),
    );
  const typeProfileVerified = nativeWidthTypeProfile(selected);
  if (active) {
    if (!present) add(context, "native-test-trace-missing", NATIVE_WIDTH_TRACE);
    if (!consumerVerified)
      add(context, "native-test-trace-consumer-unverified", NATIVE_WIDTH_TRACE);
    if (!typeProfileVerified)
      add(
        context,
        "native-test-trace-type-profile-unverified",
        NATIVE_WIDTH_TRACE,
        NATIVE_WIDTH_TYPE_PROFILE,
      );
  }
  return {
    path: NATIVE_WIDTH_TRACE,
    present,
    consumerSources,
    consumerVerified,
    typeProfile: NATIVE_WIDTH_TYPE_PROFILE,
    typeProfileVerified,
    hostDeclaration: NATIVE_WIDTH_HOST,
  };
}

function browserReferenceCompilerSource(
  context: ContractContext,
  file: string,
  nativeLib: string,
  expected: ReadonlySet<string>,
): { kind: "library" | "owned"; name: string; pin: string } {
  if (!path.isAbsolute(file)) throw new Error("nonphysical compiler input");
  if (path.relative(nativeLib, path.dirname(file)) === "") {
    const name = path.basename(file);
    // ES2024の標準型だけを受け入れ、DOM/Bun等の外部型へ拡張しない。
    if (
      !/^lib\.(?:es(?:5|20(?:1[5-9]|2[0-4]))(?:\.[a-z0-9.]+)?|decorators(?:\.legacy)?)\.d\.ts$/.test(
        name,
      )
    )
      throw new Error(`unexpected compiler library: ${name}`);
    if (
      path.relative(fs.realpathSync(file), path.resolve(file)) !== "" ||
      !fs.lstatSync(file).isFile()
    )
      throw new Error("unsafe compiler library");
    return { kind: "library", name, pin: `${file}\0${sha(fs.readFileSync(file, "utf8"))}\n` };
  }
  const name = relative(context.root, file);
  if (!expected.has(name)) throw new Error(`unexpected owned compiler input: ${name}`);
  return { kind: "owned", name, pin: `${name}\0${sha(context.read(name))}\n` };
}

function browserReferenceClosure(context: ContractContext): {
  compiledOwnedSources: string[];
  compilerLibraryCount: number;
  closureVerified: boolean;
  inputDigest: string | null;
} {
  try {
    const files = execFileSync(
      "bunx",
      [
        "--no-install",
        "tsc",
        "--listFilesOnly",
        "--project",
        safeFile(context.root, BROWSER_REFERENCE_PROFILE),
      ],
      {
        cwd: path.resolve(import.meta.dir, "../.."),
        encoding: "utf8",
      },
    )
      .trim()
      .split(/\r?\n/);
    const nativeLib = path.resolve(
      import.meta.dir,
      "../../node_modules/@typescript",
      `typescript-${process.platform}-${process.arch}`,
      "lib",
    );
    const expected = new Set([
      ...BROWSER_REFERENCE_HELPERS,
      BROWSER_REFERENCE_HOST,
      BROWSER_REFERENCE_LEAF,
    ]);
    const inputs = files.map((file) =>
      browserReferenceCompilerSource(context, file, nativeLib, expected),
    );
    const owned = new Set(inputs.filter((row) => row.kind === "owned").map((row) => row.name));
    const libs = new Set(inputs.filter((row) => row.kind === "library").map((row) => row.name));
    return {
      compiledOwnedSources: [...owned].sort(),
      compilerLibraryCount: libs.size,
      closureVerified:
        owned.size === expected.size &&
        [...expected].every((name) => owned.has(name)) &&
        libs.size === 69,
      inputDigest: sha(inputs.map((row) => row.pin).join("")),
    };
  } catch (error) {
    add(
      context,
      "browser-reference-closure-unverified",
      BROWSER_REFERENCE_PROFILE,
      error instanceof Error ? error.message : String(error),
    );
    return {
      compiledOwnedSources: [],
      compilerLibraryCount: 0,
      closureVerified: false,
      inputDigest: null,
    };
  }
}

function browserReferenceBinding(
  context: ContractContext,
  types: TypeScope[],
): OwnedValidationReport["browserReferenceHelpers"] {
  const presentPaths = [...BROWSER_REFERENCE_HELPERS].filter((name) => context.visible.has(name));
  const entries = Object.entries(BROWSER_REFERENCE_CONSUMERS);
  const selected = types.find((scope) => scope.name === BROWSER_REFERENCE_PROFILE);
  const active =
    presentPaths.length > 0 ||
    !!selected ||
    entries.slice(0, 2).some(([name]) => context.visible.has(name));
  const consumerSources = active
    ? entries.map(([name]) => ({
        path: name,
        sha256: context.visible.has(name) ? sha(context.read(name)) : null,
      }))
    : [];
  const consumerVerified =
    active &&
    entries.every(
      ([name, expected]) =>
        context.tracked.has(name) &&
        consumerSources.some((row) => row.path === name && row.sha256 === expected),
    );
  const typeProfileVerified = isolatedStrictJsProfile(
    selected,
    new Set([...BROWSER_REFERENCE_HELPERS, BROWSER_REFERENCE_HOST]),
  );
  const closure =
    active && typeProfileVerified
      ? browserReferenceClosure(context)
      : {
          compiledOwnedSources: [],
          compilerLibraryCount: 0,
          closureVerified: false,
          inputDigest: null,
        };
  const binding = {
    presentPaths,
    consumerSources,
    consumerVerified,
    typeProfile: BROWSER_REFERENCE_PROFILE,
    typeProfileVerified,
    ...closure,
  };
  if (active) browserReferenceIssues(context, binding);
  return binding;
}

function browserReferenceIssues(
  context: ContractContext,
  binding: OwnedValidationReport["browserReferenceHelpers"],
): void {
  for (const name of BROWSER_REFERENCE_HELPERS)
    if (!binding.presentPaths.includes(name))
      add(context, "browser-reference-helper-missing", name);
  if (!binding.consumerVerified)
    add(context, "browser-reference-consumer-unverified", BROWSER_REFERENCE_PROFILE);
  if (!binding.typeProfileVerified)
    add(context, "browser-reference-type-profile-unverified", BROWSER_REFERENCE_PROFILE);
  if (
    !binding.closureVerified &&
    !context.issues.some((issue) => issue.code === "browser-reference-closure-unverified")
  )
    add(context, "browser-reference-closure-unverified", BROWSER_REFERENCE_PROFILE);
}

function javascriptEnvironment(name: string, source: string): string {
  if (hasBunTests(source)) return "node-bun-test";
  if (hasPlaywrightTests(source)) return "node-playwright-test";
  if (name.includes("/tests/fixtures/")) return "browser-fixture";
  if (name.startsWith("scripts/")) return "module-tool";
  if (/\/(?:mermaid|drawio|mathjax)_renderer\/|\/html_runtime\//.test(name))
    return "native-host-shim";
  return "unclassified";
}
function hasBunTests(source: string): boolean {
  const module = parseSync(source, { syntax: "typescript", tsx: true });
  return module.body.some(
    (node) =>
      node.type === "ImportDeclaration" && !node.typeOnly && node.source.value === "bun:test",
  );
}
function hasPlaywrightTests(source: string): boolean {
  return parseSync(source, { syntax: "typescript", tsx: true }).body.some(
    (node) =>
      node.type === "ImportDeclaration" &&
      !node.typeOnly &&
      node.source.value === "playwright/test" &&
      node.specifiers.some(
        (specifier) =>
          specifier.type === "ImportSpecifier" &&
          !specifier.isTypeOnly &&
          (specifier.imported?.value ?? specifier.local.value) === "test",
      ),
  );
}
function discoverableBunTest(name: string): boolean {
  return /(?:_|\.)(?:test|spec)\.[cm]?[jt]sx?$/.test(name);
}
function javascriptTypeScope(
  context: ContractContext,
  name: string,
  environment: string,
  scopes: TypeScope[],
  generatedParserOutput: boolean,
  coverage: OwnedValidationReport["javascriptTypeCoverage"],
): void {
  const profileMatched = scopes.some((scope) => scope.options.allowJs && scope.options.checkJs);
  const mandatory =
    environment !== "native-host-shim" ||
    discoverableBunTest(name) ||
    name === PARSER_BODY_OUTPUT ||
    name === MATHJAX_RUNTIME ||
    name === NATIVE_WIDTH_TRACE ||
    name === NATIVE_WIDTH_BBOX_TRACE ||
    BROWSER_REFERENCE_HELPERS.has(name) ||
    NATIVE_TYPED_UTILITIES.has(name);
  if (mandatory) coverage.mandatoryPaths.push(name);
  if (profileMatched) coverage.profileMatchedPaths.push(name);
  if (generatedParserOutput) coverage.generatedBoundPaths.push(name);
  if (!profileMatched && !generatedParserOutput) {
    if (mandatory) add(context, "type-scope-missing", name);
    else coverage.deferredProductPaths.push(name);
  }
  // 導入済みの製品型境界は、他profileへの移動や後続扱いで失わせない。
  if (
    NATIVE_TYPED_UTILITIES.has(name) &&
    !scopes.some(
      (scope) =>
        scope.name === NATIVE_PURE_TYPE_PROFILE && scope.options.allowJs && scope.options.checkJs,
    )
  )
    add(context, "native-product-type-profile-unverified", name, NATIVE_PURE_TYPE_PROFILE);
}
function scriptStatic(
  context: ContractContext,
  name: string,
  language: Format,
  source: string,
  biome: z.infer<typeof biomeContract>,
  types: TypeScope[],
  coverage: OwnedValidationReport["javascriptTypeCoverage"],
): string | undefined {
  const lintDisabled =
    biome.linter?.enabled === false ||
    biome.overrides.some(
      (override) => included(name, override.includes) && override.linter?.enabled === false,
    );
  if (!included(name, biome.files.includes) || lintDisabled)
    add(context, "biome-scope-missing", name);
  const parserBinding = context.parserBodyOnloadBinding;
  const generatedParserOutput =
    name === PARSER_BODY_OUTPUT &&
    parserBinding?.producerVerified === true &&
    parserBinding.canonicalTypeScope &&
    parserBinding.producerTypeScope &&
    parserBinding.biomeIncluded &&
    parserBinding.localCheckRegistered &&
    parserBinding.ciCheckRegistered;
  const scopes = types.filter((scope) => scope.files.has(name));
  if (language !== "javascript") {
    if (scopes.length === 0) add(context, "type-scope-missing", name);
    return undefined;
  }
  const environment = javascriptEnvironment(name, source);
  javascriptTypeScope(context, name, environment, scopes, generatedParserOutput, coverage);
  if (environment === "unclassified") add(context, "js-environment-unclassified", name);
  if (
    environment === "native-host-shim" &&
    scopes.some(
      (scope) =>
        scope.options.lib === undefined || scope.options.lib.some((lib) => lib.includes("dom")),
    )
  )
    add(context, "native-host-dom-types-unverified", name);
  return environment;
}

const PYTHON_RUNTIME_BINDINGS = [
  'HTML_PERFORMANCE_SCIPY_VERSION := "1.18.1"',
  'HTML_PERFORMANCE_SCIPY_ENV := "KRR_HTML_PERFORMANCE_SCIPY_VERSION=" + HTML_PERFORMANCE_SCIPY_VERSION',
  'HTML_PERFORMANCE_TYPING_PATH := "scripts/release/typings"',
  'HTML_PERFORMANCE_PYTHON_REQUIREMENTS := REPO_ROOT + "/scripts/release/html-performance-requirements.lock"',
] as const;

function pythonRuntimeBindingsVerified(context: ContractContext): boolean {
  const definitions = new Set(
    context
      .read("Justfile")
      .split(/\r?\n/)
      .map((line) => line.trim()),
  );
  return PYTHON_RUNTIME_BINDINGS.every((binding) => definitions.has(binding));
}

function pythonStatic(context: ContractContext, name: string): void {
  const policy = context.policy.python;
  if (!policy || !name.startsWith("scripts/")) {
    add(context, "python-static-scope-missing", name);
    return;
  }
  const lint = shellWords(context.recipes.get(policy.lintRecipe) ?? "").join("\0");
  const types = shellWords(context.recipes.get(policy.typeRecipe) ?? "").join("\0");
  if (
    lint !==
      [
        "uvx",
        "--from",
        `ruff==${policy.ruff}`,
        "ruff",
        "check",
        "--select",
        "F,E4,E7,E9,B,UP",
        "scripts",
      ].join("\0") ||
    !registered(context, policy.lintRecipe) ||
    !pythonRuntimeBindingsVerified(context)
  )
    add(context, "python-lint-unregistered", name);
  if (
    types !==
      [
        "MYPYPATH={{HTML_PERFORMANCE_TYPING_PATH}}",
        "{{HTML_PERFORMANCE_SCIPY_ENV}}",
        "uvx",
        "--with-requirements",
        "{{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}}",
        "--from",
        `mypy==${policy.mypy}`,
        "mypy",
        "--strict",
        "scripts",
      ].join("\0") ||
    !registered(context, policy.typeRecipe) ||
    !pythonRuntimeBindingsVerified(context)
  )
    add(context, "python-type-unregistered", name);
}
const HTML_PERFORMANCE_TEST_WRAPPER = "scripts/release/run_html_performance.py";
const HTML_PERFORMANCE_TEST_WRAPPER_SHA256 =
  "37e5ec612ef7ca1899c5cd69346ecea4da6de2a8220bfc2bc96ef838566f12dc";
function pythonPerformanceTestSelection(
  context: ContractContext,
  words: string[],
  name: string,
): boolean {
  if (!name.startsWith("scripts/release/") || !context.visible.has(HTML_PERFORMANCE_TEST_WRAPPER))
    return false;
  if (!context.tracked.has(HTML_PERFORMANCE_TEST_WRAPPER)) return false;
  if (sha(context.read(HTML_PERFORMANCE_TEST_WRAPPER)) !== HTML_PERFORMANCE_TEST_WRAPPER_SHA256)
    return false;
  const expected = [
    "{{HTML_PERFORMANCE_SCIPY_ENV}}",
    "uv",
    "run",
    "--isolated",
    "--with-requirements",
    "{{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}}",
    "python",
    HTML_PERFORMANCE_TEST_WRAPPER,
    "test",
  ];
  return words.join("\0") === expected.join("\0");
}
function hasRuffConfiguration(root: string, name: string): boolean {
  if (name.endsWith("ruff.toml")) return true;
  const parsed: unknown = Bun.TOML.parse(read(root, name));
  if (typeof parsed !== "object" || parsed === null || !("tool" in parsed)) return false;
  const tool = z.record(z.string(), z.unknown()).safeParse(parsed.tool);
  return tool.success && Object.hasOwn(tool.data, "ruff");
}
function ancestorDirectories(file: string): string[] {
  const directories: string[] = [];
  let directory = path.posix.dirname(file);
  while (true) {
    directories.push(directory);
    if (directory === ".") return directories;
    const parent = path.posix.dirname(directory);
    directory = parent === directory ? "." : parent;
  }
}
function activeRuffConfig(context: ContractContext, name: string): boolean {
  const absolute = path.join(context.root, name);
  try {
    fs.lstatSync(absolute);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return false;
    throw error;
  }
  try {
    safeFile(context.root, name);
  } catch (error) {
    add(
      context,
      "unsafe-ruff-config",
      name,
      error instanceof Error ? error.message : String(error),
    );
    return true;
  }
  return hasRuffConfiguration(context.root, name);
}
function ruffConfigAncestors(context: ContractContext, pythonFiles: string[]): string[] {
  const directories = new Set(pythonFiles.flatMap(ancestorDirectories));
  const names = [...directories].flatMap((directory) =>
    ["ruff.toml", ".ruff.toml", "pyproject.toml"].map((config) =>
      path.posix.join(directory, config),
    ),
  );
  return names.filter((name) => activeRuffConfig(context, name)).sort();
}
function externalConfigExists(absolute: string): boolean {
  try {
    fs.lstatSync(absolute);
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return false;
    throw error;
  }
}
function externalConfigComponentsSafe(context: ContractContext, absolute: string): boolean {
  let current = path.parse(absolute).root;
  const segments = absolute.slice(current.length).split(path.sep);
  for (const [index, segment] of segments.entries()) {
    current = path.join(current, segment);
    const stat = fs.lstatSync(current);
    if (stat.isSymbolicLink() || (index < segments.length - 1 && !stat.isDirectory())) {
      add(context, "unsafe-ruff-config", absolute);
      return false;
    }
    if (index === segments.length - 1 && !stat.isFile()) {
      add(context, "unsafe-ruff-config", absolute);
      return false;
    }
  }
  return true;
}
function externalRuffConfigStatus(
  context: ContractContext,
  absolute: string,
): "missing" | "unsafe" | "file" {
  if (!externalConfigExists(absolute)) return "missing";
  return externalConfigComponentsSafe(context, absolute) ? "file" : "unsafe";
}
function activeExternalRuffConfig(context: ContractContext, absolute: string): boolean {
  const status = externalRuffConfigStatus(context, absolute);
  if (status === "missing") return false;
  if (status === "unsafe") return true;
  if (path.basename(absolute) !== "pyproject.toml") return true;
  const parsed: unknown = Bun.TOML.parse(fs.readFileSync(absolute, "utf8"));
  if (typeof parsed !== "object" || parsed === null || !("tool" in parsed)) return false;
  const tool = z.record(z.string(), z.unknown()).safeParse(parsed.tool);
  return tool.success && Object.hasOwn(tool.data, "ruff");
}
function externalRuffConfigs(context: ContractContext): string[] {
  const directories = new Set<string>();
  let parent = path.dirname(context.root);
  while (true) {
    directories.add(parent);
    const next = path.dirname(parent);
    if (next === parent) break;
    parent = next;
  }
  const configHome = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config");
  directories.add(path.join(configHome, "ruff"));
  const names = [...directories].flatMap((directory) =>
    ["ruff.toml", ".ruff.toml", "pyproject.toml"].map((config) => path.join(directory, config)),
  );
  return [...new Set(names)].filter((name) => activeExternalRuffConfig(context, name)).sort();
}
function checkTomliRuffConfiguration(context: ContractContext): void {
  if (!context.visible.has(TOMLI_RUFF_CONFIG)) return;
  const parsed = tomliRuffConfiguration.safeParse(Bun.TOML.parse(context.read(TOMLI_RUFF_CONFIG)));
  if (!parsed.success) add(context, "tomli-ruff-config-invalid", TOMLI_RUFF_CONFIG);
}
function checkTomliProvenance(context: ContractContext): void {
  if (!context.visible.has(TOMLI_PROVENANCE)) return;
  const parsed = tomliProvenance.safeParse(json(context.root, TOMLI_PROVENANCE));
  if (!parsed.success) {
    add(context, "tomli-provenance-invalid", TOMLI_PROVENANCE);
    return;
  }
  const expectedFiles = Object.fromEntries(
    Object.entries(TOMLI_FILES).filter(([name]) => name !== `${TOMLI_ROOT}/LICENSE`),
  );
  const actualNames = Object.keys(parsed.data.files).sort();
  const expectedNames = Object.keys(expectedFiles).sort();
  if (
    actualNames.length !== expectedNames.length ||
    actualNames.some(
      (name, index) =>
        name !== expectedNames[index] || parsed.data.files[name] !== expectedFiles[name],
    )
  )
    add(context, "tomli-provenance-files-mismatch", TOMLI_PROVENANCE);
}
function checkTomliOfficialFiles(context: ContractContext): void {
  for (const [name, expected] of Object.entries(TOMLI_FILES)) {
    if (!context.visible.has(name)) continue;
    if (sha(fs.readFileSync(safeFile(context.root, name))) !== expected)
      add(context, "tomli-official-file-mismatch", name);
  }
}
function tomliAuthorityFingerprint(
  context: ContractContext,
  names: string[],
  activeRuffConfigs: Set<string>,
): string[] {
  return names.map((name) => {
    if (!context.visible.has(name) && !activeRuffConfigs.has(name)) return `${name}\0missing\n`;
    try {
      return `${name}\0${sha(fs.readFileSync(safeFile(context.root, name)))}\n`;
    } catch {
      const absolute = path.join(context.root, name);
      if (fs.existsSync(absolute) && fs.lstatSync(absolute).isSymbolicLink())
        return `${name}\0symlink:${sha(fs.readlinkSync(absolute))}\n`;
      return `${name}\0unsafe\n`;
    }
  });
}
function checkTomliAuthorities(context: ContractContext, authorityNames: string[]): void {
  for (const name of authorityNames) {
    if (!context.visible.has(name)) add(context, "tomli-authority-missing", name);
    if (!context.tracked.has(name)) add(context, "untracked-authority", name);
  }
  for (const name of [...context.visible].filter((visible) => visible.startsWith(`${TOMLI_ROOT}/`)))
    if (!authorityNames.includes(name)) add(context, "unknown-tomli-vendor-file", name);
  checkTomliRuffConfiguration(context);
  checkTomliProvenance(context);
  checkTomliOfficialFiles(context);
}
function tomliRuffAuthority(context: ContractContext, pythonFiles: string[]): string[] {
  const activeRuffConfigs = ruffConfigAncestors(context, pythonFiles);
  const externalConfigs = externalRuffConfigs(context);
  const vendorPresent = [...context.visible].some((name) => name.startsWith(`${TOMLI_ROOT}/`));
  for (const name of activeRuffConfigs)
    if (name !== TOMLI_RUFF_CONFIG || !vendorPresent) add(context, "unknown-ruff-config", name);
  for (const name of externalConfigs) add(context, "unknown-ruff-config", name);

  const externalFingerprints = externalConfigs.map((name) => {
    try {
      return `${name}\0${sha(fs.readFileSync(name))}\n`;
    } catch {
      const stat = fs.lstatSync(name);
      return `${name}\0unsafe:${stat.isSymbolicLink() ? sha(fs.readlinkSync(name)) : "non-file"}\n`;
    }
  });
  if (!vendorPresent)
    return [
      ...activeRuffConfigs.map((name) => {
        try {
          return `${name}\0${sha(context.read(name))}\n`;
        } catch {
          return `${name}\0unsafe\n`;
        }
      }),
      ...externalFingerprints,
    ];

  const authorityNames = [
    ...Object.keys(TOMLI_FILES),
    TOMLI_PROVENANCE,
    TOMLI_RUFF_CONFIG,
    ...activeRuffConfigs.filter((name) => name !== TOMLI_RUFF_CONFIG),
  ];
  checkTomliAuthorities(context, authorityNames);
  return [
    ...tomliAuthorityFingerprint(context, authorityNames, new Set(activeRuffConfigs)),
    ...externalFingerprints,
  ];
}
function rustMembers(context: ContractContext): string[] {
  const name = context.policy.rust?.workspace ?? "Cargo.toml";
  const manifest = z
    .object({
      workspace: z.object({ members: strings, exclude: strings.default([]) }),
      package: z.unknown().optional(),
    })
    .parse(Bun.TOML.parse(context.read(name)));
  const visible = context.index;
  const candidates = [...visible.tracked, ...visible.untracked]
    .filter((file) => file.endsWith("/Cargo.toml"))
    .map((file) => path.posix.dirname(file));
  const members: string[] = manifest.package ? ["."] : [];
  for (const pattern of manifest.workspace.members) {
    if (!safeRelative(pattern)) {
      add(context, "unsafe-workspace-member", name, pattern);
      continue;
    }
    const matchesPattern = candidates.filter(
      (member) =>
        new Bun.Glob(pattern).match(member) &&
        !manifest.workspace.exclude.some((exclude) => new Bun.Glob(exclude).match(member)),
    );
    if (matchesPattern.length === 0) add(context, "rust-workspace-member-missing", name, pattern);
    members.push(...matchesPattern);
  }
  for (const member of members) {
    const file = member === "." ? name : `${member}/Cargo.toml`;
    safeFile(context.root, file);
    if (!context.tracked.has(file)) add(context, "untracked-authority", file);
  }
  return [...new Set(members)].sort((left, right) => right.length - left.length);
}

const PERFORMANCE_TEST_ONLY_COVERAGE_REGEX = String.raw`(^|[/\\])scripts[/\\]html[/\\]performance-harness[/\\]src[/\\]([^/\\]+[/\\])*[^/\\]+(_tests|_test_support)\.rs$`;

function performanceCoverageCommands(arm: (typeof PERFORMANCE_ARMS)[number]): string[] {
  const directory = `llvm-cov-performance-${arm}`;
  const domain = `performance-${arm}`;
  const cleanPrefix = `CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/${directory}" python3 scripts/build/cargo_capacity_guard.py --coverage-domain ${domain} -- {{CARGO}} llvm-cov`;
  const prefix = `CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/${directory}" python3 scripts/build/cargo_capacity_guard.py --coverage-collection-domain ${domain} -- {{CARGO}} llvm-cov`;
  const manifest = ` --manifest-path ${PERFORMANCE_MANIFEST}`;
  const selection = `--no-default-features --features ${arm}`;
  const report = `--ignore-filename-regex {{quote(COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX)}} --json --output-path "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage/${arm}.json"`;
  const collector = `${prefix}${manifest} --workspace --all-targets ${selection} --locked --profile krr-coverage ${report}{{TEST_THREAD_ARGS}}`;
  return [`${cleanPrefix} clean${manifest} --workspace --locked`, collector];
}

function performanceCoverageExclusionContract(context: ContractContext): boolean {
  const justfile = context.read("Justfile");
  const guard = context.read("scripts/build/cargo_capacity_guard.py");
  return (
    justfile.includes(
      `COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX := "${PERFORMANCE_TEST_ONLY_COVERAGE_REGEX.replaceAll("\\", "\\\\")}"`,
    ) &&
    guard.includes(String.raw`r"(^|[/\\])scripts[/\\]html[/\\]performance-harness[/\\]src[/\\]"`) &&
    guard.includes(String.raw`r"([^/\\]+[/\\])*[^/\\]+(_tests|_test_support)\.rs$"`)
  );
}
function localCoverageCommand(
  producer: "root" | "performance",
  phase: "begin" | "finish" | "validate",
): string {
  if (phase === "finish")
    return `python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-collect-finish --producer ${producer} --producer-exit 0`;
  if (phase === "validate")
    return `python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-collection-validate --producer ${producer}`;
  return `python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-begin --producer ${producer}`;
}
function rootLintRecipeLines(context: ContractContext): string[] {
  return (context.recipes.get("lint") ?? "")
    .split("\n")
    .map((line) => line.trim().replace(/^@/, ""))
    .filter(Boolean);
}
function expectedRootLintRecipeLines(): string[] {
  const deny = PERFORMANCE_CLIPPY_DENY.map((name) => `-D ${name}`).join(" ");
  return [
    "set -euo pipefail; \\",
    'retained_profile="{{REPO_ROOT}}/target/llvm-cov-target/krr-coverage"; \\',
    'retained_profile_arg=""; \\',
    "retained_args=(); \\",
    'if [[ -d "$retained_profile" && ! -L "$retained_profile" ]]; then \\',
    'retained_args=(--retain-profile "$retained_profile"); \\',
    'retained_profile_arg="$retained_profile"; \\',
    "fi; \\",
    "just capacity-admission; \\",
    `RUSTFLAGS="-D warnings" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" python3 scripts/build/cargo_capacity_guard.py "\${retained_args[@]}" --check-domain root -- {{CARGO}} clippy -j {{JOBS}} --workspace --all-targets --all-features -- ${deny}; \\`,
    "just capacity-admission; \\",
    'just performance-harness-lint "$retained_profile_arg"',
  ];
}
function expectedCapacityAdmissionRecipeLines(): string[] {
  return [
    "@set -euo pipefail; \\",
    'retained_profile="{{REPO_ROOT}}/target/llvm-cov-target/krr-coverage"; \\',
    "retained_args=(); \\",
    'if [[ -d "$retained_profile" && ! -L "$retained_profile" ]]; then \\',
    'retained_args=(--retain-profile "$retained_profile"); \\',
    "fi; \\",
    `python3 scripts/build/cargo_capacity_guard.py --root "{{REPO_ROOT}}" --target "{{REPO_ROOT}}/target" --state "{{REPO_ROOT}}/tmp/.cargo-capacity-guard.json" --require-capacity --print-json "\${retained_args[@]}"`,
  ];
}
function rootLintCargoCommand(context: ContractContext): string | undefined {
  const actual = rootLintRecipeLines(context);
  const expected = expectedRootLintRecipeLines();
  return actual.length === expected.length &&
    actual.every((line, index) => line === expected[index])
    ? actual[9]
    : undefined;
}
function performanceRecipeCommands(): Record<string, string[]> {
  const deny = PERFORMANCE_CLIPPY_DENY.map((name) => `-D ${name}`).join(" ");
  return {
    "native-coverage-cache-key": [
      'CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-key --producer {{quote(producer)}} --moon-fingerprint',
    ],
    [PERFORMANCE_RECIPES.format]: [
      `{{CARGO}} fmt --manifest-path ${PERFORMANCE_MANIFEST} --all -- --check`,
    ],
    [PERFORMANCE_RECIPES.lint]: [
      "@set -euo pipefail; \\",
      "retained_args=(); \\",
      "if [[ -n {{quote(retained_profile)}} ]]; then retained_args=(--retain-profile {{quote(retained_profile)}}); fi; \\",
      `RUSTFLAGS="-D warnings" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" python3 scripts/build/cargo_capacity_guard.py "\${retained_args[@]}" --check-domain performance-baseline -- {{CARGO}} clippy -j {{JOBS}} --manifest-path ${PERFORMANCE_MANIFEST} --all-targets --no-default-features --features baseline --locked -- ${deny}; \\`,
      `RUSTFLAGS="-D warnings" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" python3 scripts/build/cargo_capacity_guard.py "\${retained_args[@]}" --check-domain performance-candidate -- {{CARGO}} clippy -j {{JOBS}} --manifest-path ${PERFORMANCE_MANIFEST} --all-targets --no-default-features --features candidate --locked -- ${deny}`,
    ],
    [PERFORMANCE_RECIPES.coverage]: [PERFORMANCE_COVERAGE_WRAPPER],
    [PERFORMANCE_COVERAGE_RUN_RECIPE]: [
      'mkdir -p "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage"',
      localCoverageCommand("performance", "begin"),
      ...PERFORMANCE_ARMS.flatMap(performanceCoverageCommands),
      localCoverageCommand("performance", "finish"),
      localCoverageCommand("performance", "validate"),
    ],
    "fmt-check": ["{{CARGO}} fmt --all -- --check", `just ${PERFORMANCE_RECIPES.format}`],
    coverage: [
      "just root-test-and-coverage",
      `just ${PERFORMANCE_RECIPES.coverage}`,
      "just coverage-result-check",
    ],
    "coverage-result-check": [
      'python3 scripts/release/native_coverage_local_aggregate.py --repo "{{REPO_ROOT}}"',
    ],
    "root-test-and-coverage": [
      "just plantuml-install",
      'CARGO={{quote(CARGO)}} CARGO_BUILD_JOBS={{quote(JOBS)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/release/native_nextest_root_coverage.py --repo "{{REPO_ROOT}}"',
    ],
  };
}
function exactPerformanceRecipe(
  context: ContractContext,
  recipe: string,
  expected: string[],
): boolean {
  const lines = (context.recipes.get(recipe) ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  return lines.length === expected.length && lines.every((line, index) => line === expected[index]);
}
function performanceManifestAndLock(context: ContractContext, members: string[]): void {
  const dependency = z.object({
    package: z.literal("katana-render-runtime"),
    optional: z.literal(true),
  });
  const manifest = z
    .object({
      package: z
        .object({
          name: z.literal("krr-html-performance-harness"),
          version: z.string(),
          edition: z.literal("2021"),
          publish: z.literal(false),
        })
        .strict(),
      workspace: z.object({}).strict(),
      profile: z
        .object({
          dev: z.object({ debug: z.literal(1) }).strict(),
          "krr-coverage": z
            .object({
              inherits: z.literal("dev"),
              debug: z.literal(0),
              incremental: z.literal(false),
            })
            .strict(),
        })
        .strict(),
      features: z
        .object({
          baseline: z.tuple([z.literal("dep:baseline")]),
          candidate: z.tuple([
            z.literal("dep:candidate"),
            z.literal("dep:image"),
            z.literal("dep:url"),
          ]),
        })
        .strict(),
      dependencies: z
        .object({
          baseline: dependency
            .extend({ version: z.literal("=0.4.23"), registry: z.literal("crates-io") })
            .strict(),
          candidate: dependency
            .extend({ path: z.literal("../../../crates/katana-render-runtime") })
            .strict(),
          image: z
            .object({
              version: z.literal("0.25.10"),
              optional: z.literal(true),
              "default-features": z.literal(false),
              features: z.tuple([z.literal("png")]),
            })
            .strict(),
          url: z.object({ version: z.literal("2.5.8"), optional: z.literal(true) }).strict(),
          sha2: z.string(),
          nix: z
            .object({
              version: z.literal("0.31.3"),
              "default-features": z.literal(false),
              features: z.tuple([z.literal("resource")]),
            })
            .strict(),
          serde_json: z.string(),
          serde: z
            .object({ version: z.string(), features: z.tuple([z.literal("derive")]) })
            .strict(),
        })
        .strict(),
      "dev-dependencies": z.object({ tempfile: z.literal("3") }).strict(),
    })
    .strict()
    .parse(Bun.TOML.parse(context.read(PERFORMANCE_MANIFEST)));
  const lock = z
    .object({
      version: z.literal(4),
      package: z.array(
        z
          .object({
            name: z.string(),
            version: z.string(),
            source: z.string().optional(),
            checksum: z.string().optional(),
            dependencies: strings.default([]),
          })
          .passthrough(),
      ),
    })
    .strict()
    .parse(Bun.TOML.parse(context.read(`${PERFORMANCE_HARNESS}/Cargo.lock`)));
  const rootLock = z
    .object({
      version: z.literal(4),
      package: z.array(
        z
          .object({
            name: z.string(),
            version: z.string(),
            source: z.string().optional(),
            checksum: z.string().optional(),
          })
          .passthrough(),
      ),
    })
    .strict()
    .parse(Bun.TOML.parse(context.read("Cargo.lock")));
  const version = z
    .string()
    .regex(/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/)
    .refine((value) => Bun.semver.satisfies(value, value));
  const candidate = z
    .object({
      package: z.object({
        name: z.literal("katana-render-runtime"),
        version: z.union([version, z.object({ workspace: z.literal(true) }).strict()]),
        workspace: z.literal("../..").optional(),
      }),
      workspace: z.never().optional(),
    })
    .parse(Bun.TOML.parse(context.read("crates/katana-render-runtime/Cargo.toml")));
  let candidateVersion = candidate.package.version;
  if (typeof candidateVersion !== "string") {
    // 固定candidateだけを既存member一覧へ拘束し、別workspaceの版を借りない。
    if (
      (context.policy.rust?.workspace ?? "Cargo.toml") !== "Cargo.toml" ||
      !members.includes("crates/katana-render-runtime")
    )
      throw new Error("candidate workspace membership is unverified");
    const intermediate = path.join(context.root, "crates/Cargo.toml");
    if (
      candidate.package.workspace === undefined &&
      fs.lstatSync(intermediate, { throwIfNoEntry: false })
    ) {
      z.object({ workspace: z.never().optional() }).parse(
        Bun.TOML.parse(context.read("crates/Cargo.toml")),
      );
    }
    const root = z
      .object({ workspace: z.object({ package: z.object({ version }) }) })
      .parse(Bun.TOML.parse(context.read("Cargo.toml")));
    candidateVersion = root.workspace.package.version;
  }
  const armPackages = lock.package.filter((row) => row.name === "katana-render-runtime");
  const harness = lock.package.filter((row) => row.name === manifest.package.name);
  const captureDependencies = [
    ["image", "0.25.10"],
    ["url", "2.5.8"],
  ] as const;
  const registryDependencyPinned = ([name, version]: (typeof captureDependencies)[number]) => {
    const registry = "registry+https://github.com/rust-lang/crates.io-index";
    const harnessRows = lock.package.filter((row) => row.name === name);
    const rootRows = rootLock.package.filter((row) => row.name === name);
    return (
      harnessRows.length === 1 &&
      rootRows.length === 1 &&
      harnessRows[0]?.version === version &&
      rootRows[0]?.version === version &&
      harnessRows[0]?.source === registry &&
      rootRows[0]?.source === registry &&
      /^[a-f0-9]{64}$/.test(harnessRows[0]?.checksum ?? "") &&
      harnessRows[0]?.checksum === rootRows[0]?.checksum &&
      harness[0]?.dependencies.includes(name) === true
    );
  };
  if (
    harness.length !== 1 ||
    harness[0]?.version !== manifest.package.version ||
    !harness[0]?.dependencies.includes("katana-render-runtime 0.4.23") ||
    !harness[0]?.dependencies.includes(`katana-render-runtime ${candidateVersion}`) ||
    !captureDependencies.every(registryDependencyPinned) ||
    armPackages.length !== 2 ||
    !armPackages.some(
      (row) =>
        row.version === "0.4.23" &&
        row.source === "registry+https://github.com/rust-lang/crates.io-index" &&
        /^[a-f0-9]{64}$/.test(row.checksum ?? ""),
    ) ||
    !armPackages.some(
      (row) =>
        row.version === candidateVersion &&
        row.source === undefined &&
        captureDependencies.every(([name]) => row.dependencies.includes(name)),
    )
  ) {
    add(context, "performance-harness-lock-unverified", `${PERFORMANCE_HARNESS}/Cargo.lock`);
  }
}
function checkPerformanceRecipeCommands(context: ContractContext): void {
  if (!performanceCoverageExclusionContract(context))
    add(
      context,
      "performance-harness-recipe-unverified",
      "COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX",
    );
  for (const [recipe, expected] of Object.entries(performanceRecipeCommands())) {
    if (!exactPerformanceRecipe(context, recipe, expected))
      add(context, "performance-harness-recipe-unverified", recipe);
  }
}
function performanceParentSuffix(lane: string, recipe: string): string[] {
  if (lane === "coverage") return [`just ${recipe}`, "just coverage-result-check"];
  if (lane === "lint") return [`just ${recipe} "$retained_profile_arg"`];
  return [`just ${recipe}`];
}
function performanceRoutes(context: ContractContext): void {
  checkPerformanceRecipeCommands(context);
  if (
    !exactPerformanceRecipe(context, "capacity-admission", expectedCapacityAdmissionRecipeLines())
  )
    add(context, "capacity-admission-unverified", "capacity-admission");
  if (rootLintCargoCommand(context) === undefined)
    add(context, "performance-harness-recipe-unverified", "lint");
  for (const [lane, recipe] of Object.entries(PERFORMANCE_RECIPES)) {
    if (!registered(context, recipe))
      add(context, "performance-harness-recipe-unregistered", recipe);
    const parent = lane === "format" ? "fmt-check" : lane;
    const lines = (context.recipes.get(parent) ?? "").split("\n").map((line) => line.trim());
    const suffix = performanceParentSuffix(lane, recipe);
    if (lines.slice(-suffix.length).join("\0") !== suffix.join("\0"))
      add(context, "performance-harness-parent-unverified", parent);
  }
  const authority = context.read("Justfile");
  const declarations = [
    "REPO_ROOT := justfile_directory()",
    'COVERAGE_MIN_LINES := env_var_or_default("COVERAGE_MIN_LINES", "100")',
    'COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default("COVERAGE_MAX_UNCOVERED_LINES", "0")',
  ];
  if (!declarations.every((line) => authority.split("\n").includes(line)))
    add(context, "performance-harness-configuration-unverified", "Justfile");
  const workflow = z
    .object({
      jobs: z.record(
        z.string(),
        z
          .object({
            steps: z.array(z.object({ run: z.string().optional() }).passthrough()).default([]),
          })
          .passthrough(),
      ),
    })
    .parse(Bun.YAML.parse(context.read(context.policy.ciWorkflow)));
  if (
    Object.values(workflow.jobs).some((job) =>
      job.steps.some((step) => /(?:^|\n)\s*cargo fmt(?:\s|$)/.test(step.run ?? "")),
    )
  )
    add(context, "performance-harness-ci-format-bypass", context.policy.ciWorkflow);
}
function performanceHarnessInputDigest(context: ContractContext, inputs: string[]): string[] {
  const digest: string[] = [];
  for (const name of inputs) {
    if (!context.visible.has(name)) add(context, "performance-harness-input-missing", name);
    if (!context.tracked.has(name)) add(context, "untracked-authority", name);
    try {
      digest.push(`${name}\0${sha(context.read(name))}\n`);
    } catch {
      add(context, "performance-harness-input-unsafe", name);
    }
  }
  return digest;
}
function performanceHarnessContract(
  context: ContractContext,
  members: string[],
  report: Pick<OwnedValidationReport, "standalonePerformanceHarness">,
): string[] {
  const visible = [...context.visible].filter((name) => name.startsWith(`${PERFORMANCE_HARNESS}/`));
  if (
    visible.length === 0 &&
    !Object.values(PERFORMANCE_RECIPES).some((name) => context.recipes.has(name))
  )
    return [];
  const inputs = [
    ...new Set([
      PERFORMANCE_MANIFEST,
      `${PERFORMANCE_HARNESS}/Cargo.lock`,
      `${PERFORMANCE_HARNESS}/src/main.rs`,
      ...visible.filter((name) => name.endsWith(".rs")),
    ]),
  ].sort();
  const digest = performanceHarnessInputDigest(context, inputs);
  if (
    members.some(
      (member) =>
        member === "." ||
        member === PERFORMANCE_HARNESS ||
        PERFORMANCE_HARNESS.startsWith(`${member}/`) ||
        member.startsWith(`${PERFORMANCE_HARNESS}/`),
    )
  )
    add(context, "performance-harness-workspace-merged", PERFORMANCE_MANIFEST);
  if (visible.some((name) => name.endsWith("/Cargo.toml") && name !== PERFORMANCE_MANIFEST))
    add(context, "performance-harness-nested-workspace-unknown", PERFORMANCE_MANIFEST);
  try {
    performanceManifestAndLock(context, members);
  } catch (error) {
    add(
      context,
      "performance-harness-manifest-unverified",
      PERFORMANCE_MANIFEST,
      error instanceof Error ? error.message : String(error),
    );
  }
  try {
    performanceRoutes(context);
  } catch (error) {
    add(
      context,
      "performance-harness-recipe-unverified",
      "Justfile",
      error instanceof Error ? error.message : String(error),
    );
  }
  report.standalonePerformanceHarness = {
    inputPaths: inputs,
    inputDigest: sha(digest.join("")),
    localCoverageRegistered: context.local.has(PERFORMANCE_RECIPES.coverage),
    ciCoverageRegistered: context.ci.has(PERFORMANCE_RECIPES.coverage),
    verification: "not-executed",
  };
  return digest;
}

function rustStatic(context: ContractContext, name: string, members: string[]): void {
  if (name.startsWith(`${PERFORMANCE_HARNESS}/`)) return;
  const member = members.find((root) => root === "." || name.startsWith(`${root}/`));
  if (!member) {
    add(context, "rust-workspace-scope-missing", name);
    return;
  }
  const manifest = z
    .object({ package: z.record(z.string(), z.unknown()).optional() })
    .parse(Bun.TOML.parse(context.read(member === "." ? "Cargo.toml" : `${member}/Cargo.toml`)));
  if (!manifest.package) add(context, "rust-member-package-missing", name);
  const lintBody = rootLintCargoCommand(context) ?? "";
  const lint = shellWords(lintBody);
  const command =
    /^RUSTFLAGS="-D warnings"\s+CARGO_TARGET_DIR="\{\{REPO_ROOT\}\}\/target"\s+python3 scripts\/build\/cargo_capacity_guard\.py (?:"\$\{retained_args\[@\]\}" )?--check-domain root -- (?:cargo|\{\{CARGO\}\}) clippy(?:\s|$)/.test(
      lintBody,
    );
  if (
    !["clippy", "--workspace", "--all-targets", "--all-features"].every((flag) =>
      lint.includes(flag),
    ) ||
    !command ||
    lint.some((word) =>
      /^(?:-p|--package|--exclude|--manifest-path|--no-default-features|--lib|--bin|--bins|--test|--tests|--example|--examples)(?:=|$)/.test(
        word,
      ),
    ) ||
    !registered(context, context.policy.rust?.lintRecipe ?? "")
  )
    add(context, "rust-lint-unregistered", name);
}

function pythonUnits(context: ContractContext, names: string[]): string[] {
  if (names.length === 0) return [];
  const output = execFileSync(
    "python3",
    [path.join(import.meta.dir, "owned-python-test-inventory.py"), "--root", context.root],
    { input: JSON.stringify(names), encoding: "utf8" },
  );
  const records = pythonRecords.parse(JSON.parse(output));
  if (records.length !== names.length || records.some((row, index) => row.path !== names[index]))
    throw new Error("Python classifier input/output inventory differs");
  for (const row of records)
    if (row.kind === "unknown") add(context, "python-unit-classification-unknown", row.path);
  return records.filter((row) => row.kind === "unittest").map((row) => row.path);
}
function explicitBunTarget(root: string, target: string): string | undefined {
  if (target === "./") return ".";
  let relativeTarget: string;
  if (target.startsWith("./")) relativeTarget = target.slice(2);
  else if (path.isAbsolute(target)) relativeTarget = relative(root, target);
  else return undefined;
  if (relativeTarget === ".") return relativeTarget;
  return safeRelative(relativeTarget) ? relativeTarget : undefined;
}
function bunTestArgument(
  context: ContractContext,
  rest: string[],
  index: number,
): { kind: "ignore" | "target"; value: string } | undefined {
  const word = rest[index] ?? "";
  if (word === "--path-ignore-patterns") {
    const pattern = rest[index + 1];
    return pattern && !pattern.startsWith("-") ? { kind: "ignore", value: pattern } : undefined;
  }
  if (word.startsWith("-")) return undefined;
  const target = explicitBunTarget(context.root, word);
  return target === undefined ? undefined : { kind: "target", value: target };
}
function bunTestSelection(
  context: ContractContext,
  words: string[],
): { ignores: string[]; targets: string[] } | undefined {
  if (words[0] !== "bun" || words[1] !== "test") return undefined;
  const rest = words.slice(2);
  const ignores: string[] = [];
  const targets: string[] = [];
  for (let index = 0; index < rest.length; index += 1) {
    const argument = bunTestArgument(context, rest, index);
    if (!argument) return undefined;
    if (argument.kind === "ignore") {
      ignores.push(argument.value);
      index += 1;
    } else targets.push(argument.value);
  }
  return { ignores, targets };
}
function bunSelection(context: ContractContext, words: string[], name: string): boolean {
  const selection = bunTestSelection(context, words);
  if (!selection || selection.ignores.some((pattern) => matches(name, pattern))) return false;
  const discoverable = discoverableBunTest(name);
  return selection.targets.length === 0
    ? discoverable
    : selection.targets.some(
        (target) =>
          target === name ||
          (/[?*{}[\]]/.test(target) && matches(name, target)) ||
          (discoverable && (target === "." || name.startsWith(`${target}/`))),
      );
}
function pythonDiscoveryOptions(words: string[]): boolean {
  let failfastSeen = false;
  for (let index = 4; index < words.length; index += 1) {
    if (["-s", "-p"].includes(words[index] ?? "")) index += 1;
    else if (words[index] === "--failfast" && !failfastSeen) failfastSeen = true;
    else if (!["-v", "-q"].includes(words[index] ?? "")) return false;
  }
  return true;
}

function pythonSelection(context: ContractContext, words: string[], name: string): boolean {
  if (
    !["python", "python3"].includes(words[0] ?? "") ||
    words[1] !== "-m" ||
    words[2] !== "unittest" ||
    words[3] !== "discover"
  )
    return false;
  if (!pythonDiscoveryOptions(words)) return false;
  const start = words.indexOf("-s");
  const pattern = words.indexOf("-p");
  const directory = start >= 0 ? (words[start + 1] ?? "") : ".";
  const glob = pattern >= 0 ? (words[pattern + 1] ?? "") : "test*.py";
  if (
    (directory !== "." && !safeRelative(directory)) ||
    (directory !== "." && !name.startsWith(`${directory}/`)) ||
    !new Bun.Glob(glob).match(path.posix.basename(name))
  )
    return false;
  const subdirs = path.posix
    .relative(directory, path.posix.dirname(name))
    .split("/")
    .filter(Boolean);
  return subdirs.every((_part, index) =>
    fs.existsSync(
      path.join(context.root, directory, ...subdirs.slice(0, index + 1), "__init__.py"),
    ),
  );
}
const RTK_PROXY_COMMAND_DEFINITIONS = [
  'RTK := env_var_or_default("RTK", `command -v rtk 2> /dev/null || true`)',
  'RTK_PROXY_CMD := if RTK == "" { "" } else { RTK + " proxy " }',
] as const;
function rtkProxyCommandVerified(source: string): boolean {
  const lines = source.split(/\r?\n/u);
  const [rtkDefinition, proxyDefinition] = RTK_PROXY_COMMAND_DEFINITIONS;
  const rtkLines = lines.filter((line) => /^\s*(?:export\s+)?RTK\s*:=/u.test(line));
  const proxyLines = lines.filter((line) => /^\s*(?:export\s+)?RTK_PROXY_CMD\s*:=/u.test(line));
  return (
    rtkLines.length === 1 &&
    rtkLines[0] === rtkDefinition &&
    proxyLines.length === 1 &&
    proxyLines[0] === proxyDefinition
  );
}
function unitCommand(line: string, allowRtkProxyCommand = false): string[] {
  if (/[;|&`$<>\\]/.test(line)) return [];
  const words = shellWords(line.trim().replace(/^@/, ""));
  if (words[0] === "rtk" && words[1] === "proxy") return words.slice(2);
  if (words[0] === "{{RTK_CMD}}proxy") return words.slice(1);
  const proxyPrefix = "{{RTK_PROXY_CMD}}";
  const prefixedCommand = words[0]?.startsWith(proxyPrefix)
    ? words[0].slice(proxyPrefix.length)
    : undefined;
  if (allowRtkProxyCommand && prefixedCommand === "python3")
    return [prefixedCommand, ...words.slice(1)];
  return words;
}
function canonicalBunTestLine(line: string): string | undefined {
  const words = unitCommand(line);
  if (words[0] !== "bun" || words[1] !== "test") return line;
  const rest = words.slice(2);
  for (let index = 0; index < rest.length; index += 1) {
    const word = rest[index] ?? "";
    if (word === "--path-ignore-patterns") {
      const pattern = rest[index + 1];
      if (!pattern || pattern.startsWith("-")) return undefined;
      index += 1;
    } else if (word.startsWith("-")) return undefined;
    else if (!word.startsWith("./") && !path.isAbsolute(word)) return undefined;
  }
  return line;
}

function playwrightConfigVerified(context: ContractContext): boolean {
  try {
    return (
      strictInputAst(parseSync(context.read(PLAYWRIGHT_CONFIG), { syntax: "typescript" })) ===
        PLAYWRIGHT_CONFIG_AST &&
      strictInputAst(
        parseSync(context.read(PLAYWRIGHT_CONFIG_HELPER), { syntax: "typescript" }),
      ) === PLAYWRIGHT_CONFIG_HELPER_AST
    );
  } catch {
    return false;
  }
}
function playwrightTestSourceVerified(name: string, source: string): boolean {
  if (!hasPlaywrightTests(source)) return false;
  if (name === PLAYWRIGHT_GEOMETRY_TEST && !playwrightGeometryCasesVerified(source)) return false;
  if (name === PLAYWRIGHT_MACOS_PRODUCER_TEST && !playwrightMacosProducerTestVerified(source))
    return false;
  const calls = browserAstNodes(parseSync(source, { syntax: "typescript", tsx: true }))
    .map(({ node }) => node)
    .filter((node) => node.type === "CallExpression");
  const tests = calls.filter((node) => {
    const callee = astObject.parse(node.callee);
    return callee.type === "Identifier" && callee.value === "test";
  });
  return (
    tests.length > 0 &&
    !calls.some((node) => {
      const callee = astObject.parse(node.callee);
      if (callee.type !== "MemberExpression") return false;
      const object = astObject.parse(callee.object);
      const property = astObject.parse(callee.property);
      const receiver = object.type === "MemberExpression" ? astObject.parse(object.object) : object;
      const approvedBoundary =
        object.type !== "MemberExpression" &&
        (property.value === "describe" ||
          property.value === "beforeAll" ||
          property.value === "afterAll");
      return receiver.type === "Identifier" && receiver.value === "test" && !approvedBoundary;
    })
  );
}
function playwrightMacosProducerTestVerified(source: string): boolean {
  return executablePlaywrightTitles(parseSync(source, { syntax: "typescript", tsx: true })).has(
    "official raw producer captures on a freshly prepared viewport",
  );
}
function playwrightGeometryCasesVerified(source: string): boolean {
  const module = parseSync(source, { syntax: "typescript", tsx: true });
  const direct = executablePlaywrightTitles(module);
  return (
    [
      "SVG tspan boundaries preserve within-span kerning with a bundled font",
      "strict raster sizing preserves SVG markup and resolves percentage width from the viewBox",
      "paint projection evaluates actual candidate pixels",
      "keeps the neutral geometry raster exact",
    ].every((title) => direct.has(title)) &&
    geometryTableVerified(module, "paintProjectionScoreCases", "title", [
      "keeps the color-only candidate exact while changing RGB and preserving SVG bytes",
      "rejects a candidate with a missing label",
      "rejects a candidate with a missing edge",
    ]) &&
    geometryTableVerified(module, "paintProjectionIdMapCases", "name", [
      "same mapping",
      "source maps to different targets",
      "targets map from different sources",
    ])
  );
}
function executablePlaywrightTitles(module: unknown): Set<string> {
  return new Set([
    ...playwrightCallTitles(module, "test"),
    ...playwrightCallTitles(module, "describe"),
  ]);
}
function playwrightCallTitles(module: unknown, method: "test" | "describe"): string[] {
  return browserAstNodes(module).flatMap(({ node }) => {
    if (node.type !== "CallExpression") return [];
    const callee = astObject.parse(node.callee);
    if (method === "test" && (callee.type !== "Identifier" || callee.value !== "test")) return [];
    if (method === "describe" && !playwrightDescribeCallee(callee)) return [];
    const first = z.array(astObject).parse(node.arguments ?? [])[0];
    const value = first ? astObject.parse(first.expression) : undefined;
    return value?.type === "StringLiteral" ? [String(value.value)] : [];
  });
}
function playwrightDescribeCallee(callee: Record<string, unknown>): boolean {
  if (callee.type !== "MemberExpression") return false;
  const object = astObject.parse(callee.object),
    property = astObject.parse(callee.property);
  return object.type === "Identifier" && object.value === "test" && property.value === "describe";
}
function geometryTableVerified(
  module: unknown,
  tableName: string,
  fieldName: string,
  expected: readonly string[],
): boolean {
  const table = geometryTableValues(module, tableName, fieldName);
  if (table.length !== expected.length) return false;
  if (!expected.every((value) => table.includes(value))) return false;
  const loops = browserAstNodes(module).filter(({ node }) => node.type === "ForOfStatement");
  return loops.some(({ node }) => {
    const right = astObject.parse(node.right);
    const left = astObject.parse(node.left);
    const binding = z.array(astObject).parse(left.declarations ?? [])[0];
    if (right.type !== "Identifier" || right.value !== tableName || !binding) return false;
    const identifier = astObject.parse(binding.id);
    return identifier.value === "item" && loopUsesTestField(node.body, "item", fieldName);
  });
}
function geometryTableValues(module: unknown, tableName: string, fieldName: string): string[] {
  const declaration = browserAstNodes(module).find(
    ({ node }) =>
      node.type === "VariableDeclarator" && astObject.parse(node.id).value === tableName,
  );
  if (!declaration) return [];
  const init = astObject.parse(declaration.node.init);
  const array = init.type === "TsConstAssertion" ? astObject.parse(init.expression) : init;
  if (array.type !== "ArrayExpression") return [];
  return z
    .array(astObject)
    .parse(array.elements)
    .flatMap((element) => {
      const object = astObject.parse(element.expression);
      if (object.type !== "ObjectExpression") return [];
      const property = z
        .array(astObject)
        .parse(object.properties)
        .find(
          (entry) =>
            entry.type === "KeyValueProperty" && astObject.parse(entry.key).value === fieldName,
        );
      const value = property ? astObject.parse(property.value) : undefined;
      return value?.type === "StringLiteral" ? [String(value.value)] : [];
    });
}
function loopUsesTestField(body: unknown, itemName: string, fieldName: string): boolean {
  return browserAstNodes(body).some(({ node }) => {
    if (node.type !== "CallExpression") return false;
    const callee = astObject.parse(node.callee);
    if (callee.type !== "Identifier" || callee.value !== "test") return false;
    const first = z.array(astObject).parse(node.arguments ?? [])[0];
    if (!first) return false;
    return browserAstNodes(first.expression).some(
      ({ node: nested }) =>
        nested.type === "MemberExpression" &&
        astObject.parse(nested.object).value === itemName &&
        astObject.parse(nested.property).value === fieldName,
    );
  });
}
function playwrightInputsVerified(context: ContractContext): boolean {
  try {
    return (
      [PLAYWRIGHT_CONFIG, PLAYWRIGHT_CONFIG_HELPER, ...PLAYWRIGHT_TESTS].every(
        (name) =>
          context.visible.has(name) &&
          context.tracked.has(name) &&
          fs.existsSync(safeFile(context.root, name)),
      ) &&
      playwrightConfigVerified(context) &&
      PLAYWRIGHT_TESTS.every((name) => playwrightTestSourceVerified(name, context.read(name)))
    );
  } catch {
    return false;
  }
}
function playwrightMacosInputsVerified(context: ContractContext): boolean {
  try {
    return (
      [PLAYWRIGHT_MACOS_CONFIG, PLAYWRIGHT_CONFIG_HELPER, PLAYWRIGHT_MACOS_PRODUCER_TEST].every(
        (name) =>
          context.visible.has(name) &&
          context.tracked.has(name) &&
          fs.existsSync(safeFile(context.root, name)),
      ) &&
      strictInputAst(parseSync(context.read(PLAYWRIGHT_MACOS_CONFIG), { syntax: "typescript" })) ===
        PLAYWRIGHT_MACOS_CONFIG_AST &&
      strictInputAst(
        parseSync(context.read(PLAYWRIGHT_CONFIG_HELPER), { syntax: "typescript" }),
      ) === PLAYWRIGHT_CONFIG_HELPER_AST &&
      playwrightTestSourceVerified(
        PLAYWRIGHT_MACOS_PRODUCER_TEST,
        context.read(PLAYWRIGHT_MACOS_PRODUCER_TEST),
      )
    );
  } catch {
    return false;
  }
}
function playwrightMacosWorkflowRegistered(context: ContractContext): boolean {
  try {
    const workflow = context.read(".github/workflows/test-and-build.yml"),
      step = [
        "      - name: Run macOS official producer browser test",
        "        if: matrix.platform == 'mac-arm64'",
        `        run: just ${PLAYWRIGHT_MACOS_RECIPE}`,
      ].join("\n");
    const producerIndex = workflow.indexOf(step);
    const staticCheckIndexes = [
      "      - name: Check formatting",
      "      - name: Run linter",
      "      - name: Run AST linter",
      "      - name: Check dependency boundary",
    ].map((name) => workflow.indexOf(name));
    return (
      workflow.split(step).length === 2 &&
      staticCheckIndexes.every((index) => index >= 0 && index < producerIndex) &&
      workflow.includes("      - name: Install Bun\n        if: matrix.os != 'windows-latest'") &&
      workflow.includes(
        "      - name: Install JavaScript dependencies\n        if: matrix.os != 'windows-latest'",
      ) &&
      workflow.includes(
        "      - name: Install just\n        uses: taiki-e/install-action@53aaa72a1fed2d5b9189d6d54c38e29bf9be2ab9 # just\n\n      - name: Install Chrome for the macOS official producer browser test",
      ) &&
      workflow.includes(
        "      - name: Install Chrome for the macOS official producer browser test\n        if: matrix.platform == 'mac-arm64'\n        run: bunx playwright install chrome",
      )
    );
  } catch {
    return false;
  }
}
function playwrightMacosRegistered(context: ContractContext, lane: Set<string>): boolean {
  try {
    const recipe = context.recipes.get(PLAYWRIGHT_MACOS_RECIPE)?.trim();
    const runtimeCommands = unconditionalRecipeCommands(runtimeAssetRecipeBody(context));
    const localRegistered =
      lane === context.local &&
      lane.has(RUNTIME_ASSET_OTHER_RECIPE) &&
      runtimeCommands.filter((line) => line === `just ${PLAYWRIGHT_MACOS_RECIPE}`).length === 1;
    const ciRegistered = lane === context.ci && playwrightMacosWorkflowRegistered(context);
    return (
      playwrightMacosInputsVerified(context) &&
      context.read("Justfile").includes(PLAYWRIGHT_MACOS_RUN_DEFINITION) &&
      recipe === "{{MACOS_BROWSER_CONTRACT_RUN}}" &&
      (localRegistered || ciRegistered)
    );
  } catch {
    return false;
  }
}
function playwrightRegistered(context: ContractContext, lane: Set<string>): boolean {
  const lines = unconditionalRecipeCommands(runtimeAssetRecipeBody(context)).filter(
    (line) => line !== SVG_ENGINE_PROVENANCE_TEST_COMMAND,
  );
  const reviewed = normalizeMovedRuntimeAssetCommands(lines);
  return (
    reviewed !== undefined &&
    lane.has(PLAYWRIGHT_RECIPE) &&
    context.recipes.get(PLAYWRIGHT_RECIPE)?.trim() === PLAYWRIGHT_COMMAND &&
    reviewed.at(-1) === `just ${PLAYWRIGHT_RECIPE}` &&
    playwrightInputsVerified(context)
  );
}
function playwrightIssues(context: ContractContext): void {
  if (
    ![PLAYWRIGHT_CONFIG, PLAYWRIGHT_CONFIG_HELPER, ...PLAYWRIGHT_TESTS].some((name) =>
      context.visible.has(name),
    ) &&
    !context.recipes.has(PLAYWRIGHT_RECIPE) &&
    !runtimeAssetRecipeBody(context).includes(`just ${PLAYWRIGHT_RECIPE}`)
  )
    return;
  if (!playwrightInputsVerified(context))
    add(context, "playwright-inputs-unverified", PLAYWRIGHT_CONFIG);
  if (
    context.visible.has(PLAYWRIGHT_MACOS_CONFIG) ||
    context.visible.has(PLAYWRIGHT_MACOS_PRODUCER_TEST)
  ) {
    if (!playwrightMacosInputsVerified(context))
      add(context, "playwright-macos-inputs-unverified", PLAYWRIGHT_MACOS_CONFIG);
    if (
      !playwrightMacosRegistered(context, context.local) ||
      !playwrightMacosRegistered(context, context.ci)
    )
      add(context, "playwright-macos-registration-unverified", PLAYWRIGHT_MACOS_RECIPE);
  }
}

function unitRegistered(
  context: ContractContext,
  name: string,
  language: "bun" | "python" | "playwright",
  lane: Set<string>,
): boolean {
  if (language === "playwright" && name === PLAYWRIGHT_MACOS_PRODUCER_TEST)
    return playwrightMacosRegistered(context, lane);
  if (language === "playwright")
    return PLAYWRIGHT_TESTS.includes(name) && playwrightRegistered(context, lane);
  if (language === "bun" && name === PARSER_BODY_TEST) {
    const binding = context.parserBodyOnloadBinding;
    return Boolean(
      binding?.producerVerified &&
        bindingRegisteredInLane(
          context,
          lane,
          binding.localTestRegistered,
          binding.ciTestRegistered,
        ),
    );
  }
  if (language === "bun" && name === MOON_CACHE_FIXTURE) {
    const binding = context.moonCacheFixtureBinding;
    return Boolean(
      binding?.fixtureVerified &&
        binding.ownerVerified &&
        bindingRegisteredInLane(
          context,
          lane,
          binding.localCheckRegistered,
          binding.ciCheckRegistered,
        ),
    );
  }
  return unitRegisteredInRecipeBodies(context, name, language, lane);
}

function bindingRegisteredInLane(
  context: ContractContext,
  lane: Set<string>,
  local: boolean,
  ci: boolean,
): boolean {
  if (lane === context.local) return local;
  if (lane === context.ci) return ci;
  return false;
}

function unitRegisteredInRecipeBodies(
  context: ContractContext,
  name: string,
  language: "bun" | "python" | "playwright",
  lane: Set<string>,
): boolean {
  const allowRtkProxyCommand = context.rtkProxyCommandVerified;
  return [...lane].some((recipe) => {
    const bodies =
      recipe === MOON_CACHE_RECIPE
        ? [runtimeAssetRecipeBody(context), runtimeUnusedRecipeBody(context)]
        : [context.recipes.get(recipe) ?? ""];
    return bodies.some((body) => {
      const lines = body.split("\n");
      const registeredLines =
        language === "bun" ? (removeClassicGenerationEvidenceCommands(lines) ?? lines) : lines;
      return registeredLines.some((line) =>
        language === "bun"
          ? bunSelection(context, unitCommand(line, allowRtkProxyCommand), name)
          : pythonSelection(context, unitCommand(line, allowRtkProxyCommand), name) ||
            pythonPerformanceTestSelection(context, unitCommand(line, allowRtkProxyCommand), name),
      );
    });
  });
}

function parserBodyOnloadPaths(context: ContractContext): { inputs: string[]; paths: string[] } {
  const inputs = [
    PARSER_BODY_CANONICAL,
    PARSER_BODY_HOST,
    PARSER_BODY_PRODUCER,
    PARSER_BODY_TEST,
    PARSER_BODY_OUTPUT,
  ];
  const authorities = [
    "package.json",
    "bun.lock",
    "biome.json",
    ...PARSER_BODY_TYPE_PROFILES,
    "Justfile",
    "scripts/html/harness-contract-policy.json",
    context.policy.ciWorkflow,
  ];
  return { inputs, paths: [...inputs, ...authorities] };
}
function emptyParserBodyOnloadBinding(inputs: string[]): OwnedValidationReport["parserBodyOnload"] {
  return {
    path: PARSER_BODY_OUTPUT,
    canonical: PARSER_BODY_CANONICAL,
    hostDeclaration: PARSER_BODY_HOST,
    producer: PARSER_BODY_PRODUCER,
    unitTest: PARSER_BODY_TEST,
    inputPaths: inputs,
    inputDigest: sha(""),
    outputSha256: sha(""),
    producerVerified: false,
    biomeIncluded: false,
    canonicalTypeScope: false,
    producerTypeScope: false,
    outputTypeBoundary: "generated-from-strict-typescript",
    localCheckRegistered: false,
    ciCheckRegistered: false,
    localTestRegistered: false,
    ciTestRegistered: false,
    byteVerification: "not-executed",
  };
}
const MATHJAX_EVIDENCE_CONSUMER_AST_SHA256 =
  "8b47146db5270840e71f73dd276d099750e181928bb0834e58ffd9bc0e99ebb1";
function mathJaxEvidenceConsumerVerified(source: string): boolean {
  if (swcVersion !== "1.16.20") return false;
  try {
    const ast = parseSync(source, { syntax: "typescript", tsx: false });
    return (
      sha(
        JSON.stringify(ast, (key: string, value: unknown) =>
          ["span", "ctxt", "bodyCtxt", "raw"].includes(key) ? undefined : value,
        ),
      ) === MATHJAX_EVIDENCE_CONSUMER_AST_SHA256
    );
  } catch {
    return false;
  }
}

function hostEvidenceInputRegistered(context: ContractContext, name: string): boolean {
  if (!context.visible.has(name) || !context.tracked.has(name)) return false;
  try {
    const source = fs.readFileSync(safeFile(context.root, name), "utf8");
    return name.endsWith("_test.ts")
      ? unitRegistered(context, name, "bun", context.local) &&
          unitRegistered(context, name, "bun", context.ci)
      : hostEvidenceSourceVerified(name, source);
  } catch {
    return false;
  }
}
function hostEvidenceRecipesRegistered(context: ContractContext): boolean {
  const commands = unconditionalRecipeCommands(runtimeAssetRecipeBody(context));
  const preflight = unconditionalRecipeCommands(runtimePreflightRecipeBody(context));
  const result =
    context.recipes.get("runtime-asset-script-test")?.trim() ===
      "bun run moon exec krr:runtime-asset-script-test" &&
    context.recipes.get(CLASSIC_GENERATION_EVIDENCE_RECIPE)?.trim() ===
      CLASSIC_GENERATION_EVIDENCE_PRODUCER &&
    context.recipes.get(RUNTIME_UNUSED_REGRESSION)?.trim() ===
      "bun run moon exec krr:runtime-unused-script-regression" &&
    runtimeUnusedProducerRecipeBody(context).trim() ===
      "python3 scripts/release/runtime_script_cache_cli.py run --repo {{ quote(REPO_ROOT) }} --producer {{ quote(producer) }}" &&
    runtimeUnusedVerifyRecipeBody(context).trim() ===
      "python3 scripts/release/runtime_script_cache_cli.py ensure --repo {{ quote(REPO_ROOT) }} --producer {{ quote(producer) }}" &&
    context.recipes.get(RUNTIME_UNUSED_CACHE_KEY_RECIPE)?.trim() ===
      "python3 scripts/release/runtime_script_cache_cli.py key --repo {{ quote(REPO_ROOT) }} --producer {{ quote(producer) }}" &&
    context.recipes.get(RUNTIME_UNUSED_FINAL_VERIFY_RECIPE)?.trim() ===
      RUNTIME_UNUSED_FINAL_VERIFY_COMMAND &&
    runtimeAggregateRecipeBody(context).trim() === MATHJAX_UNUSED_STANDARD_GATE_COMMAND &&
    preflight.length === HOST_EVIDENCE_PRECONDITIONS.length &&
    HOST_EVIDENCE_PRECONDITIONS.every((command, index) => preflight[index] === command) &&
    registered(context, RUNTIME_ASSET_PREFLIGHT_RECIPE) &&
    unusedRegressionRecipeVerified(context) &&
    runtimeUnusedLeafRecipesVerified(context) &&
    (commands[0] === HOST_EVIDENCE_TEST_COMMAND ||
      normalizeMovedRuntimeAssetCommands(
        commands.filter((line) => line !== SVG_ENGINE_PROVENANCE_TEST_COMMAND),
      )?.[0] === HOST_EVIDENCE_TEST_COMMAND) &&
    reviewedSplitRuntimeAssetTestCommands(context) !== undefined &&
    [
      [
        "rust-facts-host-prebuild",
        'bun scripts/runtime-bundles/runtime-rust-facts-host.ts prepare {{quote(REPO_ROOT)}} {{quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json")}}',
      ],
      [
        "mathjax-owned-evidence",
        "NODE_ENV=test bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts",
      ],
    ].every(
      ([name, expected]) =>
        name !== undefined &&
        context.recipes.get(name)?.trim() === expected &&
        registered(
          context,
          name === "mathjax-owned-evidence" ? RUNTIME_ASSET_PREFLIGHT_RECIPE : name,
        ),
    );
  return result;
}
function unusedRegressionRecipeVerified(context: ContractContext): boolean {
  const commands = unconditionalRecipeCommands(runtimeUnusedRecipeBody(context));
  const expectedTest = `bun test --path-ignore-patterns 'tmp/**' ${RUNTIME_UNUSED_REGRESSION_TESTS.map((name) => `./${name}`).join(" ")}`;
  return commands.length === 1 && commands[0] === expectedTest;
}
function runtimeUnusedLeafRecipesVerified(context: ContractContext): boolean {
  return RUNTIME_UNUSED_LEAF_IDS.every((id, index) => {
    const recipe = context.recipes.get(`${id}-run`)?.trim();
    const test = RUNTIME_UNUSED_REGRESSION_TESTS[index];
    return recipe === `bun test --path-ignore-patterns 'tmp/**' ./${test}`;
  });
}
function hostEvidenceRegistrationVerified(context: ContractContext): boolean {
  const recipesVerified = hostEvidenceRecipesRegistered(context);
  const consumerVerified = mathJaxEvidenceConsumerVerified(context.read(MATHJAX_UNUSED_AUDIT_TEST));
  let verified =
    recipesVerified &&
    HOST_EVIDENCE_INPUTS.every((name) => hostEvidenceInputRegistered(context, name));
  try {
    verified = verified && consumerVerified;
  } catch {
    verified = false;
  }
  if (!verified) add(context, "mathjax-host-evidence-registration-unverified", "Justfile");
  return verified;
}

function emptyMathJaxUnusedAuditBinding(): OwnedValidationReport["mathJaxUnusedAudit"] {
  // WHY: fixtureと実repoの未検証状態を同じfail-closed値で表し、配列だけは呼出しごとに分離する。
  return {
    sourcePath: MATHJAX_UNUSED_AUDIT_SOURCE,
    testPath: MATHJAX_UNUSED_AUDIT_TEST,
    auditRecipe: MATHJAX_UNUSED_AUDIT_RECIPE,
    auditCommand: MATHJAX_UNUSED_AUDIT_COMMAND,
    standardCommand: MATHJAX_UNUSED_STANDARD_GATE_COMMAND,
    factRecipe: MATHJAX_UNUSED_FACT_RECIPE,
    hostEvidenceExecutionVerified: false,
    inputPaths: [...MATHJAX_UNUSED_INPUTS],
    inputDigest: sha(""),
    sourceSha256: sha(""),
    localCheckRegistered: false,
    ciCheckRegistered: false,
    commandVerified: false,
    inputSelectionVerified: false,
    standardCommandVerified: false,
    standardLocalCheckRegistered: false,
    standardCiCheckRegistered: false,
  };
}

function mathJaxUnusedAuditBinding(
  context: ContractContext,
): OwnedValidationReport["mathJaxUnusedAudit"] {
  // Small contract fixtures that predate this route do not model the production
  // MathJax input graph.  Keep those fixtures focused on their declared contract;
  // the real repository path below performs the full input and registration checks.
  if (!context.visible.has(MATHJAX_UNUSED_AUDIT_SOURCE)) {
    return emptyMathJaxUnusedAuditBinding();
  }
  const inputPaths = [...MATHJAX_UNUSED_INPUTS];
  const inputRecords: string[] = [];
  for (const name of inputPaths) {
    if (!context.visible.has(name)) {
      add(context, "mathjax-unused-input-missing", name);
      inputRecords.push(`${name}\0missing\n`);
      continue;
    }
    if (!context.tracked.has(name)) {
      add(context, "untracked-authority", name);
    }
    try {
      inputRecords.push(`${name}\0${sha(fs.readFileSync(safeFile(context.root, name)))}\n`);
    } catch (error) {
      add(context, "mathjax-unused-input-unsafe", name, String(error));
      inputRecords.push(`${name}\0unsafe\n`);
    }
  }
  const body = runtimeAssetRecipeBody(context);
  const inputSelectionVerified = auditedStrictInputSelection(context);
  const hostEvidenceRegistered = hostEvidenceRegistrationVerified(context);
  // WHY: audit-only実行は必ずexit 1なので、成功laneは明示gateだけを受理し、audit文字列は台帳に残す。
  const standardCommandVerified =
    strictStandardGateCommandVerified(
      runtimeAggregateRecipeBody(context),
      inputSelectionVerified,
    ) && hostEvidenceRegistered;
  const standardLocalCheckRegistered =
    standardCommandVerified &&
    inputSelectionVerified &&
    context.localRecipes.has("runtime-asset-script-test");
  const standardCiCheckRegistered =
    standardCommandVerified &&
    inputSelectionVerified &&
    context.ciRecipes.has("runtime-asset-script-test");
  const commandVerified = body
    .split("\n")
    .some((line) => line.trim() === MATHJAX_UNUSED_AUDIT_COMMAND);
  const localCheckRegistered =
    commandVerified &&
    inputSelectionVerified &&
    context.localRecipes.has(MATHJAX_UNUSED_AUDIT_RECIPE);
  const ciCheckRegistered =
    commandVerified && inputSelectionVerified && context.ciRecipes.has(MATHJAX_UNUSED_AUDIT_RECIPE);
  if (!standardCommandVerified)
    add(
      context,
      "mathjax-unused-standard-command-unverified",
      "Justfile",
      MATHJAX_UNUSED_STANDARD_GATE_COMMAND,
    );
  let sourceSha256 = sha("");
  try {
    sourceSha256 = sha(fs.readFileSync(safeFile(context.root, MATHJAX_UNUSED_AUDIT_SOURCE)));
  } catch {
    // Fixture roots may intentionally omit the production audit source.
    // The per-input checks above retain the missing-source evidence.
  }
  return {
    sourcePath: MATHJAX_UNUSED_AUDIT_SOURCE,
    testPath: MATHJAX_UNUSED_AUDIT_TEST,
    auditRecipe: MATHJAX_UNUSED_AUDIT_RECIPE,
    auditCommand: MATHJAX_UNUSED_AUDIT_COMMAND,
    standardCommand: MATHJAX_UNUSED_STANDARD_GATE_COMMAND,
    factRecipe: MATHJAX_UNUSED_FACT_RECIPE,
    hostEvidenceExecutionVerified: false,
    inputPaths,
    inputDigest: sha(inputRecords.join("")),
    sourceSha256,
    localCheckRegistered,
    ciCheckRegistered,
    commandVerified,
    inputSelectionVerified,
    standardCommandVerified,
    standardLocalCheckRegistered,
    standardCiCheckRegistered,
  };
}

function strictInputAst(node: unknown): string {
  return JSON.stringify(node, (key, value: unknown) =>
    ["span", "ctxt", "bodyCtxt", "raw"].includes(key)
      ? undefined
      : key === "spread" && value !== null
        ? true
        : value,
  );
}

function strictStandardGateCommandVerified(body: string, inputSelectionVerified: boolean): boolean {
  return (
    inputSelectionVerified &&
    body.split("\n").some((line) => line.trim() === MATHJAX_UNUSED_STANDARD_GATE_COMMAND)
  );
}

function strictInputExpression(code: string) {
  const statement = parseSync(`(${code});`, { syntax: "typescript" }).body[0];
  if (
    statement?.type !== "ExpressionStatement" ||
    statement.expression.type !== "ParenthesisExpression"
  )
    throw new Error("unknown strict input expression");
  return statement.expression.expression;
}

const STRICT_INPUT_CALLS = [
  [
    "scripts/mermaid/diagram-update.ts",
    "resolveOfficialRendererAssetInputs(argv, { ...process.env }, process.cwd(), os.homedir())",
  ],
  [
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "resolveOfficialRendererAssetInputs(inputs.argv, inputs.env, inputs.cwd, inputs.home)",
  ],
  [
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "officialStrictUnusedContext(root, raw, inputs)",
  ],
  [MATHJAX_UNUSED_AUDIT_SOURCE, "runtimeUnusedContexts(root, inputs)"],
  [
    MATHJAX_UNUSED_AUDIT_SOURCE,
    "auditRuntimeUnused(root, directory, actualOfficialStrictInputs())",
  ],
] as const;

function strictInputCallVerified(root: string, sourcePath: string, expression: string): boolean {
  const expected = astObject.parse(strictInputExpression(expression));
  const callee = strictInputAst(expected.callee);
  const module = parseSync(read(root, sourcePath), { syntax: "typescript" });
  const nodes = browserAstNodes(module);
  const actual = nodes.filter(
    ({ node }) =>
      node.type === "CallExpression" &&
      strictInputAst(node.callee) === callee &&
      strictInputAst(node) === strictInputAst(expected),
  );
  const call = actual[0]?.node;
  if (actual.length !== 1 || !call) return false;
  // 引数の形を先に一致させても、同名のlocal bindingは正規importにはならない。
  if (
    astObject.parse(expected.callee).value === "resolveOfficialRendererAssetInputs" &&
    !strictResolverCallVerified(astObject.parse(module), call, sourcePath)
  )
    return false;
  return true;
}

const STRICT_CONTEXT_SELECTION_PREFIX = `
for (const binding of officialStrictConsumerRoute) {
  const source = readUnusedPhysicalSource(root, binding.sourcePath),
    previous = raw.get(binding.sourcePath);
  if (previous && previous.sha256 !== source.sha256)
    throw new Error("changed official strict owned input");
  raw.set(binding.sourcePath, source);
}
const programs = inspectOfficialStrictPrograms(raw);
`;

function strictResolverCallVerified(
  module: Record<string, unknown>,
  call: Record<string, unknown>,
  sourcePath: string,
): boolean {
  const importPath =
    sourcePath === "scripts/mermaid/diagram-update.ts"
      ? "./official-renderer-asset-inputs"
      : "../mermaid/official-renderer-asset-inputs";
  const imports = browserAstNodes(module).filter(
    ({ node }) =>
      node.type === "ImportDeclaration" &&
      astObject.parse(node.source).value === importPath &&
      node.typeOnly === false,
  );
  const specifiers = imports.flatMap(({ node }) => z.array(astObject).parse(node.specifiers));
  const imported = specifiers[0]?.local;
  if (specifiers.length !== 1 || !imported) return false;
  const binding = pageIdentifier.parse(imported),
    receiver = pageIdentifier.parse(call.callee);
  if (binding.value !== receiver.value || binding.ctxt !== receiver.ctxt) return false;
  const uses = browserAstNodes(module).filter(
    ({ node }) => node.type === "Identifier" && node.value === binding.value,
  );
  if (
    uses.length !== 2 ||
    !uses.every(
      ({ node }) =>
        strictIdentifierOccurrence(node, imported) || strictIdentifierOccurrence(node, call.callee),
    )
  )
    return false;
  return (
    strictResolverDirectPrefixVerified(module, call, sourcePath) &&
    (sourcePath !== "scripts/mermaid/diagram-update.ts" ||
      strictCliInputReceiversVerified(module, call))
  );
}

const STRICT_CLI_ENTRY = `if (import.meta.main)
  new MermaidDiagramUpdate(CliOptions.parse(process.argv.slice(2))).run().catch((error: Error) => {
    console.error(error.message);
    process.exit(1);
  });`;

function strictInputOsImport(nodes: ReturnType<typeof browserAstNodes>): unknown {
  const imports = nodes.filter(
    ({ node }) =>
      node.type === "ImportDeclaration" &&
      astObject.parse(node.source).value === "node:os" &&
      node.typeOnly === false,
  );
  const specifiers = imports.flatMap(({ node }) => z.array(astObject).parse(node.specifiers));
  const imported = specifiers[0]?.local;
  if (specifiers.length !== 1 || specifiers[0]?.type !== "ImportDefaultSpecifier" || !imported)
    return undefined;
  return imported;
}

function strictCliInputReceiversVerified(
  module: Record<string, unknown>,
  call: Record<string, unknown>,
): boolean {
  const nodes = browserAstNodes(module);
  const imported = strictInputOsImport(nodes);
  if (!imported) return false;
  const osBinding = pageIdentifier.parse(imported);
  const receivers = browserAstNodes(call).filter(
    ({ node }) => node.type === "Identifier" && ["os", "process"].includes(String(node.value)),
  );
  if (
    receivers.length !== 3 ||
    !receivers.every(({ node }) => {
      const receiver = pageIdentifier.parse(node);
      return receiver.value === "process"
        ? receiver.ctxt === 1
        : receiver.ctxt === osBinding.ctxt && receiver.value === osBinding.value;
    })
  )
    return false;
  const parser = nodes.filter(
    ({ node }) =>
      node.type === "VariableDeclarator" && astObject.parse(node.id).value === "CliOptions",
  );
  if (parser.length !== 1 || !parser[0]) return false;
  const methods = browserAstNodes(parser[0].node.init);
  const owners = methods.filter(
    ({ node, method }) => node.type === "MethodProperty" && method === "parse",
  );
  if (
    owners.length !== 1 ||
    !owners[0] ||
    !strictCliArgvBindingVerified(owners[0].node, call) ||
    !strictCliResultOwnershipVerified(module, owners[0].node)
  )
    return false;
  const helpProcess = strictCliHelpReceiver(methods);
  const entry = strictCliEntryVerified(module, parser[0].node);
  if (!helpProcess || !entry) return false;
  return strictInputGlobalUsesVerified(nodes, [
    imported,
    ...receivers.map(({ node }) => node),
    helpProcess,
    ...browserAstNodes(entry)
      .filter(({ node }) => node.type === "Identifier" && node.value === "process")
      .map(({ node }) => node),
  ]);
}

const STRICT_CLI_RESULT_PREDECESSORS = `
const fixturesDir = path.resolve(CliOptions.get(argv, "--fixtures", "tests/fixtures/mermaid/en"));
const outputDir = path.resolve(CliOptions.get(argv, "--output", path.join(fixturesDir, "official")));
const mode = referenceMode(CliOptions.get(argv, "--reference-mode", "strictVendorOriginal"));
const referenceScope = CliOptions.get(argv, "--reference-scope", "formal");
if (referenceScope !== "formal" && referenceScope !== "diagnostic") {
  throw new Error(\`Unsupported reference scope: \${referenceScope}\`);
}
assertReferenceScope(mode, referenceScope);
`;

const STRICT_CLI_RESULT_KEYS = [
  "fixturesDir",
  "outputDir",
  "markdownOutputDir",
  "...assetInputs",
  "writeMarkdown",
  "skipErrors",
  "referenceMode",
  "referenceScope",
  "theme",
  "rasterSessionConfigPath",
  "rasterSessionConfigSha256",
];

function strictCliResultOwnershipVerified(
  module: Record<string, unknown>,
  parser: Record<string, unknown>,
): boolean {
  const statements = z.array(astObject).parse(astObject.parse(parser.body).stmts),
    declarations = z.array(astObject).parse(statements[1]?.declarations),
    selected = declarations[0],
    returned = statements.at(-1);
  const returns = browserAstNodes(parser).filter(({ node }) => node.type === "ReturnStatement");
  if (
    !selected ||
    returned?.type !== "ReturnStatement" ||
    returns.length !== 1 ||
    statements.length !== 9 ||
    strictInputAst(statements.slice(2, -1)) !==
      strictInputAst(parseSync(STRICT_CLI_RESULT_PREDECESSORS, { syntax: "typescript" }).body)
  )
    return false;
  const result = astObject.parse(returned.argument);
  if (result.type !== "ObjectExpression") return false;
  const properties = z.array(astObject).parse(result.properties),
    spread = properties[3];
  if (
    properties.length !== STRICT_CLI_RESULT_KEYS.length ||
    !properties.every(
      (property, index) => strictCliResultKey(property) === STRICT_CLI_RESULT_KEYS[index],
    ) ||
    !spread ||
    !strictIdentifierBinding(selected.id, spread.arguments)
  )
    return false;
  // 選択結果の別名への流出・代入は、正規宣言と返却spread以外の参照として拒否する。
  const uses = browserAstNodes(module).filter(
    ({ node }) => node.type === "Identifier" && node.value === "assetInputs",
  );
  return (
    uses.length === 2 &&
    uses.every(
      ({ node }) =>
        strictIdentifierOccurrence(node, selected.id) ||
        strictIdentifierOccurrence(node, spread.arguments),
    ) &&
    strictCliRendererOwnershipVerified(module)
  );
}

function strictCliResultKey(property: Record<string, unknown>): string | undefined {
  if (property.type === "Identifier") return String(property.value);
  if (property.type === "SpreadElement") return "...assetInputs";
  if (property.type !== "KeyValueProperty") return undefined;
  const key = astObject.parse(property.key);
  return key.type === "Identifier" ? String(key.value) : undefined;
}

function strictIdentifierBinding(declaration: unknown, reference: unknown): boolean {
  const binding = pageIdentifier.safeParse(declaration),
    use = pageIdentifier.safeParse(reference);
  return (
    binding.success &&
    use.success &&
    binding.data.value === use.data.value &&
    binding.data.ctxt === use.data.ctxt
  );
}

const STRICT_CLI_CONSTRUCTOR = `class MermaidDiagramUpdate {
  constructor(options: CliParsedOptions) {
    this.options = options;
    this.markdownAssets = new MarkdownReferenceAssets(options.outputDir, options.markdownOutputDir);
    this.skippedReport = new SkippedFixtureReport(options.outputDir);
  }
}`;
const STRICT_CLI_RUN = `class MermaidDiagramUpdate {
  async run() {
    this.validate();
    this.prepareOutputDirs();
    const fixtures = new FixtureRepository(this.options.fixturesDir).list();
    if (this.options.referenceMode === "strictVendorOriginal" && this.options.rasterSession === undefined) {
      if (!this.options.rasterSessionConfigPath || !this.options.rasterSessionConfigSha256)
        throw new Error("Strict official raster producer requires external session config path and SHA");
      this.options.rasterSession = RasterProductionSession.fromPinnedConfig(
        this.options.rasterSessionConfigPath, this.options.rasterSessionConfigSha256,
      );
    }
    const renderer = new OfficialMermaidRenderer(this.options);
    const writer = new MarkdownReferenceWriter(this.options.markdownOutputDir);
    await this.renderAll(renderer, writer, fixtures);
    this.skippedReport.write();
    this.assertRendered(fixtures);
  }
}`;

function strictCliRendererOwnershipVerified(module: Record<string, unknown>): boolean {
  const nodes = browserAstNodes(module),
    classes = nodes.filter(
      ({ node }) =>
        node.type === "ClassDeclaration" &&
        astObject.parse(node.identifier).value === "MermaidDiagramUpdate",
    );
  const klass = classes[0]?.node;
  if (
    classes.length !== 1 ||
    !klass ||
    z.array(astObject).parse(klass.decorators).length !== 0 ||
    klass.superClass !== null
  )
    return false;
  const members = z.array(astObject).parse(klass.body),
    ctors = members.filter((node) => node.type === "Constructor"),
    runs = members.filter(
      (node) => node.type === "ClassMethod" && astObject.parse(node.key).value === "run",
    ),
    options = members.filter((node) => astObject.safeParse(node.key).data?.value === "options");
  const ctor = ctors[0],
    run = runs[0];
  if (
    ctors.length !== 1 ||
    runs.length !== 1 ||
    !ctor ||
    !run ||
    options.length !== 1 ||
    options[0]?.type !== "ClassProperty" ||
    !strictCliMemberShape(
      options[0],
      "class MermaidDiagramUpdate { private options: CliParsedOptions; }",
      "ClassProperty",
    ) ||
    !strictCliMemberShape(ctor, STRICT_CLI_CONSTRUCTOR, "Constructor") ||
    !strictCliMemberShape(run, STRICT_CLI_RUN, "ClassMethod")
  )
    return false;
  const entry = z.array(astObject).parse(module.body).at(-1),
    entryClass = browserAstNodes(entry).find(
      ({ node }) => node.type === "Identifier" && node.value === "MermaidDiagramUpdate",
    )?.node,
    classUses = nodes.filter(
      ({ node }) => node.type === "Identifier" && node.value === "MermaidDiagramUpdate",
    );
  return (
    strictIdentifierBinding(klass.identifier, entryClass) &&
    classUses.length === 2 &&
    classUses.every(
      ({ node }) =>
        strictIdentifierOccurrence(node, klass.identifier) ||
        strictIdentifierOccurrence(node, entryClass),
    ) &&
    strictCliConstructorBindingVerified(ctor) &&
    strictCliRendererBindingVerified(nodes, run) &&
    strictCliOptionsUsesVerified(klass, ctor, run)
  );
}

function strictCliMemberShape(
  node: Record<string, unknown>,
  source: string,
  type: string,
): boolean {
  const expected = browserAstNodes(parseSync(source, { syntax: "typescript" })).find(
    ({ node: candidate }) => candidate.type === type,
  )?.node;
  return strictInputAst(node) === strictInputAst(expected);
}

function strictCliConstructorBindingVerified(ctor: Record<string, unknown>): boolean {
  const params = z.array(astObject).parse(ctor.params),
    parameter = params[0]?.pat,
    uses = browserAstNodes(ctor).filter(
      ({ node }) =>
        node.type === "Identifier" && node.value === "options" && node.ctxt !== undefined,
    );
  return (
    params.length === 1 &&
    uses.length === 5 &&
    uses.every(({ node }) => strictIdentifierBinding(parameter, node))
  );
}

function strictCliRendererBindingVerified(
  nodes: ReturnType<typeof browserAstNodes>,
  run: Record<string, unknown>,
): boolean {
  const imports = nodes.filter(
      ({ node }) =>
        node.type === "ImportDeclaration" &&
        astObject.parse(node.source).value === "./official-renderer" &&
        node.typeOnly === false,
    ),
    bindings = imports
      .flatMap(({ node }) => z.array(astObject).parse(node.specifiers))
      .filter(
        (specifier) =>
          astObject.parse(specifier.local).value === "OfficialMermaidRenderer" &&
          specifier.type === "ImportSpecifier" &&
          specifier.isTypeOnly === false &&
          (specifier.imported === null ||
            astObject.parse(specifier.imported).value === "OfficialMermaidRenderer"),
      ),
    calls = browserAstNodes(run).filter(
      ({ node }) =>
        node.type === "NewExpression" &&
        astObject.parse(node.callee).value === "OfficialMermaidRenderer",
    );
  const imported = bindings[0]?.local,
    call = calls[0]?.node;
  if (
    bindings.length !== 1 ||
    calls.length !== 1 ||
    !imported ||
    !call ||
    !strictIdentifierBinding(imported, call.callee)
  )
    return false;
  const types = nodes
      .filter(
        ({ node }) =>
          node.type === "TsTypeReference" &&
          astObject.safeParse(node.typeName).data?.value === "OfficialMermaidRenderer",
      )
      .map(({ node }) => node.typeName),
    allowed = [imported, call.callee, ...types],
    uses = nodes.filter(
      ({ node }) => node.type === "Identifier" && node.value === "OfficialMermaidRenderer",
    );
  return (
    uses.length === allowed.length &&
    uses.every(({ node }) =>
      allowed.some((reference) => strictIdentifierOccurrence(node, reference)),
    )
  );
}

const STRICT_CLI_THIS_MEMBERS = new Set([
  "options",
  "markdownAssets",
  "skippedReport",
  "renderedCount",
  "validate",
  "prepareOutputDirs",
  "renderAll",
  "assertRendered",
  "renderFixtures",
  "renderFixture",
  "renderOptionalFixture",
  "renderRequiredFixture",
  "syncMarkdownAssets",
  "writeMarkdownReference",
]);
const STRICT_CLI_OPTION_FIELDS = new Set([
  "fixturesDir",
  "outputDir",
  "markdownOutputDir",
  "mermaidJs",
  "mermaidZenumlJs",
  "writeMarkdown",
  "skipErrors",
  "referenceMode",
  "referenceScope",
  "theme",
  "rasterSessionConfigPath",
  "rasterSessionConfigSha256",
  "rasterSession",
]);
// 描画前に実行する2methodは、未知のreceiverへの流出と未検証の副作用を許可しない。
const STRICT_CLI_PRE_RENDER_MEMBERS = [
  [
    "prepareOutputDirs",
    `class MermaidDiagramUpdate {
  private prepareOutputDirs() {
    if (
      this.options.referenceMode === "strictVendorOriginal" &&
      fs.existsSync(this.options.outputDir) &&
      fs.readdirSync(this.options.outputDir).length > 0
    ) {
      throw new Error("Strict official raster production requires a fresh output directory");
    }
    fs.mkdirSync(this.options.outputDir, { recursive: true });
    if (this.options.writeMarkdown) {
      this.markdownAssets.prepare();
    }
  }
}`,
  ],
  [
    "validate",
    `class MermaidDiagramUpdate {
  private validate() {
    assertReferenceScope(this.options.referenceMode, this.options.referenceScope);
    if (
      this.options.referenceMode === "strictVendorOriginal" &&
      this.options.referenceScope === "formal" &&
      this.options.skipErrors
    ) {
      throw new Error("strictVendorOriginal formal reference generation cannot use --skip-errors");
    }
    if (!fs.existsSync(this.options.mermaidJs)) {
      throw new Error(\`mermaid.min.js not found: \${this.options.mermaidJs}\`);
    }
    if (!fs.existsSync(this.options.mermaidZenumlJs)) {
      throw new Error(\`mermaid-zenuml.min.js not found: \${this.options.mermaidZenumlJs}\`);
    }
  }
}`,
  ],
] as const;

function strictCliPreRenderMembersVerified(klass: Record<string, unknown>): boolean {
  const members = z.array(astObject).parse(klass.body);
  return STRICT_CLI_PRE_RENDER_MEMBERS.every(([name, source]) => {
    const actual = members.filter(
      (node) => node.type === "ClassMethod" && astObject.parse(node.key).value === name,
    );
    return (
      actual.length === 1 && !!actual[0] && strictCliMemberShape(actual[0], source, "ClassMethod")
    );
  });
}

function strictCliOptionsUsesVerified(
  klass: Record<string, unknown>,
  ctor: Record<string, unknown>,
  run: Record<string, unknown>,
): boolean {
  if (!strictCliPreRenderMembersVerified(klass)) return false;
  const nodes = browserAstNodes(klass),
    members = nodes.filter(({ node }) => node.type === "MemberExpression"),
    receivers = members.filter(
      ({ node }) => astObject.parse(node.object).type === "ThisExpression",
    ),
    thisUses = nodes.filter(({ node }) => node.type === "ThisExpression");
  if (
    !thisUses.every(({ node }) =>
      receivers.some(
        ({ node: member }) =>
          strictNodeOccurrence(node, member.object) &&
          astObject.parse(member.property).type === "Identifier" &&
          STRICT_CLI_THIS_MEMBERS.has(String(astObject.parse(member.property).value)),
      ),
    )
  )
    return false;
  const options = receivers.filter(
      ({ node }) => astObject.parse(node.property).value === "options",
    ),
    fields = members.filter(
      ({ node }) =>
        options.some(({ node: option }) => strictNodeOccurrence(option, node.object)) &&
        astObject.parse(node.property).type === "Identifier" &&
        STRICT_CLI_OPTION_FIELDS.has(String(astObject.parse(node.property).value)),
    ),
    ctorWrite = browserAstNodes(ctor).find(
      ({ node }) => node.type === "AssignmentExpression",
    )?.node,
    renderer = browserAstNodes(run).find(
      ({ node }) =>
        node.type === "NewExpression" &&
        astObject.parse(node.callee).value === "OfficialMermaidRenderer",
    )?.node,
    argument = z.array(astObject).parse(renderer?.arguments)[0]?.expression;
  if (
    !options.every(
      ({ node }) =>
        strictNodeOccurrence(node, ctorWrite?.left) ||
        strictNodeOccurrence(node, argument) ||
        fields.some(({ node: field }) => strictNodeOccurrence(node, field.object)),
    )
  )
    return false;
  const allowed = browserAstNodes([ctor, run]).filter(
    ({ node }) =>
      node.type === "AssignmentExpression" &&
      browserAstNodes(node.left).some(({ node: candidate }) =>
        options.some(({ node: option }) => strictNodeOccurrence(option, candidate)),
      ),
  );
  const writes = nodes.filter(
    ({ node }) =>
      node.type === "AssignmentExpression" ||
      node.type === "UpdateExpression" ||
      node.type === "ForOfStatement" ||
      node.type === "ForInStatement" ||
      (node.type === "UnaryExpression" && node.operator === "delete"),
  );
  return writes.every(
    ({ node }) =>
      !browserAstNodes(node.left ?? node.argument).some(({ node: candidate }) =>
        options.some(({ node: option }) => strictNodeOccurrence(option, candidate)),
      ) || allowed.some(({ node: write }) => strictNodeOccurrence(write, node)),
  );
}

function strictNodeOccurrence(node: unknown, expected: unknown): boolean {
  const actual = astObject.safeParse(node),
    reference = astObject.safeParse(expected);
  if (!actual.success || !reference.success || actual.data.type !== reference.data.type)
    return false;
  const span = astSpan.safeParse(actual.data.span),
    at = astSpan.safeParse(reference.data.span);
  return (
    span.success && at.success && span.data.start === at.data.start && span.data.end === at.data.end
  );
}

function strictCliArgvBindingVerified(
  owner: Record<string, unknown>,
  call: Record<string, unknown>,
): boolean {
  const params = z.array(astObject).parse(owner.params),
    args = z.array(astObject).parse(call.arguments);
  if (params.length !== 1 || !params[0] || !args[0]) return false;
  const parameter = pageIdentifier.parse(params[0].pat),
    argument = pageIdentifier.parse(args[0].expression);
  return (
    parameter.value === "argv" &&
    argument.value === parameter.value &&
    argument.ctxt === parameter.ctxt
  );
}

function strictCliHelpReceiver(methods: ReturnType<typeof browserAstNodes>): unknown {
  const owners = methods.filter(
    ({ node, method }) => node.type === "MethodProperty" && method === "exitIfHelp",
  );
  if (owners.length !== 1 || !owners[0]) return undefined;
  const owner = owners[0].node,
    statements = z.array(astObject).parse(astObject.parse(owner.body).stmts),
    test = statements[0];
  if (
    statements.length !== 1 ||
    test?.type !== "IfStatement" ||
    test.alternate !== null ||
    strictInputAst(test.test) !== strictInputAst(strictInputExpression('argv.includes("--help")'))
  )
    return undefined;
  const branch = z.array(astObject).parse(astObject.parse(test.consequent).stmts);
  const log = astObject.parse(branch[0]?.expression),
    exit = astObject.parse(branch[1]?.expression);
  if (
    branch.length !== 2 ||
    branch.some((statement) => statement.type !== "ExpressionStatement") ||
    strictInputAst(log.callee) !==
      strictInputAst(astObject.parse(strictInputExpression('console.log("help")')).callee) ||
    strictInputAst(exit) !== strictInputAst(strictInputExpression("process.exit(0)"))
  )
    return undefined;
  const params = z.array(astObject).parse(owner.params),
    references = browserAstNodes(test.test).filter(
      ({ node }) => node.type === "Identifier" && node.value === "argv",
    );
  if (params.length !== 1 || !params[0] || references.length !== 1 || !references[0])
    return undefined;
  const binding = pageIdentifier.parse(params[0].pat),
    receiver = pageIdentifier.parse(references[0].node);
  return binding.value === "argv" && receiver.ctxt === binding.ctxt
    ? astObject.parse(exit.callee).object
    : undefined;
}

function strictCliEntryVerified(
  module: Record<string, unknown>,
  parser: Record<string, unknown>,
): Record<string, unknown> | undefined {
  const statements = z.array(astObject).parse(module.body),
    entry = statements.at(-1);
  const expected = parseSync(STRICT_CLI_ENTRY, { syntax: "typescript" }).body[0];
  if (!entry || strictInputAst(entry) !== strictInputAst(expected)) return undefined;
  const nodes = browserAstNodes(entry),
    receiver = nodes.find(
      ({ node }) => node.type === "Identifier" && node.value === "CliOptions",
    )?.node;
  const binding = pageIdentifier.parse(parser.id),
    actual = pageIdentifier.parse(receiver);
  if (binding.ctxt !== actual.ctxt) return undefined;
  const allowed = [
    parser.id,
    ...browserAstNodes(parser.init)
      .filter(({ node }) => node.type === "Identifier" && node.value === "CliOptions")
      .map(({ node }) => node),
    receiver,
  ];
  const uses = browserAstNodes(module).filter(
    ({ node }) => node.type === "Identifier" && node.value === "CliOptions",
  );
  return uses.length === allowed.length &&
    uses.every(
      ({ node }) =>
        strictIdentifierBinding(parser.id, node) &&
        allowed.some((reference) => strictIdentifierOccurrence(node, reference)),
    )
    ? entry
    : undefined;
}

function strictIdentifierOccurrence(node: unknown, expected: unknown): boolean {
  const actual = pageIdentifier.parse(node),
    reference = pageIdentifier.parse(expected);
  return (
    actual.value === reference.value &&
    actual.ctxt === reference.ctxt &&
    actual.span.start === reference.span.start &&
    actual.span.end === reference.span.end
  );
}

function strictResolverDirectPrefixVerified(
  module: Record<string, unknown>,
  call: Record<string, unknown>,
  sourcePath: string,
): boolean {
  const cli = sourcePath === "scripts/mermaid/diagram-update.ts";
  const owners = browserAstNodes(module).filter(({ node, method }) =>
    cli
      ? node.type === "MethodProperty" && method === "parse"
      : node.type === "FunctionDeclaration" &&
        astObject.parse(node.identifier).value === "officialStrictUnusedContext",
  );
  if (owners.length !== 1 || !owners[0]) return false;
  const statements = z.array(astObject).parse(astObject.parse(owners[0].node.body).stmts);
  const at = cli ? 1 : 2;
  const statement = statements[at];
  if (statement?.type !== "VariableDeclaration" || statement.kind !== "const") return false;
  const declarations = z.array(astObject).parse(statement.declarations);
  const selected = declarations[0];
  if (
    declarations.length !== 1 ||
    JSON.stringify(selected?.init) !== JSON.stringify(call) ||
    astObject.parse(selected?.id).value !== (cli ? "assetInputs" : "selected")
  )
    return false;
  const prefix = parseSync(cli ? "CliOptions.exitIfHelp(argv);" : STRICT_CONTEXT_SELECTION_PREFIX, {
    syntax: "typescript",
  }).body;
  return strictInputAst(statements.slice(0, at)) === strictInputAst(prefix);
}

function strictInputFactoryVerified(root: string): boolean {
  const nodes = browserAstNodes(
    parseSync(read(root, "scripts/runtime-bundles/runtime-unused-contexts.ts"), {
      syntax: "typescript",
    }),
  );
  const factories = nodes.filter(
    ({ node }) =>
      node.type === "FunctionDeclaration" &&
      astObject.parse(node.identifier).value === "actualOfficialStrictInputs",
  );
  const returns = browserAstNodes(factories[0]?.node).filter(
    ({ node }) => node.type === "ReturnStatement",
  );
  return (
    factories.length === 1 &&
    strictInputFactoryScopeVerified(nodes, factories[0]?.node) &&
    returns.length === 1 &&
    strictInputAst(returns[0]?.node.argument) ===
      strictInputAst(
        strictInputExpression(
          "{ argv: process.argv.slice(2), env: process.env, cwd: process.cwd(), home: os.homedir() }",
        ),
      )
  );
}

function strictInputFactoryScopeVerified(
  nodes: ReturnType<typeof browserAstNodes>,
  factory: Record<string, unknown> | undefined,
): boolean {
  if (
    !factory ||
    z.array(astObject).parse(factory.params).length !== 0 ||
    z.array(astObject).parse(astObject.parse(factory.body).stmts).length !== 1
  )
    return false;
  const imported = strictInputOsImport(nodes);
  if (!imported) return false;
  const binding = pageIdentifier.parse(imported);
  const actual = browserAstNodes(factory).filter(
    ({ node }) => node.type === "Identifier" && ["os", "process"].includes(String(node.value)),
  );
  return (
    strictInputFactoryGlobalUsesVerified(nodes, actual, imported) &&
    actual.every(({ node }) => {
      const receiver = pageIdentifier.parse(node);
      return receiver.value === "process"
        ? receiver.ctxt === 1
        : receiver.value === binding.value && receiver.ctxt === binding.ctxt;
    })
  );
}

function strictInputFactoryGlobalUsesVerified(
  nodes: ReturnType<typeof browserAstNodes>,
  receivers: ReturnType<typeof browserAstNodes>,
  imported: unknown,
): boolean {
  const guards = nodes.filter(
    ({ node }) =>
      node.type === "FunctionDeclaration" &&
      astObject.parse(node.identifier).value === "officialMermaidUnusedContext",
  );
  if (guards.length !== 1 || !guards[0]) return false;
  const first = z.array(astObject).parse(astObject.parse(guards[0].node.body).stmts)[0];
  const expected = parseSync(
    'if (fs.realpathSync(process.cwd()) !== root) throw new Error("browser provider root mismatch");',
    { syntax: "typescript" },
  ).body[0];
  if (strictInputAst(first) !== strictInputAst(expected)) return false;
  const guardProcess = browserAstNodes(first).filter(
    ({ node }) => node.type === "Identifier" && node.value === "process",
  );
  const allowed = [
    imported,
    ...receivers.map(({ node }) => node),
    ...guardProcess.map(({ node }) => node),
  ];
  // 実factoryの4参照、os import、既存root guard以外のalias・代入・member変更は拒否する。
  return (
    guardProcess.length === 1 &&
    pageIdentifier.parse(guardProcess[0]?.node).ctxt === 1 &&
    strictInputGlobalUsesVerified(nodes, allowed)
  );
}

function strictInputGlobalUsesVerified(
  nodes: ReturnType<typeof browserAstNodes>,
  allowed: readonly unknown[],
): boolean {
  if (
    nodes.some(
      ({ node }) =>
        node.type === "Identifier" &&
        ["globalThis", "global", "Reflect", "window", "self"].includes(String(node.value)),
    )
  )
    return false;
  const uses = nodes.filter(
    ({ node }) => node.type === "Identifier" && ["os", "process"].includes(String(node.value)),
  );
  return (
    uses.length === allowed.length &&
    uses.every(({ node }) => {
      const receiver = pageIdentifier.parse(node);
      return (
        (receiver.value !== "process" || receiver.ctxt === 1) &&
        allowed.some((reference) => strictIdentifierOccurrence(node, reference))
      );
    })
  );
}

function officialStrictInputSelectionVerified(context: ContractContext): boolean {
  try {
    const resolver = "scripts/mermaid/official-renderer-asset-inputs.ts";
    const critical = MATHJAX_UNUSED_INPUTS;
    if (critical.some((name) => !context.visible.has(name) || !context.tracked.has(name)))
      return false;
    if (
      sha(fs.readFileSync(safeFile(context.root, resolver))) !==
      "8e6976c739355714bad12a6dccefb065a67c6c80f905cf3d6f7fdb1f334bb146"
    )
      return false;
    if (
      !STRICT_INPUT_CALLS.every(([sourcePath, expression]) =>
        strictInputCallVerified(context.root, sourcePath, expression),
      )
    )
      return false;
    if (
      !strictInputResolverImportVerified(
        context.root,
        "scripts/mermaid/diagram-update.ts",
        "./official-renderer-asset-inputs",
      ) ||
      !strictInputResolverImportVerified(
        context.root,
        "scripts/runtime-bundles/runtime-unused-contexts.ts",
        "../mermaid/official-renderer-asset-inputs",
      )
    )
      return false;
    if (
      !unitRegistered(
        context,
        "scripts/runtime-bundles/runtime-unused-check_test.ts",
        "bun",
        context.local,
      ) ||
      !unitRegistered(
        context,
        "scripts/runtime-bundles/runtime-unused-check_test.ts",
        "bun",
        context.ci,
      )
    )
      return false;
    if (reviewedSplitRuntimeAssetTestCommands(context) === undefined) return false;
    return strictInputFactoryVerified(context.root);
  } catch {
    return false;
  }
}

function strictInputResolverImportVerified(
  root: string,
  sourcePath: string,
  importPath: string,
): boolean {
  const imports = parseSync(read(root, sourcePath), { syntax: "typescript" }).body.filter(
    (node) =>
      node.type === "ImportDeclaration" && node.source.value === importPath && !node.typeOnly,
  );
  const bindings = imports.flatMap((node) =>
    node.type === "ImportDeclaration" ? node.specifiers : [],
  );
  return (
    bindings.length === 1 &&
    bindings[0]?.type === "ImportSpecifier" &&
    !bindings[0].isTypeOnly &&
    bindings[0].local.value === "resolveOfficialRendererAssetInputs" &&
    (bindings[0].imported === null ||
      (bindings[0].imported?.type === "Identifier" &&
        bindings[0].imported.value === "resolveOfficialRendererAssetInputs"))
  );
}
function auditedStrictInputSelection(context: ContractContext): boolean {
  const verified = officialStrictInputSelectionVerified(context);
  if (!verified)
    add(context, "mathjax-unused-input-selection-unverified", MATHJAX_UNUSED_AUDIT_SOURCE);
  return verified;
}

function nativeWidthHashFactsBinding(
  context: ContractContext,
): OwnedValidationReport["nativeWidthHashFacts"] {
  const empty: OwnedValidationReport["nativeWidthHashFacts"] = {
    sourcePath: NATIVE_WIDTH_HASH_SOURCE,
    testPath: NATIVE_WIDTH_HASH_TEST,
    factRecipe: NATIVE_WIDTH_HASH_RECIPE,
    auditRecipe: MATHJAX_UNUSED_AUDIT_RECIPE,
    inputPaths: [...NATIVE_WIDTH_HASH_INPUTS],
    inputDigest: sha(""),
    factRecipeVerified: false,
    testCommandVerified: false,
    localCheckRegistered: false,
    ciCheckRegistered: false,
    nativeExecutionVerified: false,
    performanceVerified: false,
  };
  if (!context.visible.has(NATIVE_WIDTH_HASH_SOURCE)) return empty;

  const inputDigest = nativeWidthHashInputDigest(context);
  const processRouteVerified = nativeWidthHashProcessRouteVerified(context);
  const factRecipeVerified = processRouteVerified && nativeWidthHashRecipeVerified(context);
  if (!factRecipeVerified)
    add(context, "native-width-hash-fact-recipe-unverified", "Justfile", NATIVE_WIDTH_HASH_RECIPE);

  const auditCommands = nativeWidthHashAuditCommands(context);
  const auditCommandVerified = unconditionalRecipeCommands(
    runtimeAggregateRecipeBody(context),
  ).includes(MATHJAX_UNUSED_STANDARD_GATE_COMMAND);
  const classicTestInputVerified = classicFactsTestInputVerified(context, auditCommands);
  const testCommandVerified =
    processRouteVerified && nativeWidthHashTestCommandVerified(context) && classicTestInputVerified;
  if (!testCommandVerified)
    add(context, "native-width-hash-test-unregistered", "Justfile", NATIVE_WIDTH_HASH_TEST);
  if (!auditCommandVerified)
    add(context, "native-width-hash-audit-unregistered", "Justfile", MATHJAX_UNUSED_AUDIT_COMMAND);

  const localCheckRegistered =
    factRecipeVerified &&
    testCommandVerified &&
    auditCommandVerified &&
    context.localRecipes.has(MATHJAX_UNUSED_AUDIT_RECIPE);
  const ciCheckRegistered =
    factRecipeVerified &&
    testCommandVerified &&
    auditCommandVerified &&
    context.ciRecipes.has(MATHJAX_UNUSED_AUDIT_RECIPE);
  if (!localCheckRegistered)
    add(context, "native-width-hash-check-unregistered", MATHJAX_UNUSED_AUDIT_RECIPE, "local");
  if (!ciCheckRegistered)
    add(context, "native-width-hash-check-unregistered", MATHJAX_UNUSED_AUDIT_RECIPE, "CI");

  return {
    ...empty,
    inputDigest,
    factRecipeVerified,
    testCommandVerified,
    localCheckRegistered,
    ciCheckRegistered,
  };
}
function classicFactsTestInputVerified(
  context: ContractContext,
  commands: readonly string[],
): boolean {
  const source = CLASSIC_FACTS_TEST.replace("_test.ts", ".ts");
  const productionNamespace = context.visible.has(source) || context.tracked.has(source);
  const registrations = commands.flatMap((command) =>
    unitCommand(command).filter((word) => word === `./${CLASSIC_FACTS_TEST}`),
  ).length;
  // 実readerの登録があるときは、新suiteの欠落を旧fixture互換として扱わない。
  if (!productionNamespace && registrations === 0) return true;
  let verified =
    registrations === 1 &&
    context.visible.has(CLASSIC_FACTS_TEST) &&
    context.tracked.has(CLASSIC_FACTS_TEST);
  try {
    verified = verified && fs.lstatSync(safeFile(context.root, CLASSIC_FACTS_TEST)).isFile();
    if (productionNamespace)
      verified =
        verified &&
        context.visible.has(source) &&
        context.tracked.has(source) &&
        fs.lstatSync(safeFile(context.root, source)).isFile();
  } catch {
    verified = false;
  }
  if (!verified) add(context, "rust-classic-script-test-unregistered", CLASSIC_FACTS_TEST);
  return verified;
}
function nativeWidthHashProcessRouteVerified(context: ContractContext): boolean {
  try {
    const source = context.read(NATIVE_WIDTH_HASH_SOURCE);
    const references = sourceCode(source, {
      name: NATIVE_WIDTH_HASH_SOURCE,
      read: (name) =>
        fs.existsSync(path.join(context.root, name)) ? context.read(name) : undefined,
    });
    const verified =
      !references.includes("!unresolved-executable") &&
      strictInputCallVerified(
        context.root,
        NATIVE_WIDTH_HASH_SOURCE,
        'Bun.spawnSync(["just", "native-width-hash-facts", repositoryRoot, actualSourceRoot], { cwd: repositoryRoot, timeout: 120_000 })',
      ) &&
      strictInputCallVerified(
        context.root,
        NATIVE_WIDTH_HASH_SOURCE,
        'Bun.spawnSync(["just", "native-width-classic-facts", repositoryRoot, actualSourceRoot], { cwd: repositoryRoot, timeout: 120_000 })',
      );
    if (!verified)
      add(context, "native-width-hash-process-route-unverified", NATIVE_WIDTH_HASH_SOURCE);
    return verified;
  } catch (error) {
    add(
      context,
      "native-width-hash-process-route-unverified",
      NATIVE_WIDTH_HASH_SOURCE,
      String(error),
    );
    return false;
  }
}

function nativeWidthHashInputDigest(context: ContractContext): string {
  const inputRecords: string[] = [];
  for (const name of NATIVE_WIDTH_HASH_INPUTS) {
    if (!context.visible.has(name)) {
      add(context, "native-width-hash-input-missing", name);
      inputRecords.push(`${name}\0missing\n`);
      continue;
    }
    if (!context.tracked.has(name)) add(context, "untracked-authority", name);
    try {
      inputRecords.push(`${name}\0${sha(fs.readFileSync(safeFile(context.root, name)))}\n`);
    } catch (error) {
      add(context, "native-width-hash-input-unsafe", name, String(error));
      inputRecords.push(`${name}\0unsafe\n`);
    }
  }
  return sha(inputRecords.join(""));
}
function nativeWidthHashRecipeVerified(context: ContractContext): boolean {
  if (!guardedFactsRecipeVerified(context.read("Justfile"), NATIVE_WIDTH_HASH_RECIPE)) return false;
  if (
    !context.visible.has(NATIVE_WIDTH_CARGO_GUARD_SOURCE) ||
    !context.tracked.has(NATIVE_WIDTH_CARGO_GUARD_SOURCE)
  )
    return false;
  try {
    const guard = safeFile(context.root, NATIVE_WIDTH_CARGO_GUARD_SOURCE);
    return (
      relative(context.root, guard) === NATIVE_WIDTH_CARGO_GUARD_SOURCE &&
      fs.lstatSync(guard).isFile()
    );
  } catch {
    return false;
  }
}
function nativeWidthHashAuditCommands(context: ContractContext): string[] {
  return runtimeAssetTestCommands(context) ?? [];
}
function nativeWidthHashTestCommandVerified(context: ContractContext): boolean {
  const unusedCommands = unconditionalRecipeCommands(runtimeUnusedRecipeBody(context));
  const registered = unusedCommands.reduce(
    (count, line) =>
      count + unitCommand(line).filter((word) => word === `./${NATIVE_WIDTH_HASH_TEST}`).length,
    0,
  );
  return reviewedSplitRuntimeAssetTestCommands(context) !== undefined && registered === 1;
}
function parserBindingPathsSafe(context: ContractContext, paths: string[]): boolean {
  let inputsSafe = true;
  for (const name of paths) {
    if (!context.visible.has(name)) {
      inputsSafe = false;
      add(context, "parser-body-onload-input-missing", name);
    } else if (!context.tracked.has(name)) {
      inputsSafe = false;
      add(context, "untracked-authority", name);
    }
    try {
      safeFile(context.root, name);
    } catch (error) {
      inputsSafe = false;
      add(
        context,
        "unsafe-or-missing-source",
        name,
        error instanceof Error ? error.message : String(error),
      );
    }
  }
  return inputsSafe;
}
function parserBindingContents(context: ContractContext, paths: string[]): Map<string, string> {
  return new Map(
    paths.map((name) => {
      try {
        return [name, context.read(name)];
      } catch {
        return [name, ""];
      }
    }),
  );
}
function parserProducerVerified(content: Map<string, string>, inputsSafe: boolean): boolean {
  const producer = content.get(PARSER_BODY_PRODUCER) ?? "";
  const producerRequirements = [
    "export const parserBodyOnloadDirectory =",
    '"crates/katana-render-runtime/src/renderer/backends/html_runtime"',
    "transformSync(canonical, {",
    'target: "es2024"',
    "isModule: false",
    "sourceMaps: false",
    "minify: false",
    '"biome"',
    '"format"',
    '"--stdin-file-path"',
    'if (generated !== expected) throw new Error("Parser body-onload generated runtime is stale")',
    'process.argv[2] !== "--check"',
  ];
  if (
    !inputsSafe ||
    !producerRequirements.every((fragment) => producer.includes(fragment)) ||
    content.get(PARSER_BODY_TEST)?.includes('from "./parser-body-onload-generation"') !== true ||
    content.get(PARSER_BODY_TEST)?.includes("checkParserBodyOnloadFiles(root)") !== true
  )
    return false;
  return parserProducerAstSupported(producer);
}
// 実 producer のレビュー済み形状だけを受け入れ、未知の実行辺は推論しない。
// 更新時は producer のレビューと実生成回帰を先に行い、自動で基準を更新しない。
const PARSER_PRODUCER_SWC_VERSION = "1.16.20";
const PARSER_PRODUCER_AST_SHA256 =
  "0b5842612fbaf3e92e09748da33fa5d10ff74e67442de5e49761ffe86fb8474d";
function parserProducerAstSupported(source: string): boolean {
  if (swcVersion !== PARSER_PRODUCER_SWC_VERSION) return false;
  try {
    const ast = parseSync(source, { syntax: "typescript", tsx: true });
    const normalized = JSON.stringify(ast, (key: string, value: unknown) =>
      ["span", "ctxt", "bodyCtxt", "raw"].includes(key) ? undefined : value,
    );
    return sha(normalized) === PARSER_PRODUCER_AST_SHA256;
  } catch {
    return false;
  }
}
function typeScopeHas(types: TypeScope[], name: string, files: string[]): boolean {
  const scope = types.find((item) => item.name === name);
  return files.every((file) => scope?.files.has(file) === true);
}
function parserRecipeHas(context: ContractContext, recipe: string, expected: string[]): boolean {
  const lines = unconditionalRecipeCommands(
    recipe === PARSER_BODY_TEST_RECIPE
      ? runtimeAssetRecipeBody(context)
      : (context.recipes.get(recipe) ?? ""),
  );
  const expectedDigest =
    recipe === PARSER_BODY_CHECK_RECIPE
      ? PARSER_CHECK_RECIPE_SHA256
      : recipe === PARSER_BODY_TEST_RECIPE
        ? PARSER_TEST_RECIPE_SHA256
        : "";
  const digestMatches =
    recipe === PARSER_BODY_TEST_RECIPE
      ? reviewedSplitRuntimeAssetTestCommands(context) !== undefined
      : sha(lines.join("\n")) === expectedDigest;
  return (
    expectedDigest.length > 0 &&
    digestMatches &&
    lines.some((line) => unitCommand(line).join("\0") === expected.join("\0"))
  );
}
function unconditionalRecipeCommands(body: string): string[] {
  const rawLines = body.split("\n").map((line) => line.trim());
  if (rawLines.some((line) => line.startsWith("#!"))) return [];
  const lines = rawLines.filter((line) => line.length > 0 && !line.startsWith("#"));
  if (
    lines.length === 0 ||
    lines.some(
      (line) =>
        /[;|&`$<>\\]/.test(line) ||
        /(?:^|\s)(?:if|then|elif|else|fi|for|while|case|esac)\b/.test(line) ||
        !["bun", "python3", "just", "uvx"].includes(
          line === HOST_EVIDENCE_PRECONDITIONS[1] || line === CLASSIC_GENERATION_EVIDENCE_CONSUMER
            ? "bun"
            : (unitCommand(line)[0] ?? ""),
        ),
    )
  )
    return [];
  return lines;
}
function reviewedSplitRuntimeAssetTestCommands(context: ContractContext): string[] | undefined {
  return runtimeAssetTestCommands(context);
}
function runtimeAssetTestCommands(context: ContractContext): string[] | undefined {
  const commands = unconditionalRecipeCommands(runtimeAssetRecipeBody(context));
  const provenanceCommands = commands.filter((line) => line === SVG_ENGINE_PROVENANCE_TEST_COMMAND);
  if (provenanceCommands.length !== 1) return undefined;
  const reviewedCommands = commands.filter((line) => line !== SVG_ENGINE_PROVENANCE_TEST_COMMAND);
  const withoutMacos = removeVerifiedMacosBrowserCommand(context, reviewedCommands);
  if (!withoutMacos) return undefined;
  const unused = unconditionalRecipeCommands(runtimeUnusedRecipeBody(context));
  if (!unusedRegressionRecipeVerified(context)) return undefined;
  const reviewed = reviewedRuntimeAssetTestCommands(withoutMacos);
  if (!reviewed) return undefined;
  return [SVG_ENGINE_PROVENANCE_TEST_COMMAND, ...reviewed, ...unused];
}
function removeVerifiedMacosBrowserCommand(
  context: ContractContext,
  lines: string[],
): string[] | undefined {
  const command = `just ${PLAYWRIGHT_MACOS_RECIPE}`;
  const indexes = lines.flatMap((line, index) => (line === command ? [index] : []));
  if (indexes.length === 0) {
    return context.recipes.has(PLAYWRIGHT_MACOS_RECIPE) ||
      context.visible.has(PLAYWRIGHT_MACOS_CONFIG) ||
      context.visible.has(PLAYWRIGHT_MACOS_PRODUCER_TEST)
      ? undefined
      : lines;
  }
  const index = indexes[0];
  if (
    indexes.length !== 1 ||
    index === undefined ||
    lines[index + 1] !== `just ${PLAYWRIGHT_RECIPE}` ||
    !macosBrowserCommandInputsVerified(context)
  ) {
    return undefined;
  }
  return lines.filter((_, position) => position !== index);
}
function macosBrowserCommandInputsVerified(context: ContractContext): boolean {
  try {
    return (
      playwrightMacosInputsVerified(context) &&
      context.read("Justfile").includes(PLAYWRIGHT_MACOS_RUN_DEFINITION) &&
      context.recipes.get(PLAYWRIGHT_MACOS_RECIPE)?.trim() === "{{MACOS_BROWSER_CONTRACT_RUN}}" &&
      playwrightMacosWorkflowRegistered(context)
    );
  } catch {
    return false;
  }
}
function reviewedRuntimeAssetTestCommands(
  lines: string[],
  appendedCommands: string[] = [],
): string[] | undefined {
  const withoutClassicEvidence = removeClassicGenerationEvidenceCommands(lines);
  if (!withoutClassicEvidence) return undefined;
  const normalizedOrder = normalizeMovedRuntimeAssetCommands(withoutClassicEvidence);
  if (!normalizedOrder) return undefined;
  const prepared = restoreReviewedBrowserContractProcess(normalizedOrder);
  if (!prepared) return undefined;
  if (
    prepared[0] === HOST_EVIDENCE_PRECONDITIONS[0] &&
    prepared[1] === HOST_EVIDENCE_PRECONDITIONS[1]
  )
    prepared.splice(0, 2);
  if (prepared[0] === HOST_EVIDENCE_TEST_COMMAND) prepared.shift();
  const canonical: string[] = [];
  for (const line of prepared) {
    const normalizedLine = canonicalBunTestLine(line);
    if (normalizedLine === undefined) return undefined;
    canonical.push(normalizedLine);
  }
  const normalized = normalizeClassicFactsTestCommands([...canonical, ...appendedCommands]);
  return normalized ? reviewedRuntimeAssetTestCommandsWithoutClassic(normalized) : undefined;
}
function normalizeMovedRuntimeAssetCommands(lines: string[]): string[] | undefined {
  const leadingAssetsIndex = uniqueCommandIndex(lines, RUNTIME_ASSET_TEST_MARKER);
  const ownedMarkerIndex = uniqueCommandIndex(
    lines,
    "./scripts/quality/owned-source-inventory_test.ts",
  );
  if (leadingAssetsIndex !== 0 && ownedMarkerIndex !== lines.length - 1) return lines;
  const indexes = movedRuntimeAssetCommandIndexes(lines);
  if (!indexes) return undefined;
  const [assetIndex, hostIndex, ownedIndex, moonIndex] = indexes;
  if (hostIndex === 0 && ownedIndex === 1 && moonIndex === 2 && assetIndex === 3) return lines;
  if (assetIndex !== 0 || hostIndex !== 1 || moonIndex !== 2 || ownedIndex !== lines.length - 1)
    return undefined;
  return [
    HOST_EVIDENCE_TEST_COMMAND,
    OWNED_VALIDATION_COMMAND,
    MOON_CACHE_OWNER_COMMAND,
    ...lines.slice(assetIndex, assetIndex + 1),
    ...lines.slice(3, -1),
  ];
}
function movedRuntimeAssetCommandIndexes(lines: string[]): number[] | undefined {
  const assetIndex = uniqueCommandIndex(lines, RUNTIME_ASSET_TEST_MARKER);
  const ownedIndexes = lines.flatMap((line, index) =>
    line === OWNED_VALIDATION_COMMAND ? [index] : [],
  );
  const hostIndexes = lines.flatMap((line, index) =>
    line === HOST_EVIDENCE_TEST_COMMAND ? [index] : [],
  );
  const moonIndexes = lines.flatMap((line, index) =>
    line === MOON_CACHE_OWNER_COMMAND ? [index] : [],
  );
  const ownedIndex = ownedIndexes[0];
  const hostIndex = hostIndexes[0];
  const moonIndex = moonIndexes[0];
  if (
    assetIndex === undefined ||
    ownedIndex === undefined ||
    ownedIndexes.length !== 1 ||
    hostIndex === undefined ||
    hostIndexes.length !== 1 ||
    moonIndex === undefined ||
    moonIndexes.length !== 1
  )
    return undefined;
  return [assetIndex, hostIndex, ownedIndex, moonIndex];
}
function uniqueCommandIndex(lines: string[], marker: string): number | undefined {
  const indexes = lines.flatMap((line, index) => {
    const words = unitCommand(line);
    return words.filter((word) => word === marker).length === 1 ? [index] : [];
  });
  return indexes.length === 1 ? indexes[0] : undefined;
}
function removeClassicGenerationEvidenceCommands(lines: string[]): string[] | undefined {
  const consumerIndexes = lines.flatMap((line, index) =>
    line.trim() === CLASSIC_GENERATION_EVIDENCE_CONSUMER ? [index] : [],
  );
  const consumerIndex = consumerIndexes[0];
  if (
    consumerIndexes.length !== 1 ||
    consumerIndex === undefined ||
    contextIndependentClassicProducerForbidden(lines)
  )
    return undefined;
  return lines.flatMap((line, index) =>
    index === consumerIndex ? [CLASSIC_GENERATION_EVIDENCE_TEST_COMMAND] : [line],
  );
}
function contextIndependentClassicProducerForbidden(lines: string[]): boolean {
  return lines.some((line) => line.trim() === CLASSIC_GENERATION_EVIDENCE_PRODUCER);
}
function restoreReviewedBrowserContractProcess(lines: string[]): string[] | undefined {
  if (lines.at(-1) !== `just ${PLAYWRIGHT_RECIPE}`) return undefined;
  const prepared = lines.slice(0, -1);
  if (
    prepared.some(
      (line) =>
        line === `just ${PLAYWRIGHT_RECIPE}` ||
        PLAYWRIGHT_TESTS.some((name) => unitCommand(line).includes(`./${name}`)),
    )
  )
    return undefined;
  const pure = "./scripts/html/interim-feature-results_test.ts";
  const badge = "./scripts/html/compare-toc-evidence_test.ts";
  const pureIndex = prepared.findIndex((line) => unitCommand(line).at(-1) === pure);
  const badgeIndex = prepared.findIndex((line) => unitCommand(line).at(-1) === badge);
  if (pureIndex < 0 || badgeIndex < 0) return undefined;
  prepared[badgeIndex] += ` ./${PLAYWRIGHT_TESTS[1]}`;
  prepared.splice(
    pureIndex + 1,
    0,
    `bun test --path-ignore-patterns 'tmp/**' ./${HTML_FEATURE_CATALOG_TEST}`,
  );
  return prepared;
}
function reviewedRuntimeAssetTestCommandsWithoutClassic(lines: string[]): string[] | undefined {
  // 既知の recipe を固定し、canonical runtime-bundles command 内だけを認識する。
  const nativeWidthIndexes = lines.flatMap((line, index) =>
    unitCommand(line).includes(`./${NATIVE_WIDTH_HASH_TEST}`) ? [index] : [],
  );
  if (nativeWidthIndexes.length > 1) return undefined;

  const normalized = [...lines];
  const nativeWidthIndex = nativeWidthIndexes[0];
  if (nativeWidthIndex !== undefined) {
    const line = normalized[nativeWidthIndex];
    const words = line ? unitCommand(line) : [];
    const runtimeBundleIndexes = normalized.flatMap((candidate, index) => {
      const candidateWords = unitCommand(candidate);
      return candidateWords.includes("./scripts/runtime-bundles/runtime-fragment-loader_test.ts")
        ? [index]
        : [];
    });
    const mathJaxIndex = words.indexOf(`./${MATHJAX_UNUSED_AUDIT_TEST}`);
    const nativeWidthIndexInCommand = words.indexOf(`./${NATIVE_WIDTH_HASH_TEST}`);
    if (
      !line ||
      runtimeBundleIndexes.length !== 1 ||
      nativeWidthIndex !== runtimeBundleIndexes[0] ||
      words[0] !== "bun" ||
      words[1] !== "test" ||
      mathJaxIndex < 0 ||
      mathJaxIndex !== words.length - 2 ||
      nativeWidthIndexInCommand !== mathJaxIndex + 1 ||
      words.at(-1) !== `./${NATIVE_WIDTH_HASH_TEST}` ||
      words.filter((word) => word === `./${NATIVE_WIDTH_HASH_TEST}`).length !== 1 ||
      line.split(`./${NATIVE_WIDTH_HASH_TEST}`).length !== 2
    ) {
      return undefined;
    }
    const withoutFocusedTest = line.replace(` ./${NATIVE_WIDTH_HASH_TEST}`, "");
    if (withoutFocusedTest === line) return undefined;
    normalized[nativeWidthIndex] = withoutFocusedTest;
  }

  const withoutAudit = removeTerminalRecipeCommand(normalized, MATHJAX_UNUSED_AUDIT_COMMAND);
  if (!withoutAudit) return undefined;
  const withoutStandardGate = withoutAudit.includes(MATHJAX_UNUSED_STANDARD_GATE_COMMAND)
    ? removeTerminalRecipeCommand(withoutAudit, MATHJAX_UNUSED_STANDARD_GATE_COMMAND)
    : withoutAudit;
  if (!withoutStandardGate) return undefined;

  const withoutReviewedTests = removeReviewedRuntimeAssetTests(withoutStandardGate);
  const digest = withoutReviewedTests ? sha(withoutReviewedTests.join("\n")) : "none";
  return withoutReviewedTests && digest === PARSER_TEST_RECIPE_SHA256
    ? withoutReviewedTests
    : undefined;
}
function removeTerminalRecipeCommand(lines: string[], command: string): string[] | undefined {
  const indexes = lines.flatMap((line, index) => (line === command ? [index] : []));
  if (indexes.length > 1) return undefined;
  if (indexes.length === 0) return lines;
  return indexes[0] === lines.length - 1 ? lines.slice(0, -1) : undefined;
}
function restoreReviewedFeatureCatalogTestProcess(lines: string[]): string[] | undefined {
  const featureTest = `./${HTML_FEATURE_CATALOG_TEST}`;
  const pureHtmlTests = [
    "./scripts/html/visual-state-fidelity_test.ts",
    "./scripts/html/dynamic-fidelity_test.ts",
    "./scripts/html/interim-feature-results_test.ts",
  ];
  const featureIndexes = lines.flatMap((line, index) =>
    unitCommand(line).includes(featureTest) ? [index] : [],
  );
  const pureIndexes = lines.flatMap((line, index) =>
    pureHtmlTests.some((testPath) => unitCommand(line).includes(testPath)) ? [index] : [],
  );
  const featureIndex = featureIndexes[0];
  const pureIndex = pureIndexes[0];
  if (
    featureIndexes.length !== 1 ||
    pureIndexes.length !== 1 ||
    featureIndex === undefined ||
    pureIndex === undefined ||
    featureIndex !== pureIndex + 1
  )
    return undefined;

  const featureLine = lines[featureIndex];
  const pureLine = lines[pureIndex];
  const common = ["bun", "test", "--path-ignore-patterns", "tmp/**"];
  if (
    !featureLine ||
    !pureLine ||
    JSON.stringify(unitCommand(featureLine)) !== JSON.stringify([...common, featureTest]) ||
    JSON.stringify(unitCommand(pureLine)) !== JSON.stringify([...common, ...pureHtmlTests])
  )
    return undefined;

  const firstPureTest = pureHtmlTests[0];
  if (!firstPureTest) return undefined;
  const restoredPureLine = pureLine.replace(firstPureTest, `${featureTest} ${firstPureTest}`);
  if (restoredPureLine === pureLine) return undefined;

  const restored = [...lines];
  restored[pureIndex] = restoredPureLine;
  restored.splice(featureIndex, 1);
  return restored;
}
function removeReviewedRuntimeAssetTests(lines: string[]): string[] | undefined {
  const withoutFrameContext = removeReviewedTestToken(
    lines,
    "./scripts/mermaid/native_frame_capture_context_test.ts",
    "./scripts/mermaid/native_frame_evidence_test.ts",
    "./scripts/mermaid/native-render-batch_test.ts",
  );
  if (!withoutFrameContext) return undefined;
  const bboxTest = "./scripts/mermaid/dom_svg_bbox_visibility_contract_test.ts";
  const withoutBbox = withoutFrameContext.some((line) => unitCommand(line).includes(bboxTest))
    ? removeReviewedTestToken(
        withoutFrameContext,
        bboxTest,
        "./scripts/mermaid/dom_plain_text_measure_contract_test.ts",
        `./${MANAGED_FULL_COMPARE_TEST}`,
      )
    : withoutFrameContext;
  if (!withoutBbox) return undefined;
  const withoutGeometry = removeReviewedTestToken(
    withoutBbox,
    "./scripts/mermaid/reference_svg_geometry_test.ts",
    "./scripts/mermaid/reference_score_policy_test.ts",
    "./scripts/mermaid/split_markdown_fixtures_test.ts",
  );
  if (!withoutGeometry) return undefined;
  const restoredBrowserInvocation = restoreReviewedFeatureCatalogTestProcess(withoutGeometry);
  if (!restoredBrowserInvocation) return undefined;
  const withoutChromiumFontTrace = removeReviewedTestToken(
    restoredBrowserInvocation,
    "./scripts/html/chromium-font-trace_test.ts",
    "./scripts/html/catalog-font-equivalence_test.ts",
    "./scripts/html/catalog-capture-producer-context_test.ts",
  );
  if (!withoutChromiumFontTrace) return undefined;
  const withoutCatalogFontEquivalence = removeReviewedTestToken(
    withoutChromiumFontTrace,
    "./scripts/html/catalog-font-equivalence_test.ts",
    "./scripts/html/stage-score-suite_test.ts",
    "./scripts/html/catalog-capture-producer-context_test.ts",
  );
  if (!withoutCatalogFontEquivalence) return undefined;
  const withoutManagedFullCompare = removeReviewedTestToken(
    withoutCatalogFontEquivalence,
    `./${MANAGED_FULL_COMPARE_TEST}`,
    "./scripts/mermaid/dom_plain_text_measure_contract_test.ts",
    "./scripts/mermaid/mermaid_pair_manifest_test.ts",
  );
  if (!withoutManagedFullCompare) return undefined;
  const withoutFingerprint = removeReviewedRuntimeBundleFingerprintTest(withoutManagedFullCompare);
  if (!withoutFingerprint) return undefined;
  const withoutNativeSourceManifest = removeReviewedNativeSourceManifestTest(withoutFingerprint);
  if (!withoutNativeSourceManifest) return undefined;
  const withoutNativeFrameBuildProof = removeReviewedNativeFrameBuildProofTest(
    withoutNativeSourceManifest,
  );
  if (!withoutNativeFrameBuildProof) return undefined;
  const withoutNativeFrameEvidence = removeReviewedNativeFrameEvidenceTest(
    withoutNativeFrameBuildProof,
  );
  if (!withoutNativeFrameEvidence) return undefined;
  const withoutNativeRenderBatch = removeReviewedNativeRenderBatchTest(withoutNativeFrameEvidence);
  if (!withoutNativeRenderBatch) return undefined;
  const withoutOfficialRawMetrics = removeReviewedTestToken(
    withoutNativeRenderBatch,
    "./scripts/mermaid/official_raw_metrics_diagnostic_test.ts",
    "./scripts/mermaid/official_reference_mode_test.ts",
    "./scripts/mermaid/official_renderer_provenance_test.ts",
  );
  return withoutOfficialRawMetrics
    ? removeReviewedTestToken(
        withoutOfficialRawMetrics,
        "./scripts/mermaid/native_product_score_test.ts",
        "./scripts/quality/duplication-check_test.ts",
        "./scripts/runtime-bundles/runtime-bundle-checks_test.ts",
      )
    : undefined;
}
function removeReviewedRuntimeBundleFingerprintTest(lines: string[]): string[] | undefined {
  if (
    !lines.some((line) => unitCommand(line).includes(`./${RUNTIME_BUNDLE_TOOL_FINGERPRINT_TEST}`))
  )
    return lines;
  return removeReviewedTestToken(
    lines,
    `./${RUNTIME_BUNDLE_TOOL_FINGERPRINT_TEST}`,
    "./scripts/runtime-bundles/runtime-unused-check_test.ts",
    `./${MATHJAX_UNUSED_AUDIT_TEST}`,
  );
}
function removeReviewedNativeSourceManifestTest(lines: string[]): string[] | undefined {
  return removeReviewedTestToken(
    lines,
    `./${NATIVE_SOURCE_MANIFEST_TEST}`,
    "./scripts/mermaid/native-cli-process_test.ts",
    "./scripts/mermaid/native_render_provenance_test.ts",
  );
}
function removeReviewedNativeRenderBatchTest(lines: string[]): string[] | undefined {
  return removeReviewedTestToken(
    lines,
    "./scripts/mermaid/native-render-batch_test.ts",
    "./scripts/mermaid/native_render_provenance_test.ts",
    "./scripts/mermaid/official_reference_mode_test.ts",
  );
}
function removeReviewedNativeFrameEvidenceTest(lines: string[]): string[] | undefined {
  return removeReviewedTestToken(
    lines,
    "./scripts/mermaid/native_frame_evidence_test.ts",
    "./scripts/mermaid/native_render_provenance_test.ts",
    "./scripts/mermaid/native-render-batch_test.ts",
  );
}
function removeReviewedNativeFrameBuildProofTest(lines: string[]): string[] | undefined {
  return removeReviewedTestToken(
    lines,
    "./scripts/mermaid/native_frame_build_proof_test.ts",
    "./scripts/mermaid/native_render_provenance_test.ts",
    "./scripts/mermaid/native_frame_evidence_test.ts",
  );
}
function removeReviewedTestToken(
  lines: string[],
  testPath: string,
  previousTestPath: string,
  nextTestPath: string,
): string[] | undefined {
  const indexes = lines.flatMap((line, index) =>
    unitCommand(line).includes(testPath) ? [index] : [],
  );
  if (indexes.length !== 1) return undefined;
  const index = indexes[0];
  if (index === undefined) return undefined;
  const line = lines[index];
  if (!line) return undefined;

  const words = unitCommand(line);
  const pathIndex = words.indexOf(testPath);
  if (
    words[0] !== "bun" ||
    words[1] !== "test" ||
    words[pathIndex - 1] !== previousTestPath ||
    words[pathIndex + 1] !== nextTestPath ||
    words.filter((word) => word === testPath).length !== 1
  )
    return undefined;

  const normalizedLine = line.replace(` ${testPath}`, "");
  if (normalizedLine === line) return undefined;
  const normalized = [...lines];
  normalized[index] = normalizedLine;
  return normalized;
}
function moonOwnerAstBinding(
  root: string,
  ownerSource: string,
): z.infer<typeof moonOwnerAstResult> | undefined {
  try {
    // 宣言された helper と実際に実行する固定 module の実体を一致させる。
    if (
      read(root, MOON_CACHE_OWNER_AST) !==
      read(path.resolve(import.meta.dir, "../.."), MOON_CACHE_OWNER_AST)
    )
      return undefined;
    const result: unknown = JSON.parse(
      execFileSync("python3", [path.join(import.meta.dir, "owned_python_owner_contract.py")], {
        encoding: "utf8",
        input: ownerSource,
      }),
    );
    const parsed = moonOwnerAstResult.safeParse(result);
    if (
      parsed.success &&
      parsed.data.valid &&
      parsed.data.limitations[0] ===
        "Checks supported AST structure, CLI entry, scheduler barrier assertions, and early returns only." &&
      parsed.data.limitations[1] ===
        "Does not execute or prove external Moon, Bun, filesystem, or subprocess behavior."
    )
      return parsed.data;
    return undefined;
  } catch {
    return undefined;
  }
}
function parserBodyOnloadBinding(
  context: ContractContext,
  biome: z.infer<typeof biomeContract>,
  types: TypeScope[],
): OwnedValidationReport["parserBodyOnload"] {
  const { inputs, paths } = parserBodyOnloadPaths(context);
  if (!inputs.some((name) => context.visible.has(name)))
    return emptyParserBodyOnloadBinding(inputs);
  const inputsSafe = parserBindingPathsSafe(context, paths);
  const content = parserBindingContents(context, paths);
  const producerVerified = parserProducerVerified(content, inputsSafe);
  if (!producerVerified)
    add(
      context,
      "parser-body-onload-producer-unverified",
      PARSER_BODY_OUTPUT,
      PARSER_BODY_PRODUCER,
    );

  const canonicalTypeScope = typeScopeHas(types, PARSER_BODY_TYPE_PROFILES[0], [
    PARSER_BODY_CANONICAL,
    PARSER_BODY_HOST,
  ]);
  const producerTypeScope = typeScopeHas(types, PARSER_BODY_TYPE_PROFILES[1], [
    PARSER_BODY_PRODUCER,
    PARSER_BODY_TEST,
  ]);
  if (!canonicalTypeScope)
    add(context, "parser-body-onload-type-scope-unverified", PARSER_BODY_CANONICAL);
  if (!producerTypeScope)
    add(context, "parser-body-onload-type-scope-unverified", PARSER_BODY_PRODUCER);

  const biomeIncluded = included(PARSER_BODY_OUTPUT, biome.files.includes);
  if (!biomeIncluded) add(context, "biome-scope-missing", PARSER_BODY_OUTPUT);
  const checkCommand = ["bun", PARSER_BODY_PRODUCER, "--check"];
  const testCommand = ["bun", "test", "--path-ignore-patterns", "tmp/**", `./${PARSER_BODY_TEST}`];
  const checkCommandPresent = parserRecipeHas(context, PARSER_BODY_CHECK_RECIPE, checkCommand);
  const testCommandPresent = parserRecipeHas(context, PARSER_BODY_TEST_RECIPE, testCommand);
  const testRecipe = context.recipes.has(RUNTIME_ASSET_OTHER_RECIPE)
    ? RUNTIME_ASSET_OTHER_RECIPE
    : PARSER_BODY_TEST_RECIPE;
  const localCheckRegistered =
    producerVerified && checkCommandPresent && context.local.has(PARSER_BODY_CHECK_RECIPE);
  const ciCheckRegistered =
    producerVerified && checkCommandPresent && context.ci.has(PARSER_BODY_CHECK_RECIPE);
  const localTestRegistered =
    producerVerified && testCommandPresent && context.local.has(testRecipe);
  const ciTestRegistered = producerVerified && testCommandPresent && context.ci.has(testRecipe);
  if (!localCheckRegistered || !ciCheckRegistered)
    add(context, "parser-body-onload-check-unregistered", PARSER_BODY_CHECK_RECIPE);
  if (!localTestRegistered || !ciTestRegistered)
    add(context, "parser-body-onload-test-unregistered", PARSER_BODY_TEST_RECIPE);

  const inputDigest = sha(
    paths.map((name) => `${name}\0${sha(content.get(name) ?? "")}\n`).join(""),
  );
  const outputSha256 = sha(content.get(PARSER_BODY_OUTPUT) ?? "");
  const binding: OwnedValidationReport["parserBodyOnload"] = {
    path: PARSER_BODY_OUTPUT,
    canonical: PARSER_BODY_CANONICAL,
    hostDeclaration: PARSER_BODY_HOST,
    producer: PARSER_BODY_PRODUCER,
    unitTest: PARSER_BODY_TEST,
    inputPaths: paths,
    inputDigest,
    outputSha256,
    producerVerified,
    biomeIncluded,
    canonicalTypeScope,
    producerTypeScope,
    outputTypeBoundary: "generated-from-strict-typescript",
    localCheckRegistered,
    ciCheckRegistered,
    localTestRegistered,
    ciTestRegistered,
    byteVerification: "not-executed",
  };
  context.parserBodyOnloadBinding = binding;
  return binding;
}
function unitIssues(context: ContractContext, units: OwnedValidationReport["units"]): void {
  for (const language of ["bun", "python", "playwright"] as const)
    for (const name of units[language]) {
      if (!unitRegistered(context, name, language, context.local))
        add(context, "unit-local-missing", name, language);
      if (!unitRegistered(context, name, language, context.ci))
        add(context, "unit-ci-missing", name, language);
    }
}

function moonCacheFixtureInputsRegistered(context: ContractContext): boolean {
  let valid = true;
  for (const name of [
    MOON_CACHE_FIXTURE,
    MOON_CACHE_OWNER,
    MOON_CACHE_OWNER_AST,
    context.policy.moon?.config ?? "moon.yml",
  ]) {
    if (!context.visible.has(name)) {
      add(context, "moon-cache-fixture-input-missing", name);
      valid = false;
    } else if (!context.tracked.has(name)) {
      add(context, "untracked-authority", name);
      valid = false;
    }
  }
  return valid;
}
function moonFixtureSourceVerified(fixtureSource: string): boolean {
  const fixtureRequirements = [
    'import { expect, test } from "bun:test"',
    "process.env.CONTRACT_ROOT",
    "process.env.CONTRACT_PHASE",
    'test("real Bun test executes the declared input"',
  ];
  return fixtureRequirements.every((fragment) => fixtureSource.includes(fragment));
}
function moonOwnerCommandRegistered(context: ContractContext): boolean {
  const bodies = [runtimeAssetRecipeBody(context), context.recipes.get(MOON_CACHE_RECIPE) ?? ""];
  const matches = bodies.flatMap((body) =>
    unconditionalRecipeCommands(body).filter((line) => line === MOON_CACHE_OWNER_COMMAND),
  );
  const classicProducerRegistered =
    context.recipes.get(CLASSIC_GENERATION_EVIDENCE_RECIPE)?.trim() ===
    CLASSIC_GENERATION_EVIDENCE_PRODUCER;
  // Moonの起動経路にはBun以外も含まれるため、Python ownerは独立した経路で検証する。
  return (
    matches.length === 1 &&
    classicProducerRegistered &&
    reviewedSplitRuntimeAssetTestCommands(context) !== undefined
  );
}
function moonCacheOwnerCheckRecipe(context: ContractContext): string {
  return context.recipes.has(RUNTIME_ASSET_OTHER_RECIPE)
    ? RUNTIME_ASSET_OTHER_RECIPE
    : MOON_CACHE_RECIPE;
}
function moonRootDagVerified(
  context: ContractContext,
  expected: z.infer<typeof moonOwnerAstResult>["schedulerTasks"],
  rootSource: string,
): boolean {
  if (!expected) return false;
  try {
    const moon = context.policy.moon;
    const actual = moonStaticTaskProjection(rootSource, moon?.project ?? "krr", "unit-test");
    const names = Object.keys(expected).sort();
    if (Object.keys(actual).sort().join("\0") !== names.join("\0")) return false;
    return (
      names.every((name) => {
        const left = actual[name];
        const right = expected[name];
        const expectedOptions = right ? Object.entries(right.options).sort() : [];
        const actualOptions = left ? Object.entries(left.options).sort() : [];
        const cacheLeafException =
          right?.options.cache === false &&
          ((name === "runtime-bundle-check" && left?.options.cache === true) ||
            (name === "unit-test" && left?.options.cache === "local") ||
            (name.endsWith("-producer") &&
              RUNTIME_UNUSED_LEAF_IDS.includes(
                name.slice(0, -"-producer".length) as (typeof RUNTIME_UNUSED_LEAF_IDS)[number],
              ) &&
              left?.options.cache === "local")) &&
          JSON.stringify(expectedOptions.filter(([key]) => key !== "cache")) ===
            JSON.stringify(actualOptions.filter(([key]) => key !== "cache"));
        // WHY: Moonは従来のunit-test DAG leafでroot coverageの共有recipeを実行する。
        const sharedRootCoverageLeaf =
          name === "unit-test" &&
          right?.leaf === "unit-test" &&
          left?.leaf === "root-test-and-coverage";
        return (
          left !== undefined &&
          right !== undefined &&
          (left.leaf === right.leaf || sharedRootCoverageLeaf) &&
          left.deps.join("\0") === right.deps.join("\0") &&
          (cacheLeafException || JSON.stringify(actualOptions) === JSON.stringify(expectedOptions))
        );
      }) && moonCoverageSharesRootRun(rootSource, moon?.project ?? "krr", actual)
    );
  } catch {
    return false;
  }
}

function moonCoverageSharesRootRun(
  source: string,
  project: string,
  unitTasks: ReturnType<typeof moonStaticTaskProjection>,
): boolean {
  try {
    const coverage = moonStaticTaskProjection(source, project, "coverage");
    const rootTask = unitTasks["unit-test"];
    const coverageTask = coverage.coverage;
    const performanceTask = coverage["performance-coverage"];
    if (
      rootTask?.leaf !== "root-test-and-coverage" ||
      coverageTask?.leaf !== "coverage-result-check" ||
      coverageTask.deps.join("\0") !== "performance-coverage" ||
      coverageTask.options.cache !== false ||
      performanceTask?.leaf !== "performance-harness-coverage" ||
      performanceTask.deps.join("\0") !== "check\0heavy-native"
    )
      return false;
    const chain: Record<string, string> = {
      "runtime-package-asset-check": "unit-test",
      "runtime-bundle-package-check": "runtime-package-asset-check",
      "html-runtime-package-check": "runtime-bundle-package-check",
      "plantuml-runtime-package-check": "html-runtime-package-check",
      "heavy-native": "plantuml-runtime-package-check",
    };
    return (
      Object.entries(chain).every(
        ([task, dependency]) => coverage[task]?.deps.join("\0") === dependency,
      ) && coverage["unit-test"]?.leaf === "root-test-and-coverage"
    );
  } catch {
    return false;
  }
}

function moonCacheFixtureBinding(
  context: ContractContext,
): OwnedValidationReport["moonCacheFixture"] {
  const inputsRegistered = moonCacheFixtureInputsRegistered(context);
  const fixtureSource = context.read(MOON_CACHE_FIXTURE);
  const ownerSource = context.read(MOON_CACHE_OWNER);
  const ownerAstSource = context.read(MOON_CACHE_OWNER_AST);
  const rootConfig = context.policy.moon?.config ?? "moon.yml";
  const rootSource = context.read(rootConfig);
  const ownerAst = inputsRegistered ? moonOwnerAstBinding(context.root, ownerSource) : undefined;
  const ownerAstVerified = ownerAst !== undefined;
  const rootDagVerified = moonRootDagVerified(
    context,
    ownerAst?.schedulerTasks ?? null,
    rootSource,
  );
  if (!rootDagVerified)
    add(
      context,
      "moon-cache-fixture-root-dag-unverified",
      context.policy.moon?.config ?? "moon.yml",
    );
  const ownerVerified = inputsRegistered && ownerAstVerified && rootDagVerified;
  const fixtureVerified = moonFixtureSourceVerified(fixtureSource);
  if (!ownerVerified)
    add(context, "moon-cache-fixture-owner-unverified", MOON_CACHE_FIXTURE, MOON_CACHE_OWNER);
  if (!fixtureVerified)
    add(context, "moon-cache-fixture-input-unverified", MOON_CACHE_FIXTURE, MOON_CACHE_OWNER);

  const ownerCommandPresent = moonOwnerCommandRegistered(context);
  if (!ownerCommandPresent)
    add(context, "moon-cache-fixture-owner-command-unverified", "Justfile", MOON_CACHE_RECIPE);
  const ownerCheckRecipe = moonCacheOwnerCheckRecipe(context);
  const localCheckRegistered =
    ownerVerified &&
    fixtureVerified &&
    ownerCommandPresent &&
    (context.localRecipes?.has(ownerCheckRecipe) ?? false);
  const ciCheckRegistered =
    ownerVerified &&
    fixtureVerified &&
    ownerCommandPresent &&
    (context.ciRecipes?.has(ownerCheckRecipe) ?? false);
  if (!localCheckRegistered)
    add(context, "moon-cache-fixture-owner-unregistered", MOON_CACHE_RECIPE, "local");
  if (!ciCheckRegistered)
    add(context, "moon-cache-fixture-owner-unregistered", MOON_CACHE_RECIPE, "CI");
  const binding: OwnedValidationReport["moonCacheFixture"] = {
    path: MOON_CACHE_FIXTURE,
    owner: MOON_CACHE_OWNER,
    ownerAstHelper: MOON_CACHE_OWNER_AST,
    ownerAstContract: "moon-owner-host-evidence-barrier-v3",
    fixtureSha256: sha(fixtureSource),
    ownerSha256: sha(ownerSource),
    ownerAstHelperSha256: sha(ownerAstSource),
    rootConfig,
    rootConfigSha256: sha(rootSource),
    fixtureVerified,
    ownerVerified,
    ownerAstVerified,
    rootDagVerified,
    localCheckRegistered,
    ciCheckRegistered,
  };
  context.moonCacheFixtureBinding = binding;
  return binding;
}

function maintainedSources(
  context: ContractContext,
  excludes: string[],
  all: string[],
  upstream: Map<string, string>,
  generated: Set<string>,
  report: OwnedValidationReport,
): string[] {
  return all.filter((file) => {
    const name = relative(context.root, file);
    if (upstream.has(file)) {
      report.verifiedAssets += 1;
      return false;
    }
    if (generated.has(file)) return false;
    if (
      !name
        .split("/")
        .slice(1)
        .some((part) => excludes.includes(part))
    )
      return true;
    if (checksumAsset(context, name)) {
      report.verifiedAssets += 1;
      return false;
    }
    add(context, "unbound-excluded-source", name);
    return true;
  });
}

function verifiedSvgEngineSources(context: ContractContext, all: string[]): Map<string, string> {
  const prefix = `${SVG_ENGINE_VENDOR_ROOT}/`;
  if (!all.some((file) => relative(context.root, file).startsWith(prefix))) return new Map();
  try {
    const paths = verifySvgEngineProvenance(context.root);
    const absolute = paths.map((name) => path.resolve(context.root, name));
    if (absolute.some((file) => !all.includes(file))) {
      add(context, "svg-engine-provenance-source-unregistered", SVG_ENGINE_VENDOR_ROOT);
      return new Map();
    }
    return new Map(absolute.map((file) => [file, "svg-engine-provenance"]));
  } catch (error) {
    add(
      context,
      "svg-engine-provenance-invalid",
      SVG_ENGINE_VENDOR_ROOT,
      error instanceof Error ? error.message : String(error),
    );
    return new Map();
  }
}

const GENERATED_RUST_OUTPUT_DIRS = [
  "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/",
  "crates/katana-render-runtime/src/markdown/generated/",
];

function generatedRustCandidates(visible: Set<string>): string[] {
  return [...visible]
    .filter(
      (name) =>
        GENERATED_RUST_OUTPUT_DIRS.some((directory) => name.startsWith(directory)) &&
        name.endsWith(".rs"),
    )
    .sort();
}

function recordGeneratedRustSet(
  context: ContractContext,
  paths: string[],
  candidates: string[],
  visible: Set<string>,
): void {
  const expected = new Set(paths);
  for (const name of paths) {
    if (!visible.has(name)) add(context, "generated-rust-output-missing", name);
    else if (!context.tracked.has(name)) add(context, "untracked-generated-rust-output", name);
  }
  for (const name of candidates)
    if (!expected.has(name)) add(context, "generated-rust-output-unexpected", name);
}

function generatedRustCheckRoute(context: ContractContext): { local: boolean; ci: boolean } {
  const recipe = "runtime-package-asset-check";
  const expectedBody = [
    'bun run scripts/runtime-assets/svg-engine-provenance.ts "{{REPO_ROOT}}"',
    "bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --check",
    '@package_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \\',
    'for kind_file in "mermaid/{{MERMAID_JS_VERSION}}/mermaid.min.js"; do \\',
    'if ! printf \'%s\\n\' "$package_files" | grep -qx "vendor/$kind_file.br"; then \\',
    'echo "missing compressed runtime package asset: vendor/$kind_file.br" >&2; \\',
    "exit 1; \\",
    "fi; \\",
    'if printf \'%s\\n\' "$package_files" | grep -qx "vendor/$kind_file"; then \\',
    'echo "raw runtime asset must not be included in the crates.io package: vendor/$kind_file" >&2; \\',
    "exit 1; \\",
    "fi; \\",
    "done",
    '@runtime_package_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \\',
    'asset_package_files="$({{CARGO}} package -p katana-render-runtime-assets --locked --allow-dirty --list)"; \\',
    'if ! printf \'%s\\n\' "$runtime_package_files" | grep -qx "vendor/drawio/{{DRAWIO_JS_VERSION}}/LICENSE"; then \\',
    'echo "missing Draw.io license from runtime crate package" >&2; exit 1; \\',
    "fi; \\",
    'if ! printf \'%s\\n\' "$asset_package_files" | grep -qx "assets/drawio.min.js.br"; then \\',
    'echo "missing compressed Draw.io asset package file" >&2; exit 1; \\',
    "fi; \\",
    'if ! printf \'%s\\n\' "$asset_package_files" | grep -qx "assets/licenses/drawio/LICENSE"; then \\',
    'echo "missing Draw.io license from assets crate package" >&2; exit 1; \\',
    "fi; \\",
    "if printf '%s\\n' \"$runtime_package_files\" | grep -Eq '^vendor/drawio/{{DRAWIO_JS_VERSION}}/drawio\\.min\\.js(\\.br)?$'; then \\",
    'echo "Draw.io source and compressed asset belong to the assets package" >&2; exit 1; \\',
    "fi",
  ];
  const actualBody = (context.recipes.get(recipe) ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const expectedCommand = [
    "bun",
    "run",
    "scripts/runtime-assets/svg-engine-provenance.ts",
    "{{REPO_ROOT}}",
  ];
  const checkCommandPresent =
    actualBody.length === expectedBody.length &&
    actualBody.every((line, index) => line === expectedBody[index]) &&
    shellWords(actualBody[0] ?? "").join("\0") === expectedCommand.join("\0");
  if (!checkCommandPresent)
    add(context, "generated-rust-check-command-unverified", "Justfile", recipe);
  const local = checkCommandPresent && (context.localRecipes?.has(recipe) ?? false);
  const ci = checkCommandPresent && (context.ciRecipes?.has(recipe) ?? false);
  if (!local) add(context, "generated-rust-check-unregistered", recipe, "local");
  if (!ci) add(context, "generated-rust-check-unregistered", recipe, "CI");
  return { local, ci };
}

function generatedRustFingerprint(
  context: ContractContext,
  existing: string[],
): { digest: string[]; inputCount: number; inputDigest: string } {
  const inputRecords = generatedRuntimeRustAssetInputPaths(context.root).map((name) => {
    if (!context.visible.has(name)) add(context, "generated-rust-input-missing", name);
    else if (!context.tracked.has(name)) add(context, "untracked-generated-rust-input", name);
    try {
      return `${name}\0${sha(fs.readFileSync(safeFile(context.root, name)))}\n`;
    } catch (error) {
      add(
        context,
        "unsafe-or-missing-source",
        name,
        error instanceof Error ? error.message : String(error),
      );
      return `${name}\0unsafe\n`;
    }
  });
  const inputDigest = sha(inputRecords.join(""));
  const digest = [
    "scripts/runtime-assets/runtime-package-asset-compressor.ts",
    "scripts/runtime-assets/runtime-asset-common.ts",
    "scripts/html/harness-contract-policy.json",
    "Justfile",
    context.policy.ciWorkflow,
    context.policy.localBridge.source,
    ...(context.policy.moon ? [context.policy.moon.config, context.policy.moon.workspace] : []),
  ].map((name) => {
    safeFile(context.root, name);
    return `${name}\0${sha(context.read(name))}\n`;
  });
  for (const name of existing) {
    try {
      digest.push(`${name}\0${sha(fs.readFileSync(safeFile(context.root, name)))}\n`);
    } catch (error) {
      add(
        context,
        "unsafe-or-missing-source",
        name,
        error instanceof Error ? error.message : String(error),
      );
    }
  }
  digest.push(`generated-rust-inputs\0${inputDigest}\n`);
  return { digest, inputCount: inputRecords.length, inputDigest };
}

function generatedRustContract(
  context: ContractContext,
  visible: Set<string>,
): {
  paths: string[];
  existing: string[];
  local: boolean;
  ci: boolean;
  digest: string[];
  inputCount: number;
  inputDigest: string;
} {
  const paths = generatedRuntimeRustAssetPaths(context.root);
  const candidates = generatedRustCandidates(visible);
  recordGeneratedRustSet(context, paths, candidates, visible);
  const route = generatedRustCheckRoute(context);
  const existing = paths.filter((name) => visible.has(name));
  const fingerprint = generatedRustFingerprint(context, existing);
  return { paths, existing, ...route, ...fingerprint };
}

function recordEnvironment(report: OwnedValidationReport, environment: string | undefined): void {
  if (environment)
    report.javascriptEnvironments[environment] =
      (report.javascriptEnvironments[environment] ?? 0) + 1;
}

function recordScriptUnits(
  units: OwnedValidationReport["units"],
  name: string,
  source: string,
): void {
  if (hasBunTests(source)) units.bun.push(name);
  if (hasPlaywrightTests(source)) units.playwright.push(name);
}
function inspectSources(
  context: ContractContext,
  files: string[],
  members: string[],
  report: OwnedValidationReport,
  biome: z.infer<typeof biomeContract>,
  types: TypeScope[],
): string[] {
  const digest: string[] = [];
  for (const file of files) {
    const name = relative(context.root, file);
    const language = OWNED_SOURCE_FORMATS[path.extname(name)];
    if (!language) throw new Error(`unsupported owned source: ${name}`);
    report.languageCounts[language] += 1;
    if (!context.tracked.has(name)) add(context, "untracked-source", name);
    const source = context.read(name);
    digest.push(`${name}\0${sha(source)}\n`);
    if (language === "rust") rustStatic(context, name, members);
    else if (language === "python") pythonStatic(context, name);
    else {
      const environment = scriptStatic(
        context,
        name,
        language,
        source,
        biome,
        types,
        report.javascriptTypeCoverage,
      );
      recordEnvironment(report, environment);
      recordScriptUnits(report.units, name, source);
    }
  }
  return digest;
}

function validateOwnedRepositorySources(
  context: ContractContext,
  root: string,
  policy: z.infer<typeof policySchema>,
  tracked: Set<string>,
  visible: Set<string>,
  report: OwnedValidationReport,
): { files: string[]; generated: ReturnType<typeof generatedRustContract> } | undefined {
  const raw = [...new Set([...context.index.tracked, ...context.index.untracked])]
    .filter((name) => OWNED_SOURCE_FORMATS[path.extname(name)])
    .sort();
  if (!sourceSafety(context, raw)) return undefined;
  for (const name of raw) {
    if (!policy.roots.includes(name.split("/")[0] ?? ""))
      add(context, "unclassified-source-root", name);
  }
  const all = collectOwnedSourcePaths(root, { ...policy, excludes: [] });
  const upstream = verifiedThirdPartyJavascript(
    root,
    all,
    policy.assetManifest,
    policy.assetResourcesRoot,
    policy.assetSourceKinds,
  );
  for (const [file, evidence] of verifiedSvgEngineSources(context, all))
    upstream.set(file, evidence);
  if (!tracked.has(policy.assetManifest)) add(context, "untracked-authority", policy.assetManifest);
  const generated = generatedRustContract(context, visible);
  report.generatedRustAssets = {
    paths: generated.paths,
    inputCount: generated.inputCount,
    inputDigest: generated.inputDigest,
    byteVerification: "not-executed",
    requiredCheck: "runtime-package-asset-check",
    localCheckRegistered: generated.local,
    ciCheckRegistered: generated.ci,
  };
  report.moonCacheFixture = moonCacheFixtureBinding(context);
  const generatedAbsolutePaths = new Set(generated.paths.map((name) => path.resolve(root, name)));
  return {
    files: maintainedSources(
      context,
      policy.excludes,
      all,
      upstream,
      generatedAbsolutePaths,
      report,
    ),
    generated,
  };
}

export function inspectOwnedValidationContract(directory: string): OwnedValidationReport {
  const root = fs.realpathSync(directory);
  const index = readOwnedRepositoryIndex(root);
  const tracked = new Set(index.tracked);
  const visible = new Set([...index.tracked, ...index.untracked]);
  const { context, pkg, biome, verifyExternalTools } = metadata(root, index, tracked, visible);
  const policy = policySchema.parse(json(root, "scripts/quality/duplication-policy.json"));
  const report: OwnedValidationReport = {
    schema: 1,
    staticContractOnly: true,
    sourceCount: 0,
    sourceDigest: "",
    languageCounts: { rust: 0, typescript: 0, javascript: 0, python: 0 },
    units: { bun: [], python: [], playwright: [] },
    javascriptEnvironments: {},
    javascriptTypeCoverage: {
      scope: "validation-tests-and-established-typed-product-boundaries",
      verification: "not-executed",
      wholeJavaScriptStrict: "not-claimed",
      mandatoryPaths: [],
      profileMatchedPaths: [],
      generatedBoundPaths: [],
      deferredProductPaths: [],
    },
    browserReferenceHelpers: {
      presentPaths: [],
      consumerSources: [],
      consumerVerified: false,
      typeProfile: BROWSER_REFERENCE_PROFILE,
      typeProfileVerified: false,
      compiledOwnedSources: [],
      compilerLibraryCount: 0,
      closureVerified: false,
      inputDigest: null,
    },
    nativeWidthTrace: {
      path: NATIVE_WIDTH_TRACE,
      present: false,
      consumerSources: [],
      consumerVerified: false,
      typeProfile: NATIVE_WIDTH_TYPE_PROFILE,
      typeProfileVerified: false,
      hostDeclaration: NATIVE_WIDTH_HOST,
    },
    verifiedAssets: 0,
    generatedRustAssets: {
      paths: [],
      inputCount: 0,
      inputDigest: sha(""),
      byteVerification: "not-executed",
      requiredCheck: "runtime-package-asset-check",
      localCheckRegistered: false,
      ciCheckRegistered: false,
    },
    moonCacheFixture: {
      path: MOON_CACHE_FIXTURE,
      owner: MOON_CACHE_OWNER,
      ownerAstHelper: MOON_CACHE_OWNER_AST,
      ownerAstContract: "moon-owner-host-evidence-barrier-v3",
      fixtureSha256: sha(""),
      ownerSha256: sha(""),
      ownerAstHelperSha256: sha(""),
      rootConfig: "moon.yml",
      rootConfigSha256: sha(""),
      fixtureVerified: false,
      ownerVerified: false,
      ownerAstVerified: false,
      rootDagVerified: false,
      localCheckRegistered: false,
      ciCheckRegistered: false,
    },
    parserBodyOnload: {
      path: PARSER_BODY_OUTPUT,
      canonical: PARSER_BODY_CANONICAL,
      hostDeclaration: PARSER_BODY_HOST,
      producer: PARSER_BODY_PRODUCER,
      unitTest: PARSER_BODY_TEST,
      inputPaths: [
        PARSER_BODY_CANONICAL,
        PARSER_BODY_HOST,
        PARSER_BODY_PRODUCER,
        PARSER_BODY_TEST,
        PARSER_BODY_OUTPUT,
      ],
      inputDigest: sha(""),
      outputSha256: sha(""),
      producerVerified: false,
      biomeIncluded: false,
      canonicalTypeScope: false,
      producerTypeScope: false,
      outputTypeBoundary: "generated-from-strict-typescript",
      localCheckRegistered: false,
      ciCheckRegistered: false,
      localTestRegistered: false,
      ciTestRegistered: false,
      byteVerification: "not-executed",
    },
    mathJaxUnusedAudit: emptyMathJaxUnusedAuditBinding(),
    nativeWidthHashFacts: {
      sourcePath: NATIVE_WIDTH_HASH_SOURCE,
      testPath: NATIVE_WIDTH_HASH_TEST,
      factRecipe: NATIVE_WIDTH_HASH_RECIPE,
      auditRecipe: MATHJAX_UNUSED_AUDIT_RECIPE,
      inputPaths: [...NATIVE_WIDTH_HASH_INPUTS],
      inputDigest: sha(""),
      factRecipeVerified: false,
      testCommandVerified: false,
      localCheckRegistered: false,
      ciCheckRegistered: false,
      nativeExecutionVerified: false,
      performanceVerified: false,
    },
    issues: context.issues,
    limitations: [
      "Rust registration means workspace member association or the bounded standalone performance harness manifest and two arm recipes; compiler module reachability is not inferred from filenames.",
      "Standalone harness metadata checks tracked inputs, arm lock association and fixed format/lint/coverage routes; Cargo lock consistency, compilation and each arm100percent coverage are not executed.",
      "JS environment labels describe current sources; matching a type project does not prove native host ABI fidelity.",
      "Strict JS scope covers validation/tests and established typed product boundaries; deferred legacy product paths remain unverified, not a whole-JavaScript strict success.",
      "The native-width trace is mandatory Rust-test infrastructure; its exact reviewed cfg/test/include/result consumer sources are bound, without inferring other Rust module reachability.",
      "TypeScript metadata accepts repository JSON configs and relative extends only; JSONC/package configs require an explicit standard-parser boundary before admission.",
      "This contract inventories maintained Git-visible sources, not ignored active config closure or historical tmp code.",
      "No standard checks, units, rendering, performance or CI are executed by this metadata contract.",
      "Generated Rust asset paths are associated with their producer and required byte check; byte equivalence itself is not executed by this metadata contract.",
      "Moon owner verification checks a bounded Python AST contract for main entry, required calls and early returns; it does not prove external Moon, Bun, filesystem or subprocess behavior.",
      "Moon fixture registration also binds its exact owner source, AST helper bytes and unconditional local/CI recipe; this metadata contract does not launch Moon or execute the fixture.",
      "The parser body-onload JavaScript output is bound to one canonical TypeScript producer, strict source scopes and local/CI check/test routes; this metadata contract does not regenerate or byte-verify it.",
      "The NativeWidth hash route binds its Rust input inventory, guarded facts recipe and focused test to the MathJax audit lanes; metadata does not execute the native route or verify performance.",
    ],
  };
  const sources = validateOwnedRepositorySources(context, root, policy, tracked, visible, report);
  if (!sources) {
    verifyExternalTools();
    return report;
  }
  const { files, generated } = sources;
  staleIncludes(context, "biome.json", biome.files.includes);
  const types = typeScopes(context, pkg);
  report.parserBodyOnload = parserBodyOnloadBinding(context, biome, types);
  report.mathJaxUnusedAudit = mathJaxUnusedAuditBinding(context);
  report.nativeWidthHashFacts = nativeWidthHashFactsBinding(context);
  report.nativeWidthTrace = nativeWidthTraceBinding(context, types);
  report.browserReferenceHelpers = browserReferenceBinding(context, types);
  if (!registered(context, "biome")) add(context, "static-recipe-unregistered", "biome");
  if (
    shellWords(context.recipes.get("biome") ?? "").join("\0") !== "bun\0run\0biome" &&
    !biomeBootstrapDispatchVerified(context.recipes)
  )
    add(context, "biome-dispatch-unverified", "Justfile");
  const lintWords = shellWords(pkg.scripts.biome ?? "");
  if (
    lintWords[0] !== "biome" ||
    lintWords[1] !== "check" ||
    lintWords.slice(2).some((word) => ![".", "--error-on-warnings"].includes(word))
  )
    add(context, "biome-command-unverified", "package.json");
  const members = rustMembers(context);
  const digest = performanceHarnessContract(context, members, report);
  digest.push(...inspectSources(context, files, members, report, biome, types));
  digest.push(...generated.digest);
  digest.push(
    ...report.nativeWidthTrace.consumerSources.map((row) => `${row.path}\0${row.sha256}\n`),
  );
  if (report.browserReferenceHelpers.inputDigest)
    digest.push(`${BROWSER_REFERENCE_PROFILE}\0${report.browserReferenceHelpers.inputDigest}\n`);
  digest.push(
    ...report.browserReferenceHelpers.consumerSources.map((row) => `${row.path}\0${row.sha256}\n`),
  );
  if (context.mathJaxInputDigest)
    digest.push(`${MATHJAX_TYPE_PROFILE}\0${context.mathJaxInputDigest}\n`);
  digest.push(
    `${MOON_CACHE_FIXTURE}\0${report.moonCacheFixture.fixtureSha256}\n`,
    `${MOON_CACHE_OWNER}\0${report.moonCacheFixture.ownerSha256}\n`,
    `${report.moonCacheFixture.rootConfig}\0${report.moonCacheFixture.rootConfigSha256}\n`,
    `${PARSER_BODY_OUTPUT}\0${report.parserBodyOnload.outputSha256}\n`,
    `${PARSER_BODY_OUTPUT}.producer\0${report.parserBodyOnload.inputDigest}\n`,
  );
  digest.push(
    ...tomliRuffAuthority(
      context,
      files.filter((file) => path.extname(file) === ".py").map((file) => relative(root, file)),
    ),
  );
  report.units.python = pythonUnits(
    context,
    files.filter((file) => file.endsWith(".py")).map((file) => relative(root, file)),
  );
  playwrightIssues(context);
  unitIssues(context, report.units);
  report.sourceCount = files.length;
  report.sourceDigest = sha(digest.join(""));
  verifyExternalTools();
  return report;
}

function routeMetadata(directory: string) {
  const root = fs.realpathSync(directory);
  const index = readOwnedRepositoryIndex(root);
  const tracked = new Set(index.tracked);
  const visible = new Set([...index.tracked, ...index.untracked]);
  return metadata(root, index, tracked, visible);
}

export function inspectNativeWidthHashRoute(directory: string): {
  facts: OwnedValidationReport["nativeWidthHashFacts"];
  issues: OwnedValidationIssue[];
} {
  const { context, verifyExternalTools } = routeMetadata(directory);
  const facts = nativeWidthHashFactsBinding(context);
  verifyExternalTools();
  return { facts, issues: context.issues };
}

export function inspectStandalonePerformanceRoute(directory: string): {
  binding: OwnedValidationReport["standalonePerformanceHarness"] | undefined;
  issues: OwnedValidationIssue[];
} {
  const { context, verifyExternalTools } = routeMetadata(directory);
  const report: Pick<OwnedValidationReport, "standalonePerformanceHarness"> = {};
  performanceHarnessContract(context, rustMembers(context), report);
  verifyExternalTools();
  return { binding: report.standalonePerformanceHarness, issues: context.issues };
}

export function inspectPlaywrightRoute(directory: string): {
  units: string[];
  localRegistered: boolean;
  ciRegistered: boolean;
  macosLocalRegistered: boolean;
  macosCiRegistered: boolean;
  issues: OwnedValidationIssue[];
} {
  const { context, verifyExternalTools } = routeMetadata(directory);
  const units = [...PLAYWRIGHT_TESTS, PLAYWRIGHT_MACOS_PRODUCER_TEST].filter(
    (name) =>
      context.visible.has(name) &&
      fs.existsSync(path.join(context.root, name)) &&
      hasPlaywrightTests(context.read(name)),
  );
  playwrightIssues(context);
  unitIssues(context, { bun: [], python: [], playwright: units });
  const localRegistered = playwrightRegistered(context, context.local);
  const ciRegistered = playwrightRegistered(context, context.ci);
  const macosLocalRegistered = playwrightMacosRegistered(context, context.local);
  const macosCiRegistered = playwrightMacosRegistered(context, context.ci);
  verifyExternalTools();
  return {
    units,
    localRegistered,
    ciRegistered,
    macosLocalRegistered,
    macosCiRegistered,
    issues: context.issues,
  };
}

export function inspectGeneratedRustCheckRoute(directory: string): {
  local: boolean;
  ci: boolean;
  issues: OwnedValidationIssue[];
} {
  const { context, verifyExternalTools } = routeMetadata(directory);
  const route = generatedRustCheckRoute(context);
  verifyExternalTools();
  return { ...route, issues: context.issues };
}

export function inspectMathJaxUnusedAuditRoute(directory: string): {
  binding: OwnedValidationReport["mathJaxUnusedAudit"];
  localTestRegistered: boolean;
  ciTestRegistered: boolean;
  issues: OwnedValidationIssue[];
} {
  const { context, verifyExternalTools } = routeMetadata(directory);
  const binding = mathJaxUnusedAuditBinding(context);
  const localTestRegistered = unitRegistered(context, binding.testPath, "bun", context.local);
  const ciTestRegistered = unitRegistered(context, binding.testPath, "bun", context.ci);
  verifyExternalTools();
  return { binding, localTestRegistered, ciTestRegistered, issues: context.issues };
}

export function inspectMoonCacheFixtureRoute(directory: string): {
  binding: OwnedValidationReport["moonCacheFixture"];
  bunUnitRegistered: boolean;
  issues: OwnedValidationIssue[];
} {
  const { context, verifyExternalTools } = routeMetadata(directory);
  const binding = moonCacheFixtureBinding(context);
  context.moonCacheFixtureBinding = binding;
  const bunUnitRegistered =
    hasBunTests(context.read(binding.path)) &&
    unitRegistered(context, binding.path, "bun", context.local) &&
    unitRegistered(context, binding.path, "bun", context.ci);
  verifyExternalTools();
  return { binding, bunUnitRegistered, issues: context.issues };
}

export function inspectParserBodyOnloadRoute(directory: string): {
  binding: OwnedValidationReport["parserBodyOnload"];
  issues: OwnedValidationIssue[];
} {
  const { context, pkg, biome, verifyExternalTools } = routeMetadata(directory);
  const binding = parserBodyOnloadBinding(context, biome, parserBodyOnloadTypeScopes(context, pkg));
  verifyExternalTools();
  return { binding, issues: context.issues };
}

if (import.meta.main) {
  try {
    const report = inspectOwnedValidationContract(process.cwd());
    console.log(JSON.stringify(report, null, 2));
    if (report.issues.length > 0) process.exitCode = 1;
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
