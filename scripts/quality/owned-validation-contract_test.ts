import { afterAll, afterEach, beforeAll, describe, expect, spyOn, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { z } from "zod";
import {
  type ExternalToolRequest,
  justRecipes,
  RUNTIME_UNUSED_REGRESSION_TESTS,
  shellWords,
  sourceCode,
} from "../html/harness-contract-graph";
import { resolveOfficialRendererAssetInputs } from "../mermaid/official-renderer-asset-inputs";
import {
  generatedRuntimeRustAssetInputPaths,
  generatedRuntimeRustAssetPaths,
} from "../runtime-assets/runtime-package-asset-compressor";
import {
  MATHJAX_ABI,
  MATHJAX_DECL_PROFILE,
  MATHJAX_PROVENANCE,
  MATHJAX_RUNTIME,
  MATHJAX_TYPE_PROFILE,
} from "./mathjax-declaration-contract";
import { readOwnedRepositoryIndex } from "./owned-source-inventory";
import {
  javaScriptTypeReader,
  queryTypeProject,
  strictJavaScriptProject,
  typeProjectNames,
  typeScriptToolBinding,
} from "./owned-type-projects";
import {
  inspectGeneratedRustCheckRoute,
  inspectMathJaxUnusedAuditRoute,
  inspectMoonCacheFixtureRoute,
  inspectNativeWidthHashRoute,
  inspectOwnedValidationContract,
  inspectParserBodyOnloadRoute,
  inspectPlaywrightRoute,
  inspectStandalonePerformanceRoute,
  MATHJAX_UNUSED_INPUTS,
  NATIVE_WIDTH_HASH_INPUTS,
} from "./owned-validation-contract";

// WHY: 実リポジトリ全体の契約確認は、軽量fixtureとは別の実測に基づく上限で実行する。
const REPOSITORY_GRAPH_TEST_TIMEOUT_MS = 15_000;

const fixtures: string[] = [];
let fixtureTemplate: string | undefined;
const duplication = JSON.parse(
  fs.readFileSync(path.join(import.meta.dir, "duplication-policy.json"), "utf8"),
);
const sources = {
  "scripts/local.py": '# Lane("assets", "check-standard")\n',
  "scripts/tool.ts": "export const value = 1;\n",
  "scripts/runtime-assets/runtime-asset-common.ts": "export const catalogFixture = true;\n",
  "scripts/runtime-assets/runtime-package-asset-compressor.ts": 'export const mode = "--check";\n',
  "scripts/tool_test.ts": 'import { test } from "bun:test"; test("value", () => {});\n',
  "scripts/helper.js": "export const helper = 1;\n",
  "scripts/tool_test.py":
    "import unittest\nclass TestValue(unittest.TestCase):\n    def test_value(self):\n        self.assertEqual(1, 1)\n",
  "scripts/build/cargo_capacity_guard.py": "# fixture guard source\n",
  "scripts/runtime-bundles/bundle-runtime.ts":
    'export const fixtureProducerMode = "--classic-generation-evidence";\n',
  "crates/demo/src/lib.rs": "mod deep;\n",
  "crates/demo/src/deep/regular.rs": "pub const VALUE: u32 = 1;\n",
};
const recipeBodies: Record<string, string> = {
  check: "python3 scripts/local.py",
  "check-standard":
    "just biome\njust typecheck\njust python-lint\njust python-typecheck\njust lint\njust unit\njust runtime-package-asset-check\njust runtime-asset-script-test",
  biome: "bun run biome",
  typecheck: "bun run typecheck",
  "python-lint": "uvx --from ruff==0.16.10 ruff check --select F,E4,E7,E9,B,UP scripts",
  "python-typecheck": "uvx --from mypy==2.4.0 mypy --strict scripts",
  lint: 'RUSTFLAGS="-D warnings" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" python3 scripts/build/cargo_capacity_guard.py --check-domain root -- {{CARGO}} clippy -j {{JOBS}} --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::todo -D clippy::unimplemented -D clippy::dbg_macro -D clippy::panic -D clippy::wildcard_imports -D clippy::too_many_lines -D clippy::cognitive_complexity',
  unit: "bun test --path-ignore-patterns 'tmp/**' ./scripts\npython3 -m unittest discover -s scripts -p '*_test.py'",
  "runtime-asset-script-test":
    "python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon",
  "runtime-package-asset-check": [
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
  ].join("\n"),
};
const fixtureJustDefinitions = [
  'set shell := ["bash", "-uc"]',
  "REPO_ROOT := justfile_directory()",
  'RTK := env_var_or_default("RTK", `command -v rtk 2> /dev/null || true`)',
  'RTK_CMD := if RTK == "" { "" } else { RTK + " " }',
  'CARGO := env_var_or_default("CARGO", RTK_CMD + "cargo")',
  'CARGO_BUILD := "python3 " + quote(REPO_ROOT + "/scripts/build/cargo_capacity_guard.py") + " --retain-build-profile debug -- " + CARGO',
].join("\n");
const cargoCapacityGuardPath = "scripts/build/cargo_capacity_guard.py";
const runtimeAssetUnitFixtureSource =
  'import { test } from "bun:test";\ntest("canonical recipe fixture source", () => {});\n';
const playwrightGeometryTestPath = "scripts/mermaid/reference_svg_geometry_browser_test.ts";
const playwrightUnitPaths = [
  "scripts/html/feature-catalog_test.ts",
  "scripts/html/chrome_badge_metrics_test.ts",
  "scripts/html/reference-capture_browser_test.ts",
  playwrightGeometryTestPath,
] as const;
const playwrightConfigPath = "scripts/html/playwright.config.ts";
const playwrightFixtureSource =
  'import { test } from "playwright/test";\ntest("canonical browser fixture", () => {});\n';

function cleanupFixtures(): void {
  for (const root of fixtures.splice(0)) fs.rmSync(root, { recursive: true, force: true });
}
afterEach(cleanupFixtures);
beforeAll(() => {
  try {
    fixtureTemplate = buildFixture();
    const templateIndex = fixtures.indexOf(fixtureTemplate);
    if (templateIndex < 0) throw new Error("fixture template was not registered");
    fixtures.splice(templateIndex, 1);
  } catch (error) {
    cleanupFixtures();
    throw error;
  }
});
afterAll(() => {
  cleanupFixtures();
  if (fixtureTemplate !== undefined) fs.rmSync(fixtureTemplate, { recursive: true, force: true });
});
function write(root: string, name: string, value: string): void {
  const file = path.join(root, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, value);
}
function writeJson(root: string, name: string, value: unknown): void {
  write(root, name, `${JSON.stringify(value)}\n`);
}
function updateFixtureTypecheck(root: string, update: (script: string) => string): void {
  const packagePath = path.join(root, "package.json");
  const pkg = JSON.parse(fs.readFileSync(packagePath, "utf8")) as {
    scripts: { typecheck: string };
  };
  pkg.scripts.typecheck = update(pkg.scripts.typecheck);
  writeJson(root, "package.json", pkg);
}
function recipes(root: string, overrides: Record<string, string> = {}): void {
  const bodies = { ...fixtureRecipeBodies(), ...overrides };
  const strictRuntimeFixture =
    fs.existsSync(path.join(root, "scripts/runtime-bundles/runtime-rust-facts-host.ts")) ||
    fs.existsSync(path.join(root, "scripts/runtime-bundles/runtime-unused-check.ts"));
  bodies["runtime-asset-script-other-run"] =
    overrides["runtime-asset-script-other-run"] ??
    overrides["runtime-asset-script-test"] ??
    bodies["runtime-asset-script-other-run"] ??
    "";
  if (strictRuntimeFixture) {
    bodies["check-standard"] =
      `${bodies["check-standard"] ?? ""}\njust rust-facts-host-prebuild\njust mathjax-owned-evidence`;
  } else {
    const body = bodies["runtime-asset-script-other-run"] ?? "";
    bodies["runtime-asset-script-other-run"] = body
      .split("\n")
      .filter(
        (line) =>
          !line.startsWith("bun scripts/runtime-bundles/runtime-rust-facts-host.ts verify ") &&
          line !==
            "NODE_ENV=test bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts verify",
      )
      .join("\n");
  }
  bodies["runtime-asset-script-test"] = strictRuntimeFixture
    ? "bun run moon exec krr:runtime-asset-script-test"
    : "just runtime-asset-script-other-run\njust runtime-unused-script-regression-run";
  write(
    root,
    "Justfile",
    `${fixtureJustDefinitions}\n\n${Object.entries(bodies)
      .map(
        ([name, body]) =>
          `${["native-width-hash-facts", "native-width-classic-facts", "mermaid-classic-script-facts"].includes(name) ? `${name} root=REPO_ROOT source_root=REPO_ROOT:` : ["mathjax-consumer-facts", "rust-facts-host-build", "rust-facts-host-metadata"].includes(name) ? `${name} root=REPO_ROOT:` : `${name}:`}\n${body
            .split("\n")
            .map((line) => `    ${line}`)
            .join("\n")}\n`,
      )
      .join("\n")}`,
  );
}
function actualRecipeBody(name: string, preserveSilentPrefix = false): string {
  const actualRoot = path.resolve(import.meta.dir, "../..");
  const actualRecipes = justRecipes(fs.readFileSync(path.join(actualRoot, "Justfile"), "utf8"));
  return (
    actualRecipes.get(
      name === "runtime-asset-script-test" || name === "runtime-asset-script-test-run"
        ? "runtime-asset-script-other-run"
        : name,
    ) ?? ""
  )
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => (preserveSilentPrefix ? line : line.replace(/^@/, "")))
    .join("\n");
}
const nativeWidthHashHeader = "native-width-hash-facts root=REPO_ROOT source_root=REPO_ROOT:";
const mathJaxFactsRecipe =
  '@CARGO_TARGET_DIR={{quote(root + "/target")}} bun {{quote(root + "/scripts/runtime-bundles/runtime-rust-facts-host.ts")}} run {{quote(root)}} mathjax {{quote(root)}} {{quote(root + "/tmp/quality/rust-facts-host/current/receipt.json")}}';

function actualRuntimeAssetTestInvocations(): string[][] {
  return actualRecipeBody("runtime-asset-script-other-run")
    .split("\n")
    .flatMap((line): string[][] => {
      const words = shellWords(line);
      const bun = words.indexOf("bun");
      if (bun < 0 || words[bun + 1] !== "test") return [];
      return [
        words
          .filter((word) => /\.(?:tsx?|jsx?)$/.test(word))
          .map((word) => word.replace(/^\.\//, "")),
      ];
    });
}
function actualRuntimeAssetUnitPaths(): string[] {
  const paths = [
    ...actualRuntimeAssetTestInvocations().flat(),
    ...actualRecipeBody("runtime-unused-script-regression-run")
      .split("\n")
      .flatMap((line) => {
        const words = shellWords(line);
        const bun = words.indexOf("bun");
        return bun < 0 || words[bun + 1] !== "test"
          ? []
          : words
              .filter((word) => /\.(?:tsx?|jsx?)$/.test(word))
              .map((word) => word.replace(/^\.\//, ""));
      }),
  ];
  return [...new Set(paths)].sort();
}
function fixtureRecipeBodies(): Record<string, string> {
  const runtimeAssetScriptTest = actualRecipeBody("runtime-asset-script-other-run")
    .split("\n")
    .filter((line) => !line.startsWith("bun scripts/runtime-bundles/runtime-unused-check.ts "))
    .join("\n");
  const runtimeUnusedLeafIds = [
    "runtime-unused-fragment-loader",
    "runtime-unused-bundle-definitions",
    "runtime-unused-check",
    "runtime-unused-tool-fingerprint",
    "runtime-unused-mathjax",
    "runtime-unused-native-width",
    "runtime-unused-rust-classic",
  ];
  const runtimeUnusedLeafRecipes = Object.fromEntries(
    runtimeUnusedLeafIds.map((id, index) => [
      `${id}-run`,
      `bun test --path-ignore-patterns 'tmp/**' ./${RUNTIME_UNUSED_REGRESSION_TESTS[index]}`,
    ]),
  );
  return {
    ...recipeBodies,
    "check-standard": `${recipeBodies["check-standard"] ?? ""}\njust runtime-bundle-check`,
    "runtime-asset-script-test": runtimeAssetScriptTest,
    "runtime-asset-script-aggregate-run": actualRecipeBody("runtime-asset-script-aggregate-run"),
    "runtime-asset-script-preflight-run": actualRecipeBody("runtime-asset-script-preflight-run"),
    "runtime-classic-generation-evidence": actualRecipeBody("runtime-classic-generation-evidence"),
    "runtime-asset-script-other-run": runtimeAssetScriptTest,
    "runtime-unused-script-regression": actualRecipeBody("runtime-unused-script-regression"),
    "runtime-unused-script-producer-run": actualRecipeBody("runtime-unused-script-producer-run"),
    "runtime-unused-script-verify-run": actualRecipeBody("runtime-unused-script-verify-run"),
    "runtime-unused-script-cache-key": actualRecipeBody("runtime-unused-script-cache-key"),
    "runtime-unused-script-final-verify-run": actualRecipeBody(
      "runtime-unused-script-final-verify-run",
    ),
    "runtime-unused-script-regression-run": actualRecipeBody(
      "runtime-unused-script-regression-run",
    ),
    ...runtimeUnusedLeafRecipes,
    "runtime-bundle-check": actualRecipeBody("runtime-bundle-check"),
    "rust-facts-host-build": actualRecipeBody("rust-facts-host-build", true),
    "rust-facts-host-metadata": actualRecipeBody("rust-facts-host-metadata", true),
    "rust-facts-host-prebuild": actualRecipeBody("rust-facts-host-prebuild"),
    "mathjax-owned-evidence": actualRecipeBody("mathjax-owned-evidence"),
    "browser-contract-test": actualRecipeBody("browser-contract-test"),
  };
}
function writeRuntimeAssetUnitFixtures(root: string): void {
  const actualRoot = path.resolve(import.meta.dir, "../..");
  const duplicationSources = new Set([
    "scripts/quality/duplication-check_test.ts",
    "scripts/quality/duplication-check.ts",
    "scripts/quality/duplication-schema.ts",
    "scripts/quality/owned-source-inventory.ts",
  ]);
  for (const name of actualRuntimeAssetUnitPaths())
    write(
      root,
      name,
      duplicationSources.has(name)
        ? fs.readFileSync(path.join(actualRoot, name), "utf8")
        : runtimeAssetUnitFixtureSource,
    );
  for (const name of [...duplicationSources].filter(
    (name) => !fs.existsSync(path.join(root, name)),
  ))
    write(root, name, fs.readFileSync(path.join(actualRoot, name), "utf8"));
  for (const name of playwrightUnitPaths)
    write(
      root,
      name,
      name === playwrightGeometryTestPath
        ? fs.readFileSync(path.join(actualRoot, name), "utf8")
        : playwrightFixtureSource,
    );
  write(
    root,
    playwrightConfigPath,
    fs.readFileSync(path.join(actualRoot, playwrightConfigPath), "utf8"),
  );
}
function writeCurrentMoonFixtureInputs(root: string): void {
  const actualRoot = path.resolve(import.meta.dir, "../..");
  for (const name of [
    "scripts/quality/moon_cache_contract_test.py",
    "scripts/quality/owned_python_owner_contract.py",
    "tests/fixtures/quality-cache/moon-cache-subject.test.ts",
    "tests/fixtures/quality-cache/moon-cache-subject.ts",
    "moon.yml",
    "scripts/release/native_coverage_result_cli.py",
    "scripts/release/native_coverage_local_aggregate.py",
  ])
    write(root, name, fs.readFileSync(path.join(actualRoot, name), "utf8"));
}
function writeParserBindingFixture(root: string): void {
  const directory = "crates/katana-render-runtime/src/renderer/backends/html_runtime";
  const canonical = `${directory}/parser_body_onload.ts`;
  const output = `${directory}/parser_body_onload.js`;
  const host = `${directory}/host_parser_onload.d.ts`;
  const producer = "scripts/runtime-bundles/parser-body-onload-generation.ts";
  const unitTest = "scripts/runtime-bundles/parser-body-onload-generation.test.ts";
  const domBootstrap = `${directory}/dom_bootstrap_test.js`;
  const actualRoot = path.resolve(import.meta.dir, "../..");
  for (const name of [canonical, output, host, producer, unitTest, domBootstrap])
    write(root, name, fs.readFileSync(path.join(actualRoot, name), "utf8"));
  const options = {
    target: "ES2024",
    module: "ESNext",
    moduleResolution: "Bundler",
    lib: ["ES2024"],
    types: [],
    strict: true,
    noImplicitAny: true,
    strictNullChecks: true,
    noUncheckedIndexedAccess: true,
    exactOptionalPropertyTypes: true,
    useUnknownInCatchVariables: true,
    noEmit: true,
    noUnusedLocals: true,
    noUnusedParameters: true,
  };
  writeJson(root, "scripts/quality/tsconfig-owned-native-html-parser.json", {
    compilerOptions: options,
    files: [`../../${canonical}`, `../../${host}`],
  });
  writeJson(root, "scripts/quality/tsconfig-owned-native-html-parser-tool.json", {
    compilerOptions: { ...options, allowJs: true, checkJs: true },
    files: [
      producer.replace("scripts/", "../"),
      unitTest.replace("scripts/", "../"),
      "../../crates/katana-render-runtime/src/renderer/backends/html_runtime/dom_bootstrap_test.js",
    ],
  });
  writeJson(root, "package.json", {
    scripts: {
      biome: "biome check --error-on-warnings",
      typecheck:
        "tsc --noEmit && tsc -p scripts/quality/tsconfig-owned-native-html-parser.json --noEmit && tsc -p scripts/quality/tsconfig-owned-native-html-parser-tool.json --noEmit",
    },
    devDependencies: {
      "@swc/core": "1.13.5",
      typescript: "7.0.2",
      biome: "2.4.8",
      zod: "4.6.7",
      playwright: "1.64.1",
    },
  });
  writeJson(root, "tsconfig.json", {
    compilerOptions: {
      strict: true,
      noUnusedLocals: true,
      noUnusedParameters: true,
      useUnknownInCatchVariables: true,
      lib: ["es2024"],
      allowJs: true,
      checkJs: true,
    },
    include: [
      "scripts/**/*.ts",
      "scripts/**/*.js",
      "crates/**/*.js",
      "tests/fixtures/quality-cache/*.ts",
    ],
    exclude: [
      "crates/katana-render-runtime/src/renderer/backends/html_runtime/parser_body_onload.js",
    ],
  });
  writeJson(root, "biome.json", {
    files: {
      includes: [
        "scripts/**/*.ts",
        "scripts/**/*.js",
        "tests/fixtures/quality-cache/*.ts",
        "crates/katana-render-runtime/src/renderer/backends/html_runtime/**/*.js",
        "crates/katana-render-runtime/src/renderer/backends/html_runtime/parser_body_onload.ts",
        "crates/katana-render-runtime/src/renderer/backends/html_runtime/host_parser_onload.d.ts",
      ],
    },
  });
  recipes(root, {
    "check-standard": fixtureRecipeBodies()["check-standard"] ?? "",
  });
}
function track(root: string): void {
  execFileSync("git", ["add", "--", "."], { cwd: root });
}
function nativeWidthHashFixture(): string {
  const root = fixture();
  const actualRoot = path.resolve(import.meta.dir, "../..");
  for (const name of NATIVE_WIDTH_HASH_INPUTS)
    write(root, name, fs.readFileSync(path.join(actualRoot, name), "utf8"));
  const classicFactsTest = "scripts/runtime-bundles/runtime-unused-rust-classic_test.ts";
  write(root, classicFactsTest, fs.readFileSync(path.join(actualRoot, classicFactsTest), "utf8"));
  write(
    root,
    cargoCapacityGuardPath,
    fs.readFileSync(path.join(actualRoot, cargoCapacityGuardPath), "utf8"),
  );
  recipes(root, {
    "native-width-hash-facts": actualRecipeBody("native-width-hash-facts", true),
    "native-width-classic-facts": actualRecipeBody("native-width-classic-facts", true),
    "runtime-asset-script-test": actualRecipeBody("runtime-asset-script-test"),
  });
  track(root);
  return root;
}
function expectNativeWidthHashRouteFailure(root: string, expectedIssueCode: string): void {
  const report = inspectNativeWidthHashRoute(root);
  expect(report.facts.factRecipeVerified).toBe(false);
  expect(report.facts.localCheckRegistered).toBe(false);
  expect(report.facts.ciCheckRegistered).toBe(false);
  expect(report.issues.map((issue) => issue.code)).toContain(expectedIssueCode);
}
function expectNativeWidthRegistrationRejected(
  mutate: (body: string) => string,
  recipeName = "runtime-unused-script-regression-run",
): void {
  const root = nativeWidthHashFixture();
  recipes(root, {
    "native-width-hash-facts": actualRecipeBody("native-width-hash-facts", true),
    "native-width-classic-facts": actualRecipeBody("native-width-classic-facts", true),
    [recipeName]: mutate(actualRecipeBody(recipeName)),
  });
  track(root);
  const report = inspectNativeWidthHashRoute(root);
  expect(report.facts.factRecipeVerified).toBe(true);
  expect(report.facts.testCommandVerified).toBe(false);
  expect(report.facts.localCheckRegistered).toBe(false);
  expect(report.facts.ciCheckRegistered).toBe(false);
}
const nativeWidthHashTestPath = "scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts";
const mathJaxUnusedTestPath = "scripts/runtime-bundles/runtime-unused-mathjax_test.ts";
const managedFullCompareTestPath = "scripts/mermaid/managed_full_compare_test.ts";
const managedFullComparePreviousTestPath =
  "scripts/mermaid/dom_plain_text_measure_contract_test.ts";
const managedFullCompareNextTestPath = "scripts/mermaid/mermaid_pair_manifest_test.ts";
function moveNativeWidthToFirstTestCommand(body: string): string {
  const lines = body.split("\n");
  const nativeIndex = lines.findIndex((line) => line.includes(nativeWidthHashTestPath));
  if (nativeIndex < 0) throw new Error("canonical NativeWidth command location missing");
  const nativeLine = lines[nativeIndex];
  if (!nativeLine) throw new Error("canonical NativeWidth command missing");
  const explicitNativePath = `./${nativeWidthHashTestPath}`;
  const withoutNative = nativeLine.replace(` ${explicitNativePath}`, "");
  if (withoutNative === nativeLine) throw new Error("NativeWidth token is not a final argument");
  const commandStart = nativeLine.indexOf("./");
  if (commandStart < 0) throw new Error("canonical Bun test command missing");
  lines[nativeIndex] = withoutNative;
  lines.unshift(`${nativeLine.slice(0, commandStart)}${explicitNativePath}`);
  return lines.join("\n");
}
test.each([
  ["missing", (body: string) => body.replace(` ./${nativeWidthHashTestPath}`, "")],
  [
    "changed",
    (body: string) =>
      body.replace(
        nativeWidthHashTestPath,
        "./scripts/runtime-bundles/unreviewed-native-width_test.ts",
      ),
  ],
  [
    "doubled",
    (body: string) =>
      body.replace(
        nativeWidthHashTestPath,
        `./${nativeWidthHashTestPath} ./${nativeWidthHashTestPath}`,
      ),
  ],
  [
    "reordered",
    (body: string) =>
      body.replace(
        `./${mathJaxUnusedTestPath} ./${nativeWidthHashTestPath}`,
        `./${nativeWidthHashTestPath} ./${mathJaxUnusedTestPath}`,
      ),
  ],
  ["unexpected command", (body: string) => `${body}\nbun -e 'process.exit(0)'`],
  ["moved to an earlier command", moveNativeWidthToFirstTestCommand],
] as const)("NativeWidth route rejects %s focused-registration drift", (_name, mutate) => {
  expectNativeWidthRegistrationRejected(mutate);
});
test.each([
  ["missing", (body: string) => body.replace(` ./${managedFullCompareTestPath}`, "")],
  [
    "changed",
    (body: string) =>
      body.replace(managedFullCompareTestPath, "./scripts/mermaid/unreviewed-managed-test.ts"),
  ],
  [
    "duplicated",
    (body: string) =>
      body.replace(
        ` ./${managedFullCompareTestPath}`,
        ` ./${managedFullCompareTestPath} ./${managedFullCompareTestPath}`,
      ),
  ],
  [
    "moved away from its canonical neighbors",
    (body: string) =>
      body.replace(
        `./${managedFullComparePreviousTestPath} ./${managedFullCompareTestPath} ./${managedFullCompareNextTestPath}`,
        `./${managedFullCompareTestPath} ./${managedFullComparePreviousTestPath} ./${managedFullCompareNextTestPath}`,
      ),
  ],
] as const)("NativeWidth route rejects managed full-compare registration %s", (_name, mutate) => {
  expectNativeWidthRegistrationRejected(mutate, "runtime-asset-script-other-run");
});
function buildFixture(): string {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-owned-contract-")));
  fixtures.push(root);
  execFileSync("git", ["init", "-q", root]);
  write(root, ".gitignore", "tmp/\nnode_modules/\n");
  for (const [name, source] of Object.entries(sources)) write(root, name, source);
  writeRuntimeAssetUnitFixtures(root);
  writeCurrentMoonFixtureInputs(root);
  writeJson(root, "scripts/quality/duplication-policy.json", duplication);
  writeJson(root, duplication.assetManifest, {
    version: 1,
    resourcesRoot: duplication.assetResourcesRoot,
    resources: [],
  });
  writeJson(root, "scripts/html/harness-contract-policy.json", {
    schema: 1,
    codeRoots: ["scripts/", "crates/"],
    localRecipes: ["check-standard"],
    localBridge: { recipe: "check", source: "scripts/local.py" },
    ciWorkflow: ".github/workflows/check.yml",
    recipeDispatch: {},
    python: {
      lintRecipe: "python-lint",
      typeRecipe: "python-typecheck",
      ruff: "0.16.10",
      mypy: "2.4.0",
    },
    rust: { lintRecipe: "lint", workspace: "Cargo.toml" },
  });
  writeJson(root, "package.json", {
    scripts: {
      biome: "biome check --error-on-warnings",
      typecheck: "tsc --noEmit",
    },
    devDependencies: { typescript: "7.0.2", biome: "2.4.8", playwright: "1.64.1" },
  });
  write(root, "bun.lock", "{}\n");
  writeJson(root, "biome.json", {
    files: {
      includes: ["scripts/**/*.ts", "scripts/**/*.js", "tests/fixtures/quality-cache/*.ts"],
    },
  });
  writeJson(root, "tsconfig.json", {
    compilerOptions: {
      strict: true,
      noUnusedLocals: true,
      noUnusedParameters: true,
      useUnknownInCatchVariables: true,
      lib: ["es2024"],
      allowJs: true,
      checkJs: true,
    },
    include: ["scripts/**/*.ts", "scripts/**/*.js", "tests/fixtures/quality-cache/*.ts"],
  });
  write(
    root,
    "Cargo.toml",
    '[workspace]\nmembers = ["crates/demo", "crates/katana-render-runtime"]\n',
  );
  write(root, "crates/demo/Cargo.toml", '[package]\nname = "demo"\nversion = "0.1.0"\n');
  write(
    root,
    "crates/katana-render-runtime/Cargo.toml",
    '[package]\nname = "katana-render-runtime"\nversion = "0.1.0"\n',
  );
  const resource =
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/shapes/basic.xml";
  write(root, resource, "fixture-resource-name-only");
  for (const name of generatedRuntimeRustAssetInputPaths(root)) {
    if (name === resource) continue;
    const contents = `export const fixtureRuntimeInput = ${JSON.stringify(name)};\n`;
    write(root, name, contents);
    write(
      root,
      `${name}.sha256`,
      `${createHash("sha256").update(contents).digest("hex")}  ${path.posix.basename(name)}\n`,
    );
  }
  for (const name of generatedRuntimeRustAssetPaths(root))
    write(root, name, `fixture generated output: ${name}\n`);
  write(
    root,
    ".github/workflows/check.yml",
    "jobs:\n  check:\n    steps:\n      - name: standard\n        run: just check-standard\n",
  );
  writeParserBindingFixture(root);
  track(root);
  return root;
}
function fixture(): string {
  const template = fixtureTemplate;
  if (template === undefined) throw new Error("fixture template is not ready");
  const parent = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-owned-contract-")));
  fixtures.push(parent);
  const root = path.join(parent, "fixture");
  fs.cpSync(template, root, {
    recursive: true,
    mode: fs.constants.COPYFILE_FICLONE,
  });
  return root;
}
function parserFixture(): string {
  return fixture();
}
const tomliVendorPaths = [
  "__init__.py",
  "_types.py",
  "_re.py",
  "_parser.py",
  "py.typed",
  "LICENSE",
  "upstream-provenance.json",
  "ruff.toml",
] as const;
function tomliVendorFixture(trackVendor = true): string {
  const root = fixture();
  for (const name of tomliVendorPaths)
    write(
      root,
      `scripts/release/_tomli/${name}`,
      fs.readFileSync(path.join(import.meta.dir, "../release/_tomli", name), "utf8"),
    );
  if (trackVendor) track(root);
  return root;
}
function codes(root: string): Set<string> {
  return new Set(
    inspectOwnedValidationContract(root).issues.map((issue) => `${issue.code}:${issue.path}`),
  );
}

test("fixture clones keep source bytes and Git indexes independent", () => {
  const first = fixture();
  const original = fs.readFileSync(path.join(first, "scripts/tool.ts"), "utf8");
  write(first, "scripts/tool.ts", "export const value = 2;\n");
  write(first, "scripts/clone-only.ts", "export const cloneOnly = true;\n");
  track(first);
  const second = fixture();
  expect(fs.readFileSync(path.join(second, "scripts/tool.ts"), "utf8")).toBe(original);
  expect(readOwnedRepositoryIndex(first).tracked).toContain("scripts/tool.ts");
  expect(readOwnedRepositoryIndex(first).tracked).toContain("scripts/clone-only.ts");
  expect(readOwnedRepositoryIndex(second).tracked).toContain("scripts/tool.ts");
  expect(readOwnedRepositoryIndex(second).tracked).not.toContain("scripts/clone-only.ts");
  const firstTool = fs.statSync(path.join(first, "scripts/tool.ts"));
  const secondTool = fs.statSync(path.join(second, "scripts/tool.ts"));
  expect(firstTool.ino).not.toBe(secondTool.ino);
  expect(firstTool.nlink).toBe(1);
  expect(secondTool.nlink).toBe(1);
});

test("runtime asset registration uses Playwright without dropping sibling Bun tests", () => {
  const invocations = actualRuntimeAssetTestInvocations();
  const chromeTest = "scripts/html/feature-catalog_test.ts";
  const pureHtmlTests = [
    "scripts/html/visual-state-fidelity_test.ts",
    "scripts/html/dynamic-fidelity_test.ts",
    "scripts/html/interim-feature-results_test.ts",
  ];

  expect(invocations.filter((paths) => paths.includes(chromeTest))).toEqual([]);
  expect(invocations.find((paths) => paths.includes(pureHtmlTests[0] ?? ""))).toEqual(
    pureHtmlTests,
  );
  const runtimeAssetCommands = actualRecipeBody("runtime-asset-script-test").split("\n");
  expect(runtimeAssetCommands.at(-2)).toBe("just browser-contract-test");
  expect(runtimeAssetCommands.at(-1)).toBe(
    "bun test --path-ignore-patterns 'tmp/**' ./scripts/quality/owned-source-inventory_test.ts ./scripts/quality/owned-validation-contract_test.ts",
  );
  expect(actualRecipeBody("browser-contract-test")).toBe(
    "node node_modules/playwright/cli.js test --config scripts/html/playwright.config.ts",
  );
  for (const playwrightUnit of playwrightUnitPaths)
    expect(actualRuntimeAssetUnitPaths()).not.toContain(playwrightUnit);
  expect(actualRuntimeAssetUnitPaths()).toEqual(expect.arrayContaining(pureHtmlTests));
});

test.each(["current", "previous canonical"] as const)(
  "owned validation accepts runtime asset command order: %s",
  (order) => {
    const commands = actualRecipeBody("runtime-asset-script-other-run").split("\n");
    const asset = commands.find((line) =>
      line.includes("./scripts/runtime-assets/runtime-asset-common_test.ts"),
    );
    const host = commands.find((line) => line.includes("runtime-rust-facts-host_test.ts"));
    const owned = commands.find((line) =>
      line.includes("./scripts/quality/owned-source-inventory_test.ts"),
    );
    const moon = commands.find((line) =>
      line.startsWith("python3 scripts/quality/moon_cache_contract_test.py"),
    );
    if (!asset || !host || !owned || !moon) throw new Error("missing runtime asset order fixture");
    const body =
      order === "current"
        ? commands.join("\n")
        : [
            host,
            owned,
            moon,
            asset,
            ...commands.filter((line) => ![asset, host, owned, moon].includes(line)),
          ].join("\n");
    const root = strictUnusedRegistrationFixture();
    recipes(root, { "runtime-asset-script-other-run": body });
    track(root);
    const report = inspectMathJaxUnusedAuditRoute(root);
    expect(report.binding).toMatchObject({
      standardCommandVerified: true,
      standardLocalCheckRegistered: true,
      standardCiCheckRegistered: true,
    });
    const browserRoute = inspectPlaywrightRoute(root);
    expect(browserRoute.localRegistered).toBe(true);
    expect(browserRoute.ciRegistered).toBe(true);
    expect(report.issues.filter((issue) => issue.code.startsWith("mathjax-host-evidence"))).toEqual(
      [],
    );
  },
);

test.each(["missing owned", "duplicate owned", "changed arguments", "browser moved"] as const)(
  "owned validation rejects runtime asset order mutation: %s",
  (mutation) => {
    const commands = actualRecipeBody("runtime-asset-script-other-run").split("\n");
    const ownedIndex = commands.findIndex((line) =>
      line.includes("./scripts/quality/owned-source-inventory_test.ts"),
    );
    const browserIndex = commands.indexOf("just browser-contract-test");
    if (ownedIndex < 0 || browserIndex < 0)
      throw new Error("missing runtime asset mutation fixture");
    if (mutation === "missing owned") commands.splice(ownedIndex, 1);
    if (mutation === "duplicate owned") commands.push(commands[ownedIndex] ?? "");
    if (mutation === "changed arguments")
      commands[ownedIndex] = `${commands[ownedIndex]} --filter altered`;
    if (mutation === "browser moved") {
      const browser = commands.splice(browserIndex, 1)[0];
      if (browser === undefined) throw new Error("missing browser command");
      commands.splice(3, 0, browser);
    }
    const body = commands.join("\n");
    expect(body).not.toBe(actualRecipeBody("runtime-asset-script-other-run"));
    const root = strictUnusedRegistrationFixture();
    recipes(root, { "runtime-asset-script-other-run": body });
    track(root);
    const report = inspectMathJaxUnusedAuditRoute(root);
    expect(report.binding.standardCommandVerified).toBe(false);
    expect(report.binding.standardLocalCheckRegistered).toBe(false);
    expect(report.binding.standardCiCheckRegistered).toBe(false);
    expect(report.issues.map((issue) => issue.code)).toContain(
      "mathjax-host-evidence-registration-unverified",
    );
    if (mutation === "browser moved") {
      const browserRoute = inspectPlaywrightRoute(root);
      expect(browserRoute.localRegistered).toBe(false);
      expect(browserRoute.ciRegistered).toBe(false);
    }
  },
);

test.each(["missing", "moved", "duplicate"] as const)(
  "runtime asset registration rejects a %s Playwright process",
  (mutation) => {
    const featureInvocation = "just browser-contract-test";
    const pureInvocation =
      "bun test --path-ignore-patterns 'tmp/**' ./scripts/html/visual-state-fidelity_test.ts ./scripts/html/dynamic-fidelity_test.ts ./scripts/html/interim-feature-results_test.ts";
    const recipe = actualRecipeBody("runtime-asset-script-test");
    const mutated =
      mutation === "missing"
        ? recipe.replace(featureInvocation, "")
        : mutation === "moved"
          ? recipe
              .replace(featureInvocation, "")
              .replace(pureInvocation, `${featureInvocation}\n${pureInvocation}`)
          : `${recipe}\n${featureInvocation}`;
    const root = fixture();
    recipes(root, { "runtime-asset-script-test": mutated });
    track(root);

    expect(codes(root)).toContain(
      "moon-cache-fixture-owner-unregistered:runtime-asset-script-test",
    );
  },
);

test("Playwright owned units retain both standard lanes and the fixed Chrome config", () => {
  const root = fixture();
  const route = inspectPlaywrightRoute(root);
  expect(route.issues).toEqual([]);
  expect(route.units).toEqual([...playwrightUnitPaths]);
  expect(route.localRegistered).toBe(true);
  expect(route.ciRegistered).toBe(true);
});

test.each([
  ["channel", 'channel: "chrome"', 'channel: "chromium"'],
  ["timeout", "timeout: 5000", "timeout: 6000"],
  ["retry", "retries: 0", "retries: 1"],
  ["workers", "workers: 1", "workers: 2"],
  ["parallel", "fullyParallel: false", "fullyParallel: true"],
  ["only", "forbidOnly: true", "forbidOnly: false"],
  ["headless", "headless: true", "headless: false"],
  ["viewport", "width: 1280", "width: 1281"],
  ["test directory", 'testDir: ".."', 'testDir: "."'],
  [
    "geometry spec deletion",
    '"mermaid/reference_svg_geometry_browser_test.ts"',
    '"mermaid/removed_test.ts"',
  ],
  [
    "extra entry",
    '"mermaid/reference_svg_geometry_browser_test.ts",',
    '"mermaid/reference_svg_geometry_browser_test.ts", "mermaid/extra_test.ts",',
  ],
  ["output", "../../tmp/quality/", "tmp/quality/"],
  ["unknown setting", "retries: 0", "retries: 0, grep: /never/"],
  ["config binding", 'from "playwright/test"', 'from "other-runner"'],
] as const)("Playwright registration rejects %s config drift", (_label, before, after) => {
  const root = fixture();
  const source = fs.readFileSync(path.join(root, playwrightConfigPath), "utf8");
  expect(source).toContain(before);
  write(root, playwrightConfigPath, source.replace(before, after));
  const route = inspectPlaywrightRoute(root);
  expect(route.localRegistered).toBe(false);
  expect(route.ciRegistered).toBe(false);
  expect(route.issues.map((issue) => issue.code)).toContain("playwright-inputs-unverified");
});

test.each([
  "strict raster sizing preserves SVG markup and resolves percentage width from the viewBox",
  "paint projection evaluates actual candidate pixels",
  "keeps the color-only candidate exact while changing RGB and preserving SVG bytes",
  "rejects a candidate with a missing label",
  "rejects a candidate with a missing edge",
  "keeps the neutral geometry raster exact",
  "same mapping",
  "source maps to different targets",
  "targets map from different sources",
])("Playwright geometry registration rejects a removed browser case: %s", (caseName) => {
  const root = fixture(),
    geometryPath = playwrightGeometryTestPath,
    actualSource = fs.readFileSync(path.resolve(import.meta.dir, "../..", geometryPath), "utf8");
  expect(actualSource.split(caseName)).toHaveLength(2);
  const mutated = actualSource.replace(caseName, "removed browser case");
  write(root, geometryPath, `${mutated}\nconst unusedCaseTitle = ${JSON.stringify(caseName)};\n`);
  const route = inspectPlaywrightRoute(root);
  expect(route.localRegistered).toBe(false);
  expect(route.ciRegistered).toBe(false);
  expect(route.issues.map((issue) => issue.code)).toContain("playwright-inputs-unverified");
});

test.each([
  ["missing", ""],
  ["Bun", "bun test ./scripts/html/feature-catalog_test.ts"],
  ["config override", "node node_modules/playwright/cli.js test --config other.ts"],
  ["filter", `${actualRecipeBody("browser-contract-test")} --grep never`],
  ["masked failure", `${actualRecipeBody("browser-contract-test")} || true`],
] as const)("Playwright registration rejects a %s runner recipe", (_label, command) => {
  const root = fixture();
  recipes(root, { "browser-contract-test": command });
  track(root);
  const route = inspectPlaywrightRoute(root);
  expect(route.localRegistered).toBe(false);
  expect(route.ciRegistered).toBe(false);
});

test.each(["local", "ci"] as const)(
  "Playwright registration rejects an omitted %s standard lane",
  (lane) => {
    const root = fixture();
    const standard = fixtureRecipeBodies()["check-standard"] ?? "";
    const omitted = standard.replace("just runtime-asset-script-test", "");
    recipes(root, {
      "check-standard": lane === "local" ? omitted : standard,
      "ci-standard": lane === "ci" ? omitted : standard,
    });
    write(
      root,
      ".github/workflows/check.yml",
      "jobs:\n  check:\n    steps:\n      - run: just ci-standard\n",
    );
    track(root);
    const route = inspectPlaywrightRoute(root);
    expect(lane === "local" ? route.localRegistered : route.ciRegistered).toBe(false);
    expect(route.issues.map((issue) => issue.code)).toContain(`unit-${lane}-missing`);
  },
);

test("Playwright imports classify owned units without admitting an unknown entry", () => {
  const root = fixture();
  const extra = "scripts/html/unregistered-browser_test.ts";
  write(root, extra, playwrightFixtureSource);
  track(root);
  const report = inspectOwnedValidationContract(root);
  expect(report.units.playwright).toEqual([...playwrightUnitPaths, extra].sort());
  expect(report.units.bun).not.toContain(extra);
  expect(report.issues.filter((issue) => issue.path === extra).map((issue) => issue.code)).toEqual([
    "unit-local-missing",
    "unit-ci-missing",
  ]);
});

test.each([playwrightConfigPath, ...playwrightUnitPaths])(
  "Playwright registration rejects the missing physical input %s",
  (name) => {
    const root = fixture();
    fs.unlinkSync(path.join(root, name));
    const route = inspectPlaywrightRoute(root);
    expect(route.localRegistered).toBe(false);
    expect(route.issues.map((issue) => issue.code)).toContain("playwright-inputs-unverified");
  },
);

test("Playwright registration rejects all inputs disappearing from the owned inventory", () => {
  const root = fixture();
  const inputs = [playwrightConfigPath, ...playwrightUnitPaths];
  for (const name of inputs) execFileSync("git", ["rm", "--cached", "--", name], { cwd: root });
  for (const name of inputs) fs.unlinkSync(path.join(root, name));
  const route = inspectPlaywrightRoute(root);
  expect(route.issues.map((issue) => issue.code)).toContain("playwright-inputs-unverified");
  expect(route.localRegistered).toBe(false);
  expect(route.ciRegistered).toBe(false);
});

test.each([
  ["type-only", 'import type { test } from "playwright/test";'],
  ["deleted cases", 'import { test } from "playwright/test";'],
  ["skip", playwrightFixtureSource.replace('test("canonical', 'test.skip("canonical')],
  ["unapproved lifecycle hook", `${playwrightFixtureSource}test.beforeEach(() => {});`],
  [
    "describe skip",
    `${playwrightFixtureSource.replace('test("canonical', 'test.describe.skip("group", () => { test("canonical')} });`,
  ],
  ["per-test timeout", `${playwrightFixtureSource}\ntest.setTimeout(6000);`],
] as const)("Playwright registration rejects %s test boundaries", (_label, source) => {
  const root = fixture();
  write(root, playwrightUnitPaths[0], source);
  const route = inspectPlaywrightRoute(root);
  expect(route.localRegistered).toBe(false);
  expect(route.issues.map((issue) => issue.code)).toContain("playwright-inputs-unverified");
});

test("actual Git inventory distinguishes tracked and nonignored untracked sources", () => {
  const root = fixture();
  write(root, "scripts/new.ts", "export const added = 1;\n");
  write(root, "tmp/ignored.ts", "ignored\n");
  const index = readOwnedRepositoryIndex(root);
  expect(index.tracked).toContain("scripts/tool_test.py");
  expect(index.untracked).toEqual(["scripts/new.ts"]);
  expect(codes(root)).toContain("untracked-source:scripts/new.ts");
});
test("fixture sources stay classified when its shell registration is unsupported", () => {
  const root = fixture();
  recipes(root, {
    "runtime-asset-script-test": `${actualRecipeBody("runtime-asset-script-test")}\necho unsupported owner route`,
  });
  const report = inspectOwnedValidationContract(root);
  expect(report.issues.map((issue) => `${issue.code}:${issue.path}`)).toEqual([
    "moon-cache-fixture-owner-command-unverified:Justfile",
    "moon-cache-fixture-owner-unregistered:runtime-asset-script-test",
    "moon-cache-fixture-owner-unregistered:runtime-asset-script-test",
    "parser-body-onload-test-unregistered:runtime-asset-script-test",
    "unit-local-missing:scripts/runtime-bundles/parser-body-onload-generation.test.ts",
    "unit-ci-missing:scripts/runtime-bundles/parser-body-onload-generation.test.ts",
    "unit-local-missing:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
    "unit-ci-missing:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
    ...[...playwrightUnitPaths]
      .sort()
      .flatMap((name) => [`unit-local-missing:${name}`, `unit-ci-missing:${name}`]),
  ]);
  const runtimeUnitPaths = actualRuntimeAssetUnitPaths();
  expect(report.languageCounts).toEqual({
    rust: 2,
    typescript:
      14 +
      playwrightUnitPaths.length +
      runtimeUnitPaths.filter((name) => name.endsWith(".ts")).length,
    javascript: 2 + runtimeUnitPaths.filter((name) => name.endsWith(".js")).length,
    python: 7,
  });
  expect(report.units).toEqual({
    bun: [
      ...runtimeUnitPaths,
      "scripts/tool_test.ts",
      "tests/fixtures/quality-cache/moon-cache-subject.test.ts",
    ].sort(),
    python: ["scripts/tool_test.py"],
    playwright: [...playwrightUnitPaths].sort(),
  });
  expect(report.moonCacheFixture).toEqual({
    path: "tests/fixtures/quality-cache/moon-cache-subject.test.ts",
    owner: "scripts/quality/moon_cache_contract_test.py",
    ownerAstHelper: "scripts/quality/owned_python_owner_contract.py",
    ownerAstContract: "moon-owner-host-evidence-barrier-v3",
    fixtureSha256: expect.stringMatching(/^[a-f0-9]{64}$/),
    ownerSha256: expect.stringMatching(/^[a-f0-9]{64}$/),
    ownerAstHelperSha256: expect.stringMatching(/^[a-f0-9]{64}$/),
    fixtureVerified: true,
    ownerVerified: true,
    ownerAstVerified: true,
    rootDagVerified: true,
    rootConfig: "moon.yml",
    rootConfigSha256: expect.stringMatching(/^[a-f0-9]{64}$/),
    localCheckRegistered: false,
    ciCheckRegistered: false,
  });
  expect(report.sourceDigest).toMatch(/^[a-f0-9]{64}$/);
  expect(report.generatedRustAssets).toEqual({
    paths: generatedRuntimeRustAssetPaths(root),
    inputCount: 3,
    inputDigest: expect.stringMatching(/^[a-f0-9]{64}$/),
    byteVerification: "not-executed",
    requiredCheck: "runtime-package-asset-check",
    localCheckRegistered: true,
    ciCheckRegistered: true,
  });
});
test("generated Rust outputs are separately fingerprinted without claiming byte verification", () => {
  const root = fixture();
  const before = inspectOwnedValidationContract(root);
  expect(before.issues).toEqual([]);
  expect(before.verifiedAssets).toBe(2);
  expect(before.generatedRustAssets.paths).toHaveLength(3);
  expect(before.generatedRustAssets.inputCount).toBe(3);
  expect(before.generatedRustAssets.inputDigest).toMatch(/^[a-f0-9]{64}$/);
  expect(before.generatedRustAssets.byteVerification).toBe("not-executed");
  expect(before.generatedRustAssets.localCheckRegistered).toBe(true);
  expect(before.generatedRustAssets.ciCheckRegistered).toBe(true);
  const output = before.generatedRustAssets.paths[0];
  if (output === undefined) throw new Error("expected generated Rust output");
  write(root, output, "changed generated bytes\n");
  const after = inspectOwnedValidationContract(root);
  expect(after.sourceDigest).not.toBe(before.sourceDigest);
  expect(after.verifiedAssets).toBe(2);
  expect(after.generatedRustAssets.byteVerification).toBe("not-executed");
});
test("the actual repository's generated Rust check route is canonical and registered", () => {
  const root = path.resolve(import.meta.dir, "../..");
  const route = inspectGeneratedRustCheckRoute(root);
  expect(route.issues.filter((issue) => issue.code.startsWith("generated-rust-check-"))).toEqual(
    [],
  );
  expect(route.local).toBe(true);
  expect(route.ci).toBe(true);
});
test("the actual repository binds the MathJax standard gate and retains the audit ledger", () => {
  const report = inspectMathJaxUnusedAuditRoute(path.resolve(import.meta.dir, "../.."));
  expect(report.binding).toMatchObject({
    sourcePath: "scripts/runtime-bundles/runtime-unused-check.ts",
    testPath: "scripts/runtime-bundles/runtime-unused-mathjax_test.ts",
    auditRecipe: "runtime-asset-script-test",
    auditCommand:
      "bun scripts/runtime-bundles/runtime-unused-check.ts --mermaid-js {{quote(MERMAID_JS)}} --mermaid-zenuml-js {{quote(MERMAID_ZENUML_JS)}}",
    standardCommand: "bun scripts/runtime-bundles/runtime-unused-check.ts --gate",
    factRecipe: "mathjax-consumer-facts",
    commandVerified: false,
    localCheckRegistered: false,
    ciCheckRegistered: false,
    standardCommandVerified: true,
    standardLocalCheckRegistered: true,
    standardCiCheckRegistered: true,
  });
  expect(report.binding.inputPaths).toContain(
    "crates/kdr-linter/src/bin/mathjax_consumer_facts.rs",
  );
  expect(report.localTestRegistered).toBe(true);
  expect(report.ciTestRegistered).toBe(true);
  expect(report.issues.filter((issue) => issue.code.startsWith("mathjax-unused-audit-"))).toEqual(
    [],
  );
});
test("the scoped NativeWidth binding accepts actual hashes and focused-test routes", () => {
  const report = inspectNativeWidthHashRoute(path.resolve(import.meta.dir, "../.."));
  expect(Object.isFrozen(NATIVE_WIDTH_HASH_INPUTS)).toBe(true);
  expect(report.facts.inputPaths).toEqual([...NATIVE_WIDTH_HASH_INPUTS]);
  expect(report.facts).toMatchObject({
    sourcePath: "scripts/runtime-bundles/runtime-unused-native-width-hash.ts",
    testPath: "scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts",
    factRecipe: "native-width-hash-facts",
    auditRecipe: "runtime-asset-script-test",
    factRecipeVerified: true,
    testCommandVerified: true,
    localCheckRegistered: true,
    ciCheckRegistered: true,
    nativeExecutionVerified: false,
    performanceVerified: false,
  });
  expect(report.facts.inputPaths).toContain(
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs",
  );
  expect(
    report.issues
      .filter((issue) => issue.code.startsWith("native-width-hash-"))
      .map((issue) => `${issue.code}:${issue.path}`),
  ).toEqual([]);
});
test("the NativeWidth facts recipe requires its canonical root parameters and safe command body", () => {
  expect(actualRecipeBody("native-width-hash-facts", true)).toBe(
    '@CARGO_TARGET_DIR={{quote(root + "/target")}} bun {{quote(root + "/scripts/runtime-bundles/runtime-rust-facts-host.ts")}} run {{quote(root)}} native-width {{quote(source_root)}} {{quote(root + "/tmp/quality/rust-facts-host/current/receipt.json")}}',
  );
  expect(
    fs.readFileSync(path.join(path.resolve(import.meta.dir, "../.."), "Justfile"), "utf8"),
  ).toContain(nativeWidthHashHeader);
  expect(actualRecipeBody("mathjax-consumer-facts", true)).toBe(mathJaxFactsRecipe);
});
test("owned validation checks root and standalone Cargo lock resolution before heavy lanes", () => {
  expect(actualRecipeBody("owned-validation-contract-check").split("\n")).toEqual([
    "bun scripts/quality/owned-validation-contract.ts",
    '{{CARGO}} metadata --locked --all-features --format-version 1 --manifest-path "{{REPO_ROOT}}/Cargo.toml" > /dev/null',
    '{{CARGO}} metadata --locked --no-default-features --features baseline --format-version 1 --manifest-path "{{REPO_ROOT}}/scripts/html/performance-harness/Cargo.toml" > /dev/null',
    '{{CARGO}} metadata --locked --no-default-features --features candidate --format-version 1 --manifest-path "{{REPO_ROOT}}/scripts/html/performance-harness/Cargo.toml" > /dev/null',
  ]);
});
const nativeWidthJustMutations = [
  ["shell mode", fixtureJustDefinitions.split("\n")[0] ?? "", 'set shell := ["sh", "-c"]'],
  ["repository root", "REPO_ROOT := justfile_directory()", 'REPO_ROOT := "."'],
  [
    "RTK environment default",
    'RTK := env_var_or_default("RTK", `command -v rtk 2> /dev/null || true`)',
    'RTK := env_var_or_default("RTK", "rtk")',
  ],
  ["RTK command chain", 'RTK_CMD := if RTK == "" { "" } else { RTK + " " }', 'RTK_CMD := "rtk "'],
  [
    "CARGO environment chain",
    'CARGO := env_var_or_default("CARGO", RTK_CMD + "cargo")',
    'CARGO := env_var_or_default("CARGO", "cargo")',
  ],
  [
    "guard command replaced with cargo",
    fixtureJustDefinitions.split("\n")[5] ?? "",
    'CARGO_BUILD := "cargo"',
  ],
  [
    "guard command replaced with shell",
    fixtureJustDefinitions.split("\n")[5] ?? "",
    'CARGO_BUILD := "sh -c echo forged"',
  ],
  [
    "guard profile changed",
    fixtureJustDefinitions.split("\n")[5] ?? "",
    fixtureJustDefinitions.split("\n")[5]?.replace("debug --", "release --") ?? "",
  ],
  [
    "guard separator removed",
    fixtureJustDefinitions.split("\n")[5] ?? "",
    fixtureJustDefinitions.split("\n")[5]?.replace("debug -- ", "debug ") ?? "",
  ],
  ["missing CARGO_BUILD definition", fixtureJustDefinitions.split("\n")[5] ?? "", ""],
  [
    "duplicate CARGO_BUILD definition",
    fixtureJustDefinitions.split("\n")[5] ?? "",
    `${fixtureJustDefinitions.split("\n")[5] ?? ""}\n${fixtureJustDefinitions.split("\n")[5] ?? ""}`,
  ],
] as const;
test.each(nativeWidthJustMutations)(
  "scoped NativeWidth binding rejects unsafe Just configuration: %s",
  (_name, before, after) => {
    const root = nativeWidthHashFixture();
    const baseline = inspectNativeWidthHashRoute(root);
    expect(baseline.facts.factRecipeVerified).toBe(true);
    expect(baseline.facts.localCheckRegistered).toBe(true);
    expect(baseline.facts.ciCheckRegistered).toBe(true);
    expect(baseline.facts.nativeExecutionVerified).toBe(false);
    const justfile = fs.readFileSync(path.join(root, "Justfile"), "utf8");
    expect(justfile).toContain(before);
    write(root, "Justfile", justfile.replace(before, after));
    track(root);
    const report = inspectNativeWidthHashRoute(root);
    expect(report.facts.factRecipeVerified).toBe(false);
    expect(report.facts.localCheckRegistered).toBe(false);
    expect(report.facts.ciCheckRegistered).toBe(false);
    expect(report.facts.nativeExecutionVerified).toBe(false);
    expect(report.issues.map((issue) => issue.code)).toContain(
      "native-width-hash-fact-recipe-unverified",
    );
  },
);
test.each(["missing", "untracked"] as const)(
  "scoped NativeWidth binding requires the physical tracked guard source: %s",
  (kind) => {
    const root = nativeWidthHashFixture();
    const baseline = inspectNativeWidthHashRoute(root);
    expect(baseline.facts.factRecipeVerified).toBe(true);
    if (kind === "missing") {
      fs.unlinkSync(path.join(root, cargoCapacityGuardPath));
      track(root);
    } else {
      execFileSync("git", ["rm", "--cached", "--", cargoCapacityGuardPath], { cwd: root });
    }
    const report = inspectNativeWidthHashRoute(root);
    expect(report.facts.factRecipeVerified).toBe(false);
    expect(report.facts.localCheckRegistered).toBe(false);
    expect(report.facts.ciCheckRegistered).toBe(false);
  },
);
test.each([
  ["reordered parameters", "native-width-hash-facts source_root=REPO_ROOT root=REPO_ROOT:"],
  ["changed repository default", "native-width-hash-facts root=. source_root=REPO_ROOT:"],
  ["changed source default", "native-width-hash-facts root=REPO_ROOT source_root=.:"],
  [
    "extra parameter",
    "native-width-hash-facts root=REPO_ROOT source_root=REPO_ROOT extra=REPO_ROOT:",
  ],
])("scoped NativeWidth binding rejects %s in the recipe header", (_name, changedHeader) => {
  const root = nativeWidthHashFixture();
  const justfile = fs.readFileSync(path.join(root, "Justfile"), "utf8");
  expect(justfile).toContain(nativeWidthHashHeader);
  write(root, "Justfile", justfile.replace(nativeWidthHashHeader, changedHeader));
  track(root);
  expectNativeWidthHashRouteFailure(root, "native-width-hash-fact-recipe-unverified");
});
test("scoped NativeWidth binding rejects a removed focused audit test", () => {
  const root = nativeWidthHashFixture();
  const recipe = actualRecipeBody("runtime-unused-script-regression-run");
  recipes(root, {
    "native-width-hash-facts": actualRecipeBody("native-width-hash-facts"),
    "native-width-classic-facts": actualRecipeBody("native-width-classic-facts"),
    "runtime-unused-script-regression-run": recipe.replace(
      "scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts",
      "scripts/runtime-bundles/unregistered-native-width-hash_test.ts",
    ),
  });
  track(root);
  const report = inspectNativeWidthHashRoute(root);
  expect(report.facts.testCommandVerified).toBe(false);
  expect(report.facts.localCheckRegistered).toBe(false);
  expect(report.facts.ciCheckRegistered).toBe(false);
  expect(report.issues.map((issue) => issue.code)).toContain("native-width-hash-test-unregistered");
});
test("scoped NativeWidth binding rejects a changed guarded facts route", () => {
  const root = nativeWidthHashFixture();
  const canonicalRecipe = actualRecipeBody("native-width-hash-facts", true);
  const baseline = inspectNativeWidthHashRoute(root);
  expect(baseline.facts.factRecipeVerified).toBe(true);
  recipes(root, {
    "native-width-hash-facts": canonicalRecipe.replace(
      "run {{quote(root)}} native-width",
      "run {{quote(root)}} mathjax",
    ),
    "runtime-unused-script-regression-run": actualRecipeBody(
      "runtime-unused-script-regression-run",
    ),
  });
  track(root);
  expectNativeWidthHashRouteFailure(root, "native-width-hash-fact-recipe-unverified");
});
test(
  "the actual repository binds the Moon fixture to its real owner in both standard lanes",
  () => {
    const report = inspectMoonCacheFixtureRoute(path.resolve(import.meta.dir, "../.."));
    expect(report.issues.filter((issue) => issue.code.startsWith("moon-cache-fixture-"))).toEqual(
      [],
    );
    expect(report.binding.path).toBe("tests/fixtures/quality-cache/moon-cache-subject.test.ts");
    expect(report.binding.owner).toBe("scripts/quality/moon_cache_contract_test.py");
    expect(report.binding.ownerAstHelper).toBe("scripts/quality/owned_python_owner_contract.py");
    expect(report.binding.ownerAstContract).toBe("moon-owner-host-evidence-barrier-v3");
    expect(report.binding.fixtureVerified).toBe(true);
    expect(report.binding.ownerAstVerified).toBe(true);
    expect(report.binding.rootDagVerified).toBe(true);
    expect(report.binding.localCheckRegistered).toBe(true);
    expect(report.binding.ciCheckRegistered).toBe(true);
    expect(report.binding.ownerAstHelperSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(report.bunUnitRegistered).toBe(true);
  },
  REPOSITORY_GRAPH_TEST_TIMEOUT_MS,
);
const moonRootFixtureConfig = z.object({
  taskOptions: z.record(z.string(), z.unknown()),
  tasks: z.record(
    z.string(),
    z.object({
      command: z.union([z.string(), z.array(z.string())]),
      deps: z.array(z.string()).optional(),
      options: z.unknown().optional(),
      args: z.array(z.string()).optional(),
      inputs: z.unknown().optional(),
      outputs: z.unknown().optional(),
      checks: z.unknown().optional(),
    }),
  ),
});
type MoonRootFixtureConfig = z.infer<typeof moonRootFixtureConfig>;
function moonRootFixtureTask(config: MoonRootFixtureConfig, name: string) {
  const task = config.tasks[name];
  if (!task) throw new Error(`expected fixture Moon task ${name}`);
  return task;
}

test.each([
  [
    "dependency",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "lint").deps = ["fmt-check"];
    },
  ],
  [
    "leaf",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "typecheck").command = ["just", "biome"];
    },
  ],
  [
    "unit task does not use guarded shared root producer",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "unit-test").command = ["just", "unit-test"];
    },
  ],
  [
    "coverage leaf does not invoke independent performance producer",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "coverage").command = ["just", "coverage"];
    },
  ],
  [
    "package check chain bypasses the shared root test task",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "runtime-package-asset-check").deps = [
        "runtime-asset-script-test",
      ];
    },
  ],
  [
    "options",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "typecheck").options = { shell: true };
    },
  ],
  [
    "extra task",
    (config: MoonRootFixtureConfig) => {
      config.tasks["unreviewed-static"] = { command: ["just", "biome"] };
      moonRootFixtureTask(config, "static").deps = [
        "source-static",
        "dependency-leak",
        "unreviewed-static",
      ];
    },
  ],
  [
    "missing task",
    (config: MoonRootFixtureConfig) => {
      delete config.tasks.typecheck;
    },
  ],
  [
    "unsafe options",
    (config: MoonRootFixtureConfig) => {
      config.taskOptions.expectFailure = true;
    },
  ],
  [
    "invalid options",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "typecheck").options = "unsupported";
    },
  ],
  [
    "extra leaf arguments",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "typecheck").args = ["--unreviewed"];
    },
  ],
  [
    "cycle",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "fmt-check").deps = ["static"];
    },
  ],
  [
    "unknown option",
    (config: MoonRootFixtureConfig) => {
      moonRootFixtureTask(config, "typecheck").options = { unreviewed: false };
    },
  ],
] as const)("Moon owner rejects root-only scheduler drift: %s", (_label, mutate) => {
  const root = fixture();
  const config = moonRootFixtureConfig.parse(
    Bun.YAML.parse(fs.readFileSync(path.join(root, "moon.yml"), "utf8")),
  );
  mutate(config);
  writeJson(root, "moon.yml", config);
  const report = inspectOwnedValidationContract(root);
  expect(report.moonCacheFixture.ownerAstVerified).toBe(true);
  expect(report.moonCacheFixture.ownerVerified).toBe(false);
  expect(report.moonCacheFixture.rootDagVerified).toBe(false);
  expect(report.issues).toContainEqual(
    expect.objectContaining({ code: "moon-cache-fixture-root-dag-unverified", path: "moon.yml" }),
  );
});

test("the actual repository binds only parser body-onload output to its producer and standard checks", () => {
  const report = inspectParserBodyOnloadRoute(path.resolve(import.meta.dir, "../.."));
  expect(
    report.issues
      .filter((issue) => issue.code.startsWith("parser-body-onload-"))
      .map((issue) => issue.code),
  ).toEqual([]);
  expect(report.binding).toMatchObject({
    path: "crates/katana-render-runtime/src/renderer/backends/html_runtime/parser_body_onload.js",
    canonical:
      "crates/katana-render-runtime/src/renderer/backends/html_runtime/parser_body_onload.ts",
    hostDeclaration:
      "crates/katana-render-runtime/src/renderer/backends/html_runtime/host_parser_onload.d.ts",
    producer: "scripts/runtime-bundles/parser-body-onload-generation.ts",
    unitTest: "scripts/runtime-bundles/parser-body-onload-generation.test.ts",
    producerVerified: true,
    biomeIncluded: true,
    canonicalTypeScope: true,
    producerTypeScope: true,
    outputTypeBoundary: "generated-from-strict-typescript",
    localCheckRegistered: true,
    ciCheckRegistered: true,
    localTestRegistered: true,
    ciTestRegistered: true,
    byteVerification: "not-executed",
  });
  expect(report.binding.outputSha256).toMatch(/^[a-f0-9]{64}$/);
  expect(report.binding.inputDigest).toMatch(/^[a-f0-9]{64}$/);
  expect(report.binding.inputPaths).toContain("package.json");
  expect(report.binding.inputPaths).toContain("bun.lock");
  expect(report.binding.inputPaths).toContain("Justfile");
});
test.each([
  ["local", "unit-test"],
  ["local", "root-test-and-coverage"],
  ["ci", "unit-test"],
  ["ci", "root-test-and-coverage"],
] as const)(
  "parser registration requires the command-bearing lane, not standalone %s %s",
  (lane, standalone) => {
    const root = parserFixture();
    const standard =
      "just runtime-bundle-check\n" +
      "just runtime-asset-script-other-run\n" +
      "just runtime-unused-script-regression-run";
    const laneBody = `just ${standalone}`;
    const unitBody = recipeBodies.unit;
    if (unitBody === undefined) throw new Error("missing unit fixture recipe");
    recipes(root, {
      [standalone]: unitBody,
      "check-standard": lane === "local" ? laneBody : standard,
      "ci-standard": lane === "ci" ? laneBody : standard,
    });
    write(
      root,
      ".github/workflows/check.yml",
      "jobs:\n  check:\n    steps:\n      - name: Check\n        run: just ci-standard\n",
    );
    track(root);

    const report = inspectParserBodyOnloadRoute(root);
    expect(report.binding).toMatchObject({
      localCheckRegistered: lane !== "local",
      ciCheckRegistered: lane !== "ci",
      localTestRegistered: lane !== "local",
      ciTestRegistered: lane !== "ci",
    });
  },
);
test.each([
  "./scripts/mermaid/native-source-manifest_test.ts",
  "./scripts/mermaid/reference_svg_geometry_test.ts",
])("the required runtime asset regression cannot be omitted: %s", (sourceManifestTest) => {
  const root = parserFixture();
  const recipe = actualRecipeBody("runtime-asset-script-test");
  const accepted = inspectParserBodyOnloadRoute(root);
  expect(recipe).toContain(sourceManifestTest);
  expect(
    accepted.issues
      .filter((issue) => issue.code.startsWith("parser-body-onload-"))
      .map((issue) => issue.code),
  ).toEqual([]);

  recipes(root, {
    "runtime-asset-script-test": recipe.replace(` ${sourceManifestTest}`, ""),
  });
  track(root);
  const rejected = inspectOwnedValidationContract(root);
  expect(rejected.issues).toContainEqual(
    expect.objectContaining({
      code: "parser-body-onload-test-unregistered",
      path: "runtime-asset-script-test",
    }),
  );
});
test.each([
  [
    "evidence",
    "missing",
    "./scripts/mermaid/native_frame_evidence_test.ts",
    "./scripts/mermaid/native_frame_build_proof_test.ts",
    "./scripts/mermaid/native-render-batch_test.ts",
  ],
  [
    "evidence",
    "duplicated",
    "./scripts/mermaid/native_frame_evidence_test.ts",
    "./scripts/mermaid/native_frame_build_proof_test.ts",
    "./scripts/mermaid/native-render-batch_test.ts",
  ],
  [
    "evidence",
    "moved",
    "./scripts/mermaid/native_frame_evidence_test.ts",
    "./scripts/mermaid/native_frame_build_proof_test.ts",
    "./scripts/mermaid/native-render-batch_test.ts",
  ],
  [
    "build proof",
    "missing",
    "./scripts/mermaid/native_frame_build_proof_test.ts",
    "./scripts/mermaid/native_render_provenance_test.ts",
    "./scripts/mermaid/native_frame_evidence_test.ts",
  ],
  [
    "build proof",
    "duplicated",
    "./scripts/mermaid/native_frame_build_proof_test.ts",
    "./scripts/mermaid/native_render_provenance_test.ts",
    "./scripts/mermaid/native_frame_evidence_test.ts",
  ],
  [
    "build proof",
    "moved",
    "./scripts/mermaid/native_frame_build_proof_test.ts",
    "./scripts/mermaid/native_render_provenance_test.ts",
    "./scripts/mermaid/native_frame_evidence_test.ts",
  ],
  [
    "product score",
    "missing",
    "./scripts/mermaid/native_product_score_test.ts",
    "./scripts/quality/duplication-check_test.ts",
    "./scripts/runtime-bundles/runtime-bundle-checks_test.ts",
  ],
  [
    "product score",
    "duplicated",
    "./scripts/mermaid/native_product_score_test.ts",
    "./scripts/quality/duplication-check_test.ts",
    "./scripts/runtime-bundles/runtime-bundle-checks_test.ts",
  ],
  [
    "product score",
    "moved",
    "./scripts/mermaid/native_product_score_test.ts",
    "./scripts/quality/duplication-check_test.ts",
    "./scripts/runtime-bundles/runtime-bundle-checks_test.ts",
  ],
] as const)(
  "native %s registration rejects %s drift",
  (_testName, mutation, token, previous, next) => {
    const root = parserFixture();
    const recipe = actualRecipeBody("runtime-asset-script-test");
    expect(recipe).toContain(token);
    const mutated =
      mutation === "missing"
        ? recipe.replace(` ${token}`, "")
        : mutation === "duplicated"
          ? recipe.replace(` ${token}`, ` ${token} ${token}`)
          : recipe
              .replace(` ${token}`, "")
              .replace(` ${previous} ${next}`, ` ${token} ${previous} ${next}`);
    expect(mutated).not.toBe(recipe);
    recipes(root, { "runtime-asset-script-test": mutated });
    track(root);
    expect(inspectOwnedValidationContract(root).issues).toContainEqual(
      expect.objectContaining({
        code: "parser-body-onload-test-unregistered",
        path: "runtime-asset-script-test",
      }),
    );
  },
);
test.each([
  ["missing", (recipe: string, token: string) => recipe.replace(` ${token}`, "")],
  [
    "moved",
    (recipe: string, token: string) =>
      recipe
        .replace(` ${token}`, "")
        .replace(
          " ./scripts/runtime-bundles/runtime-unused-check_test.ts",
          ` ${token} ./scripts/runtime-bundles/runtime-unused-check_test.ts`,
        ),
  ],
  ["duplicated", (recipe: string, token: string) => recipe.replace(token, `${token} ${token}`)],
])(
  "the runtime bundle fingerprint test is required in its reviewed recipe position (%s)",
  (_label, mutate) => {
    const root = parserFixture();
    const fingerprintTest = "./scripts/runtime-bundles/runtime-bundle-tool-fingerprint_test.ts";
    const recipe = actualRecipeBody("runtime-unused-script-regression-run");
    expect(recipe).toContain(fingerprintTest);
    expect(
      inspectParserBodyOnloadRoute(root).issues.filter((issue) =>
        issue.code.startsWith("parser-body-onload-"),
      ),
    ).toEqual([]);

    recipes(root, { "runtime-unused-script-regression-run": mutate(recipe, fingerprintTest) });
    track(root);
    expect(inspectOwnedValidationContract(root).issues).toContainEqual(
      expect.objectContaining({
        code: "parser-body-onload-test-unregistered",
        path: "runtime-asset-script-test",
      }),
    );
  },
);
test("the scoped parser route skips unrelated TypeScript profiles", () => {
  const root = parserFixture();
  const unrelatedProfile = "scripts/quality/tsconfig-owned-native-width-trace.json";
  updateFixtureTypecheck(root, (script) => `${script} && tsc -p ${unrelatedProfile} --noEmit`);
  writeJson(root, unrelatedProfile, {
    compilerOptions: { strict: false },
    files: ["../tool.ts"],
  });
  track(root);

  const report = inspectParserBodyOnloadRoute(root);
  expect(report.binding.canonicalTypeScope).toBe(true);
  expect(report.binding.producerTypeScope).toBe(true);
  expect(report.issues).not.toContainEqual(
    expect.objectContaining({ code: "type-project-not-strict", path: unrelatedProfile }),
  );
  const fullReport = inspectOwnedValidationContract(root);
  expect(fullReport.issues).toContainEqual(
    expect.objectContaining({ code: "type-project-not-strict", path: unrelatedProfile }),
  );
});
test("the scoped parser route rejects a non-strict required profile", () => {
  const root = parserFixture();
  const profile = "scripts/quality/tsconfig-owned-native-html-parser.json";
  const configPath = path.join(root, profile);
  const config = JSON.parse(fs.readFileSync(configPath, "utf8")) as {
    compilerOptions: { strict: boolean };
    files: string[];
  };
  config.compilerOptions.strict = false;
  writeJson(root, profile, config);
  track(root);

  const report = inspectParserBodyOnloadRoute(root);
  expect(report.issues).toContainEqual(
    expect.objectContaining({ code: "type-project-not-strict", path: profile }),
  );
});
test("the scoped parser route rejects a missing required profile registration", () => {
  const root = parserFixture();
  const profile = "scripts/quality/tsconfig-owned-native-html-parser.json";
  updateFixtureTypecheck(root, (script) => script.replace(` && tsc -p ${profile} --noEmit`, ""));
  track(root);

  const report = inspectParserBodyOnloadRoute(root);
  expect(report.binding.canonicalTypeScope).toBe(false);
  expect(report.issues).toContainEqual(
    expect.objectContaining({
      code: "parser-body-onload-type-scope-unverified",
      path: report.binding.canonical,
    }),
  );
});
test("parser output binding fingerprints exact bytes and keeps byte verification explicit", () => {
  const root = parserFixture();
  const before = inspectOwnedValidationContract(root);
  expect(before.issues.filter((issue) => issue.code.startsWith("parser-body-onload-"))).toEqual([]);
  expect(before.parserBodyOnload.producerVerified).toBe(true);
  expect(before.parserBodyOnload.byteVerification).toBe("not-executed");
  write(root, before.parserBodyOnload.path, "const parserBodyOnload = false;\n");
  track(root);
  const after = inspectOwnedValidationContract(root);
  expect(after.parserBodyOnload.outputSha256).not.toBe(before.parserBodyOnload.outputSha256);
  expect(after.parserBodyOnload.inputDigest).not.toBe(before.parserBodyOnload.inputDigest);
  expect(after.sourceDigest).not.toBe(before.sourceDigest);
  expect(after.parserBodyOnload.byteVerification).toBe("not-executed");
});
test.each([
  ["write command", "bun scripts/runtime-bundles/parser-body-onload-generation.ts --write"],
  ["commented command", "# bun scripts/runtime-bundles/parser-body-onload-generation.ts --check"],
  [
    "shebang conditional command",
    "#!/bin/sh\nif false; then\nbun scripts/runtime-bundles/parser-body-onload-generation.ts --check;\nfi",
  ],
  [
    "extra successful no-op command",
    "bun -e 'process.exit(0)'\nbun scripts/runtime-bundles/parser-body-onload-generation.ts --check",
  ],
])("parser output binding rejects a check route changed to a %s", (_label, replacement) => {
  const root = parserFixture();
  const justfile = fs.readFileSync(path.join(root, "Justfile"), "utf8");
  write(
    root,
    "Justfile",
    justfile.replace(
      "bun scripts/runtime-bundles/parser-body-onload-generation.ts --check",
      replacement,
    ),
  );
  track(root);
  expect(codes(root)).toContain("parser-body-onload-check-unregistered:runtime-bundle-check");
});
test.each([
  [
    "exported entry initializer exits successfully before the entry",
    "if (import.meta.main) {",
    "export const earlySuccess = import.meta.main && process.exit(0);\n\nif (import.meta.main) {",
  ],
  [
    "generated output is compared with itself instead of the checked-in reference",
    "checkParserBodyOnload(generateParserBodyOnload(canonical, root), expected);",
    "checkParserBodyOnload(generateParserBodyOnload(canonical, root), generateParserBodyOnload(canonical, root) || expected);",
  ],
  [
    "top-level main guard exits before the check",
    "if (import.meta.main) {",
    "if (import.meta.main) {\n  process.exit(0);\n}\n\nif (import.meta.main) {",
  ],
  [
    "generation check function returns before its assertion",
    "export function checkParserBodyOnload(generated: string, expected: string): void {",
    "export function checkParserBodyOnload(generated: string, expected: string): void {\n  return;",
  ],
])("parser producer rejects early-success structure: %s", (_label, before, after) => {
  const root = parserFixture();
  const producer = "scripts/runtime-bundles/parser-body-onload-generation.ts";
  const source = fs.readFileSync(path.join(root, producer), "utf8");
  write(root, producer, source.replace(before, after));
  track(root);
  expect(codes(root)).toContain(
    "parser-body-onload-producer-unverified:crates/katana-render-runtime/src/renderer/backends/html_runtime/parser_body_onload.js",
  );
});
test("parser output binding rejects removal of the canonical TypeScript scope", () => {
  const root = parserFixture();
  writeJson(root, "scripts/quality/tsconfig-owned-native-html-parser.json", {
    compilerOptions: {
      strict: true,
      noImplicitAny: true,
      strictNullChecks: true,
      noUnusedLocals: true,
      noUnusedParameters: true,
      useUnknownInCatchVariables: true,
      lib: ["ES2024"],
      allowJs: true,
      checkJs: true,
    },
    files: ["../tool.ts"],
  });
  track(root);
  const report = inspectOwnedValidationContract(root);
  expect(report.issues).toContainEqual(
    expect.objectContaining({
      code: "parser-body-onload-type-scope-unverified",
      path: report.parserBodyOnload.canonical,
    }),
  );
  expect(report.issues).toContainEqual(
    expect.objectContaining({ code: "type-scope-missing", path: report.parserBodyOnload.path }),
  );
});
test.each([
  [
    'parser.add_argument("--moon", nargs="+", required=True)',
    'parser.add_argument("--moon", required=True)',
  ],
  ['"env": {**common_env, "CONTRACT_PHASE": "unit"}', '"env": common_env'],
  ["moon_command: list[str] = args.moon", 'moon_command: list[str] = ["moon"]'],
])("Moon fixture owner binding rejects changed source contract %s", (before, after) => {
  const root = fixture();
  const owner = "scripts/quality/moon_cache_contract_test.py";
  const source = fs.readFileSync(path.join(root, owner), "utf8");
  write(root, owner, source.replace(before, after));
  expect(codes(root)).toContain(
    `moon-cache-fixture-owner-unverified:tests/fixtures/quality-cache/moon-cache-subject.test.ts`,
  );
});
test("Moon fixture owner binding rejects removal of the scheduler failure-barrier assertion", () => {
  const root = fixture();
  const owner = "scripts/quality/moon_cache_contract_test.py";
  const source = fs.readFileSync(path.join(root, owner), "utf8");
  const barrierAssertion = "assert scheduler_failure_barrier_is_closed(failure_phases)";
  expect(source.split(barrierAssertion)).toHaveLength(2);
  write(root, owner, source.replace(barrierAssertion, "assert True"));
  const issueCodes = codes(root);
  expect(issueCodes).toContain(
    "moon-cache-fixture-owner-unverified:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
  );
  expect(issueCodes).toContain("moon-cache-fixture-owner-unregistered:runtime-asset-script-test");
});
test.each([
  ["direct return before CLI setup", "def main() -> int:\n", "def main() -> int:\n    return 0\n"],
  [
    "conditional return before CLI setup",
    "def main() -> int:\n",
    "def main() -> int:\n    if True:\n        return 0\n",
  ],
])("Moon fixture owner rejects early-success main: %s", (_label, before, after) => {
  const root = fixture();
  const owner = "scripts/quality/moon_cache_contract_test.py";
  const source = fs.readFileSync(path.join(root, owner), "utf8");
  write(root, owner, source.replace(before, after));
  expect(codes(root)).toContain(
    "moon-cache-fixture-owner-unverified:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
  );
});
test("Moon fixture itself must consume both owner environment values", () => {
  const root = fixture();
  const fixturePath = "tests/fixtures/quality-cache/moon-cache-subject.test.ts";
  const source = fs.readFileSync(path.join(root, fixturePath), "utf8");
  write(root, fixturePath, source.replace("process.env.CONTRACT_PHASE", "process.env.WRONG_PHASE"));
  expect(codes(root)).toContain(
    "moon-cache-fixture-input-unverified:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
  );
  expect(codes(root)).toContain(
    "unit-local-missing:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
  );
  expect(codes(root)).toContain(
    "unit-ci-missing:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
  );
});
test.each([
  [
    "python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon",
    "python3 scripts/quality/moon_cache_contract_test.py --moon /usr/bin/moon",
  ],
  [
    "python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon",
    "if false; then python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon; fi",
  ],
  [
    "python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon",
    "#!/bin/sh\nif false; then\npython3 scripts/quality/moon_cache_contract_test.py --moon bun run moon;\nfi",
  ],
  [
    "python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon",
    "# python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon",
  ],
  [
    "python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon",
    "bun -e 'process.exit(0)'\npython3 scripts/quality/moon_cache_contract_test.py --moon bun run moon",
  ],
])("Moon fixture owner binding rejects altered recipe command %s", (before, after) => {
  const root = fixture();
  const body = actualRecipeBody("runtime-asset-script-test");
  recipes(root, { "runtime-asset-script-test": body.replace(before, after) });
  const issueCodes = codes(root);
  expect(issueCodes).toContain("moon-cache-fixture-owner-command-unverified:Justfile");
  expect(issueCodes).toContain("moon-cache-fixture-owner-unregistered:runtime-asset-script-test");
  expect(issueCodes).toContain("parser-body-onload-test-unregistered:runtime-asset-script-test");
  expect(issueCodes).toContain(
    "unit-local-missing:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
  );
  expect(issueCodes).toContain(
    "unit-ci-missing:tests/fixtures/quality-cache/moon-cache-subject.test.ts",
  );
});
test.each(["local", "ci"] as const)(
  "Moon fixture owner cannot register only one lane: %s",
  (lane) => {
    const root = fixture();
    const base = fixtureRecipeBodies()["check-standard"] ?? "";
    const ownerCall = "\njust runtime-asset-script-test";
    const localBody = lane === "local" ? base.replace(ownerCall, "") : base;
    const ciBody = lane === "ci" ? base.replace(ownerCall, "") : base;
    recipes(root, { "check-standard": localBody, "ci-standard": ciBody });
    write(
      root,
      ".github/workflows/check.yml",
      "jobs:\n  check:\n    steps:\n      - name: standard\n        run: just ci-standard\n",
    );
    const report = inspectOwnedValidationContract(root);
    const missingLane = lane === "local" ? "unit-local-missing" : "unit-ci-missing";
    expect(report.issues).toContainEqual(
      expect.objectContaining({
        code: missingLane,
        path: "tests/fixtures/quality-cache/moon-cache-subject.test.ts",
      }),
    );
  },
);
test("Moon fixture source bytes contribute to the owned contract fingerprint", () => {
  const root = fixture();
  const before = inspectOwnedValidationContract(root);
  const fixturePath = "tests/fixtures/quality-cache/moon-cache-subject.test.ts";
  write(
    root,
    fixturePath,
    `${fs.readFileSync(path.join(root, fixturePath), "utf8")}\n// changed\n`,
  );
  const after = inspectOwnedValidationContract(root);
  expect(after.moonCacheFixture.fixtureSha256).not.toBe(before.moonCacheFixture.fixtureSha256);
  expect(after.sourceDigest).not.toBe(before.sourceDigest);
});
test.each([
  ["noUnusedLocals", false],
  ["noUnusedLocals", undefined],
  ["noUnusedParameters", false],
  ["noUnusedParameters", undefined],
  ["useUnknownInCatchVariables", false],
  ["useUnknownInCatchVariables", undefined],
] as const)("strict TypeScript project rejects weakened %s=%s", (option, value) => {
  const root = fixture();
  const compilerOptions: Record<string, unknown> = {
    strict: true,
    noUnusedLocals: true,
    noUnusedParameters: true,
    useUnknownInCatchVariables: true,
    lib: ["es2024"],
    allowJs: true,
    checkJs: true,
  };
  if (value === undefined) delete compilerOptions[option];
  else compilerOptions[option] = value;
  writeJson(root, "tsconfig.json", {
    compilerOptions,
    include: ["scripts/**/*.ts", "scripts/**/*.js", "tests/fixtures/quality-cache/*.ts"],
  });
  const report = inspectOwnedValidationContract(root);
  expect(report.issues).toContainEqual(
    expect.objectContaining({
      code: "type-project-strict-check-missing",
      path: "tsconfig.json",
      detail: option,
    }),
  );
});
test("generated Rust fingerprint binds same-path resource bytes and added or removed paths", () => {
  const root = fixture();
  const before = inspectOwnedValidationContract(root);
  const shape =
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/shapes/basic.xml";
  write(root, shape, "changed resource bytes");
  track(root);
  const changed = inspectOwnedValidationContract(root);
  expect(changed.generatedRustAssets.inputCount).toBe(before.generatedRustAssets.inputCount);
  expect(changed.generatedRustAssets.inputDigest).not.toBe(before.generatedRustAssets.inputDigest);
  expect(changed.sourceDigest).not.toBe(before.sourceDigest);

  const added =
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/shapes/added.xml";
  write(root, added, "additional same-group resource");
  track(root);
  const withAdded = inspectOwnedValidationContract(root);
  expect(withAdded.generatedRustAssets.inputCount).toBe(before.generatedRustAssets.inputCount + 1);
  expect(withAdded.generatedRustAssets.inputDigest).not.toBe(
    changed.generatedRustAssets.inputDigest,
  );
  expect(withAdded.generatedRustAssets.paths).toEqual(before.generatedRustAssets.paths);

  fs.rmSync(path.join(root, added));
  track(root);
  const withRemoved = inspectOwnedValidationContract(root);
  expect(withRemoved.generatedRustAssets.inputCount).toBe(before.generatedRustAssets.inputCount);
  expect(withRemoved.generatedRustAssets.inputDigest).toBe(changed.generatedRustAssets.inputDigest);
});
test("generated Rust resource symlinks fail closed during source fingerprinting", () => {
  const root = fixture();
  const outsideRoot = fs.mkdtempSync(path.join(os.tmpdir(), "krr-generated-rust-outside-"));
  fixtures.push(outsideRoot);
  const outside = path.join(outsideRoot, "outside-resource.xml");
  const linked = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/shapes/linked.xml",
  );
  fs.writeFileSync(outside, "outside target");
  fs.symlinkSync(outside, linked);
  track(root);
  expect(codes(root)).toContain(
    "unsafe-or-missing-source:crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/shapes/linked.xml",
  );
});
test("unknown and missing generated Rust outputs fail closed", () => {
  const root = fixture();
  const unexpected =
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/handwritten.rs";
  write(root, unexpected, "pub const UNBOUND: u8 = 0;\n");
  track(root);
  expect(codes(root)).toContain(`generated-rust-output-unexpected:${unexpected}`);

  const expected = generatedRuntimeRustAssetPaths(root)[0];
  if (expected === undefined) throw new Error("expected generated Rust output path");
  fs.rmSync(path.join(root, expected));
  expect(codes(root)).toContain(`unsafe-or-missing-source:${expected}`);
});
test.each([
  `#!/bin/sh\n${recipeBodies["runtime-package-asset-check"] ?? ""}`,
  `if false; then\n${recipeBodies["runtime-package-asset-check"] ?? ""}\nfi`,
  `${recipeBodies["runtime-package-asset-check"] ?? ""}\n# hidden comment`,
  `${recipeBodies["runtime-package-asset-check"] ?? ""}\necho extra command`,
  (recipeBodies["runtime-package-asset-check"] ?? "").replace("--check", "--check >/dev/null"),
  (recipeBodies["runtime-package-asset-check"] ?? "").replace("--check", "--write"),
])("generated Rust byte-check requires its canonical complete recipe body", (body) => {
  const root = fixture();
  recipes(root, { "runtime-package-asset-check": body });
  const report = inspectOwnedValidationContract(root);
  expect(report.issues.map((issue) => `${issue.code}:${issue.path}`)).toContain(
    "generated-rust-check-command-unverified:Justfile",
  );
  expect(report.generatedRustAssets.localCheckRegistered).toBe(false);
  expect(report.generatedRustAssets.ciCheckRegistered).toBe(false);
});
test("generated Rust byte-check must be reachable from local and CI checks", () => {
  const root = fixture();
  recipes(root, {
    "check-standard":
      fixtureRecipeBodies()["check-standard"]?.replace("\njust runtime-package-asset-check", "") ??
      "",
  });
  write(
    root,
    ".github/workflows/check.yml",
    "jobs:\n  check:\n    steps:\n      - name: standard\n        run: just unit\n",
  );
  track(root);
  expect(codes(root)).toContain("generated-rust-check-unregistered:runtime-package-asset-check");
});
test("official Tomli files, provenance, and its strict Python 3.8 Ruff config are fingerprinted", () => {
  const root = tomliVendorFixture();
  const report = inspectOwnedValidationContract(root);
  expect(report.issues).toEqual([]);
  expect(report.sourceDigest).toMatch(/^[a-f0-9]{64}$/);
  const originalDigest = report.sourceDigest;
  write(
    root,
    "scripts/release/_tomli/ruff.toml",
    'target-version = "py39"\nrequired-version = "==0.16.10"\n',
  );
  track(root);
  const mutated = inspectOwnedValidationContract(root);
  expect(mutated.sourceDigest).not.toBe(originalDigest);
  expect(mutated.issues).toContainEqual({
    code: "tomli-ruff-config-invalid",
    path: "scripts/release/_tomli/ruff.toml",
    detail: "",
  });
});
test("Tomli provenance bytes contribute to the source fingerprint", () => {
  const root = tomliVendorFixture();
  const original = inspectOwnedValidationContract(root);
  const provenancePath = path.join(root, "scripts/release/_tomli/upstream-provenance.json");
  const provenance = JSON.parse(fs.readFileSync(provenancePath, "utf8"));
  fs.writeFileSync(provenancePath, `${JSON.stringify(provenance, null, 4)}\n`);
  track(root);
  const reformatted = inspectOwnedValidationContract(root);
  expect(reformatted.issues).toEqual([]);
  expect(reformatted.sourceDigest).not.toBe(original.sourceDigest);
});
test("Tomli provenance file pins compare by keys and values, independent of JSON property order", () => {
  const root = tomliVendorFixture();
  const provenancePath = path.join(root, "scripts/release/_tomli/upstream-provenance.json");
  const provenance = JSON.parse(fs.readFileSync(provenancePath, "utf8"));
  provenance.files = Object.fromEntries(Object.entries(provenance.files).reverse());
  fs.writeFileSync(provenancePath, `${JSON.stringify(provenance, null, 2)}\n`);
  track(root);
  expect(inspectOwnedValidationContract(root).issues).toEqual([]);
});
test("Tomli Ruff and provenance authority must be Git tracked", () => {
  const root = tomliVendorFixture(false);
  const missingAuthorities = codes(root);
  expect(missingAuthorities).toContain("untracked-authority:scripts/release/_tomli/ruff.toml");
  expect(missingAuthorities).toContain(
    "untracked-authority:scripts/release/_tomli/upstream-provenance.json",
  );
});
test.each([
  ['target-version = "py39"\nrequired-version = "==0.16.10"\n', "tomli-ruff-config-invalid"],
  ['target-version = "py38"\nrequired-version = "==0.16.9"\n', "tomli-ruff-config-invalid"],
  [
    'target-version = "py38"\nrequired-version = "==0.16.10"\nignore = ["F"]\n',
    "tomli-ruff-config-invalid",
  ],
  [
    'target-version = "py38"\nrequired-version = "==0.16.10"\nselect = ["F"]\n',
    "tomli-ruff-config-invalid",
  ],
  [
    'target-version = "py38"\nrequired-version = "==0.16.10"\nexclude = ["*"]\n',
    "tomli-ruff-config-invalid",
  ],
])("Tomli Ruff config cannot weaken or alter static checks: %s", (source, expected) => {
  const root = tomliVendorFixture();
  write(root, "scripts/release/_tomli/ruff.toml", source);
  track(root);
  expect(codes(root)).toContain(`${expected}:scripts/release/_tomli/ruff.toml`);
});
test("unknown Git-visible Ruff configs are rejected", () => {
  const root = tomliVendorFixture();
  write(root, "scripts/release/ruff.toml", 'target-version = "py38"\n');
  write(root, "ruff.toml", 'target-version = "py38"\n');
  write(root, ".ruff.toml", 'target-version = "py38"\n');
  track(root);
  const initialIssueCodes = codes(root);
  expect(initialIssueCodes).toContain("unknown-ruff-config:scripts/release/ruff.toml");
  expect(initialIssueCodes).toContain("unknown-ruff-config:ruff.toml");
  expect(initialIssueCodes).toContain("unknown-ruff-config:.ruff.toml");
  write(root, "pyproject.toml", '[tool.ruff]\ntarget-version = "py38"\n');
  track(root);
  expect(codes(root)).toContain("unknown-ruff-config:pyproject.toml");
});
test("ignored Ruff ancestor configs are rejected and fingerprinted", () => {
  const root = tomliVendorFixture();
  const before = inspectOwnedValidationContract(root);
  write(root, ".gitignore", "tmp/\nnode_modules/\nscripts/ruff.toml\n");
  write(root, "scripts/ruff.toml", 'ignore = ["F"]\n');
  track(root);
  const index = readOwnedRepositoryIndex(root);
  expect(index.tracked).not.toContain("scripts/ruff.toml");
  expect(index.untracked).not.toContain("scripts/ruff.toml");
  const report = inspectOwnedValidationContract(root);
  expect(report.issues).toContainEqual({
    code: "unknown-ruff-config",
    path: "scripts/ruff.toml",
    detail: "",
  });
  expect(report.sourceDigest).not.toBe(before.sourceDigest);
});
test("active XDG Ruff config is rejected and fingerprinted", () => {
  const originalXdgConfigHome = process.env.XDG_CONFIG_HOME;
  const configHome = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-ruff-xdg-")));
  fixtures.push(configHome);
  process.env.XDG_CONFIG_HOME = configHome;
  try {
    const root = tomliVendorFixture();
    const before = inspectOwnedValidationContract(root);
    const configPath = path.join(configHome, "ruff", "ruff.toml");
    fs.mkdirSync(path.dirname(configPath), { recursive: true });
    fs.writeFileSync(configPath, 'ignore = ["F"]\n');
    const report = inspectOwnedValidationContract(root);
    expect(report.issues).toContainEqual({
      code: "unknown-ruff-config",
      path: configPath,
      detail: "",
    });
    expect(report.sourceDigest).not.toBe(before.sourceDigest);
  } finally {
    if (originalXdgConfigHome === undefined) delete process.env.XDG_CONFIG_HOME;
    else process.env.XDG_CONFIG_HOME = originalXdgConfigHome;
  }
});
test("Tomli provenance and official file bytes cannot drift from the wheel pins", () => {
  const root = tomliVendorFixture();
  write(root, "scripts/release/_tomli/upstream-provenance.json", '{"distribution":"tomli"}\n');
  track(root);
  expect(codes(root)).toContain(
    "tomli-provenance-invalid:scripts/release/_tomli/upstream-provenance.json",
  );
  write(root, "scripts/release/_tomli/_parser.py", "# changed bytes\n");
  track(root);
  expect(codes(root)).toContain("tomli-official-file-mismatch:scripts/release/_tomli/_parser.py");
});
test("standard Biome bootstrap binds current verify and the maintained full gate", () => {
  const root = fixture();
  recipes(root, { biome: actualRecipeBody("biome"), "biome-run": actualRecipeBody("biome-run") });
  expect(codes(root)).not.toContain("biome-dispatch-unverified:Justfile");
});
test.each([
  ["host acquisition", " verify ", " prepare "],
  ["current receipt", "/current/receipt.json", "/stale/receipt.json"],
  ["full gate", "runtime-unused-check.ts --gate", "runtime-unused-check.ts"],
  [
    "unknown allowance",
    "runtime-unused-check.ts --gate",
    "runtime-unused-check.ts --gate --allow-unknown",
  ],
  ["raw Biome", "bun scripts/runtime-bundles/runtime-unused-check.ts --gate", "bun run biome"],
])("standard Biome bootstrap rejects %s bypass", (_label, before, after) => {
  const root = fixture(),
    body = actualRecipeBody("biome-run");
  expect(body.replace(before, after)).not.toBe(body);
  recipes(root, { biome: actualRecipeBody("biome"), "biome-run": body.replace(before, after) });
  expect(codes(root)).toContain("biome-dispatch-unverified:Justfile");
});
test.each([
  ["biome", "bun run typecheck", "biome-dispatch-unverified:Justfile"],
  ["typecheck", "bun run biome", "typecheck-dispatch-unverified:Justfile"],
  [
    "lint",
    "cargo clippy --workspace --all-targets",
    "rust-lint-unregistered:crates/demo/src/lib.rs",
  ],
  [
    "python-lint",
    "uvx --from ruff==0.16.10 ruff check scripts",
    "python-lint-unregistered:scripts/tool_test.py",
  ],
  [
    "python-typecheck",
    "uvx --from mypy==2.4.0 mypy scripts",
    "python-type-unregistered:scripts/tool_test.py",
  ],
  ["unit", "bun test scripts/tool.ts", "unit-local-missing:scripts/tool_test.ts"],
  ["unit", "bun test scripts", "unit-local-missing:scripts/tool_test.ts"],
  [
    "unit",
    "bun test --path-ignore-patterns 'scripts/**' scripts",
    "unit-ci-missing:scripts/tool_test.ts",
  ],
  ["unit", "bun test -t value scripts", "unit-local-missing:scripts/tool_test.ts"],
  ["unit", "echo bun test scripts", "unit-local-missing:scripts/tool_test.ts"],
  ["unit", "bun test --path-ignore-patterns", "unit-local-missing:scripts/tool_test.ts"],
  [
    "lint",
    "echo cargo clippy --workspace --all-targets --all-features",
    "rust-lint-unregistered:crates/demo/src/lib.rs",
  ],
  [
    "lint",
    "cargo clippy --workspace --all-targets --all-features -p demo",
    "rust-lint-unregistered:crates/demo/src/lib.rs",
  ],
  ["unit", "bun test --test-name-pattern=value scripts", "unit-local-missing:scripts/tool_test.ts"],
  ["unit", "if false; then bun test scripts; fi", "unit-local-missing:scripts/tool_test.ts"],
  [
    "unit",
    "python3 -m unittest discover -s scripts -p 'test*.py'",
    "unit-ci-missing:scripts/tool_test.py",
  ],
])("standard %s recipe cannot hide a missing actual contract", (name, body, expected) => {
  const root = fixture();
  recipes(root, { [name]: body });
  expect(codes(root)).toContain(expected);
});
test.each([
  [
    "biome.json",
    { files: { includes: ["scripts/tool.ts"] } },
    "biome-scope-missing:scripts/tool_test.ts",
  ],
  [
    "biome.json",
    { files: { includes: ["scripts/**", "!scripts/tool_test.ts"] } },
    "biome-scope-missing:scripts/tool_test.ts",
  ],
  [
    "biome.json",
    {
      files: { includes: ["scripts/**"] },
      overrides: [{ includes: ["scripts/*_test.ts"], linter: { enabled: false } }],
    },
    "biome-scope-missing:scripts/tool_test.ts",
  ],
  [
    "tsconfig.json",
    { compilerOptions: { strict: true }, files: ["scripts/tool.ts"] },
    "type-scope-missing:scripts/tool_test.ts",
  ],
  [
    "tsconfig.json",
    { compilerOptions: { strict: false }, include: ["scripts/**/*.ts"] },
    "type-project-not-strict:tsconfig.json",
  ],
])("actual static file selection rejects %s omission", (name, config, expected) => {
  const root = fixture();
  writeJson(root, name, config);
  expect(codes(root)).toContain(expected);
});
test.each([
  ["scripts/untyped-validation.js", true],
  ["crates/demo/html_runtime/probe_test.js", true],
  ["crates/demo/html_runtime/probe.test.js", true],
  ["crates/demo/html_runtime/probe.test.jsx", true],
  ["crates/demo/html_runtime/probe_spec.mjs", true],
  ["crates/demo/html_runtime/probe.spec.cjs", true],
  ["crates/demo/tests/fixtures/browser/action.js", true],
  ["crates/demo/html_runtime/legacy.js", false],
  ["crates/demo/unknown.js", true],
] as const)("strict JS scope keeps role and deferred status: %s", (name, mandatory) => {
  const root = fixture();
  write(root, name, "globalThis.ProbeValue = 1;\n");
  writeJson(root, "biome.json", { files: { includes: ["scripts/**", "crates/**"] } });
  const config = z
    .looseObject({ exclude: z.array(z.string()).optional() })
    .parse(JSON.parse(fs.readFileSync(path.join(root, "tsconfig.json"), "utf8")));
  config.exclude = [...(config.exclude ?? []), name];
  writeJson(root, "tsconfig.json", config);
  track(root);
  const report = inspectOwnedValidationContract(root);
  const missing = report.issues.filter(
    (issue) => issue.code === "type-scope-missing" && issue.path === name,
  );
  expect(missing.length > 0).toBe(mandatory);
  expect(report.javascriptTypeCoverage.mandatoryPaths.includes(name)).toBe(mandatory);
  expect(report.javascriptTypeCoverage.deferredProductPaths.includes(name)).toBe(!mandatory);
  expect(report.javascriptTypeCoverage.profileMatchedPaths).not.toContain(name);
  expect(report.javascriptTypeCoverage.verification).toBe("not-executed");
  expect(report.javascriptTypeCoverage.wholeJavaScriptStrict).toBe("not-claimed");
  expect(report.languageCounts.javascript).toBeGreaterThan(0);
  if (name === "crates/demo/unknown.js")
    expect(report.issues).toContainEqual(
      expect.objectContaining({ code: "js-environment-unclassified", path: name }),
    );
});

test("an established product JS profile cannot silently become deferred", () => {
  const root = fixture();
  const utility =
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_bounds_attrs.js";
  write(root, utility, "function typedUtility(value) { return value; }\n");
  writeJson(root, "biome.json", { files: { includes: ["scripts/**", "crates/**/*.js"] } });
  track(root);
  const report = inspectOwnedValidationContract(root);
  expect(report.javascriptTypeCoverage.profileMatchedPaths).toContain(utility);
  expect(report.javascriptTypeCoverage.mandatoryPaths).toContain(utility);
  expect(report.javascriptTypeCoverage.deferredProductPaths).not.toContain(utility);
  expect(report.issues).toContainEqual(
    expect.objectContaining({ code: "native-product-type-profile-unverified", path: utility }),
  );
});

const browserReferenceDirectory =
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime";
const browserReferenceHelpers = [`${browserReferenceDirectory}/mermaid_zenuml_bridge.js`];
const browserReferenceHost = `${browserReferenceDirectory}/browser_reference_host.d.ts`;
const browserReferenceLeaf = "scripts/mermaid/official-renderer-browser-globals.d.ts";
const browserReferenceProfile = "scripts/quality/tsconfig-owned-browser-reference.json";
function browserReferenceFixture(): string {
  const root = fixture();
  const actual = path.resolve(import.meta.dir, "../..");
  for (const name of [
    ...browserReferenceHelpers,
    browserReferenceHost,
    browserReferenceLeaf,
    browserReferenceProfile,
    "scripts/quality/tsconfig-owned-native-pure-utilities.json",
    "scripts/mermaid/official-renderer-i18n.ts",
    "scripts/mermaid/official-renderer.ts",
    `${browserReferenceDirectory}/dom_text_metrics.js`,
  ])
    write(root, name, fs.readFileSync(path.join(actual, name), "utf8"));
  const pkg = z
    .looseObject({ scripts: z.record(z.string(), z.string()) })
    .parse(JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")));
  pkg.scripts.typecheck += ` && tsc --noEmit -p ${browserReferenceProfile}`;
  writeJson(root, "package.json", pkg);
  writeJson(root, "biome.json", { files: { includes: ["scripts/**", "crates/**", "tests/**"] } });
  track(root);
  return root;
}
function browserReferenceIssue(root: string, code: string): void {
  const report = inspectOwnedValidationContract(root);
  expect(report.issues).toContainEqual(
    expect.objectContaining({ code, path: browserReferenceProfile }),
  );
  for (const name of browserReferenceHelpers)
    expect(report.javascriptTypeCoverage.deferredProductPaths).not.toContain(name);
}

test("browser reference adapters bind real consumers and exact isolated compiler closure", () => {
  const report = inspectOwnedValidationContract(browserReferenceFixture());
  expect(report.browserReferenceHelpers).toMatchObject({
    consumerVerified: true,
    typeProfileVerified: true,
    closureVerified: true,
    compilerLibraryCount: 69,
    compiledOwnedSources: [
      ...browserReferenceHelpers,
      browserReferenceHost,
      browserReferenceLeaf,
    ].sort(),
  });
  for (const name of browserReferenceHelpers)
    expect(report.javascriptTypeCoverage.mandatoryPaths).toContain(name);
  expect(report.issues.filter((issue) => issue.code.startsWith("browser-reference-"))).toEqual([]);
});
test.each([
  [
    "scripts/mermaid/official-renderer-i18n.ts",
    '"dom_text_metrics.js",\n  "svg_bounds_attrs.js",',
    '"svg_bounds_attrs.js",\n  "dom_text_metrics.js",',
  ],
  [
    "scripts/mermaid/official-renderer.ts",
    "await page.addScriptTag({ path: this.options.mermaidZenumlJs });",
    "await page.addScriptTag({ path: MermaidRuntimeScripts.zenumlBridgePath() });",
  ],
  [
    `${browserReferenceDirectory}/dom_text_metrics.js`,
    "KATANA_ASCII_TEXT_WIDTHS",
    "UNREVIEWED_ASCII_TEXT_WIDTHS",
  ],
] as const)(
  "browser reference source guard rejects provider/caller drift: %s",
  (name, original, changed) => {
    const root = browserReferenceFixture();
    const file = path.join(root, name);
    const source = fs.readFileSync(file, "utf8");
    expect(source).toContain(original);
    write(root, name, source.replace(original, changed));
    browserReferenceIssue(root, "browser-reference-consumer-unverified");
  },
);
test("browser reference dedicated profile cannot fall back to generic JS coverage", () => {
  const root = browserReferenceFixture();
  const pkg = z
    .looseObject({ scripts: z.record(z.string(), z.string()) })
    .parse(JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")));
  pkg.scripts.typecheck =
    pkg.scripts.typecheck?.replace(` && tsc --noEmit -p ${browserReferenceProfile}`, "") ?? "";
  writeJson(root, "package.json", pkg);
  writeJson(root, "tsconfig.json", {
    compilerOptions: {
      strict: true,
      noUnusedLocals: true,
      noUnusedParameters: true,
      useUnknownInCatchVariables: true,
      allowJs: true,
      checkJs: true,
    },
    include: ["scripts/**/*.ts", "scripts/**/*.js", "crates/**/*.js"],
  });
  browserReferenceIssue(root, "browser-reference-type-profile-unverified");
});
test.each([
  ["lib", ["ES2024", "DOM"]],
  ["types", ["bun"]],
  ["strict", false],
  ["checkJs", false],
  ["noUnusedLocals", false],
  ["useUnknownInCatchVariables", false],
  ["noUncheckedIndexedAccess", false],
  ["skipLibCheck", true],
  ["skipDefaultLibCheck", true],
] as const)("browser reference dedicated profile rejects %s weakening", (name, value) => {
  const root = browserReferenceFixture();
  mutateMathJaxOptions(root, browserReferenceProfile, { [name]: value });
  browserReferenceIssue(root, "browser-reference-type-profile-unverified");
});
test.each(["extra-owned", "DOM"] as const)(
  "browser reference actual compiler closure rejects %s",
  (kind) => {
    const root = browserReferenceFixture();
    if (kind === "DOM")
      write(
        root,
        browserReferenceHost,
        `/// <reference lib="dom" />\n${fs.readFileSync(path.join(root, browserReferenceHost), "utf8")}`,
      );
    else {
      const helper = browserReferenceHelpers.find((name) =>
        name.endsWith("mermaid_zenuml_bridge.js"),
      );
      if (!helper) throw new Error("bridge fixture missing");
      write(
        root,
        helper,
        `${fs.readFileSync(path.join(root, helper), "utf8")}\n/** @type {import("./unreviewed-types").Unexpected} */ const extra = {};\n`,
      );
      write(
        root,
        `${browserReferenceDirectory}/unreviewed-types.d.ts`,
        "export interface Unexpected {}\n",
      );
      track(root);
    }
    browserReferenceIssue(root, "browser-reference-closure-unverified");
  },
);

const nativeWidthTraceDirectory = "crates/katana-render-runtime/src/markdown/mermaid_renderer";
const nativeWidthTracePath = `${nativeWidthTraceDirectory}/js_runtime_native_width_trace.js`;
const nativeWidthHostPath = `${nativeWidthTraceDirectory}/js_runtime_native_width_trace_host.d.ts`;
const nativeWidthProfilePath = "scripts/quality/tsconfig-owned-native-width-trace.json";
function nativeWidthTraceFixture(): string {
  const root = fixture();
  const actual = path.resolve(import.meta.dir, "../..");
  for (const name of [
    nativeWidthTracePath,
    nativeWidthHostPath,
    `${nativeWidthTraceDirectory}/js_runtime.rs`,
    `${nativeWidthTraceDirectory}/js_runtime_native_width_tests.rs`,
    `${nativeWidthTraceDirectory}/js_runtime_native_width_oracle_boundary_tests.rs`,
    `${nativeWidthTraceDirectory}/js_runtime_native_width_helpers.rs`,
    nativeWidthProfilePath,
    "scripts/quality/tsconfig-owned-native-pure-utilities.json",
  ])
    write(root, name, fs.readFileSync(path.join(actual, name), "utf8"));
  const pkg = z
    .looseObject({ scripts: z.record(z.string(), z.string()) })
    .parse(JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")));
  pkg.scripts.typecheck += ` && tsc --noEmit -p ${nativeWidthProfilePath}`;
  writeJson(root, "package.json", pkg);
  writeJson(root, "biome.json", { files: { includes: ["scripts/**", "crates/**", "tests/**"] } });
  track(root);
  return root;
}

test("native width isolated compiler rejects wrong bridge, node and DOM contracts", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "native-width-types-"));
  fixtures.push(root);
  const actual = path.resolve(import.meta.dir, "../..");
  const negative = path.join(root, "negative.js");
  write(
    root,
    "negative.js",
    [
      '__katanaMeasureText = () => "wrong";',
      "__katanaMeasureSvgTextMetrics = () => ({ advance: 1, ascent: 2 });",
      "document.body;",
      'setProbeFont(1, "bold");',
    ].join("\n"),
  );
  writeJson(root, "tsconfig.json", {
    extends: path.join(actual, nativeWidthProfilePath),
    files: [
      path.join(actual, nativeWidthTracePath),
      path.join(actual, nativeWidthHostPath),
      negative,
    ],
  });
  let diagnostics = "";
  try {
    execFileSync(
      "bunx",
      ["--no-install", "tsc", "--noEmit", "-p", path.join(root, "tsconfig.json")],
      {
        cwd: actual,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
  } catch (error) {
    const failed = z.object({ status: z.number(), stdout: z.string() }).parse(error);
    expect(failed.status).toBeGreaterThan(0);
    diagnostics = failed.stdout;
  }
  for (const line of [1, 2, 3, 4]) expect(diagnostics).toContain(`negative.js(${line},`);
  expect(diagnostics).toContain("NativeWidthSvgMetrics");
  expect(diagnostics).toContain("NativeWidthTraceDocument");
  expect(diagnostics).toContain("NativeWidthTraceNode");
});

test("actual Rust native width test trace remains mandatory with isolated strict types", () => {
  const report = inspectOwnedValidationContract(nativeWidthTraceFixture());
  expect(report.nativeWidthTrace).toMatchObject({
    present: true,
    consumerVerified: true,
    typeProfileVerified: true,
  });
  expect(report.nativeWidthTrace.consumerSources).toEqual([
    {
      path: `${nativeWidthTraceDirectory}/js_runtime.rs`,
      sha256: "f72e281fa5ebc48344c43a872014b8fdcea898eacfcd7ea0685454b02d9e4385",
    },
    {
      path: `${nativeWidthTraceDirectory}/js_runtime_native_width_tests.rs`,
      sha256: "64cfc2be0e4369098cde79d1faad8e8539fc5681945bf15fc6ed4d4d267603c0",
    },
    {
      path: `${nativeWidthTraceDirectory}/js_runtime_native_width_helpers.rs`,
      sha256: "3564e66c024a3540569e41c495d07bd49de2f72363b085277a61af54ab3622fc",
    },
    {
      path: `${nativeWidthTraceDirectory}/js_runtime_native_width_oracle_boundary_tests.rs`,
      sha256: "a52369ca8ded04fe4043bfd79962266621efef8518bfb06ff19180ff5230dc92",
    },
  ]);
  expect(report.javascriptTypeCoverage.mandatoryPaths).toContain(nativeWidthTracePath);
  expect(report.javascriptTypeCoverage.deferredProductPaths).not.toContain(nativeWidthTracePath);
  expect(report.issues.filter((issue) => issue.code.startsWith("native-test-trace-"))).toEqual([]);
});

test.each([
  [
    "js_runtime.rs",
    '#[cfg(test)]\n#[path = "js_runtime_native_width_tests.rs"]',
    '#[path = "js_runtime_native_width_tests.rs"]',
  ],
  [
    "js_runtime_native_width_tests.rs",
    '#[path = "js_runtime_native_width_helpers.rs"]',
    '#[path = "unreviewed_helpers.rs"]',
  ],
  [
    "js_runtime_native_width_helpers.rs",
    'include_str!("js_runtime_native_width_trace.js")',
    'include_str!("unreviewed_trace.js")',
  ],
  ["js_runtime_native_width_helpers.rs", "probes: nativeWidthProbeResult()", "probes: {}"],
  [
    "js_runtime_native_width_oracle_boundary_tests.rs",
    "DiagramRuntimeScript::borrowed(script_label, script)",
    'DiagramRuntimeScript::borrowed("unreviewed-native-width.js", script)',
  ],
  [
    "js_runtime_native_width_oracle_boundary_tests.rs",
    "const cssArgs = ['AV', 'Noto Sans', 16, 400, false];",
    "const cssArgs = ['AV', 'Noto Sans', 17, 400, false];",
  ],
] as const)("Rust test trace rejects actual consumer drift: %s %s", (name, original, changed) => {
  const root = nativeWidthTraceFixture();
  const filename = `${nativeWidthTraceDirectory}/${name}`;
  const source = fs.readFileSync(path.join(root, filename), "utf8");
  expect(source).toContain(original);
  write(root, filename, source.replace(original, changed));
  const report = inspectOwnedValidationContract(root);
  expect(report.nativeWidthTrace.consumerVerified).toBe(false);
  expect(report.javascriptTypeCoverage.deferredProductPaths).not.toContain(nativeWidthTracePath);
  expect(report.issues).toContainEqual(
    expect.objectContaining({
      code: "native-test-trace-consumer-unverified",
      path: nativeWidthTracePath,
    }),
  );
});

test.each([
  nativeWidthTracePath,
  `${nativeWidthTraceDirectory}/js_runtime_native_width_helpers.rs`,
  `${nativeWidthTraceDirectory}/js_runtime_native_width_oracle_boundary_tests.rs`,
])("Rust test trace rejects missing reviewed input: %s", (name) => {
  const root = nativeWidthTraceFixture();
  fs.unlinkSync(path.join(root, name));
  const report = inspectOwnedValidationContract(root);
  if (name !== nativeWidthTracePath) expect(report.nativeWidthTrace.consumerVerified).toBe(false);
  expect(report.issues).toContainEqual(
    expect.objectContaining({
      code: "unsafe-or-missing-source",
      path: name,
    }),
  );
  expect(report.javascriptTypeCoverage.deferredProductPaths).not.toContain(nativeWidthTracePath);
});

test("actual Rust test trace cannot fall back to another JS profile", () => {
  const root = nativeWidthTraceFixture();
  const pkg = z
    .looseObject({ scripts: z.record(z.string(), z.string()) })
    .parse(JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")));
  pkg.scripts.typecheck =
    pkg.scripts.typecheck?.replace(` && tsc --noEmit -p ${nativeWidthProfilePath}`, "") ?? "";
  writeJson(root, "package.json", pkg);
  const report = inspectOwnedValidationContract(root);
  expect(report.javascriptTypeCoverage.mandatoryPaths).toContain(nativeWidthTracePath);
  expect(report.nativeWidthTrace.typeProfileVerified).toBe(false);
  expect(report.issues).toContainEqual(
    expect.objectContaining({
      code: "native-test-trace-type-profile-unverified",
      path: nativeWidthTracePath,
    }),
  );
});

test.each([
  ["lib", ["ES2024", "DOM"]],
  ["types", ["node"]],
  ["checkJs", false],
  ["noUncheckedIndexedAccess", false],
  ["exactOptionalPropertyTypes", false],
] as const)("Rust test trace cannot weaken isolated %s", (name, value) => {
  const root = nativeWidthTraceFixture();
  const config = z
    .looseObject({ compilerOptions: z.record(z.string(), z.unknown()).optional() })
    .parse(JSON.parse(fs.readFileSync(path.join(root, nativeWidthProfilePath), "utf8")));
  config.compilerOptions = { [name]: value };
  writeJson(root, nativeWidthProfilePath, config);
  const report = inspectOwnedValidationContract(root);
  expect(report.nativeWidthTrace.typeProfileVerified).toBe(false);
  expect(report.issues).toContainEqual(
    expect.objectContaining({
      code: "native-test-trace-type-profile-unverified",
      path: nativeWidthTracePath,
    }),
  );
});

test("JS environment does not accept ordinary DOM types as a native host contract", () => {
  const root = fixture();
  const host = "crates/demo/html_runtime/host.js";
  write(root, host, "globalThis.HostShim = 1;\n");
  writeJson(root, "biome.json", { files: { includes: ["scripts/**", "crates/**/*.js"] } });
  writeJson(root, "tsconfig.json", {
    compilerOptions: { strict: true, allowJs: true, checkJs: true, lib: ["es2024", "dom"] },
    include: ["scripts/**/*.ts", "scripts/**/*.js", "crates/**/*.js"],
  });
  track(root);
  expect(codes(root)).toContain(`native-host-dom-types-unverified:${host}`);
});
test.each(["scripts/link.ts", "scripts/linked/tool.ts"])(
  "source symlink cannot disappear through shared selector: %s",
  (name) => {
    const root = fixture();
    fs.symlinkSync(
      path.join(root, name.includes("linked/") ? "scripts" : "scripts/tool.ts"),
      path.join(root, name.includes("linked/") ? "scripts/linked" : name),
    );
    if (name.includes("linked/")) {
      const blob = execFileSync("git", ["hash-object", "-w", "--stdin"], {
        cwd: root,
        input: sources["scripts/tool.ts"],
        encoding: "utf8",
      }).trim();
      execFileSync("git", ["update-index", "--add", "--cacheinfo", `100644,${blob},${name}`], {
        cwd: root,
      });
    }
    expect(codes(root)).toContain(`unsafe-or-missing-source:${name}`);
  },
);
test("deleted cached source is rejected before collecting or reading other code", () => {
  const root = fixture();
  fs.unlinkSync(path.join(root, "scripts/tool.ts"));
  const report = inspectOwnedValidationContract(root);
  expect(report.issues.some((issue) => issue.code === "unsafe-or-missing-source")).toBe(true);
  expect(report.sourceCount).toBe(0);
});
test("safe spaces remain physical Git paths and backslash spelling is rejected", () => {
  const root = fixture();
  write(root, "scripts/safe space.ts", "export const spaced = 1;\n");
  track(root);
  expect(codes(root)).not.toContain("unsafe-or-missing-source:scripts/safe space.ts");
  write(root, "scripts/back\\slash.ts", "export const escaped = 1;\n");
  expect(codes(root)).toContain("unsafe-or-missing-source:scripts/back\\slash.ts");
});
test("a separate Rust workspace remains an explicit missing registration", () => {
  const root = fixture();
  write(root, "scripts/performance/src/main.rs", "fn main() {}\n");
  track(root);
  expect(codes(root)).toContain("rust-workspace-scope-missing:scripts/performance/src/main.rs");
});
test("generated/vendor directory labels alone cannot exclude owned code", () => {
  const root = fixture();
  const verifiedAssetBaseline = inspectOwnedValidationContract(root).verifiedAssets;
  const name = "scripts/generated/owned.ts";
  const value = "export const generated = 1;\n";
  write(root, name, value);
  track(root);
  expect(codes(root)).toContain(`unbound-excluded-source:${name}`);
  write(root, `${name}.sha256`, `${createHash("sha256").update(value).digest("hex")}  owned.ts\n`);
  track(root);
  expect(inspectOwnedValidationContract(root).verifiedAssets).toBe(verifiedAssetBaseline + 1);
  write(root, name, "export const changed = 2;\n");
  expect(() => inspectOwnedValidationContract(root)).toThrow("asset checksum mismatch");
});
test("independent local and CI registration sets cannot substitute for each other", () => {
  const root = fixture();
  write(
    root,
    ".github/workflows/check.yml",
    "jobs:\n  check:\n    steps:\n      - name: standard\n        run: just biome\n",
  );
  const missing = codes(root);
  expect(missing).toContain("unit-ci-missing:scripts/tool_test.ts");
  expect(missing).not.toContain("unit-local-missing:scripts/tool_test.ts");
});
test("Python imports are parsed without executing code and uncertain binding fails closed", () => {
  const root = fixture();
  write(
    root,
    "scripts/tool_test.py",
    "import unittest as unit\nunit.TestCase = object\nraise RuntimeError('must not execute')\nclass TestValue(unit.TestCase):\n    pass\n",
  );
  expect(codes(root)).toContain("python-unit-classification-unknown:scripts/tool_test.py");
});
test("nonstandard Bun unit filename needs explicit selection and nested Python needs discovery package", () => {
  const root = fixture();
  write(root, "scripts/custom.ts", sources["scripts/tool_test.ts"]);
  write(root, "scripts/nested/tool_test.py", sources["scripts/tool_test.py"]);
  track(root);
  const missingBeforeRegistration = codes(root);
  expect(missingBeforeRegistration).toContain("unit-local-missing:scripts/custom.ts");
  expect(missingBeforeRegistration).toContain("unit-ci-missing:scripts/nested/tool_test.py");
  recipes(root, { unit: `${recipeBodies.unit}\nbun test ./scripts/custom.ts` });
  write(root, "scripts/nested/__init__.py", "");
  track(root);
  const missingAfterRegistration = codes(root);
  expect(missingAfterRegistration).not.toContain("unit-local-missing:scripts/custom.ts");
  expect(missingAfterRegistration).not.toContain("unit-ci-missing:scripts/nested/tool_test.py");
  recipes(root, { unit: `${recipeBodies.unit}\nbun test ${path.join(root, "scripts/custom.ts")}` });
  expect(codes(root)).not.toContain("unit-local-missing:scripts/custom.ts");
});

test("stale static includes are diagnosed even when other patterns cover current files", () => {
  const root = fixture();
  writeJson(root, "biome.json", { files: { includes: ["scripts/**", "crates/old-name/**/*.ts"] } });
  writeJson(root, "tsconfig.json", {
    compilerOptions: { strict: true, allowJs: true, checkJs: true },
    include: ["scripts/**/*.ts", "scripts/**/*.js", "crates/old-name/**/*.ts"],
  });
  const report = inspectOwnedValidationContract(root);
  expect(
    report.issues
      .filter((row) => row.code === "stale-static-include")
      .map((row) => row.path)
      .sort(),
  ).toEqual(["biome.json", "tsconfig.json"]);
});
test.each(["", " --failfast"] as const)(
  "whole Bun discovery and Python unittest discovery%s retain actual units",
  (failfast) => {
    const root = fixture();
    recipes(root, {
      unit: `bun test ./\nrtk proxy python3 -m unittest discover -s scripts -p '*_test.py'${failfast}`,
    });
    const report = inspectOwnedValidationContract(root);
    expect(report.units.python).toContain("scripts/tool_test.py");
    expect(
      report.issues.filter(
        (issue) =>
          ["unit-local-missing", "unit-ci-missing"].includes(issue.code) &&
          issue.path === "scripts/tool_test.py",
      ),
    ).toEqual([]);
  },
);

test.each([
  [
    "duplicate failfast",
    "python3 -m unittest discover -s scripts -p '*_test.py' --failfast --failfast",
  ],
  [
    "narrowed discovery root",
    "python3 -m unittest discover -s scripts/quality -p '*_test.py' --failfast",
  ],
  [
    "test-name filter",
    "python3 -m unittest discover -s scripts -p '*_test.py' --failfast -k test_value",
  ],
  [
    "unknown option",
    "python3 -m unittest discover -s scripts -p '*_test.py' --failfast --unknown-option",
  ],
  [
    "conditional discovery",
    "if false; then\npython3 -m unittest discover -s scripts -p '*_test.py' --failfast;\nfi",
  ],
] as const)("Python discovery rejects %s as unit coverage", (_label, body) => {
  const root = fixture();
  recipes(root, { unit: body });
  expect(codes(root)).toContain("unit-local-missing:scripts/tool_test.py");
});
test.each(["tsc --noEmit --strict false", "tsc --noEmit scripts/tool.ts", "tsc --noEmit || true"])(
  "compiler command overrides cannot be hidden by showConfig: %s",
  (command) => {
    const root = fixture();
    writeJson(root, "package.json", {
      scripts: { biome: "biome check --error-on-warnings", typecheck: command },
    });
    expect(() => inspectOwnedValidationContract(root)).toThrow("unsupported TypeScript checking");
  },
);

test("workspace globs associate modules while missing and untracked manifest authority fails", () => {
  const root = fixture();
  write(root, "Cargo.toml", '[workspace]\nmembers = ["crates/*"]\n');
  expect(inspectOwnedValidationContract(root).issues).toEqual([]);
  write(root, "crates/second/Cargo.toml", '[package]\nname = "second"\nversion = "0.1.0"\n');
  expect(codes(root)).toContain("untracked-authority:crates/second/Cargo.toml");
  write(
    root,
    "Cargo.toml",
    '[workspace]\nmembers = ["crates/demo", "crates/missing", "../outside"]\n',
  );
  const missing = codes(root);
  expect(missing).toContain("rust-workspace-member-missing:Cargo.toml");
  expect(missing).toContain("unsafe-workspace-member:Cargo.toml");
});

test("compiler include/extends physical boundaries reject external or missing metadata", () => {
  const root = fixture();
  writeJson(root, "base.json", {
    compilerOptions: {
      strict: true,
      noUnusedLocals: true,
      noUnusedParameters: true,
      useUnknownInCatchVariables: true,
      allowJs: true,
      checkJs: true,
      lib: ["es2024"],
    },
  });
  writeJson(root, "tsconfig.json", {
    extends: "./base.json",
    include: ["scripts/**/*.ts", "scripts/**/*.js", "tests/fixtures/quality-cache/*.ts"],
  });
  expect(codes(root)).toContain("untracked-authority:base.json");
  track(root);
  expect(inspectOwnedValidationContract(root).issues).toEqual([]);
  writeJson(root, "tsconfig.json", { extends: "../../external.json" });
  expect(() => inspectOwnedValidationContract(root)).toThrow("unsafe repository-relative");
  writeJson(root, "tsconfig.json", { include: ["../external/**/*.ts"] });
  expect(() => inspectOwnedValidationContract(root)).toThrow("external TypeScript selection");
  writeJson(root, "tsconfig.json", { extends: "./missing.json" });
  expect(() => inspectOwnedValidationContract(root)).toThrow();
});

const newlyRegisteredUnits = [
  "scripts/quality/owned-unit-contract-alpha_test.ts",
  "scripts/quality/owned-unit-contract-beta_test.ts",
  "scripts/quality/owned_python_test_inventory_test.py",
] as const;
type RegistrationLane = "local" | "ci";
function newUnitCommand(omit?: (typeof newlyRegisteredUnits)[number]): string {
  const bun = newlyRegisteredUnits
    .filter((name) => name.endsWith(".ts") && name !== omit)
    .map((name) => `./${name}`);
  const python =
    newlyRegisteredUnits[2] === omit
      ? ""
      : "\npython3 -m unittest discover -s scripts/quality -p '*_test.py'";
  return `${recipeBodies.unit?.replace(" ./scripts\n", " ./scripts/tool_test.ts\n")}\nbun test --path-ignore-patterns 'tmp/**' ${bun.join(" ")}${python}`;
}
function newUnitFixture(omit?: {
  lane: RegistrationLane;
  name: (typeof newlyRegisteredUnits)[number];
}): string {
  const root = fixture();
  for (const name of newlyRegisteredUnits)
    write(
      root,
      name,
      name.endsWith(".py") ? sources["scripts/tool_test.py"] : sources["scripts/tool_test.ts"],
    );
  const base = fixtureRecipeBodies()["check-standard"] ?? "";
  recipes(root, {
    "check-standard": base.replace("just unit", "just unit-local"),
    "ci-standard": base.replace("just unit", "just unit-ci"),
    "unit-local": newUnitCommand(omit?.lane === "local" ? omit.name : undefined),
    "unit-ci": newUnitCommand(omit?.lane === "ci" ? omit.name : undefined),
  });
  write(
    root,
    ".github/workflows/check.yml",
    "jobs:\n  check:\n    steps:\n      - name: standard\n        run: just ci-standard\n",
  );
  track(root);
  return root;
}

test("new owned Bun 2 and Python 1 actual paths are registered in both standard lanes", () => {
  const root = newUnitFixture();
  const report = inspectOwnedValidationContract(root);
  expect(report.issues).toEqual([]);
  expect(report.units.bun).toEqual(
    [
      ...actualRuntimeAssetUnitPaths(),
      newlyRegisteredUnits[0],
      newlyRegisteredUnits[1],
      "scripts/tool_test.ts",
      "tests/fixtures/quality-cache/moon-cache-subject.test.ts",
    ].sort(),
  );
  expect(report.units.python).toEqual([newlyRegisteredUnits[2], "scripts/tool_test.py"]);
  expect(report.units.playwright).toEqual([...playwrightUnitPaths].sort());
  expect(readOwnedRepositoryIndex(root).untracked).toEqual([]);
});
test.each([...newlyRegisteredUnits])(
  "a one-sided standard lane omission is rejected for actual path %s",
  (name) => {
    for (const lane of ["local", "ci"] as const) {
      const root = newUnitFixture({ lane, name });
      const report = inspectOwnedValidationContract(root);
      const missing = report.issues.filter((issue) => issue.code.startsWith("unit-"));
      expect(missing).toEqual([
        {
          code: `unit-${lane}-missing`,
          path: name,
          detail: name.endsWith(".py") ? "python" : "bun",
        },
      ]);
    }
  },
);
test("new owned registrations do not exempt another missing Bun unit", () => {
  const root = newUnitFixture();
  const name = "scripts/another_test.ts";
  write(root, name, sources["scripts/tool_test.ts"]);
  track(root);
  const missing = codes(root);
  expect(missing).toContain(`unit-local-missing:${name}`);
  expect(missing).toContain(`unit-ci-missing:${name}`);
});

const mathJaxFixtureProvenance = z
  .object({
    declarations: z.array(z.object({ path: z.string() })),
  })
  .parse(
    JSON.parse(
      fs.readFileSync(path.join(import.meta.dir, "mathjax-declaration-provenance.json"), "utf8"),
    ),
  );
function mathJaxFixture(): string {
  const root = fixture();
  const actual = path.resolve(import.meta.dir, "../..");
  writeJson(root, "tsconfig.json", {
    compilerOptions: auxiliaryStrictOptions,
    files: ["scripts/helper.js"],
  });
  for (const name of [
    MATHJAX_RUNTIME,
    MATHJAX_ABI,
    MATHJAX_TYPE_PROFILE,
    MATHJAX_DECL_PROFILE,
    MATHJAX_PROVENANCE,
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime.rs",
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime_scripts.rs",
    "bun.lock",
    "node_modules/@mathjax/src/package.json",
    ...mathJaxFixtureProvenance.declarations.map((file) => file.path),
  ])
    write(root, name, fs.readFileSync(path.join(actual, name), "utf8"));
  const packagePath = path.join(root, "package.json");
  const pkg = z
    .object({
      scripts: z.record(z.string(), z.string()),
      devDependencies: z.record(z.string(), z.string()),
    })
    .parse(JSON.parse(fs.readFileSync(packagePath, "utf8")));
  pkg.scripts.typecheck += ` && tsc --noEmit -p ${MATHJAX_TYPE_PROFILE} && tsc --noEmit -p ${MATHJAX_DECL_PROFILE}`;
  writeJson(root, "package.json", pkg);
  const biome = z
    .object({ files: z.object({ includes: z.array(z.string()) }) })
    .parse(JSON.parse(fs.readFileSync(path.join(root, "biome.json"), "utf8")));
  biome.files.includes.push(MATHJAX_RUNTIME, MATHJAX_ABI);
  writeJson(root, "biome.json", biome);
  track(root);
  return root;
}
function mutateMathJaxOptions(
  root: string,
  profile: string,
  changes: Record<string, unknown>,
): void {
  const file = path.join(root, profile);
  const cfg = z
    .object({
      compilerOptions: z.record(z.string(), z.unknown()),
      files: z.array(z.string()).optional(),
    })
    .parse(JSON.parse(fs.readFileSync(file, "utf8")));
  Object.assign(cfg.compilerOptions, changes);
  writeJson(root, profile, cfg);
}
test("MathJax exact vendor closure retains ALL owned strict companion coverage", () => {
  const root = mathJaxFixture();
  const report = inspectOwnedValidationContract(root);
  expect(
    report.issues.filter(
      (issue) =>
        issue.code.startsWith("mathjax-") || issue.code === "type-project-declaration-skipping",
    ),
  ).toEqual([]);
  expect(
    report.issues.some(
      (issue) => issue.path === MATHJAX_ABI && issue.code === "typescript-type-unregistered",
    ),
  ).toBe(false);
});
test.each([
  ["unknown catch", { useUnknownInCatchVariables: false }],
  ["unused variables", { noUnusedLocals: false }],
  ["DOM leak", { lib: ["ES2024", "DOM"] }],
  ["ambient types leak", { types: ["node"] }],
  ["companion skipping", { skipLibCheck: true }],
] as const)("MathJax profile rejects %s weakening", (_name, changes) => {
  const root = mathJaxFixture();
  mutateMathJaxOptions(
    root,
    _name === "companion skipping" ? MATHJAX_DECL_PROFILE : MATHJAX_TYPE_PROFILE,
    changes,
  );
  expect(codes(root)).toContain(`mathjax-declaration-boundary-invalid:${MATHJAX_TYPE_PROFILE}`);
});
test.each([
  [
    "owned any annotation",
    MATHJAX_RUNTIME,
    (value: string) => value.replace("@param {string} output", "@param {any} output"),
  ],
  [
    "owned API escape",
    MATHJAX_RUNTIME,
    (value: string) => `${value}\nmathjax.context.window.unknownCall();\n`,
  ],
  [
    "owned ABI poison",
    MATHJAX_ABI,
    (value: string) => `${value}\ninterface OwnedPoison { value: MissingOwnedType; }\n`,
  ],
  ["self rewritten provenance", MATHJAX_PROVENANCE, (value: string) => `${value} `],
  [
    "vendor declaration changed",
    mathJaxFixtureProvenance.declarations[0]?.path ?? "",
    (value: string) => `${value}\n// changed vendor\n`,
  ],
] as const)("MathJax boundary rejects %s", (_name, name, mutate) => {
  const root = mathJaxFixture();
  expect(name).not.toBe("");
  write(root, name, mutate(fs.readFileSync(path.join(root, name), "utf8")));
  expect(codes(root)).toContain(`mathjax-declaration-boundary-invalid:${MATHJAX_TYPE_PROFILE}`);
});
test("MathJax paired declaration profile cannot be omitted", () => {
  const root = mathJaxFixture();
  const file = path.join(root, "package.json");
  const pkg = z
    .object({
      scripts: z.record(z.string(), z.string()),
      devDependencies: z.record(z.string(), z.string()),
    })
    .parse(JSON.parse(fs.readFileSync(file, "utf8")));
  pkg.scripts.typecheck =
    pkg.scripts.typecheck?.replace(` && tsc --noEmit -p ${MATHJAX_DECL_PROFILE}`, "") ?? "";
  writeJson(root, "package.json", pkg);
  expect(codes(root)).toContain(`mathjax-declaration-boundary-invalid:${MATHJAX_TYPE_PROFILE}`);
});
test("MathJax companion source set cannot leave owned declarations unchecked", () => {
  const root = mathJaxFixture();
  expect(mathJaxJavaScriptReader(root)(MATHJAX_RUNTIME)).toBe(true);
  const file = path.join(root, MATHJAX_DECL_PROFILE);
  const cfg = z
    .object({ compilerOptions: z.record(z.string(), z.unknown()), files: z.array(z.string()) })
    .parse(JSON.parse(fs.readFileSync(file, "utf8")));
  cfg.files = [];
  writeJson(root, MATHJAX_DECL_PROFILE, cfg);
  expect(mathJaxJavaScriptReader(root)(MATHJAX_RUNTIME)).toBe(false);
});
test.each(["skipLibCheck", "skipDefaultLibCheck", "noCheck"] as const)(
  "ordinary TypeScript profile rejects %s",
  (flag) => {
    const fixture = auxiliaryTypeFixture();
    const baseline = javaScriptTypeReader(fixture.root, fixture.pkg, fixture.tracked);
    expect(baseline("scripts/helper.js")).toBe(true);
    const config = z
      .object({ compilerOptions: z.record(z.string(), z.unknown()), files: z.array(z.string()) })
      .parse(JSON.parse(fs.readFileSync(path.join(fixture.root, "scripts/aux.json"), "utf8")));
    writeJson(fixture.root, "scripts/aux.json", {
      ...config,
      compilerOptions: { ...config.compilerOptions, [flag]: true },
    });
    const afterMutation = javaScriptTypeReader(fixture.root, fixture.pkg, fixture.tracked);
    expect(afterMutation("scripts/helper.js")).toBe(false);
  },
);

test("MathJax extra owned declarations and inherited ordinary skipping reject", () => {
  const root = mathJaxFixture();
  const profile = path.join(root, MATHJAX_TYPE_PROFILE);
  const cfg = z
    .object({ compilerOptions: z.record(z.string(), z.unknown()), files: z.array(z.string()) })
    .parse(JSON.parse(fs.readFileSync(profile, "utf8")));
  write(root, "scripts/extra-owned.d.ts", "interface OwnedPoison { value: MissingOwnedType; }\n");
  cfg.files.push("../extra-owned.d.ts");
  writeJson(root, MATHJAX_TYPE_PROFILE, cfg);
  expect(codes(root)).toContain(`mathjax-declaration-boundary-invalid:${MATHJAX_TYPE_PROFILE}`);
  writeJson(root, "scripts/ordinary-parent.json", { compilerOptions: { skipLibCheck: true } });
  const ordinary = z
    .object({
      compilerOptions: z.record(z.string(), z.unknown()),
      include: z.array(z.string()).optional(),
    })
    .parse(JSON.parse(fs.readFileSync(path.join(root, "tsconfig.json"), "utf8")));
  writeJson(root, "tsconfig.json", { ...ordinary, extends: "./scripts/ordinary-parent.json" });
  expect(codes(root)).toContain("type-project-declaration-skipping:tsconfig.json");
});
test("MathJax vendor symlink cannot inherit the official exception", () => {
  const root = mathJaxFixture();
  const vendor = mathJaxFixtureProvenance.declarations[0]?.path;
  if (!vendor) throw new Error("MathJax declaration fixture missing");
  const file = path.join(root, vendor);
  const saved = `${file}.saved`;
  fs.renameSync(file, saved);
  fs.symlinkSync(saved, file);
  expect(codes(root)).toContain(`mathjax-declaration-boundary-invalid:${MATHJAX_TYPE_PROFILE}`);
});

const auxiliaryStrictOptions = {
  target: "ES2024",
  module: "ESNext",
  moduleResolution: "Bundler",
  lib: ["ES2024"],
  types: [],
  strict: true,
  noImplicitAny: true,
  strictNullChecks: true,
  noUnusedLocals: true,
  noUnusedParameters: true,
  useUnknownInCatchVariables: true,
  noUncheckedIndexedAccess: true,
  exactOptionalPropertyTypes: true,
  allowJs: true,
  checkJs: true,
  noEmit: true,
};
function auxiliaryTypeFixture() {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "auxiliary-type-project-")));
  fixtures.push(root);
  fs.mkdirSync(path.join(root, "scripts"));
  const pkg = { scripts: { typecheck: "tsc --noEmit && tsc --noEmit -p scripts/aux.json" } };
  fs.writeFileSync(path.join(root, "package.json"), JSON.stringify(pkg));
  fs.writeFileSync(path.join(root, "bun.lock"), "fixture-lock");
  fs.writeFileSync(path.join(root, "scripts/helper.js"), "export const helper = 1;\n");
  fs.writeFileSync(path.join(root, "scripts/tool.ts"), "export const tool = 1;\n");
  fs.writeFileSync(
    path.join(root, "tsconfig.json"),
    JSON.stringify({ compilerOptions: auxiliaryStrictOptions, files: ["scripts/tool.ts"] }),
  );
  fs.writeFileSync(
    path.join(root, "scripts/aux.json"),
    JSON.stringify({ compilerOptions: auxiliaryStrictOptions, files: ["helper.js"] }),
  );
  return { root, pkg, tracked: new Set(["tsconfig.json", "scripts/aux.json"]) };
}

test("auxiliary TypeScript projects preserve exact supported dispatch and reject unknown options", () => {
  expect(
    typeProjectNames({
      scripts: { typecheck: "tsc --noEmit && tsc -p scripts/aux.json --noEmit" },
    }),
  ).toEqual(["tsconfig.json", "scripts/aux.json"]);
  for (const command of [
    undefined,
    "",
    "tsc",
    "tsc --noEmit --skipLibCheck",
    "tsc --noEmit -p a.json -p b.json",
    "tsc --noEmit; true",
    "bunx tsc --noEmit",
  ])
    expect(() => typeProjectNames({ scripts: { typecheck: command } })).toThrow();
});

test("auxiliary TypeScript project uses actual compiler files and rejects strict weakening", () => {
  const fixture = auxiliaryTypeFixture();
  const typed = javaScriptTypeReader(fixture.root, fixture.pkg, fixture.tracked);
  expect(typed("scripts/helper.js")).toBe(true);
  expect(typed("scripts/helper.js")).toBe(true);
  for (const flag of [
    "strict",
    "noImplicitAny",
    "strictNullChecks",
    "noUnusedLocals",
    "noUnusedParameters",
    "useUnknownInCatchVariables",
    "noUncheckedIndexedAccess",
    "exactOptionalPropertyTypes",
    "allowJs",
    "checkJs",
    "noEmit",
  ])
    expect(strictJavaScriptProject({ ...auxiliaryStrictOptions, [flag]: false })).toBe(false);
  for (const flag of ["skipLibCheck", "skipDefaultLibCheck", "noCheck"])
    expect(strictJavaScriptProject({ ...auxiliaryStrictOptions, [flag]: true })).toBe(false);
});

test.each(["scripts/aux.json", "package.json", "bun.lock", "scripts/helper.js"])(
  "auxiliary TypeScript snapshot rejects changed input %s",
  (file) => {
    const fixture = auxiliaryTypeFixture();
    const typed = javaScriptTypeReader(fixture.root, fixture.pkg, fixture.tracked);
    expect(typed("scripts/helper.js")).toBe(true);
    fs.appendFileSync(path.join(fixture.root, file), " ");
    expect(() => typed("scripts/helper.js")).toThrow("TypeScript snapshot input changed");
  },
);

test.each(["unregistered", "omitted", "unchecked", "untracked", "external", "cycle"])(
  "auxiliary TypeScript project rejects %s source authority",
  (kind) => {
    const fixture = auxiliaryTypeFixture();
    if (kind === "unregistered") fixture.pkg.scripts.typecheck = "tsc --noEmit";
    if (kind === "untracked") fixture.tracked.delete("scripts/aux.json");
    const config =
      kind === "external"
        ? { files: ["../../outside.js"] }
        : kind === "cycle"
          ? { extends: "./aux.json", files: ["helper.js"] }
          : {
              compilerOptions: { ...auxiliaryStrictOptions, checkJs: kind !== "unchecked" },
              files: [kind === "omitted" ? "tool.ts" : "helper.js"],
            };
    fs.writeFileSync(path.join(fixture.root, "scripts/tool.ts"), "export const other = 1;\n");
    fs.writeFileSync(path.join(fixture.root, "scripts/aux.json"), JSON.stringify(config));
    const typed = javaScriptTypeReader(fixture.root, fixture.pkg, fixture.tracked);
    if (["untracked", "external", "cycle"].includes(kind))
      expect(() => typed("scripts/helper.js")).toThrow();
    else expect(typed("scripts/helper.js")).toBe(false);
  },
);

function mathJaxJavaScriptReader(root: string) {
  const pkg = z
    .object({ scripts: z.record(z.string(), z.string()) })
    .parse(JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")));
  const tracked = new Set(readOwnedRepositoryIndex(root).tracked);
  return javaScriptTypeReader(root, pkg, tracked);
}

test("MathJax auxiliary reader admits the actual reviewed runtime with ordinary strict coverage", () => {
  expect(mathJaxJavaScriptReader(path.resolve(import.meta.dir, "../.."))(MATHJAX_RUNTIME)).toBe(
    true,
  );
});

test("MathJax auxiliary reader admits the complete paired-profile fixture", () => {
  const root = mathJaxFixture();
  expect(mathJaxJavaScriptReader(root)(MATHJAX_RUNTIME)).toBe(true);
});

test.each([
  "consumer-missing",
  "companion-missing",
  "consumer-noCheck",
  "consumer-default-skip",
  "companion-skip",
])("MathJax auxiliary reader rejects %s", (kind) => {
  const root = mathJaxFixture();
  if (kind.endsWith("missing")) {
    const pkg = z
      .object({
        scripts: z.record(z.string(), z.string()),
        devDependencies: z.record(z.string(), z.string()),
      })
      .parse(JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")));
    const missing = kind === "consumer-missing" ? MATHJAX_TYPE_PROFILE : MATHJAX_DECL_PROFILE;
    expect(pkg.scripts.typecheck).toContain(` && tsc --noEmit -p ${missing}`);
    pkg.scripts.typecheck =
      pkg.scripts.typecheck?.replace(` && tsc --noEmit -p ${missing}`, "") ?? "";
    writeJson(root, "package.json", pkg);
  } else if (kind === "companion-skip")
    mutateMathJaxOptions(root, MATHJAX_DECL_PROFILE, { skipLibCheck: true });
  else
    mutateMathJaxOptions(root, MATHJAX_TYPE_PROFILE, {
      [kind === "consumer-noCheck" ? "noCheck" : "skipDefaultLibCheck"]: true,
    });
  expect(mathJaxJavaScriptReader(root)(MATHJAX_RUNTIME)).toBe(false);
});

const performanceRoot = "scripts/html/performance-harness";
const performanceManifest = `${performanceRoot}/Cargo.toml`;
function performanceHarnessRecipes(root: string, overrides: Record<string, string> = {}): void {
  const names = [
    "native-coverage-cache-key",
    "fmt-check",
    "lint",
    "coverage",
    "coverage-result-check",
    "root-test-and-coverage",
    "performance-harness-fmt-check",
    "performance-harness-lint",
    "performance-harness-coverage",
  ];
  const commands = Object.fromEntries(names.map((name) => [name, actualRecipeBody(name)]));
  recipes(root, {
    ...commands,
    "check-standard": `${fixtureRecipeBodies()["check-standard"]}\njust fmt-check\njust coverage`,
    ...overrides,
  });
  const declarations = [
    "REPO_ROOT := justfile_directory()",
    'COVERAGE_MIN_LINES := env_var_or_default("COVERAGE_MIN_LINES", "100")',
    'COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default("COVERAGE_MAX_UNCOVERED_LINES", "0")',
  ];
  write(
    root,
    "Justfile",
    `${declarations.join("\n")}\n${fs.readFileSync(path.join(root, "Justfile"), "utf8")}`,
  );
}
function performanceHarnessFixture(): string {
  const root = fixture();
  const actual = path.resolve(import.meta.dir, "../..");
  write(root, performanceManifest, fs.readFileSync(path.join(actual, performanceManifest), "utf8"));
  write(root, `${performanceRoot}/src/main.rs`, "fn main() {}\n");
  write(
    root,
    "scripts/build/cargo_capacity_guard.py",
    "# 固定CLI辺だけを検査するmetadata fixture。\n",
  );
  write(
    root,
    "scripts/release/native_nextest_root_coverage.py",
    fs.readFileSync(path.join(actual, "scripts/release/native_nextest_root_coverage.py"), "utf8"),
  );
  write(
    root,
    `${performanceRoot}/Cargo.lock`,
    `version = 4
[[package]]
name = "krr-html-performance-harness"
version = "0.1.0"
dependencies = ["image", "katana-render-runtime 0.4.23", "katana-render-runtime 0.1.0", "url"]
[[package]]
name = "katana-render-runtime"
version = "0.4.23"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "${"a".repeat(64)}"
[[package]]
name = "katana-render-runtime"
version = "0.1.0"
dependencies = ["image", "url"]
[[package]]
name = "image"
version = "0.25.10"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "85ab80394333c02fe689eaf900ab500fbd0c2213da414687ebf995a65d5a6104"
[[package]]
name = "url"
version = "2.5.8"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed"
`,
  );
  write(
    root,
    "Cargo.lock",
    `version = 4
[[package]]
name = "image"
version = "0.25.10"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "85ab80394333c02fe689eaf900ab500fbd0c2213da414687ebf995a65d5a6104"
[[package]]
name = "url"
version = "2.5.8"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed"
`,
  );
  performanceHarnessRecipes(root);
  track(root);
  return root;
}
test("standalone performance harness binds both owned Rust arms without executing coverage", () => {
  const root = performanceHarnessFixture();
  const manifest = Bun.TOML.parse(fs.readFileSync(path.join(root, performanceManifest), "utf8"));
  expect(manifest).toHaveProperty("profile", {
    dev: { debug: 1 },
    "krr-coverage": { inherits: "dev", debug: 0, incremental: false },
  });
  write(root, `${performanceRoot}/tests/arm.rs`, "#[test] fn arm() {}\n");
  track(root);
  const before = inspectOwnedValidationContract(root);
  expect(before.issues).toEqual([]);
  expect(before.standalonePerformanceHarness).toMatchObject({
    localCoverageRegistered: true,
    ciCoverageRegistered: true,
    verification: "not-executed",
  });
  expect(before.standalonePerformanceHarness?.inputPaths).toContain(
    `${performanceRoot}/tests/arm.rs`,
  );
  write(
    root,
    `${performanceRoot}/Cargo.lock`,
    `${fs.readFileSync(path.join(root, performanceRoot, "Cargo.lock"), "utf8")}\n# fingerprint change\n`,
  );
  expect(inspectOwnedValidationContract(root).sourceDigest).not.toBe(before.sourceDigest);
});
test.each([
  ["workspace", "[workspace]", '[workspace]\nmembers = ["nested"]'],
  ["publish", "publish = false", "publish = true"],
  [
    "arms",
    'candidate = ["dep:candidate", "dep:image", "dep:url"]',
    'candidate = ["dep:candidate", "dep:image", "dep:url"]\ndefault = ["baseline"]',
  ],
  [
    "missing frame capture dependency feature",
    'candidate = ["dep:candidate", "dep:image", "dep:url"]',
    'candidate = ["dep:candidate", "dep:url"]',
  ],
  [
    "unreviewed frame backend feature",
    'candidate = ["dep:candidate", "dep:image", "dep:url"]',
    'candidate = ["dep:candidate", "dep:image", "dep:url", "dep:jpeg"]',
  ],
  ["baseline", 'version = "=0.4.23"', 'version = "0.4.23"'],
  ["candidate", "../../../crates/katana-render-runtime", "../../../crates/demo"],
  [
    "image nonoptional",
    'optional = true }\nurl = { version = "2.5.8"',
    'optional = false }\nurl = { version = "2.5.8"',
  ],
  ["image default features enabled", "default-features = false", "default-features = true"],
  ["image default features omitted", ", default-features = false", ""],
  ["image extra backend", 'features = ["png"]', 'features = ["png", "jpeg"]'],
  [
    "image git source",
    'features = ["png"], optional = true }',
    'features = ["png"], optional = true, git = "https://example.invalid/image" }',
  ],
  [
    "image path source",
    'features = ["png"], optional = true }',
    'features = ["png"], optional = true, path = "../image" }',
  ],
  [
    "image registry override",
    'features = ["png"], optional = true }',
    'features = ["png"], optional = true, registry = "other" }',
  ],
  [
    "url nonoptional",
    'url = { version = "2.5.8", optional = true }',
    'url = { version = "2.5.8", optional = false }',
  ],
  [
    "url git source",
    'url = { version = "2.5.8", optional = true }',
    'url = { version = "2.5.8", optional = true, git = "https://example.invalid/url" }',
  ],
  ["missing tempfile dev-dependency", '[dev-dependencies]\ntempfile = "3"', "[dev-dependencies]"],
  [
    "wrong tempfile dev-dependency version",
    '[dev-dependencies]\ntempfile = "3"',
    '[dev-dependencies]\ntempfile = "4"',
  ],
  [
    "unknown tempfile dev-dependency",
    '[dev-dependencies]\ntempfile = "3"',
    '[dev-dependencies]\ntempfile = "3"\nunknown = "1"',
  ],
  [
    "url path source",
    'url = { version = "2.5.8", optional = true }',
    'url = { version = "2.5.8", optional = true, path = "../url" }',
  ],
  ["unknown", 'sha2 = "0.11"', 'sha2 = "0.11"\nunknown = "1"'],
  [
    "missing image dependency",
    'image = { version = "0.25.10", default-features = false, features = ["png"], optional = true }\n',
    "",
  ],
  ["missing url dependency", 'url = { version = "2.5.8", optional = true }\n', ""],
  ["missing profile", "[profile.dev]\ndebug = 1", ""],
  [
    "missing coverage profile",
    '[profile.krr-coverage]\ninherits = "dev"\ndebug = 0\nincremental = false\n',
    "",
  ],
  [
    "wrong coverage profile",
    '[profile.krr-coverage]\ninherits = "dev"\ndebug = 0\nincremental = false\n',
    '[profile.krr-coverage]\ninherits = "dev"\ndebug = 1\nincremental = false\n',
  ],
  ["missing debug", "debug = 1", ""],
  ["debug disabled", "debug = 1", "debug = 0"],
  ["debug full", "debug = 1", "debug = 2"],
  ["debug boolean", "debug = 1", "debug = true"],
  ["debug string", "debug = 1", 'debug = "line-tables-only"'],
  ["other release profile", "debug = 1", "debug = 1\n[profile.release]\ndebug = 1"],
  ["other test profile", "debug = 1", "debug = 1\n[profile.test]\ndebug = 1"],
  ["optimization", "debug = 1", "debug = 1\nopt-level = 0"],
  ["incremental", "debug = 1", "debug = 1\nincremental = false"],
  ["overflow checks", "debug = 1", "debug = 1\noverflow-checks = false"],
  ["debug assertions", "debug = 1", "debug = 1\ndebug-assertions = false"],
])("standalone performance manifest rejects %s drift", (_name, before, after) => {
  const root = performanceHarnessFixture();
  const source = fs.readFileSync(path.join(root, performanceManifest), "utf8");
  expect(source).toContain(before);
  write(root, performanceManifest, source.replace(before, after));
  expect(codes(root)).toContain(`performance-harness-manifest-unverified:${performanceManifest}`);
});
test.each([
  [
    "image root lock version",
    'name = "image"\nversion = "0.25.10"',
    'name = "image"\nversion = "0.25.11"',
  ],
  ["url root lock version", 'name = "url"\nversion = "2.5.8"', 'name = "url"\nversion = "2.5.9"'],
  [
    "image root lock checksum",
    'name = "image"\nversion = "0.25.10"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "85ab80394333c02fe689eaf900ab500fbd0c2213da414687ebf995a65d5a6104"',
    `name = "image"\nversion = "0.25.10"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "${"a".repeat(64)}"`,
  ],
] as const)(
  "standalone performance capture dependency rejects %s drift",
  (_name, before, after) => {
    const root = performanceHarnessFixture();
    const source = fs.readFileSync(path.join(root, "Cargo.lock"), "utf8");
    expect(source).toContain(before);
    const changed = source.replace(before, after);
    write(root, "Cargo.lock", changed);
    expect(codes(root)).toContain(
      `performance-harness-lock-unverified:${performanceRoot}/Cargo.lock`,
    );
  },
);
test.each(["Cargo.toml", "Cargo.lock", "src/main.rs"])(
  "standalone performance input %s cannot become untracked",
  (name) => {
    const root = performanceHarnessFixture();
    execFileSync("git", ["rm", "--cached", "--", `${performanceRoot}/${name}`], { cwd: root });
    expect(codes(root)).toContain(`untracked-authority:${performanceRoot}/${name}`);
  },
);
test("standalone performance lock association rejects a different candidate version", () => {
  const root = performanceHarnessFixture();
  const name = `${performanceRoot}/Cargo.lock`;
  write(
    root,
    name,
    fs
      .readFileSync(path.join(root, name), "utf8")
      .replace('version = "0.1.0"\n', 'version = "0.2.0"\n'),
  );
  expect(codes(root)).toContain(`performance-harness-lock-unverified:${name}`);
});
test("standalone performance scope rejects nested workspace and unowned root Rust", () => {
  const root = performanceHarnessFixture();
  write(root, `${performanceRoot}/nested/Cargo.toml`, "[workspace]\n");
  write(root, "scripts/other/src/main.rs", "fn main() {}\n");
  track(root);
  const issues = codes(root);
  expect(issues).toContain(`performance-harness-nested-workspace-unknown:${performanceManifest}`);
  expect(issues).toContain("rust-workspace-scope-missing:scripts/other/src/main.rs");
});
test.each([
  ["performance-harness-fmt-check", "-- --check", "", "format"],
  ["performance-harness-lint", "--features candidate", "--features baseline", "arm"],
  [
    "performance-harness-lint",
    "--check-domain performance-baseline",
    "--check-domain performance-candidate",
    "domain",
  ],
  [
    "performance-harness-lint",
    "cargo_capacity_guard.py --check-domain performance-baseline -- {{CARGO}}",
    "cargo_capacity_guard.py -- {{CARGO}}",
    "missing domain",
  ],
  ["performance-harness-lint", "--all-targets", "--lib", "targets"],
  ["performance-harness-lint", "--locked", "", "locked"],
  ["performance-harness-lint", "-D clippy::panic", "", "deny"],
  [
    "performance-harness-lint",
    "-D warnings -D clippy::unwrap_used",
    "-D clippy::unwrap_used",
    "strict warnings",
  ],
  ["performance-harness-lint", "--no-default-features", "--all-features", "features"],
  [
    "performance-harness-lint",
    "--features baseline",
    "--features baseline,candidate",
    "combined arms",
  ],
  [
    "performance-harness-lint",
    "--manifest-path scripts/html/performance-harness/Cargo.toml",
    "--manifest-path crates/katana-render-runtime/Cargo.toml",
    "manifest",
  ],
  [
    "performance-harness-coverage",
    "--coverage-collection-domain performance-candidate",
    "--coverage-collection-domain performance-baseline",
    "domain",
  ],
  [
    "performance-harness-coverage",
    "llvm-cov-performance-candidate",
    "llvm-cov-performance-baseline",
    "profiles",
  ],
  ["performance-harness-coverage", "candidate.json", "baseline.json", "report"],
  [
    "performance-harness-coverage",
    "--coverage-collection-domain performance-candidate",
    "--coverage-domain performance-candidate",
    "strict collector mode",
  ],
  ["performance-harness-coverage", "--profile krr-coverage", "--profile dev", "profile"],
  ["performance-harness-coverage", "--profile krr-coverage", "", "missing profile"],
  [
    "performance-harness-coverage",
    "--coverage-collection-domain performance-baseline",
    "--coverage-domain performance-baseline",
    "baseline collector mode",
  ],
  ["performance-harness-coverage", "--json", "--no-run --json", "no-run"],
  ["coverage", "just root-test-and-coverage", "echo coverage", "shared root producer"],
  ["coverage", "just coverage-result-check", "echo accepted", "uncached result validation"],
  ["root-test-and-coverage", "CARGO={{quote(CARGO)}}", "CARGO=cargo", "CARGO override"],
  [
    "root-test-and-coverage",
    "TEST_THREADS={{quote(TEST_THREADS)}}",
    "TEST_THREADS=1",
    "thread override",
  ],
  ["root-test-and-coverage", '--repo "{{REPO_ROOT}}"', '--repo "tmp/repo"', "repo binding"],
  [
    "root-test-and-coverage",
    "scripts/release/native_nextest_root_coverage.py",
    "scripts/release/other_root_coverage.py",
    "delegated producer",
  ],
  ["root-test-and-coverage", "just plantuml-install", "just other-install", "PlantUML dependency"],
  [
    "performance-harness-coverage",
    "--producer-exit 0",
    "--producer-exit 1",
    "producer success binding",
  ],
  [
    "root-test-and-coverage",
    "python3 scripts/release/native_nextest_root_coverage.py",
    "echo success && python3 scripts/release/native_nextest_root_coverage.py",
    "masked root producer",
  ],
  ["fmt-check", "{{CARGO}} fmt --all -- --check", "echo formatting", "root format"],
  ["lint", "--all-features", "--no-default-features", "root lint"],
  ["lint", "--check-domain root", "--check-domain performance-baseline", "root lint domain"],
])("standalone performance recipe %s rejects %s → %s (%s)", (recipe, before, after) => {
  const root = performanceHarnessFixture();
  const original = actualRecipeBody(recipe);
  expect(original).toContain(before);
  const mutated = original.replace(before, after);
  expect(mutated).not.toBe(original);
  performanceHarnessRecipes(root, { [recipe]: mutated });
  expect(inspectStandalonePerformanceRoute(root).issues).toContainEqual(
    expect.objectContaining({ code: "performance-harness-recipe-unverified", path: recipe }),
  );
});
test.each(["echo ", "if false; then ", "-"])(
  "standalone performance fixed recipe rejects unsupported prefix %s",
  (prefix) => {
    const root = performanceHarnessFixture();
    performanceHarnessRecipes(root, {
      "performance-harness-fmt-check": prefix + actualRecipeBody("performance-harness-fmt-check"),
    });
    expect(codes(root)).toContain(
      "performance-harness-recipe-unverified:performance-harness-fmt-check",
    );
  },
);
test.each(["local", "ci"])(
  "standalone performance coverage remains NG without %s registration",
  (lane) => {
    const root = performanceHarnessFixture();
    if (lane === "local") {
      const standard = fixtureRecipeBodies()["check-standard"];
      if (standard === undefined) throw new Error("missing standard fixture recipe");
      performanceHarnessRecipes(root, { "check-standard": standard });
    } else {
      write(
        root,
        ".github/workflows/check.yml",
        "jobs:\n  check:\n    steps:\n      - run: just lint\n",
      );
    }
    expect(codes(root)).toContain(
      "performance-harness-recipe-unregistered:performance-harness-coverage",
    );
  },
);
test("standalone performance detects direct CI root-format bypass", () => {
  const root = performanceHarnessFixture();
  write(
    root,
    ".github/workflows/check.yml",
    "jobs:\n  check:\n    steps:\n      - run: just check-standard\n      - run: cargo fmt --all -- --check\n",
  );
  expect(codes(root)).toContain("performance-harness-ci-format-bypass:.github/workflows/check.yml");
});

test.each(["missing", "symlink"])(
  "standalone performance authority fails closed for %s lock",
  (mode) => {
    const root = performanceHarnessFixture();
    const name = `${performanceRoot}/Cargo.lock`;
    const file = path.join(root, name);
    fs.unlinkSync(file);
    if (mode === "symlink") {
      const outside = fs.realpathSync(
        fs.mkdtempSync(path.join(os.tmpdir(), "krr-performance-lock-")),
      );
      fixtures.push(outside);
      write(outside, "Cargo.lock", "version = 4\n");
      fs.symlinkSync(path.join(outside, "Cargo.lock"), file);
    }
    expect(codes(root)).toContain(`performance-harness-input-unsafe:${name}`);
  },
);

test("standalone performance cannot be merged into the root workspace", () => {
  const root = performanceHarnessFixture();
  write(
    root,
    "Cargo.toml",
    `[workspace]\nmembers = ["crates/demo", "crates/katana-render-runtime", "${performanceRoot}"]\n`,
  );
  expect(codes(root)).toContain(`performance-harness-workspace-merged:${performanceManifest}`);
});

test.each(["||", "&&", ";", "\n"])(
  "standalone performance fixed recipe rejects shell control %s",
  (operator) => {
    const root = performanceHarnessFixture();
    const recipe = "performance-harness-coverage";
    performanceHarnessRecipes(root, {
      [recipe]: actualRecipeBody(recipe).replace(" python3 ", ` ${operator} python3 `),
    });
    expect(codes(root)).toContain(`performance-harness-recipe-unverified:${recipe}`);
  },
);

function inheritedPerformanceCandidateFixture(): string {
  const root = performanceHarnessFixture();
  const actual = path.resolve(import.meta.dir, "../..");
  const manifest = fs.readFileSync(
    path.join(actual, "crates/katana-render-runtime/Cargo.toml"),
    "utf8",
  );
  const workspace = z
    .object({ workspace: z.object({ package: z.object({ version: z.string() }) }) })
    .parse(Bun.TOML.parse(fs.readFileSync(path.join(actual, "Cargo.toml"), "utf8")));
  write(root, "crates/katana-render-runtime/Cargo.toml", manifest);
  write(
    root,
    "Cargo.toml",
    `[workspace]
members = ["crates/demo", "crates/katana-render-runtime"]
[workspace.package]
version = "${workspace.workspace.package.version}"
`,
  );
  const name = `${performanceRoot}/Cargo.lock`;
  const lock = fs
    .readFileSync(path.join(root, name), "utf8")
    .replace(
      '"katana-render-runtime 0.1.0"',
      `"katana-render-runtime ${workspace.workspace.package.version}"`,
    )
    .replace(
      'name = "katana-render-runtime"\nversion = "0.1.0"',
      `name = "katana-render-runtime"\nversion = "${workspace.workspace.package.version}"`,
    );
  write(root, name, lock);
  return root;
}

test("standalone performance candidate inherits the actual root workspace package version", () => {
  const root = inheritedPerformanceCandidateFixture();
  const candidate = z
    .object({ package: z.object({ version: z.object({ workspace: z.literal(true) }) }) })
    .parse(
      Bun.TOML.parse(
        fs.readFileSync(path.join(root, "crates/katana-render-runtime/Cargo.toml"), "utf8"),
      ),
    );
  expect(candidate.package.version).toEqual({ workspace: true });
  const report = inspectOwnedValidationContract(root);
  expect(report.issues).toEqual([]);
  expect(report.standalonePerformanceHarness?.verification).toBe("not-executed");
});

test.each([
  [
    "false inheritance",
    "crates/katana-render-runtime/Cargo.toml",
    "version.workspace = true",
    "version.workspace = false",
  ],
  [
    "unknown inheritance",
    "crates/katana-render-runtime/Cargo.toml",
    "version.workspace = true",
    "version.workspace = true\nversion.unknown = true",
  ],
  [
    "outside workspace",
    "crates/katana-render-runtime/Cargo.toml",
    "version.workspace = true",
    'version.workspace = true\nworkspace = "../../../outside"',
  ],
  ["missing inherited version", "Cargo.toml", '[workspace.package]\nversion = "0.4.24"', ""],
  ["fake inherited version", "Cargo.toml", 'version = "0.4.24"', 'version = "fake"'],
  ["nonstring inherited version", "Cargo.toml", 'version = "0.4.24"', "version = 424"],
  ["missing member", "Cargo.toml", ', "crates/katana-render-runtime"', ""],
])("standalone inherited candidate rejects %s", (_kind, name, before, after) => {
  const root = inheritedPerformanceCandidateFixture();
  const original = fs.readFileSync(path.join(root, name), "utf8");
  expect(original).toContain(before);
  write(root, name, original.replace(before, after));
  expect(codes(root)).toContain(`performance-harness-manifest-unverified:${performanceManifest}`);
});

test.each(["excluded member", "candidate workspace", "intermediate workspace", "lock mismatch"])(
  "standalone inherited candidate rejects %s authority drift",
  (kind) => {
    const root = inheritedPerformanceCandidateFixture();
    if (kind === "excluded member")
      write(
        root,
        "Cargo.toml",
        '[workspace]\nmembers = ["crates/demo", "crates/katana-render-runtime"]\nexclude = ["crates/katana-render-runtime"]\n[workspace.package]\nversion = "0.4.24"\n',
      );
    else if (kind === "candidate workspace")
      write(
        root,
        "crates/katana-render-runtime/Cargo.toml",
        '[package]\nname = "katana-render-runtime"\nversion.workspace = true\n[workspace]\n',
      );
    else if (kind === "intermediate workspace") write(root, "crates/Cargo.toml", "[workspace]\n");
    else
      write(
        root,
        "Cargo.toml",
        '[workspace]\nmembers = ["crates/demo", "crates/katana-render-runtime"]\n[workspace.package]\nversion = "0.4.25"\n',
      );
    track(root);
    expect(codes(root)).toContain(
      `performance-harness-${kind === "lock mismatch" ? "lock" : "manifest"}-unverified:${kind === "lock mismatch" ? `${performanceRoot}/Cargo.lock` : performanceManifest}`,
    );
  },
);

test("official compiler transport preserves spaced scalar paths and exact project membership", () => {
  const fixture = auxiliaryTypeFixture();
  const name = "scripts/spaced profile.json";
  writeJson(fixture.root, name, {
    compilerOptions: auxiliaryStrictOptions,
    files: ["helper.js"],
  });
  const selected = queryTypeProject(fixture.root, name);
  expect([...selected.files]).toEqual(["scripts/helper.js"]);
  expect(selected.authorities).toEqual([name]);
  expect(strictJavaScriptProject(selected.options)).toBe(true);
  expect(process.cwd()).toBe(path.resolve(import.meta.dir, "../.."));
});

test.each(["missing", "poison"])("official compiler transport rejects %s config", (kind) => {
  const fixture = auxiliaryTypeFixture();
  const name = `scripts/${kind}.json`;
  if (kind === "poison") write(fixture.root, name, '{"compilerOptions":');
  expect(() => queryTypeProject(fixture.root, name)).toThrow();
});

test("auxiliary TypeScript snapshot rejects changed official bootstrap bytes", () => {
  const fixture = auxiliaryTypeFixture();
  const typed = javaScriptTypeReader(fixture.root, fixture.pkg, fixture.tracked);
  expect(typed("scripts/helper.js")).toBe(true);
  const compiler = path.resolve(import.meta.dir, "../../node_modules/typescript");
  const buffers = ["package.json", "bin/tsc", "lib/tsc.js"].map((name) =>
    fs.readFileSync(path.join(compiler, name)),
  );
  // 実依存ファイルを変更せず、既存byte再照合の読取境界だけを置換する単体試験。
  const read = spyOn(fs, "readFileSync")
    .mockReturnValueOnce(buffers[0] ?? Buffer.alloc(0))
    .mockReturnValueOnce(buffers[1] ?? Buffer.alloc(0))
    .mockReturnValueOnce(Buffer.concat([buffers[2] ?? Buffer.alloc(0), Buffer.from("\n")]));
  try {
    expect(() => typed("scripts/helper.js")).toThrow(
      "TypeScript compiler input changed: node_modules/typescript/lib/tsc.js",
    );
  } finally {
    read.mockRestore();
  }
});

function actualCompilerToolRequest(): { root: string; request: ExternalToolRequest } {
  const root = path.resolve(import.meta.dir, "../..");
  const caller = "scripts/quality/owned-type-projects.ts";
  let request: ExternalToolRequest | undefined;
  sourceCode(fs.readFileSync(path.join(root, caller), "utf8"), {
    name: caller,
    read: () => undefined,
    resolveExternalTool: (current) => {
      request = current;
      return undefined;
    },
    onExternalToolBinding: () => {
      throw new Error("unbound request probe produced a tool binding");
    },
  });
  if (!request) throw new Error("actual TypeScript bootstrap route was not recognized");
  return { root, request };
}

function compilerCaptureReads(root: string, request: ExternalToolRequest) {
  const binding = typeScriptToolBinding(root)(request);
  if (!binding) throw new Error("actual TypeScript tool binding missing");
  const names = [
    ...binding.inputPins.map((pin) => pin.name),
    "node_modules/typescript/package.json",
    `node_modules/@typescript/typescript-${process.platform}-${process.arch}/package.json`,
    "package.json",
    "bun.lock",
  ];
  return { binding, names, buffers: names.map((name) => fs.readFileSync(path.join(root, name))) };
}

test("actual official compiler is separately bound as observed tool pins", () => {
  const { root, request } = actualCompilerToolRequest();
  const resolve = typeScriptToolBinding(root);
  const binding = resolve(request);
  expect(binding).toBeDefined();
  expect(binding?.verification).toBe("observed-local-pins");
  expect(binding?.artifactVerified).toBe(false);
  expect(binding?.packageVersion).toBe("7.0.2");
  expect(binding?.inputPins.map((pin) => pin.name)).toContain("node_modules/typescript/lib/tsc.js");
  expect(binding?.lockIdentities.every((identity) => identity.defaultRegistry)).toBe(true);
  if (!binding) throw new Error("actual TypeScript tool binding missing");
  binding.runtime.sha256 = "0".repeat(64);
  binding.lockIdentities.splice(0);
  binding.inputPins.splice(0);
  expect(resolve(request)?.runtime.sha256).not.toBe(binding.runtime.sha256);
  expect(resolve(request)?.lockIdentities).toHaveLength(2);
});

test.each(["caller", "target", "callerSourceSha256", "callerAstSha256", "swcVersion"])(
  "TypeScript tool rejects unknown %s without accepting another route",
  (field) => {
    const { root, request } = actualCompilerToolRequest();
    const resolve = typeScriptToolBinding(root);
    expect(resolve(request)).toBeDefined();
    expect(resolve({ ...request, [field]: "unknown" })).toBeUndefined();
  },
);

test.each([
  "node_modules/typescript/bin/tsc",
  "node_modules/typescript/lib/tsc.js",
  "node_modules/typescript/lib/getExePath.js",
  `node_modules/@typescript/typescript-${process.platform}-${process.arch}/package.json`,
  `node_modules/@typescript/typescript-${process.platform}-${process.arch}/lib/tsc${process.platform === "win32" ? ".exe" : ""}`,
  "package.json",
  "bun.lock",
  "scripts/quality/owned-type-projects.ts",
])("TypeScript tool snapshot rejects changed bytes at %s", (name) => {
  const { root, request } = actualCompilerToolRequest();
  const resolve = typeScriptToolBinding(root);
  const binding = resolve(request);
  if (!binding) throw new Error("actual TypeScript tool binding missing");
  const buffers = binding.inputPins.map((pin) => fs.readFileSync(pin.physicalPath));
  const read = spyOn(fs, "readFileSync");
  binding.inputPins.forEach((pin, index) => {
    const buffer = buffers[index];
    if (!buffer) throw new Error("compiler input fixture bytes missing");
    read.mockReturnValueOnce(
      pin.name === name ? Buffer.concat([buffer, Buffer.from("\n")]) : buffer,
    );
  });
  try {
    expect(() => resolve(request)).toThrow(`TypeScript compiler input changed: ${name}`);
  } finally {
    read.mockRestore();
  }
});

test.each([
  "dependency",
  "workspace dependency",
  "registry",
  "integrity",
  "native version",
  "native platform",
])("TypeScript tool rejects initial unknown %s identity", (kind) => {
  const { root, request } = actualCompilerToolRequest();
  const nativePackage = `node_modules/@typescript/typescript-${process.platform}-${process.arch}/package.json`;
  const { names, buffers } = compilerCaptureReads(root, request);
  const pkg = z
    .object({ devDependencies: z.record(z.string(), z.string()) })
    .passthrough()
    .parse(JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")));
  const lock = z
    .object({
      workspaces: z
        .object({
          "": z.object({ devDependencies: z.record(z.string(), z.string()) }).passthrough(),
        })
        .passthrough(),
      packages: z.record(z.string(), z.array(z.unknown())),
    })
    .passthrough()
    .parse(Bun.JSONC.parse(fs.readFileSync(path.join(root, "bun.lock"), "utf8")));
  if (kind === "dependency") pkg.devDependencies.typescript = "7.0.3";
  else if (kind === "workspace dependency")
    lock.workspaces[""].devDependencies.typescript = "7.0.3";
  else if (kind.startsWith("native ")) {
    const native = z
      .object({ version: z.string(), os: z.array(z.string()) })
      .passthrough()
      .parse(JSON.parse(fs.readFileSync(path.join(root, nativePackage), "utf8")));
    if (kind === "native version") native.version = "7.0.3";
    else native.os = ["unknown"];
    const bytes = Buffer.from(JSON.stringify(native));
    names.forEach((name, index) => {
      if (name === nativePackage) buffers[index] = bytes;
    });
  } else {
    const selected = lock.packages[`@typescript/typescript-${process.platform}-${process.arch}`];
    if (!selected) throw new Error("actual native lock fixture missing");
    selected[kind === "registry" ? 1 : 3] =
      kind === "registry" ? "https://unknown.invalid" : `sha512-${"A".repeat(86)}==`;
  }
  const resolve = typeScriptToolBinding(root);
  const read = spyOn(fs, "readFileSync");
  buffers.forEach((buffer, index) => {
    read.mockReturnValueOnce(
      names[index] === "package.json"
        ? Buffer.from(JSON.stringify(pkg))
        : names[index] === "bun.lock"
          ? Buffer.from(JSON.stringify(lock))
          : buffer,
    );
  });
  try {
    expect(() => resolve(request)).toThrow();
  } finally {
    read.mockRestore();
  }
});

test("TypeScript tool rejects another repository and symlinked compiler path", () => {
  const { root, request } = actualCompilerToolRequest();
  expect(typeScriptToolBinding(auxiliaryTypeFixture().root)(request)).toBeUndefined();
  const stat = fs.lstatSync(path.join(root, "node_modules/typescript/package.json"));
  stat.isSymbolicLink = () => true;
  const read = spyOn(fs, "lstatSync").mockReturnValueOnce(stat);
  try {
    expect(() => typeScriptToolBinding(root)(request)).toThrow("source path contains symlink");
  } finally {
    read.mockRestore();
  }
});

test("TypeScript tool rejects changed running Bun bytes", () => {
  const { root, request } = actualCompilerToolRequest();
  const resolve = typeScriptToolBinding(root);
  const binding = resolve(request);
  if (!binding) throw new Error("actual TypeScript tool binding missing");
  const buffers = binding.inputPins.map((pin) => fs.readFileSync(pin.physicalPath));
  const runtime = fs.readFileSync(binding.runtime.path);
  const read = spyOn(fs, "readFileSync");
  for (const buffer of buffers) read.mockReturnValueOnce(buffer);
  read.mockReturnValueOnce(Buffer.concat([runtime, Buffer.from("\n")]));
  try {
    expect(() => resolve(request)).toThrow("TypeScript Bun runtime changed");
  } finally {
    read.mockRestore();
  }
});

test("TypeScript tool pins owner lock before parsing its registry identity", () => {
  const { root, request } = actualCompilerToolRequest();
  const resolve = typeScriptToolBinding(root);
  const originalRead = fs.readFileSync;
  let parsed = false;
  const read = spyOn(fs, "readFileSync").mockImplementation(
    new Proxy(originalRead, {
      apply(target, receiver: unknown, args: unknown[]) {
        const bytes: unknown = Reflect.apply(target, receiver, args);
        if (typeof bytes !== "string" && !Buffer.isBuffer(bytes))
          throw new Error("unexpected compiler fixture read result");
        if (String(args[0]) !== path.join(root, "bun.lock")) return bytes;
        if (typeof bytes === "string") {
          parsed = true;
          return bytes;
        }
        return parsed ? Buffer.concat([bytes, Buffer.from("\n")]) : bytes;
      },
    }),
  );
  try {
    expect(() => resolve(request)).toThrow("TypeScript compiler input changed: bun.lock");
  } finally {
    read.mockRestore();
  }
});

const strictUnusedAuditInputPaths: string[] = [...MATHJAX_UNUSED_INPUTS];
const classicGenerationUnitTestPaths = [
  "scripts/runtime-bundles/runtime-unused-classic-capture_test.ts",
  "scripts/runtime-bundles/runtime-unused-classic-binding_test.ts",
  "scripts/runtime-bundles/runtime-classic-generation_test.ts",
  "scripts/runtime-bundles/runtime-unused-semantic-adapter_test.ts",
];

function strictUnusedRegistrationFixture(): string {
  const root = fixture(),
    actualRoot = path.resolve(import.meta.dir, "../..");
  for (const name of strictUnusedAuditInputPaths) {
    if (name !== "Justfile")
      write(root, name, fs.readFileSync(path.join(actualRoot, name), "utf8"));
  }
  for (const name of classicGenerationUnitTestPaths)
    write(root, name, fs.readFileSync(path.join(actualRoot, name), "utf8"));
  recipes(root, { "runtime-asset-script-test": actualRecipeBody("runtime-asset-script-test-run") });
  track(root);
  return root;
}
function installMoonGraphPolicy(root: string): void {
  const actualRoot = path.resolve(import.meta.dir, "../..");
  const actualPolicy = JSON.parse(
    fs.readFileSync(path.join(actualRoot, "scripts/html/harness-contract-policy.json"), "utf8"),
  ) as { moon: unknown };
  const fixturePolicyPath = path.join(root, "scripts/html/harness-contract-policy.json");
  const fixturePolicy = JSON.parse(fs.readFileSync(fixturePolicyPath, "utf8")) as Record<
    string,
    unknown
  >;
  fixturePolicy.moon = actualPolicy.moon;
  writeJson(root, "scripts/html/harness-contract-policy.json", fixturePolicy);
  write(
    root,
    ".moon/workspace.yml",
    fs.readFileSync(path.join(actualRoot, ".moon/workspace.yml"), "utf8"),
  );
}

type RecipeMutation = readonly [label: string, before: string, after: string, recipeName: string];

const classicGenerationRecipeMutations = [
  [
    "producer write mode",
    "bun scripts/runtime-bundles/bundle-runtime.ts --check --classic-generation-evidence",
    "bun scripts/runtime-bundles/bundle-runtime.ts --write --classic-generation-evidence",
    "runtime-classic-generation-evidence",
  ],
  [
    "evidence output path",
    "tmp/quality/classic-generation-evidence/current.json",
    "tmp/quality/classic-generation-evidence/replayed.json",
    "runtime-asset-script-test",
  ],
  [
    "Bun consumer scope",
    "./scripts/runtime-bundles/runtime-classic-generation_test.ts",
    "./scripts/runtime-bundles/runtime-unused-check_test.ts",
    "runtime-asset-script-test",
  ],
] satisfies readonly RecipeMutation[];

test.each(classicGenerationRecipeMutations)(
  "classic generation evidence registration rejects changed %s",
  (_label, before, after, recipeName) => {
    const root = strictUnusedRegistrationFixture();
    const commandIssue = "moon-cache-fixture-owner-command-unverified";
    expect(inspectMoonCacheFixtureRoute(root).issues.map((issue) => issue.code)).not.toContain(
      commandIssue,
    );
    const body = actualRecipeBody(recipeName ?? "runtime-asset-script-test");
    expect(body).toContain(before);
    recipes(root, { [recipeName ?? "runtime-asset-script-test"]: body.replace(before, after) });
    track(root);
    expect(inspectMoonCacheFixtureRoute(root).issues.map((issue) => issue.code)).toContain(
      commandIssue,
    );
  },
);

test("classic generation consumer registers each evidence test in local and CI", () => {
  const root = strictUnusedRegistrationFixture();
  const report = inspectOwnedValidationContract(root);
  const issueCodes = report.issues.map((issue) => `${issue.code}:${issue.path}`);
  for (const name of classicGenerationUnitTestPaths) {
    expect(report.units.bun).toContain(name);
    expect(issueCodes).not.toContain(`unit-local-missing:${name}`);
    expect(issueCodes).not.toContain(`unit-ci-missing:${name}`);
  }
});

test.each([
  ["runtime-asset-script-preflight", "mathjax-owned-evidence"],
  ["runtime-classic-generation-evidence", "runtime-asset-script-preflight"],
  ["runtime-asset-script-other", "runtime-classic-generation-evidence"],
  ["runtime-unused-fragment-loader-producer", "runtime-classic-generation-evidence"],
  ["runtime-unused-bundle-definitions-producer", "runtime-unused-fragment-loader-producer"],
  ["runtime-unused-check-producer", "runtime-unused-bundle-definitions-producer"],
  ["runtime-unused-tool-fingerprint-producer", "runtime-unused-check-producer"],
  ["runtime-unused-mathjax-producer", "runtime-unused-tool-fingerprint-producer"],
  ["runtime-unused-native-width-producer", "runtime-unused-mathjax-producer"],
  ["runtime-unused-rust-classic-producer", "runtime-unused-native-width-producer"],
  ["runtime-unused-script-regression", "runtime-unused-rust-classic"],
] as const)(
  "Moon runtime asset leaf %s requires its shared predecessor",
  (taskName, dependency) => {
    const root = strictUnusedRegistrationFixture();
    installMoonGraphPolicy(root);
    const config = moonRootFixtureConfig.parse(
      Bun.YAML.parse(fs.readFileSync(path.join(root, "moon.yml"), "utf8")),
    );
    const task = moonRootFixtureTask(config, taskName);
    task.deps = (task.deps ?? []).filter((name) => name !== dependency);
    writeJson(root, "moon.yml", config);
    const issue = inspectOwnedValidationContract(root).issues.find(
      (entry) => entry.code === "execution-graph-contract",
    );
    expect(issue).toBeDefined();
  },
);

test.each([
  [
    "producer command bypass",
    (task: z.infer<typeof moonRootFixtureConfig>["tasks"][string]) => {
      task.command = ["just", "runtime-unused-script-verify-run", "other"];
    },
    "runtime unused producer command differs: runtime-unused-fragment-loader",
  ],
  [
    "producer shared predecessor bypass",
    (task: z.infer<typeof moonRootFixtureConfig>["tasks"][string]) => {
      task.deps = [];
    },
    "runtime unused producer dependency differs: runtime-unused-fragment-loader",
  ],
  [
    "producer extra output",
    (task: z.infer<typeof moonRootFixtureConfig>["tasks"][string]) => {
      task.outputs = [
        "tmp/quality/runtime-script-results/runtime-unused-fragment-loader.json",
        "tmp/quality/runtime-script-results/runtime-unused-fragment-loader-current-group-001.xml",
        "tmp/quality/runtime-script-results/extra.xml",
      ];
    },
    "moon contract invalid: runtime unused producer outputs differ",
  ],
  [
    "producer cache mode",
    (task: z.infer<typeof moonRootFixtureConfig>["tasks"][string]) => {
      task.options = { cache: true };
    },
    "moon contract invalid: runtime unused producer cache differs",
  ],
  [
    "producer unknown setting",
    (task: z.infer<typeof moonRootFixtureConfig>["tasks"][string]) => {
      (task as unknown as Record<string, unknown>).env = { PROFILE: "unexpected" };
    },
    "moon contract invalid: runtime bundle cache task contains an unknown setting",
  ],
  [
    "producer fingerprint",
    (task: z.infer<typeof moonRootFixtureConfig>["tasks"][string]) => {
      task.checks = [
        {
          check: "fingerprint",
          script: "just runtime-unused-script-cache-key runtime-unused-check",
          hash: "stdout",
        },
      ];
    },
    "runtime unused producer fingerprint differs: runtime-unused-fragment-loader",
  ],
] as const)("Moon cached producer rejects %s", (_label, mutate, _expected) => {
  const root = strictUnusedRegistrationFixture();
  installMoonGraphPolicy(root);
  const config = moonRootFixtureConfig.parse(
    Bun.YAML.parse(fs.readFileSync(path.join(root, "moon.yml"), "utf8")),
  );
  mutate(moonRootFixtureTask(config, "runtime-unused-fragment-loader-producer"));
  writeJson(root, "moon.yml", config);
  track(root);
  expect(inspectOwnedValidationContract(root).issues).toContainEqual(
    expect.objectContaining({ code: "execution-graph-contract" }),
  );
});

test.each([
  [
    "missing leaf task",
    (config: MoonRootFixtureConfig) => delete config.tasks["runtime-unused-check-producer"],
  ],
  [
    "legacy producer remains",
    (config: MoonRootFixtureConfig) => {
      config.tasks["runtime-unused-script-regression-producer"] = {
        command: ["just", "runtime-unused-script-producer-run"],
      };
    },
  ],
  [
    "final verify task removed",
    (config: MoonRootFixtureConfig) => delete config.tasks["runtime-unused-script-regression"],
  ],
] as const)("Moon runtime unused graph rejects %s", (_label, mutate) => {
  const root = strictUnusedRegistrationFixture();
  installMoonGraphPolicy(root);
  const config = moonRootFixtureConfig.parse(
    Bun.YAML.parse(fs.readFileSync(path.join(root, "moon.yml"), "utf8")),
  );
  mutate(config);
  writeJson(root, "moon.yml", config);
  track(root);
  expect(inspectOwnedValidationContract(root).issues).toContainEqual(
    expect.objectContaining({ code: "execution-graph-contract" }),
  );
});

test.each([
  [
    "duplicate leaf argv",
    (root: string) => {
      const body = actualRecipeBody("runtime-unused-check-run");
      recipes(root, {
        "runtime-unused-check-run": `${body} ./scripts/runtime-bundles/runtime-unused-check_test.ts`,
      });
    },
  ],
  [
    "missing final verify recipe",
    (root: string) => recipes(root, { "runtime-unused-script-final-verify-run": "" }),
  ],
  [
    "wrong profile fingerprint",
    (root: string) => {
      const config = moonRootFixtureConfig.parse(
        Bun.YAML.parse(fs.readFileSync(path.join(root, "moon.yml"), "utf8")),
      );
      const task = moonRootFixtureTask(config, "runtime-unused-check-producer");
      task.checks = [
        {
          check: "fingerprint",
          script: "just runtime-unused-script-cache-key runtime-unused-mathjax",
          hash: "stdout",
        },
      ];
      writeJson(root, "moon.yml", config);
    },
  ],
] as const)("Moon runtime unused contract rejects %s", (_label, mutate) => {
  const root = strictUnusedRegistrationFixture();
  installMoonGraphPolicy(root);
  mutate(root);
  track(root);
  expect(inspectOwnedValidationContract(root).issues).toContainEqual(
    expect.objectContaining({ code: "execution-graph-contract" }),
  );
});

test("unused-script leaf keeps all seven regressions before the aggregate strict gate", () => {
  const root = strictUnusedRegistrationFixture();
  const body = actualRecipeBody("runtime-unused-script-regression-run");
  recipes(root, {
    "runtime-unused-script-regression-run": body.replace(
      "./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
      "./scripts/runtime-bundles/runtime-unused-mathjax_test.ts",
    ),
  });
  track(root);
  const report = inspectMathJaxUnusedAuditRoute(root);
  expect(report.binding.standardCommandVerified).toBe(false);
  expect(report.binding.standardLocalCheckRegistered).toBe(false);
  expect(report.binding.standardCiCheckRegistered).toBe(false);
});

test.each(["semantic-adapter", "semantic-node", "semantic-adapter_test", "entrypoint-owner"])(
  "shared semantic registration rejects missing %s source",
  (role) => {
    const root = strictUnusedRegistrationFixture();
    const source = `scripts/runtime-bundles/runtime-unused-${role}.ts`;
    expect(strictUnusedAuditInputPaths).toContain(source);
    fs.unlinkSync(path.join(root, source));
    track(root);
    const report = inspectMathJaxUnusedAuditRoute(root);
    expect(report.issues).toContainEqual(
      expect.objectContaining({ code: "mathjax-unused-input-missing", path: source }),
    );
  },
);

const hostEvidenceRecipeMutations = [
  [
    "missing host test",
    " ./scripts/runtime-bundles/runtime-rust-facts-host_test.ts",
    "",
    "runtime-asset-script-test",
  ],
  [
    "duplicate evidence test",
    " ./scripts/runtime-bundles/runtime-mathjax-audit-evidence_test.ts",
    " ./scripts/runtime-bundles/runtime-mathjax-audit-evidence_test.ts ./scripts/runtime-bundles/runtime-mathjax-audit-evidence_test.ts",
    "runtime-asset-script-test",
  ],
  [
    "missing host precondition",
    "bun scripts/runtime-bundles/runtime-rust-facts-host.ts verify",
    "bun scripts/runtime-bundles/runtime-rust-facts-host.ts prepare",
    "runtime-asset-script-preflight-run",
  ],
  [
    "missing classic capture regression",
    "bun test --path-ignore-patterns 'tmp/**' ./scripts/mermaid/native_classic_capture_test.ts",
    "",
    "runtime-asset-script-test",
  ],
  ...[
    "runtime-unused-classic-capture_test.ts",
    "runtime-unused-classic-binding_test.ts",
    "runtime-classic-generation_test.ts",
    "runtime-unused-semantic-adapter_test.ts",
  ].map(
    (name): RecipeMutation => [
      `missing ${name} regression`,
      ` ./scripts/runtime-bundles/${name}`,
      "",
      "runtime-asset-script-test",
    ],
  ),
  [
    "missing managed native build regression",
    "bun test --path-ignore-patterns 'tmp/**' ./scripts/mermaid/native_cli_managed_build_test.ts",
    "",
    "runtime-asset-script-test",
  ],
  [
    "missing managed native full reader regression",
    "bun test --path-ignore-patterns 'tmp/**' ./scripts/mermaid/native_cli_managed_build_reader_test.ts",
    "",
    "runtime-asset-script-test",
  ],
  [
    "missing MathJax test environment",
    "NODE_ENV=test bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts verify",
    "bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts verify",
    "runtime-asset-script-preflight-run",
  ],
  [
    "different MathJax test environment",
    "NODE_ENV=test bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts verify",
    "NODE_ENV=production bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts verify",
    "runtime-asset-script-preflight-run",
  ],
  [
    "acquisition inside consumer",
    "bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts verify",
    "bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts",
    "runtime-asset-script-preflight-run",
  ],
] satisfies readonly RecipeMutation[];

describe("host evidence registration mutations", () => {
  beforeAll(() => {
    const root = strictUnusedRegistrationFixture();
    expect(inspectMathJaxUnusedAuditRoute(root).binding.standardCommandVerified).toBe(true);
  });

  test.each(hostEvidenceRecipeMutations)(
    "host evidence registration rejects %s",
    (_label, before, after, recipeName) => {
      const root = strictUnusedRegistrationFixture();
      const body = actualRecipeBody(recipeName);
      expect(body).toContain(before);
      recipes(root, {
        [recipeName]: body.replace(before, after),
      });
      track(root);
      const report = inspectMathJaxUnusedAuditRoute(root);
      expect(report.binding.standardCommandVerified).toBe(false);
      expect(report.issues.map((issue) => issue.code)).toContain(
        "mathjax-host-evidence-registration-unverified",
      );
    },
  );
});
test.each([
  ["fabricated report", "const report = evidence.report;", "const report = { messages: [] };"],
  [
    "stale replay",
    "evidence = readMathJaxAuditEvidence(root);",
    "evidence = JSON.parse(fs.readFileSync('tmp/stale-evidence.json', 'utf8'));",
  ],
])("MathJax evidence consumer rejects %s substitution", (_label, before, after) => {
  const root = strictUnusedRegistrationFixture();
  expect(inspectMathJaxUnusedAuditRoute(root).binding.standardCommandVerified).toBe(true);
  const file = path.join(root, mathJaxUnusedTestPath);
  const source = fs.readFileSync(file, "utf8");
  expect(source).toContain(before);
  fs.writeFileSync(file, source.replace(before, after));
  track(root);
  const report = inspectMathJaxUnusedAuditRoute(root);
  expect(report.binding.standardCommandVerified).toBe(false);
  expect(report.binding.hostEvidenceExecutionVerified).toBe(false);
});

test("official strict registration binds the exact expanded owned inventory and same selected asset command", () => {
  const root = strictUnusedRegistrationFixture(),
    report = inspectMathJaxUnusedAuditRoute(root);
  expect(report.binding.inputPaths).toEqual(strictUnusedAuditInputPaths);
  expect(new Set(report.binding.inputPaths).size).toBe(report.binding.inputPaths.length);
  expect(report.binding).toMatchObject({
    inputSelectionVerified: true,
    commandVerified: false,
    localCheckRegistered: false,
    ciCheckRegistered: false,
    standardCommandVerified: true,
    standardLocalCheckRegistered: true,
    standardCiCheckRegistered: true,
  });
  expect(report.issues.filter((issue) => issue.code.startsWith("mathjax-unused-"))).toEqual([]);
});

test("mandatory runtime unused registration rejects omitted standard gate", () => {
  const root = strictUnusedRegistrationFixture();
  const baseline = inspectMathJaxUnusedAuditRoute(root);
  expect(baseline.binding.standardLocalCheckRegistered).toBe(true);
  const body = actualRecipeBody("runtime-asset-script-aggregate-run");
  expect(body).toContain("bun scripts/runtime-bundles/runtime-unused-check.ts --gate");
  recipes(root, {
    "runtime-asset-script-aggregate-run": body.replace(
      "bun scripts/runtime-bundles/runtime-unused-check.ts --gate",
      "bun scripts/runtime-bundles/runtime-unused-check.ts",
    ),
  });
  track(root);
  const report = inspectMathJaxUnusedAuditRoute(root);
  expect(report.binding.standardCommandVerified).toBe(false);
  expect(report.binding.standardLocalCheckRegistered).toBe(false);
  expect(report.binding.standardCiCheckRegistered).toBe(false);
  expect(report.issues.map((issue) => issue.code)).toContain(
    "mathjax-unused-standard-command-unverified",
  );
});

const strictRegistrationMutations = [
  [
    "missing CLI resolver import",
    "scripts/mermaid/diagram-update.ts",
    'import { resolveOfficialRendererAssetInputs } from "./official-renderer-asset-inputs";',
    "",
  ],
  [
    "missing context resolver import",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    'import { resolveOfficialRendererAssetInputs } from "../mermaid/official-renderer-asset-inputs";',
    "",
  ],
  [
    "missing CLI resolver call",
    "scripts/mermaid/diagram-update.ts",
    "const assetInputs = resolveOfficialRendererAssetInputs(",
    "const assetInputs = otherResolver(",
  ],
  [
    "changed context home input",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "    inputs.home,",
    '    "/guessed-home",',
  ],
  [
    "missing strict context registration",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "    officialStrictUnusedContext(root, raw, inputs),",
    "",
  ],
  [
    "changed audit input binding",
    "scripts/runtime-bundles/runtime-unused-check.ts",
    "runtimeUnusedContexts(root, inputs)",
    "runtimeUnusedContexts(root)",
  ],
  [
    "changed actual argv input",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "argv: process.argv.slice(2)",
    "argv: []",
  ],
  [
    "changed actual env input",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "env: process.env",
    "env: {}",
  ],
  [
    "changed actual cwd input",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "cwd: process.cwd()",
    'cwd: "/guessed-cwd"',
  ],
  [
    "changed actual home input",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "home: os.homedir()",
    'home: "/guessed-home"',
  ],
  [
    "changed resolver default",
    "scripts/mermaid/official-renderer-asset-inputs.ts",
    path
      .relative(
        path.resolve(import.meta.dir, "../.."),
        resolveOfficialRendererAssetInputs(
          [],
          {},
          path.resolve(import.meta.dir, "../.."),
          os.homedir(),
        ).mermaidJs,
      )
      .replaceAll(path.sep, "/"),
    "~/.local/katana/other.min.js",
  ],
] as const;

describe("official strict registration mutations", () => {
  beforeAll(() => {
    const root = strictUnusedRegistrationFixture();
    expect(inspectMathJaxUnusedAuditRoute(root).binding.inputSelectionVerified).toBe(true);
  });

  test.each(strictRegistrationMutations)(
    "official strict registration rejects %s",
    (_kind, name, before, after) => {
      const root = strictUnusedRegistrationFixture();
      const source = fs.readFileSync(path.join(root, name), "utf8");
      expect(source).toContain(before);
      write(root, name, source.replace(before, after));
      track(root);
      const report = inspectMathJaxUnusedAuditRoute(root);
      expect(report.binding.inputSelectionVerified).toBe(false);
      expect(report.binding.standardLocalCheckRegistered).toBe(false);
      expect(report.binding.standardCiCheckRegistered).toBe(false);
      expect(report.issues.map((issue) => issue.code)).toContain(
        "mathjax-unused-input-selection-unverified",
      );
    },
  );
});

test.each([
  "scripts/mermaid/official-renderer-asset-inputs.ts",
  "scripts/mermaid/strict-svg-external-images.ts",
  "scripts/runtime-bundles/runtime-unused-check_test.ts",
])("official strict registration rejects omitted physical owned input %s", (name) => {
  const root = strictUnusedRegistrationFixture();
  expect(inspectMathJaxUnusedAuditRoute(root).binding.standardLocalCheckRegistered).toBe(true);
  fs.unlinkSync(path.join(root, name));
  track(root);
  const report = inspectMathJaxUnusedAuditRoute(root);
  expect(report.binding.inputSelectionVerified).toBe(false);
  expect(report.binding.standardLocalCheckRegistered).toBe(false);
  expect(report.binding.standardCiCheckRegistered).toBe(false);
  expect(report.issues).toContainEqual({
    code: "mathjax-unused-input-missing",
    path: name,
    detail: "",
  });
});

test("official strict registration rejects omitted focused test registration", () => {
  const root = strictUnusedRegistrationFixture();
  expect(inspectMathJaxUnusedAuditRoute(root).binding.standardLocalCheckRegistered).toBe(true);
  const recipe = actualRecipeBody("runtime-unused-script-regression-run");
  const focusedTest = " ./scripts/runtime-bundles/runtime-unused-check_test.ts";
  expect(recipe).toContain(focusedTest);
  recipes(root, {
    "runtime-unused-script-regression-run": recipe.replace(focusedTest, ""),
  });
  track(root);
  const report = inspectMathJaxUnusedAuditRoute(root);
  expect(report.binding.inputSelectionVerified).toBe(false);
  expect(report.binding.standardLocalCheckRegistered).toBe(false);
  expect(report.binding.standardCiCheckRegistered).toBe(false);
});

test.each(["  return {} as NativeWidthHashFacts;\n", '  throw new Error("dead spawn");\n'])(
  "NativeWidth owner rejects a terminal statement before the actual producer %s",
  (prefix) => {
    const root = nativeWidthHashFixture(),
      name = "scripts/runtime-bundles/runtime-unused-native-width-hash.ts";
    const baseline = inspectNativeWidthHashRoute(root);
    expect(baseline.facts).toMatchObject({
      factRecipeVerified: true,
      testCommandVerified: true,
      localCheckRegistered: true,
      ciCheckRegistered: true,
    });
    const original = fs.readFileSync(path.join(root, name), "utf8");
    const changed = original.replace(
      "  const child = Bun.spawnSync(",
      `${prefix}  const child = Bun.spawnSync(`,
    );
    expect(changed).not.toBe(original);
    write(root, name, changed);
    track(root);
    expect(
      sourceCode(changed, {
        name,
        read: (input) =>
          fs.existsSync(path.join(root, input))
            ? fs.readFileSync(path.join(root, input), "utf8")
            : undefined,
      }),
    ).toContain("!unresolved-executable");
    const report = inspectNativeWidthHashRoute(root);
    expect(report.facts).toMatchObject({
      factRecipeVerified: false,
      testCommandVerified: false,
      localCheckRegistered: false,
      ciCheckRegistered: false,
      nativeExecutionVerified: false,
      performanceVerified: false,
    });
    expect(report.issues.map((issue) => issue.code)).toContain(
      "native-width-hash-process-route-unverified",
    );
  },
);

const strictSelectionScopeMutations = [
  [
    "CLI shadow resolver",
    "scripts/mermaid/diagram-update.ts",
    "    const assetInputs = resolveOfficialRendererAssetInputs(",
    "    const resolveOfficialRendererAssetInputs = () => ({mermaidJs: '/forged-mermaid.js', mermaidZenumlJs: '/forged-zenuml.js'});\n    const assetInputs = resolveOfficialRendererAssetInputs(",
  ],
  [
    "context shadow resolver",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "  const selected = resolveOfficialRendererAssetInputs(",
    "  const resolveOfficialRendererAssetInputs = () => ({mermaidJs: '/forged-mermaid.js', mermaidZenumlJs: '/forged-zenuml.js'});\n  const selected = resolveOfficialRendererAssetInputs(",
  ],
  [
    "context unreachable resolver",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "  const selected = resolveOfficialRendererAssetInputs(",
    "  throw new Error('dead input selection');\n  const selected = resolveOfficialRendererAssetInputs(",
  ],
  [
    "CLI unreachable resolver",
    "scripts/mermaid/diagram-update.ts",
    "    const assetInputs = resolveOfficialRendererAssetInputs(",
    "    throw new Error('dead input selection');\n    const assetInputs = resolveOfficialRendererAssetInputs(",
  ],
  [
    "context early return",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "  const selected = resolveOfficialRendererAssetInputs(",
    "  return {} as RuntimeUnusedContext;\n  const selected = resolveOfficialRendererAssetInputs(",
  ],
  [
    "context module assignment",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function officialStrictUnusedContext(",
    "resolveOfficialRendererAssetInputs = () => ({});\nexport function officialStrictUnusedContext(",
  ],
  [
    "context escaped resolver alias",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "  const selected = resolveOfficialRendererAssetInputs(",
    "  const escapedResolver = resolveOfficialRendererAssetInputs;\n  const selected = escapedResolver(",
  ],
  [
    "factory early throw",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function actualOfficialStrictInputs(): OfficialStrictInputOptions {",
    "export function actualOfficialStrictInputs(): OfficialStrictInputOptions {\n  throw new Error('dead actual inputs');",
  ],
  [
    "factory process parameter shadow",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function actualOfficialStrictInputs():",
    "export function actualOfficialStrictInputs(process: typeof globalThis.process):",
  ],
  [
    "factory global process shadow",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function actualOfficialStrictInputs():",
    "const process = { argv: [], env: {}, cwd: () => '/forged' };\nexport function actualOfficialStrictInputs():",
  ],
  [
    "factory local process shadow",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function actualOfficialStrictInputs(): OfficialStrictInputOptions {",
    "export function actualOfficialStrictInputs(): OfficialStrictInputOptions {\n  const process = { argv: [], env: {}, cwd: () => '/forged' };",
  ],
  [
    "factory module os shadow",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function actualOfficialStrictInputs():",
    "const os = { homedir: () => '/forged' };\nexport function actualOfficialStrictInputs():",
  ],
  [
    "factory os member mutation",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function actualOfficialStrictInputs():",
    "os.homedir = () => '/forged';\nexport function actualOfficialStrictInputs():",
  ],
  [
    "factory os alias mutation",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function actualOfficialStrictInputs():",
    "const escapedOs = os; escapedOs.homedir = () => '/forged';\nexport function actualOfficialStrictInputs():",
  ],
  [
    "factory process member mutation",
    "scripts/runtime-bundles/runtime-unused-contexts.ts",
    "export function actualOfficialStrictInputs():",
    "process.env = {};\nexport function actualOfficialStrictInputs():",
  ],
  [
    "CLI receiver module process shadow",
    "scripts/mermaid/diagram-update.ts",
    "const CliOptions = {",
    "const process={env:{},cwd:()=>'/forged-cli-root',argv:[]};\nconst CliOptions = {",
  ],
  [
    "CLI receiver os homedir member mutation",
    "scripts/mermaid/diagram-update.ts",
    "const CliOptions = {",
    "os.homedir=()=>'/forged-cli-home';\nconst CliOptions = {",
  ],
  [
    "CLI receiver os alias member mutation",
    "scripts/mermaid/diagram-update.ts",
    "const CliOptions = {",
    "const escapedOs=os; escapedOs.homedir=()=>'/forged-cli-home';\nconst CliOptions = {",
  ],
  [
    "CLI receiver process env member mutation",
    "scripts/mermaid/diagram-update.ts",
    "const CliOptions = {",
    "process.env = {};\nconst CliOptions = {",
  ],
  [
    "CLI receiver reflected process mutation",
    "scripts/mermaid/diagram-update.ts",
    "const CliOptions = {",
    'globalThis["process"]={env:{},cwd:()=>"/forged",argv:[]};\nconst CliOptions = {',
  ],
  [
    "CLI receiver main argv replacement",
    "scripts/mermaid/diagram-update.ts",
    "CliOptions.parse(process.argv.slice(2))",
    "CliOptions.parse([])",
  ],
  [
    "CLI receiver help mutates argv before selection",
    "scripts/mermaid/diagram-update.ts",
    "  exitIfHelp(argv: string[]) {",
    "  exitIfHelp(argv: string[]) {\n    argv.splice(0);",
  ],
  [
    "CLI receiver parse facade reassignment",
    "scripts/mermaid/diagram-update.ts",
    "if (import.meta.main)\n",
    "CliOptions.parse = () => ({} as CliParsedOptions);\nif (import.meta.main)\n",
  ],
  [
    "CLI result Mermaid return override",
    "scripts/mermaid/diagram-update.ts",
    "      ...assetInputs,",
    "      ...assetInputs, mermaidJs: '/forged-consumer-mermaid.js',",
  ],
  [
    "CLI result Zenuml return override",
    "scripts/mermaid/diagram-update.ts",
    "      ...assetInputs,",
    "      ...assetInputs, mermaidZenumlJs: '/forged-consumer-zenuml.js',",
  ],
  [
    "CLI result both return overrides",
    "scripts/mermaid/diagram-update.ts",
    "      ...assetInputs,",
    "      ...assetInputs, mermaidJs: '/forged-consumer-mermaid.js', mermaidZenumlJs: '/forged-consumer-zenuml.js',",
  ],
  [
    "CLI result computed asset override",
    "scripts/mermaid/diagram-update.ts",
    "      ...assetInputs,",
    "      ...assetInputs, ['mermaid' + 'Js']: '/forged-consumer-mermaid.js',",
  ],
  [
    "CLI result unknown spread after selection",
    "scripts/mermaid/diagram-update.ts",
    "      ...assetInputs,",
    "      ...assetInputs, ...{mermaidJs: '/forged-consumer-mermaid.js'},",
  ],
  [
    "CLI result unknown spread before selection",
    "scripts/mermaid/diagram-update.ts",
    "      ...assetInputs,",
    "      ...{mermaidJs: '/forged-consumer-mermaid.js'}, ...assetInputs,",
  ],
  [
    "CLI result selected object alias mutation",
    "scripts/mermaid/diagram-update.ts",
    "    const fixturesDir = path.resolve(",
    "    const escapedAssets = assetInputs; escapedAssets.mermaidJs = '/forged-consumer-mermaid.js';\n    const fixturesDir = path.resolve(",
  ],
  [
    "CLI result renderer asset mutation",
    "scripts/mermaid/diagram-update.ts",
    "    const renderer = new OfficialMermaidRenderer(this.options);",
    "    this.options.mermaidJs = '/forged-consumer-mermaid.js';\n    const renderer = new OfficialMermaidRenderer(this.options);",
  ],
  [
    "CLI result validation asset mutation",
    "scripts/mermaid/diagram-update.ts",
    "    assertReferenceScope(this.options.referenceMode, this.options.referenceScope);",
    "    this.options.mermaidJs = '/forged-consumer-mermaid.js';\n    assertReferenceScope(this.options.referenceMode, this.options.referenceScope);",
  ],
  [
    "CLI result escaped options mutation",
    "scripts/mermaid/diagram-update.ts",
    "    assertReferenceScope(this.options.referenceMode, this.options.referenceScope);",
    "    const escapedOptions = this.options; escapedOptions.mermaidJs = '/forged-consumer-mermaid.js';\n    assertReferenceScope(this.options.referenceMode, this.options.referenceScope);",
  ],
  [
    "CLI result shadow renderer",
    "scripts/mermaid/diagram-update.ts",
    "    const renderer = new OfficialMermaidRenderer(this.options);",
    "    const OfficialMermaidRenderer = class { constructor(_options: RendererOptions) {} };\n    const renderer = new OfficialMermaidRenderer(this.options);",
  ],
  [
    "CLI result unreachable final return",
    "scripts/mermaid/diagram-update.ts",
    "    assertReferenceScope(mode, referenceScope);",
    "    assertReferenceScope(mode, referenceScope);\n    throw new Error('dead selected return');",
  ],
  [
    "CLI pre-render options ForOf write",
    "scripts/mermaid/diagram-update.ts",
    "  private validate() {",
    "  private validate() {\n    for (this.options.mermaidJs of ['/forged-consumer-mermaid.js']) {}",
  ],
  [
    "CLI pre-render this valueOf alias write",
    "scripts/mermaid/diagram-update.ts",
    "  private validate() {",
    "  private validate() {\n    const receiver = this.valueOf() as this; receiver.options.mermaidJs = '/forged-consumer-mermaid.js';",
  ],
  [
    "CLI pre-render ForIn target in later class method",
    "scripts/mermaid/diagram-update.ts",
    "    await renderer.render(fixture);",
    "    for (this.options.mermaidJs in {'/forged-consumer-mermaid.js': true}) {}\n    await renderer.render(fixture);",
  ],
  [
    "CLI pre-render ForOf destructured target in later class method",
    "scripts/mermaid/diagram-update.ts",
    "    await renderer.render(fixture);",
    "    for ({mermaidJs: this.options.mermaidJs} of [{mermaidJs: '/forged-consumer-mermaid.js'}]) {}\n    await renderer.render(fixture);",
  ],
  [
    "CLI pre-render unknown this call in later class method",
    "scripts/mermaid/diagram-update.ts",
    "    await renderer.render(fixture);",
    "    const receiver = this.valueOf() as this; receiver.options.mermaidJs = '/forged-consumer-mermaid.js';\n    await renderer.render(fixture);",
  ],
  [
    "CLI pre-render unknown options call in later class method",
    "scripts/mermaid/diagram-update.ts",
    "    await renderer.render(fixture);",
    "    const receiver = this.options.valueOf() as CliParsedOptions; receiver.mermaidJs = '/forged-consumer-mermaid.js';\n    await renderer.render(fixture);",
  ],
] as const;

describe("official strict asset selection scope", () => {
  let canonicalBinding: { inputPaths: string[]; inputDigest: string } | undefined;

  beforeAll(() => {
    const root = strictUnusedRegistrationFixture();
    const report = inspectMathJaxUnusedAuditRoute(root);
    expect(report.binding.inputPaths).toEqual(strictUnusedAuditInputPaths);
    expect(report.binding).toMatchObject({
      inputSelectionVerified: true,
      commandVerified: false,
      localCheckRegistered: false,
      ciCheckRegistered: false,
      standardCommandVerified: true,
      standardLocalCheckRegistered: true,
      standardCiCheckRegistered: true,
    });
    canonicalBinding = {
      inputPaths: [...report.binding.inputPaths],
      inputDigest: report.binding.inputDigest,
    };
  });

  test.each(strictSelectionScopeMutations)(
    "official strict asset selection scope refuses %s with otherwise canonical fields",
    (_kind, name, before, after) => {
      const baseline = canonicalBinding;
      if (!baseline) throw new Error("strict asset selection baseline was not initialized");

      const root = strictUnusedRegistrationFixture();
      expect(strictUnusedAuditInputPaths).toEqual(baseline.inputPaths);
      const source = fs.readFileSync(path.join(root, name), "utf8");
      expect(source).toContain(before);
      write(root, name, source.replace(before, after));
      track(root);
      const report = inspectMathJaxUnusedAuditRoute(root);
      expect(report.binding).toMatchObject({
        inputSelectionVerified: false,
        commandVerified: false,
        localCheckRegistered: false,
        ciCheckRegistered: false,
        standardCommandVerified: false,
        standardLocalCheckRegistered: false,
        standardCiCheckRegistered: false,
      });
      expect(report.binding.inputDigest).not.toBe(baseline.inputDigest);
      expect(report.issues.map((issue) => issue.code)).toContain(
        "mathjax-unused-input-selection-unverified",
      );
    },
  );
});
