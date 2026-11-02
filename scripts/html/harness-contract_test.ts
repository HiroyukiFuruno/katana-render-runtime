import { describe, expect, spyOn, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { runInNewContext } from "node:vm";
import { parseSync, version as swcVersion, transformSync } from "@swc/core";
import { z } from "zod";
import { changedCompilerInputRead } from "../quality/compiler-input-test-helper";
import {
  createContractRead,
  inspectNativeWidthHashRoute,
  MATHJAX_UNUSED_INPUTS,
  NATIVE_WIDTH_HASH_INPUTS,
} from "../quality/owned-validation-contract";
import { runtimeUnusedImportInputs } from "../runtime-bundles/runtime-mathjax-audit-evidence-inputs";
import {
  checkHarnessContract,
  declaredValidationCode,
  type HarnessSnapshot,
} from "./harness-contract";
import {
  moonRunContract,
  repositorySnapshot,
  resolveRepositoryCode,
  runtimeBundleCacheActionAuthorized,
  validateRuntimeBundleCacheArray,
  validateRuntimeBundleCacheBlob,
  verifyMoonRun,
} from "./harness-contract-cli";
import {
  type ContractPolicy,
  type ExternalToolBinding,
  type ExternalToolRequest,
  executionGraph,
  guardedFactsRecipeVerified,
  hostEvidenceSourceVerified,
  justRecipes,
  moonProjectionClosure,
  moonStaticTaskProjection,
  type NativeConsumerBinding,
  normalizeClassicFactsTestCommands,
  policyContract,
  RUNTIME_UNUSED_LEAF_IDS,
  shellWords,
  sourceCode,
  type UnitExecutableBinding,
} from "./harness-contract-graph";
import { MOON_QUALITY_INPUTS } from "./moon-quality-inputs";

// WHY: 実リポジトリの依存グラフを読むケースには、実測した処理量に応じた上限を設ける。
const REPOSITORY_GRAPH_TEST_TIMEOUT_MS = 15_000;

test.each([
  [
    "runtime",
    "runtime-rust-facts-host.ts",
    '["mathjax", "native-width", "classic", "macho-noso"].includes(modeOrReceipt ?? "")',
    '["mathjax", "native-width", "classic"].includes(modeOrReceipt ?? "")',
  ],
  ["protocol", "runtime-rust-facts-host-protocol.ts", '    "macho-noso": ["--macho-noso"],\n', ""],
])(
  "Rust facts host %s source pin rejects compact-query drift",
  (_label, basename, before, after) => {
    const name = `scripts/runtime-bundles/${basename}`;
    const source = fs.readFileSync(
      path.resolve(import.meta.dir, `../runtime-bundles/${basename}`),
      "utf8",
    );
    expect(hostEvidenceSourceVerified(name, source)).toBe(true);
    const drifted = source.replace(before, after);
    expect(drifted).not.toBe(source);
    expect(hostEvidenceSourceVerified(name, drifted)).toBe(false);
  },
);

function currentMoonConfig(): Record<string, unknown> {
  const source = fs.readFileSync(path.resolve(import.meta.dir, "../../moon.yml"), "utf8");
  return z.record(z.string(), z.unknown()).parse(Bun.YAML.parse(source));
}

test.each([
  ["coverage", "coverage-result-check", ["unit-test"]],
  [
    "performance-coverage-acceptance",
    "performance-coverage-result-check",
    ["performance-coverage"],
  ],
])("native coverage caches keep %s validator uncached", (entry, leaf, dependencies) => {
  const tasks = moonStaticTaskProjection(JSON.stringify(currentMoonConfig()), "krr", entry);
  expect(tasks["performance-coverage"]?.options.cache).toBe("local");
  expect(tasks[entry]?.options.cache).toBe(false);
  expect(tasks[entry]?.leaf).toBe(leaf);
  expect(tasks[entry]?.deps).toEqual(dependencies);
  if (entry === "coverage") {
    expect(tasks["unit-test"]?.options.cache).toBe("local");
    expect(tasks["unit-test"]?.deps).toEqual(["performance-coverage"]);
  }
});

test.each([
  ["options", { cache: true }],
  ["outputs", []],
  ["checks", []],
  ["inputs", []],
  ["deps", []],
  ["env", { COVERAGE_MIN_LINES: "99" }],
])("native coverage cache rejects altered %s", (field, value) => {
  for (const name of ["unit-test", "performance-coverage"]) {
    const config = currentMoonConfig();
    const tasks = z.record(z.string(), z.unknown()).parse(config.tasks);
    const task = z.record(z.string(), z.unknown()).parse(tasks[name]);
    task[String(field)] = value;
    tasks[name] = task;
    config.tasks = tasks;
    expect(() => moonStaticTaskProjection(JSON.stringify(config), "krr", "coverage")).toThrow();
  }
});

function withModulePackage(action: (root: string, directory: string) => void): void {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "managed-module-resolution-"));
  const root = fs.realpathSync(directory);
  fs.mkdirSync(path.join(root, "scripts/html"), { recursive: true });
  fs.mkdirSync(path.join(root, "data"));
  fs.writeFileSync(path.join(root, "scripts/html/helper.ts"), "export const helper = true;");
  fs.writeFileSync(path.join(root, "data/helper.ts"), "export const data = true;");
  fs.writeFileSync(
    path.join(root, "package.json"),
    JSON.stringify({
      imports: { "#owned/helper": "./scripts/html/helper.ts", "#unmanaged": "./data/helper.ts" },
    }),
  );
  try {
    action(root, directory);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

function fixture(): HarnessSnapshot {
  const policy: ContractPolicy = {
    schema: 1,
    codeRoots: ["scripts/"],
    localRecipes: ["check-assets"],
    localBridge: { recipe: "check", source: "scripts/hooks/run_parallel_checks.py" },
    ciWorkflow: ".github/workflows/test.yml",
    recipeDispatch: {},
  };
  const files = new Map([
    [
      "Justfile",
      "check:\n    python3 scripts/hooks/run_parallel_checks.py\ncheck-assets:\n    just html-test\nhtml-test:\n    bun scripts/html/main.ts --output tmp/report.json\n",
    ],
    ["scripts/hooks/run_parallel_checks.py", 'Lane("assets", "check-assets")'],
    [".github/workflows/test.yml", "run: just html-test"],
    [
      "scripts/html/main.ts",
      'import { helper } from "./helper"; helper("tmp/generated-output.ts");',
    ],
    ["scripts/html/helper.ts", "export function helper(output: string) { return output; }"],
  ]);
  const read = (name: string) => files.get(name);
  return {
    root: "/workspace",
    policy,
    read,
    graph: executionGraph(policy, read, {}, new Set()),
    codeFiles: ["scripts/html/main.ts", "scripts/html/helper.ts"],
    declaredCode: [],
    tracked: new Set(["scripts/html/main.ts", "scripts/html/helper.ts"]),
    biomeIncludes: ["scripts/**/*.ts", "!tmp"],
    typeIncludes: ["scripts/**/*.ts"],
    typeExcludes: ["tmp/**"],
  };
}

function replaceSources(snapshot: HarnessSnapshot, changes: Record<string, string>): void {
  const prior = snapshot.read;
  snapshot.read = (name) => changes[name] ?? prior(name);
  const coverageContext = coverageFixtureContexts.get(snapshot);
  snapshot.graph = executionGraph(
    snapshot.policy,
    snapshot.read,
    {},
    coverageContext?.dependencies ?? new Set(),
    snapshot.codeFiles,
    coverageContext?.resolveCode,
  );
}

const coverageFixtureContexts = new WeakMap<
  HarnessSnapshot,
  { dependencies: Set<string>; resolveCode: (reference: string, importer?: string) => string }
>();

type TestMoonTask = {
  command: string | string[];
  deps: unknown[];
  inputs?: string[];
  outputs?: string[];
  checks?: Array<Record<string, unknown>>;
  options?: Record<string, unknown>;
};

function moonFixture(): HarnessSnapshot {
  const snapshot = fixture();
  const staticNames = [
    "lint",
    "python-lint",
    "python-typecheck",
    "biome",
    "typecheck",
    "duplication-check",
    "owned-validation-contract-check",
  ];
  const tasks: Record<string, TestMoonTask> = Object.fromEntries(
    staticNames.map((name) => [name, { command: ["just", name], deps: [] }]),
  );
  tasks.static = { command: "noop", deps: staticNames };
  tasks["html-test"] = { command: ["just", "html-test"], deps: ["static"] };
  tasks.check = { command: "noop", deps: ["html-test"] };
  snapshot.policy.moon = {
    workspace: ".moon/workspace.yml",
    config: "moon.yml",
    project: "krr",
    entry: "check",
    barrier: "static",
    ciJob: "test",
    requiredStatic: staticNames,
    requiredHeavy: ["html-test"],
  };
  snapshot.policy.python = {
    lintRecipe: "python-lint",
    typeRecipe: "python-typecheck",
    ruff: "0.16.10",
    mypy: "2.4.0",
  };
  snapshot.policy.rust = { lintRecipe: "lint", workspace: "Cargo.toml" };
  snapshot.policy.codeRoots.push("crates/");
  replaceSources(snapshot, {
    ".moon/workspace.yml":
      "projects:\n  krr: '.'\nversionConstraint: '=2.6.5'\ncache:\n  cas:\n    verifyIntegrity: true\nexperiments:\n  casOutputsCache: true\n",
    "moon.yml": JSON.stringify({
      taskOptions: { cache: false, runInCI: "always", shell: false },
      tasks,
    }),
    [snapshot.policy.localBridge.source]:
      'command = ["bun", "run", "moon", "--concurrency", str(jobs), "exec", "krr:check", "--on-failure", "continue", "--no-actions"]',
    [snapshot.policy.ciWorkflow]: "jobs:\n  test:\n    steps:\n      - run: just check\n",
    Justfile: `${snapshot.read("Justfile")}HTML_PERFORMANCE_SCIPY_VERSION := "1.18.1"\nHTML_PERFORMANCE_SCIPY_ENV := "KRR_HTML_PERFORMANCE_SCIPY_VERSION=" + HTML_PERFORMANCE_SCIPY_VERSION\nHTML_PERFORMANCE_TYPING_PATH := "scripts/release/typings"\nHTML_PERFORMANCE_PYTHON_REQUIREMENTS := REPO_ROOT + "/scripts/release/html-performance-requirements.lock"\nlint:\n    cargo clippy --workspace --all-targets --all-features -- -D warnings\npython-lint:\n    uvx --from ruff==0.16.10 ruff check --select F,E4,E7,E9,B,UP scripts\npython-typecheck:\n    MYPYPATH={{HTML_PERFORMANCE_TYPING_PATH}} {{HTML_PERFORMANCE_SCIPY_ENV}} uvx --with-requirements {{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}} --from mypy==2.4.0 mypy --strict scripts\nbiome:\n    bun run biome\ntypecheck:\n    bun run typecheck\nduplication-check:\n    bun scripts/quality/duplication-check.ts\nowned-validation-contract-check:\n    bun scripts/quality/owned-validation-contract.ts\n`,
    "Cargo.toml": '[workspace]\nmembers = ["crates/renderer"]\n',
    "crates/renderer/Cargo.toml": '[package]\nname = "renderer"\n',
    "crates/renderer/src/lib.rs": "pub fn render() {}",
    "scripts/html/managed.py": "def check() -> None: pass",
    "scripts/quality/duplication-check.ts": "export const duplicationCheck = true;",
    "scripts/quality/owned-validation-contract.ts": "export const ownedValidationContract = true;",
  });
  snapshot.tracked.add("scripts/quality/duplication-check.ts");
  snapshot.tracked.add("scripts/quality/owned-validation-contract.ts");
  return snapshot;
}

function changeMoon(
  snapshot: HarnessSnapshot,
  edit: (tasks: Record<string, TestMoonTask>) => void,
): void {
  const config = JSON.parse(snapshot.read("moon.yml") ?? "") as {
    tasks: Record<string, TestMoonTask>;
  };
  edit(config.tasks);
  replaceSources(snapshot, { "moon.yml": JSON.stringify(config) });
}

const nativeRouteProducer = `
import { verifyNativeBuildReceipt } from "./native-build-receipt";
import { spawnNativeCli } from "./native-cli-process";
function prepareRender(input: Options) {
  const build = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime);
  return { build };
}
async function run(input: Options) {
  const prepared = prepareRender(input);
  const rows = await renderFixtures([], input, prepared.build);
  const afterBuild = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime);
  return { rows, afterBuild };
}
async function renderFixtures(fixtures: Fixture[], input: Options, build: Receipt) {
  for (const fixture of fixtures) await renderFixture(fixture, input, build);
}
async function renderFixture(fixture: Fixture, input: Options, build: Receipt) {
  const child = await spawnNativeCli(input.binary, [], "stdout", "stderr");
  return child;
}`;

const nativeRouteProcess = `import { spawn } from "node:child_process";
export async function spawnNativeCli(binary: string, argv: string[]) {
  const child = spawn(binary, argv.slice(1));
  return child;
}`;

const observerRouteSource = `
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { physicalFileIdentity } from "./raster-comparison-binding";
export function collectorPath() {
  return path.join(path.dirname(fileURLToPath(import.meta.url)), "browser-process-observe.py");
}
function requireApprovedBrowserObservationRoute(observerPython: string) {
  physicalFileIdentity(observerPython);
  physicalFileIdentity(collectorPath());
}
export function observeActualBrowserIdentity(browser: Browser, request: Request) {
  requireApprovedBrowserObservationRoute(request.observerPython);
  const result = spawnSync(request.observerPython, [collectorPath(), "1", "2"]);
  return result;
}`;

function processRouteFixture(): HarnessSnapshot {
  const snapshot = moonFixture();
  const sources = {
    "scripts/html/main.ts":
      'import "./helper"; import "../mermaid/native-render-provenance"; import "../mermaid/raster-browser-observation";',
    "scripts/mermaid/native-cli-process.ts": nativeRouteProcess,
    "scripts/mermaid/native-render-provenance.ts": nativeRouteProducer,
    "scripts/mermaid/native-build-receipt.ts": "export function verifyNativeBuildReceipt() {}",
    "scripts/mermaid/raster-browser-observation.ts": observerRouteSource,
    "scripts/mermaid/raster-comparison-binding.ts": "export function physicalFileIdentity() {}",
    "scripts/mermaid/browser-process-observe.py": "def observe() -> None: pass",
  };
  for (const name of Object.keys(sources)) snapshot.tracked.add(name);
  replaceSources(snapshot, sources);
  return snapshot;
}

describe("finite Rust host and MathJax evidence registration", () => {
  const root = path.resolve(import.meta.dir, "../..");
  const justfile = fs.readFileSync(path.join(root, "Justfile"), "utf8");
  const moonSource = fs.readFileSync(path.join(root, "moon.yml"), "utf8");
  const policy = policyContract.parse(
    JSON.parse(
      fs.readFileSync(path.join(root, "scripts/html/harness-contract-policy.json"), "utf8"),
    ),
  );
  const graph = (moon: string, recipes = justfile, codeRoots = policy.codeRoots) =>
    executionGraph(
      { ...policy, codeRoots },
      (name) => {
        if (name === "moon.yml") return moon;
        if (name === "Justfile") return recipes;
        const file = path.join(root, name);
        return fs.existsSync(file) ? fs.readFileSync(file, "utf8") : undefined;
      },
      {},
      new Set(),
    );
  // WHY: Moon設定の改変は、ソースASTを再走査せず同じグラフの依存契約で確認する。
  const orderGraph = (moon: string) => graph(moon, justfile, []);
  const currentErrors = graph(moonSource).errors;
  const currentOrderErrors = currentErrors.filter((error) => error.includes("order bypass"));
  test.each([
    [
      "prebuild",
      "capacity-admission",
      "source-static-foundation",
      "moon host bootstrap order bypass",
    ],
    ["biome", "biome", "rust-facts-host-prebuild", "moon host bootstrap order bypass"],
    [
      "final foundation",
      "source-static",
      "source-static-foundation",
      "moon host bootstrap order bypass",
    ],
    ["final biome", "source-static", "biome", "moon host bootstrap order bypass"],
    ["acquisition", "mathjax-owned-evidence", "static", "moon host evidence order bypass"],
    [
      "acquisition host",
      "mathjax-owned-evidence",
      "rust-facts-host-prebuild",
      "moon host bootstrap order bypass",
    ],
    [
      "classic generation",
      "runtime-classic-generation-evidence",
      "runtime-asset-script-preflight",
      "moon host evidence order bypass",
    ],
  ])("rejects %s as a parallel sibling or static bypass", (_label, after, before, prefix) => {
    expect(currentOrderErrors).toEqual([]);
    const changed = Bun.YAML.parse(moonSource) as { tasks: Record<string, { deps: string[] }> };
    const task = changed.tasks[after];
    if (!task) throw new Error("fixture task missing");
    task.deps = task.deps.filter((name) => name !== before);
    expect(orderGraph(JSON.stringify(changed)).errors).toContain(
      `${prefix}: ${before} -> ${after}`,
    );
  });
  test("rejects a missing unit dependency through the dependency contract", () => {
    const prefix = "moon contract invalid: ";
    expect(currentErrors.filter((error) => error.startsWith(prefix))).toEqual([]);
    const changed = Bun.YAML.parse(moonSource) as { tasks: Record<string, { deps: string[] }> };
    const task = changed.tasks["unit-test"];
    if (!task) throw new Error("fixture unit-test missing");
    task.deps = ["static"];
    const errors = orderGraph(JSON.stringify(changed)).errors.filter((error) =>
      error.startsWith(prefix),
    );
    expect(errors).toHaveLength(1);
    const diagnostic = errors[0];
    if (diagnostic === undefined) throw new Error("unit dependency diagnostic missing");
    const violations: unknown = JSON.parse(diagnostic.slice(prefix.length));
    expect(violations).toContainEqual(expect.objectContaining({ code: "custom", path: ["deps"] }));
  });
  test("foundation includes format and rejects the old host-dependent cycle", () => {
    const changed = Bun.YAML.parse(moonSource) as { tasks: Record<string, { deps: string[] }> };
    const foundation = changed.tasks["source-static-foundation"];
    if (!foundation) throw new Error("fixture foundation missing");
    foundation.deps = foundation.deps.filter((name) => name !== "fmt-check");
    expect(orderGraph(JSON.stringify(changed)).errors).toContain(
      "moon foundation leaf missing: fmt-check",
    );
    foundation.deps.push("biome");
    expect(
      orderGraph(JSON.stringify(changed)).errors.some((error) => error.includes("cycle")),
    ).toBe(true);
  });
  test.each([
    ["verify", "prepare"],
    ["/current/receipt.json", "/stale/receipt.json"],
    ["runtime-unused-check.ts --gate", "runtime-unused-check.ts"],
    ["runtime-unused-check.ts --gate", "runtime-unused-check.ts --gate --allow-unknown"],
    ["bun scripts/runtime-bundles/runtime-unused-check.ts --gate", "bun run biome"],
  ])(
    "Biome dispatch rejects %s drift",
    (before, after) => {
      const body = justRecipes(justfile).get("biome-run") ?? "";
      const changed = justfile.replace(body, body.replace(before, after));
      expect(changed).not.toBe(justfile);
      expect(graph(moonSource, changed).errors).toContain("Biome bootstrap dispatch differs");
    },
    REPOSITORY_GRAPH_TEST_TIMEOUT_MS,
  );
  test("rejects a wrapper that dispatches back into itself through Moon", () => {
    const changed = justfile.replace(
      "bun run moon exec krr:runtime-asset-script-test",
      "bun run moon exec krr:rust-facts-host-prebuild",
    );
    expect(graph(moonSource, changed).errors).toContain("runtime script Moon wrapper differs");
  });
  test.each([
    "mathjax-consumer-facts",
    "native-width-hash-facts",
    "mermaid-classic-script-facts",
  ] as const)("%s rejects hidden build, fallback, receipt, and tool drift", (recipe) => {
    expect(guardedFactsRecipeVerified(justfile, recipe)).toBe(true);
    const body = justRecipes(justfile).get(recipe) ?? "";
    for (const mutated of [
      `${body}    {{CARGO_BUILD}} build --locked -p kdr-linter --bin mathjax_consumer_facts\n`,
      body.replace(" run {{quote(root)}}", " prepare {{quote(root)}}"),
      body.replace("/current/receipt.json", "/stale/receipt.json"),
      body.replace("}} bun {{quote(root", "}} {{CARGO}} {{quote(root"),
      `${body}    bun -e 'console.log("[]")'\n`,
    ]) {
      expect(mutated).not.toBe(body);
      expect(guardedFactsRecipeVerified(justfile.replace(body, mutated), recipe)).toBe(false);
    }
  });
  test("counts top-level facts variables but ignores recipe-local CARGO assignment", () => {
    expect(justfile).toContain("root-test-and-coverage: plantuml-install");
    expect(justRecipes(justfile).get("root-test-and-coverage")).toContain("CARGO={{quote(CARGO)}}");
    expect(guardedFactsRecipeVerified(justfile, "mathjax-consumer-facts")).toBe(true);
    const globalShadow = justfile.replace(
      'export CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")',
      'export CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")\nCARGO := "cargo"',
    );
    expect(globalShadow).not.toBe(justfile);
    expect(guardedFactsRecipeVerified(globalShadow, "mathjax-consumer-facts")).toBe(false);
    const cargoNotExported = justfile.replace(
      'export CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")',
      'CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")',
    );
    expect(cargoNotExported).not.toBe(justfile);
    expect(guardedFactsRecipeVerified(cargoNotExported, "mathjax-consumer-facts")).toBe(false);
    const duplicateProxy = justfile.replace(
      'RTK_PROXY_CMD := if RTK == "" { "" } else { RTK + " proxy " }',
      'RTK_PROXY_CMD := if RTK == "" { "" } else { RTK + " proxy " }\nRTK_PROXY_CMD := ""',
    );
    expect(duplicateProxy).not.toBe(justfile);
    expect(guardedFactsRecipeVerified(duplicateProxy, "mathjax-consumer-facts")).toBe(false);
    const proxyBypassesRtk = justfile.replace(
      'RTK_PROXY_CMD := if RTK == "" { "" } else { RTK + " proxy " }',
      'RTK_PROXY_CMD := if RTK == "" { "" } else { "rtk proxy " }',
    );
    expect(proxyBypassesRtk).not.toBe(justfile);
    expect(guardedFactsRecipeVerified(proxyBypassesRtk, "mathjax-consumer-facts")).toBe(false);
    const cargoBypassesProxy = justfile.replace(
      'export CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")',
      'export CARGO := env_var_or_default("CARGO", "cargo")',
    );
    expect(cargoBypassesProxy).not.toBe(justfile);
    expect(guardedFactsRecipeVerified(cargoBypassesProxy, "mathjax-consumer-facts")).toBe(false);
  });
});

describe("fixed native artifact and Python collector bindings", () => {
  test.each([
    ["current candidate probe route", (source: string) => source, false],
    [
      "changed executable binding",
      (source: string) => source.replaceAll("options.probeBinary.path", "options.other.path"),
      true,
    ],
    [
      "extra probe argument",
      (source: string) => source.replaceAll('"--scroll-y",', '"--forged", "1", "--scroll-y",'),
      true,
    ],
    [
      "different argv binding",
      (source: string) => source.replace("Bun.spawnSync(argv,", "Bun.spawnSync(forged,"),
      true,
    ],
    [
      "mutated argv",
      (source: string) =>
        source.replace("Bun.spawnSync(argv,", 'argv.push("--forged"); Bun.spawnSync(argv,'),
      true,
    ],
    [
      "spread argv element",
      (source: string) =>
        source.replace("options.probeBinary.path,", "...options.probeBinary.path,"),
      true,
    ],
    [
      "callee shadow",
      (source: string) =>
        source.replace(
          "  const result = Bun.spawnSync(argv, {",
          "  const Bun = { spawnSync: Bun.spawnSync };\n  const result = Bun.spawnSync(argv, {",
        ),
      true,
    ],
  ])("keeps native frame probe binding exact: %s", (_label, mutate, rejected) => {
    const name = "scripts/mermaid/native-frame-evidence.ts";
    const read = (file: string) =>
      fs.readFileSync(path.join(import.meta.dir, "../..", file), "utf8");
    const references = sourceCode(mutate(read(name)), { name, read });
    if (rejected) expect(references).toContain("!unresolved-executable");
    else expect(references).not.toContain("!unresolved-executable");
  });

  test("binds actual current producers without running native or browser captures", () => {
    const read = (name: string) =>
      fs.readFileSync(path.join(import.meta.dir, "../..", name), "utf8");
    for (const name of [
      "scripts/mermaid/native-cli-process.ts",
      "scripts/mermaid/raster-browser-observation.ts",
    ]) {
      const references = sourceCode(read(name), {
        name,
        importer: "scripts/mermaid/native-render-provenance.ts",
        read,
      });
      expect(references).not.toContain("!unresolved-executable");
      expect(references).toContain(
        name.endsWith("native-cli-process.ts")
          ? "node:child_process"
          : "!execute-local:browser-process-observe.py",
      );
    }
  });

  test("resolves only the owned literal RTK dry-run recipe inventory", () => {
    const name = "scripts/quality/owned-validation-contract_test.ts";
    const read = (file: string) =>
      fs.readFileSync(path.join(import.meta.dir, "../..", file), "utf8");
    const source = read(name);
    const context = { name, read };
    expect(sourceCode(source, context)).not.toContain("!unresolved-executable");
    for (const [label, mutate] of [
      ["operation", (value: string) => value.replace('"--dry-run"', '"--evaluate"')],
      [
        "spread executable",
        (value: string) =>
          value.replace(
            'spawnSync("just", ["--dry-run", recipe]',
            'spawnSync(..."just", ["--dry-run", recipe]',
          ),
      ],
      [
        "spread option",
        (value: string) => value.replace('["--dry-run", recipe]', '[..."--dry-run", recipe]'),
      ],
      [
        "recipe inventory",
        (value: string) =>
          value.replace('"html-input-state-fidelity-interim-check",', '"other-recipe",'),
      ],
      [
        "interim recipe missing",
        (value: string) => value.replace('    "html-dynamic-fidelity-interim-check",\n', ""),
      ],
      [
        "loop binding",
        (value: string) => value.replace('["--dry-run", recipe]', '["--dry-run", otherRecipe]'),
      ],
      [
        "loop mutability",
        (value: string) => value.replace("for (const recipe of [", "for (let recipe of ["),
      ],
      [
        "capacity argument",
        (value: string) =>
          value.replace('["--dry-run", "build-capacity-check"]', '["--dry-run", "other-recipe"]'),
      ],
      [
        "options",
        (value: string) =>
          value.replace(
            '      encoding: "utf8",\n      env,',
            '      encoding: "buffer",\n      env,',
          ),
      ],
    ] as const) {
      const changed = mutate(source);
      expect(changed, label).not.toBe(source);
      expect(sourceCode(changed, context), label).toContain("!unresolved-executable");
    }
  });

  const classicFactsCaller = "scripts/runtime-bundles/runtime-unused-rust-classic.ts";
  const classicFactsSource = fs.readFileSync(
    path.join(import.meta.dir, "../runtime-bundles/runtime-unused-rust-classic.ts"),
    "utf8",
  );
  const classicJustfile = fs.readFileSync(path.join(import.meta.dir, "../../Justfile"), "utf8");
  const factsGuardSource = fs.readFileSync(
    path.join(import.meta.dir, "../build/cargo_capacity_guard.py"),
    "utf8",
  );
  const classicFactsReferences = (
    source = classicFactsSource,
    justfile = classicJustfile,
    guard = true,
  ) =>
    sourceCode(source, {
      name: classicFactsCaller,
      read: (name) =>
        name === "Justfile"
          ? justfile
          : name === "scripts/build/cargo_capacity_guard.py" && guard
            ? factsGuardSource
            : undefined,
    });
  test("recognizes the actual root-only Classic facts caller and guarded two-root recipe", () => {
    expect(classicFactsReferences()).not.toContain("!unresolved-executable");
    expect(guardedFactsRecipeVerified(classicJustfile, "mermaid-classic-script-facts")).toBe(true);
    expect(
      sourceCode(classicFactsSource, {
        name: "scripts/runtime-bundles/unknown.ts",
        read: () => undefined,
      }),
    ).toContain("!unresolved-executable");
    expect(classicFactsReferences(classicFactsSource, classicJustfile, false)).toContain(
      "!unresolved-executable",
    );
  });
  test.each([
    [
      "source root used for compilation",
      "repositoryRoot, actualSourceRoot]",
      "actualSourceRoot, actualSourceRoot]",
    ],
    ["wrong cwd", "cwd: repositoryRoot", "cwd: actualSourceRoot"],
    ["wrong recipe", '"mermaid-classic-script-facts"', '"native-width-hash-facts"'],
    [
      "uncanonical repository",
      'fs.realpathSync(path.resolve(import.meta.dir, "../.."))',
      'path.resolve(import.meta.dir, "../..")',
    ],
    [
      "wrong repository path",
      'path.resolve(import.meta.dir, "../..")',
      'path.resolve(import.meta.dir, "..")',
    ],
    [
      "wrong data canonicalization",
      "fs.realpathSync(sourceRoot)",
      "fs.realpathSync(repositoryRoot)",
    ],
    [
      "shadow Bun",
      "inspectRustClassicScriptFacts(sourceRoot: string)",
      "inspectRustClassicScriptFacts(sourceRoot: string, Bun: unknown)",
    ],
    [
      "shadow filesystem",
      "inspectRustClassicScriptFacts(sourceRoot: string)",
      "inspectRustClassicScriptFacts(sourceRoot: string, fs: unknown)",
    ],
    [
      "shadow path",
      "inspectRustClassicScriptFacts(sourceRoot: string)",
      "inspectRustClassicScriptFacts(sourceRoot: string, path: unknown)",
    ],
    ["early return", "  const child =", "  return {} as RustClassicScriptFacts;\n  const child ="],
    ["early throw", "  const child =", '  throw new Error("dead spawn");\n  const child ='],
    ["dead nested spawn", "const child = Bun.spawnSync(", "const child = false && Bun.spawnSync("],
    ["extra spawn", "  const stdout =", "  Bun.spawnSync([unknown]);\n  const stdout ="],
    [
      "physical precheck replaced",
      "const before = physicalInputs(actualSourceRoot)",
      "const before = unknownPhysicalInputs(actualSourceRoot)",
    ],
    [
      "physical postcheck replaced",
      "const after = physicalInputs(actualSourceRoot)",
      "const after = before",
    ],
    [
      "unknown schema initializer",
      "const rangeSchema = z.strictObject",
      "const rangeSchema = unknownCallback",
    ],
    [
      "unknown entry initializer",
      'scriptEntry("mermaid.min.js", "bundle")',
      'unknownCallback("mermaid.min.js", "bundle")',
    ],
    ["wrong physical helper import", '"./runtime-unused-origin"', '"./unreviewed-origin"'],
    [
      "postcheck termination",
      "  const after =",
      "  return Object.freeze(report);\n  const after =",
    ],
    ["wrong report validator", "validateOutput(value, before)", "unknownValidator(value, before)"],
    [
      "hash comparison removed",
      "after.get(relative)?.sha256 !== before.get(relative)?.sha256",
      "false",
    ],
    ["inventory truncated", '  "Cargo.lock",', ""],
    [
      "physical callback terminal",
      "const absolute = path.join(root, relative);",
      'throw new Error("dead precheck");\n      const absolute = path.join(root, relative);',
    ],
    [
      "reader moved into callback",
      "export function inspectRustClassicScriptFacts",
      "function unknownReader",
    ],
    [
      "module global filesystem mutation",
      "export const RUST_CLASSIC_INPUT_PATHS",
      "fs.realpathSync = replacement;\nexport const RUST_CLASSIC_INPUT_PATHS",
    ],
    [
      "unknown callback before reader",
      "const RUST_CLASSIC_INPUT_PATHS",
      "const interrupted = unknownCallback();\nconst RUST_CLASSIC_INPUT_PATHS",
    ],
  ])("Classic facts route rejects %s", (_label, before, after) => {
    expect(classicFactsSource).toContain(before);
    const changed = classicFactsSource.replace(before, after);
    expect(changed).not.toBe(classicFactsSource);
    expect(classicFactsReferences()).not.toContain("!unresolved-executable");
    expect(classicFactsReferences(changed)).toContain("!unresolved-executable");
  });
  test.each([
    [
      "undefined guarded build",
      'CARGO_BUILD := "python3 " + quote(REPO_ROOT + "/scripts/build/cargo_capacity_guard.py") + " --retain-build-profile debug -- " + CARGO',
      "",
    ],
    ["unguarded build", "{{CARGO_BUILD}} build --locked", "{{CARGO}} build --locked"],
    ["compile data root", 'quote(root + "/target")', 'quote(source_root + "/target")'],
    ["wrong Classic mode", "run {{quote(root)}} classic", "run {{quote(root)}} native-width"],
    ["unquoted data root", "classic {{quote(source_root)}}", "classic {{source_root}}"],
    [
      "wrong Classic header",
      "mermaid-classic-script-facts root=REPO_ROOT source_root=REPO_ROOT:",
      "mermaid-classic-script-facts root=REPO_ROOT:",
    ],
  ])("Classic facts recipe rejects %s", (_label, before, after) => {
    expect(classicJustfile).toContain(before);
    const changed = classicJustfile.replaceAll(before, after);
    expect(classicFactsReferences()).not.toContain("!unresolved-executable");
    expect(guardedFactsRecipeVerified(changed, "mermaid-classic-script-facts")).toBe(false);
    expect(classicFactsReferences(classicFactsSource, changed)).toContain("!unresolved-executable");
  });

  const runtimeAssetRecipes = justRecipes(classicJustfile);
  const assetCommands = [
    ...(runtimeAssetRecipes.get("runtime-asset-script-other-run") ?? "").split("\n"),
    ...(runtimeAssetRecipes.get("runtime-unused-script-regression-run") ?? "").split("\n"),
  ]
    .map((line) => line.trim())
    .filter(Boolean);
  test("normalizes only the actual canonical Classic final test registration", () => {
    const normalized = normalizeClassicFactsTestCommands(assetCommands);
    expect(normalized).toBeDefined();
    expect(normalized?.join("\n")).not.toContain("runtime-unused-rust-classic_test.ts");
    expect(normalized?.join("\n")).toContain("runtime-unused-native-width-hash_test.ts");
  });
  test.each([
    [
      "duplicate",
      "./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
      "./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts ./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
    ],
    [
      "wrong preceding test",
      "./scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts ./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
      "unreviewed_test.ts ./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
    ],
    ["wrong command", "bun test --path-ignore-patterns", "bun run --path-ignore-patterns"],
    [
      "extra trailing argv",
      "./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
      "./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts extra",
    ],
    [
      "noncanonical bundle command",
      "./scripts/runtime-bundles/runtime-fragment-loader_test.ts",
      "unreviewed-loader_test.ts",
    ],
  ])("Classic test registration rejects %s", (_label, before, after) => {
    expect(assetCommands.join("\n")).toContain(before);
    expect(normalizeClassicFactsTestCommands(assetCommands)).toBeDefined();
    expect(
      normalizeClassicFactsTestCommands(assetCommands.map((line) => line.replace(before, after))),
    ).toBeUndefined();
  });

  const mathjaxFactsCaller = "scripts/runtime-bundles/runtime-unused-mathjax.ts";
  const mathjaxFactsSource = fs.readFileSync(
    path.join(import.meta.dir, "../runtime-bundles/runtime-unused-mathjax.ts"),
    "utf8",
  );
  const mathjaxFactsReferences = (source = mathjaxFactsSource, justfile = classicJustfile) =>
    sourceCode(source, {
      name: mathjaxFactsCaller,
      read: (name) =>
        name === "Justfile"
          ? justfile
          : name === "scripts/build/cargo_capacity_guard.py"
            ? factsGuardSource
            : undefined,
    });
  test("registered production Classic reader requires its canonical test tail", () => {
    withModulePackage((root) => {
      const actualRoot = path.resolve(import.meta.dir, "../..");
      const authorityPaths = [
        "package.json",
        "biome.json",
        "tsconfig.json",
        "scripts/html/harness-contract-policy.json",
        "scripts/hooks/run_parallel_checks.py",
        "scripts/html/harness-contract-cli.ts",
        ".github/workflows/test-and-build.yml",
        ".moon/workspace.yml",
        "moon.yml",
        "scripts/build/cargo_capacity_guard.py",
        classicFactsCaller,
        "scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
        "scripts/html/playwright.macos.config.ts",
        "scripts/html/playwright-config-base.ts",
        "scripts/mermaid/official_raw_producer_macos_browser_test.ts",
      ];
      for (const name of new Set([...NATIVE_WIDTH_HASH_INPUTS, ...authorityPaths])) {
        const destination = path.join(root, name);
        fs.mkdirSync(path.dirname(destination), { recursive: true });
        fs.copyFileSync(path.join(actualRoot, name), destination);
      }
      execFileSync("git", ["-C", root, "init", "--quiet"]);
      execFileSync("git", ["-C", root, "add", "."]);
      const baseline = inspectNativeWidthHashRoute(root);
      expect(baseline.facts).toMatchObject({
        factRecipeVerified: true,
        testCommandVerified: true,
        localCheckRegistered: true,
        ciCheckRegistered: true,
      });
      const before = fs.readFileSync(path.join(root, "Justfile"), "utf8");
      const tail = " ./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts";
      const compatibilityBody = justRecipes(before).get("runtime-unused-script-regression-run");
      if (!compatibilityBody) throw new Error("compatibility recipe missing");
      expect(compatibilityBody.split(tail)).toHaveLength(2);
      const changedCompatibilityBody = compatibilityBody.replace(tail, "");
      fs.writeFileSync(
        path.join(root, "Justfile"),
        before.replace(compatibilityBody, changedCompatibilityBody),
      );
      const removed = inspectNativeWidthHashRoute(root);
      expect(removed.issues.map((issue) => issue.code)).toContain(
        "rust-classic-script-test-unregistered",
      );
      expect(removed.facts).toMatchObject({
        factRecipeVerified: true,
        testCommandVerified: false,
        localCheckRegistered: false,
        ciCheckRegistered: false,
        nativeExecutionVerified: false,
        performanceVerified: false,
      });
      const receiptPath = process.env.KRR_CLASSIC_TAIL_REGRESSION_RECEIPT;
      if (receiptPath) {
        const paths = [...new Set([...NATIVE_WIDTH_HASH_INPUTS, ...authorityPaths])];
        fs.writeFileSync(
          receiptPath,
          JSON.stringify(
            {
              fixtureKind: "actual copied source and real Git index; static registration only",
              fixtureAuthorityCount: paths.length,
              sourcePins: paths.map((name) => ({
                path: name,
                sha256: createHash("sha256")
                  .update(fs.readFileSync(path.join(root, name)))
                  .digest("hex"),
              })),
              baselineFacts: baseline.facts,
              removedTailFacts: removed.facts,
              removedTailIssues: removed.issues.filter(
                (issue) =>
                  issue.code.startsWith("native-width-hash-") ||
                  issue.code === "rust-classic-script-test-unregistered",
              ),
              nativeExecutionVerified: false,
              consumerClosureVerified: false,
            },
            null,
            2,
          ),
        );
      }
    });
  });
  test("normal metadata readers do not retain source across inspections", () => {
    withModulePackage((root) => {
      const firstRead = createContractRead(root);
      expect(firstRead("data/helper.ts")).toContain("data = true");
      fs.writeFileSync(path.join(root, "data/helper.ts"), "export const fresh = true;");
      const secondRead = createContractRead(root);
      expect(secondRead("data/helper.ts")).toContain("fresh = true");
    });
  });
  test("the actual repository restores NativeWidth registration with both facts readers", () => {
    expect(mathjaxFactsReferences()).not.toContain("!unresolved-executable");
    const report = inspectNativeWidthHashRoute(path.resolve(import.meta.dir, "../.."));
    expect(report.facts).toMatchObject({
      factRecipeVerified: true,
      testCommandVerified: true,
      localCheckRegistered: true,
      ciCheckRegistered: true,
      nativeExecutionVerified: false,
      performanceVerified: false,
    });
    expect(report.issues).toEqual([]);
  });
  test.each([
    [
      "global Bun replacement",
      "const MATHJAX_RUST_FACTS_SCHEMA",
      "globalThis.Bun = replacement;\nconst MATHJAX_RUST_FACTS_SCHEMA",
    ],
    [
      "shared read helper input removed",
      '  "crates/katana-render-runtime/src/markdown/runtime_assets.rs",\n',
      "",
    ],
    [
      "shared read helper input substituted",
      '"crates/katana-render-runtime/src/markdown/runtime_assets.rs"',
      '"crates/katana-render-runtime/src/markdown/other_assets.rs"',
    ],
    [
      "caller repository replaced by source data",
      "fs.realpathSync(root)",
      "fs.realpathSync(sourceRoot)",
    ],
    [
      "two-root argv substituted",
      '"mathjax-consumer-facts", repositoryRoot]',
      '"mathjax-consumer-facts", repositoryRoot, root]',
    ],
    ["wrong cwd", "cwd: repositoryRoot", "cwd: root"],
    [
      "shadow Bun",
      "inspectMathJaxRustFacts(root: string)",
      "inspectMathJaxRustFacts(root: string, Bun: unknown)",
    ],
    [
      "shadow filesystem",
      "inspectMathJaxRustFacts(root: string)",
      "inspectMathJaxRustFacts(root: string, fs: unknown)",
    ],
    ["wrong path import", 'from "node:path"', 'from "./unreviewed-path"'],
    [
      "path mutator alias",
      "const MATHJAX_RUST_FACTS_SCHEMA",
      "const pathAlias = path; pathAlias.resolve = replacement;\nconst MATHJAX_RUST_FACTS_SCHEMA",
    ],
    [
      "direct path mutator",
      "const MATHJAX_RUST_FACTS_SCHEMA",
      "path.resolve = replacement;\nconst MATHJAX_RUST_FACTS_SCHEMA",
    ],
    ["wrong filesystem import", 'from "node:fs"', 'from "node:path"'],
    [
      "filesystem member alias",
      "const repositoryRoot = fs.realpathSync(root)",
      "const canonicalize = fs.realpathSync; const repositoryRoot = canonicalize(root)",
    ],
    ["early return", "  const child =", "  return {} as MathJaxRustStaticFacts;\n  const child ="],
    ["early throw", "  const child =", '  throw new Error("dead spawn");\n  const child ='],
    ["nested dead spawn", "const child = Bun.spawnSync(", "const child = false && Bun.spawnSync("],
    [
      "unvalidated output",
      "return parseMathJaxRustFactsOutput(repositoryRoot, stdout)",
      "return JSON.parse(stdout)",
    ],
    [
      "input checksum skipped",
      "readSource(root, name).sha256 !== facts.inputSha256[name]",
      "false",
    ],
    ["range guard skipped", "if (end <= start)", "if (false)"],
    ["input path inventory truncated", '  "crates/katana-render-runtime/src/lib.rs",', ""],
    ["wrong physical source helper", '"./runtime-unused-origin"', '"./unreviewed-origin"'],
    [
      "global filesystem replacement",
      "const MATHJAX_RUST_FACTS_SCHEMA",
      "fs.realpathSync = replacement;\nconst MATHJAX_RUST_FACTS_SCHEMA",
    ],
  ])("MathJax facts route rejects %s", (_label, before, after) => {
    expect(mathjaxFactsSource).toContain(before);
    expect(mathjaxFactsReferences()).not.toContain("!unresolved-executable");
    const changed = mathjaxFactsSource.replace(before, after);
    expect(changed).not.toBe(mathjaxFactsSource);
    expect(mathjaxFactsReferences(changed)).toContain("!unresolved-executable");
  });
  test.each([
    [
      "undefined guarded build",
      'CARGO_BUILD := "python3 " + quote(REPO_ROOT + "/scripts/build/cargo_capacity_guard.py") + " --retain-build-profile debug -- " + CARGO',
      "",
    ],
    ["unguarded executable", "{{CARGO_BUILD}} build --locked", "{{CARGO}} build --locked"],
    [
      "wrong root header",
      "mathjax-consumer-facts root=REPO_ROOT:",
      "mathjax-consumer-facts root=REPO_ROOT source_root=REPO_ROOT:",
    ],
    ["unquoted runtime root", "mathjax {{quote(root)}}", "mathjax {{root}}"],
  ])("MathJax facts recipe rejects %s", (_label, before, after) => {
    expect(classicJustfile).toContain(before);
    expect(mathjaxFactsReferences()).not.toContain("!unresolved-executable");
    const changed = classicJustfile.replaceAll(before, after);
    expect(guardedFactsRecipeVerified(changed, "mathjax-consumer-facts")).toBe(false);
    expect(mathjaxFactsReferences(mathjaxFactsSource, changed)).toContain("!unresolved-executable");
  });

  const nativeWidthFactsCaller = "scripts/runtime-bundles/runtime-unused-native-width-hash.ts";
  const nativeWidthFactsSource = fs.readFileSync(
    path.join(import.meta.dir, "../runtime-bundles/runtime-unused-native-width-hash.ts"),
    "utf8",
  );
  const nativeWidthFactsRead = (name: string) => {
    if (name === nativeWidthFactsCaller) return nativeWidthFactsSource;
    if (name === "Justfile") return classicJustfile;
    return undefined;
  };

  test("recognizes only the reviewed NativeWidth facts caller process route", () => {
    expect(
      sourceCode(nativeWidthFactsSource, {
        name: nativeWidthFactsCaller,
        read: nativeWidthFactsRead,
      }),
    ).not.toContain("!unresolved-executable");
    expect(
      sourceCode(nativeWidthFactsSource, {
        name: "scripts/runtime-bundles/unreviewed-facts.ts",
        read: nativeWidthFactsRead,
      }),
    ).toContain("!unresolved-executable");
  });

  const nativeWidthFactsCallMutations = [
    ["wrong executable", '"just"', '"bun"'],
    ["wrong recipe", '"native-width-hash-facts"', '"mathjax-consumer-facts"'],
    [
      "wrong root expression",
      'path.resolve(import.meta.dir, "../..")',
      'path.resolve(import.meta.dir, "..")',
    ],
    ["wrong realpath input", "fs.realpathSync(sourceRoot)", "fs.realpathSync(repositoryRoot)"],
    [
      "reordered data arguments",
      "repositoryRoot, actualSourceRoot]",
      "actualSourceRoot, repositoryRoot]",
    ],
    ["wrong cwd", "cwd: repositoryRoot", "cwd: actualSourceRoot"],
    [
      "extra argv",
      '"native-width-hash-facts", repositoryRoot, actualSourceRoot]',
      '"native-width-hash-facts", repositoryRoot, actualSourceRoot, "extra"]',
    ],
    [
      "shadowed Bun",
      "inspectNativeWidthHashFacts(\n  sourceRoot: string,",
      "inspectNativeWidthHashFacts(\n  Bun: unknown,\n  sourceRoot: string,",
    ],
    [
      "shadowed path import",
      "inspectNativeWidthHashFacts(\n  sourceRoot: string,",
      "inspectNativeWidthHashFacts(\n  path: unknown,\n  sourceRoot: string,",
    ],
    [
      "shadowed fs import",
      "inspectNativeWidthHashFacts(\n  sourceRoot: string,",
      "inspectNativeWidthHashFacts(\n  fs: unknown,\n  sourceRoot: string,",
    ],
    [
      "reassigned path import",
      "const repositoryRoot = path.resolve(",
      "path = {} as typeof path;\n  const repositoryRoot = path.resolve(",
    ],
    [
      "reassigned fs import",
      "const actualSourceRoot = fs.realpathSync(",
      "fs = {} as typeof fs;\n  const actualSourceRoot = fs.realpathSync(",
    ],
    [
      "reassigned repository root",
      "const child = Bun.spawnSync(",
      "repositoryRoot = sourceRoot;\n  const child = Bun.spawnSync(",
    ],
    [
      "reassigned actual source root",
      "const child = Bun.spawnSync(",
      "actualSourceRoot = repositoryRoot;\n  const child = Bun.spawnSync(",
    ],
    [
      "reassigned source parameter",
      "const child = Bun.spawnSync(",
      "sourceRoot = repositoryRoot;\n  const child = Bun.spawnSync(",
    ],
    ["duplicate call", "\n  const stdout =", "\n  Bun.spawnSync([unbound]);\n  const stdout ="],
  ] as const;

  test.each(nativeWidthFactsCallMutations)(
    "NativeWidth facts route rejects %s",
    (_name, before, after) => {
      expect(nativeWidthFactsSource).toContain(before);
      const changed = nativeWidthFactsSource.replace(before, after);
      expect(changed).not.toBe(nativeWidthFactsSource);
      expect(
        sourceCode(changed, { name: nativeWidthFactsCaller, read: nativeWidthFactsRead }),
      ).toContain("!unresolved-executable");
    },
  );

  const nativeWidthPhysicalMutations = [
    [
      "preceding unconditional return",
      "  const child =",
      "  return {} as NativeWidthHashFacts;\n  const child =",
    ],
    [
      "preceding unconditional throw",
      "  const child =",
      '  throw new Error("dead spawn");\n  const child =',
    ],
    [
      "preceding block return",
      "  const child =",
      "  { return {} as NativeWidthHashFacts; }\n  const child =",
    ],
    [
      "preceding conditional terminal",
      "  const child =",
      "  if (true) return {} as NativeWidthHashFacts;\n  const child =",
    ],
    ["preceding nonterminating loop", "  const child =", "  while (true) {}\n  const child ="],
    [
      "extra prefix initializer",
      "  const child =",
      '  const stopped = (() => { throw new Error("dead spawn"); })();\n  const child =',
    ],
    [
      "extra root declaration initializer",
      "fs.realpathSync(sourceRoot);",
      'fs.realpathSync(sourceRoot), stopped = (() => { throw new Error("dead spawn"); })();',
    ],
    [
      "path Object.assign",
      "  const repositoryRoot =",
      "  Object.assign(path, { resolve: () => sourceRoot });\n  const repositoryRoot =",
    ],
    [
      "filesystem Object.assign",
      "  const repositoryRoot =",
      "  Object.assign(fs, { realpathSync: () => sourceRoot });\n  const repositoryRoot =",
    ],
    [
      "Bun Object.assign",
      "  const repositoryRoot =",
      "  Object.assign(Bun, { spawnSync: () => ({ exitCode: 0, stdout: new Uint8Array(), stderr: new Uint8Array() }) });\n  const repositoryRoot =",
    ],
    [
      "path defineProperty",
      "  const repositoryRoot =",
      "  Object.defineProperty(path, 'resolve', { value: () => sourceRoot });\n  const repositoryRoot =",
    ],
    [
      "path Reflect.set",
      "  const repositoryRoot =",
      "  Reflect.set(path, 'resolve', () => sourceRoot);\n  const repositoryRoot =",
    ],
    [
      "unknown path mutator",
      "  const repositoryRoot =",
      "  mutateUnknown(path);\n  const repositoryRoot =",
    ],
    [
      "path alias write",
      "  const repositoryRoot =",
      "  const alias = path; alias.resolve = () => sourceRoot;\n  const repositoryRoot =",
    ],
    [
      "computed path member",
      'path.resolve(import.meta.dir, "../..")',
      'path["resolve"](import.meta.dir, "../..")',
    ],
    ["computed filesystem member", "fs.realpathSync(sourceRoot)", 'fs["realpathSync"](sourceRoot)'],
    ["computed Bun member", "Bun.spawnSync(", 'Bun["spawnSync"]('],
    [
      "Bun method reassignment",
      "  const repositoryRoot =",
      "  Bun.spawnSync = replacement;\n  const repositoryRoot =",
    ],
    [
      "global Bun reassignment",
      "  const repositoryRoot =",
      "  globalThis.Bun = replacement;\n  const repositoryRoot =",
    ],
    [
      "nested executing spawn",
      '  const child = Bun.spawnSync(\n    ["just", "native-width-hash-facts", repositoryRoot, actualSourceRoot],\n    { cwd: repositoryRoot, timeout: 120_000 },\n  );',
      '  const child = (() => Bun.spawnSync(["just", "native-width-hash-facts", repositoryRoot, actualSourceRoot], { cwd: repositoryRoot, timeout: 120_000 }))();',
    ],
    [
      "dead nested spawn",
      '  const child = Bun.spawnSync(\n    ["just", "native-width-hash-facts", repositoryRoot, actualSourceRoot],\n    { cwd: repositoryRoot, timeout: 120_000 },\n  );',
      '  const child = (() => { if (false) return Bun.spawnSync(["just", "native-width-hash-facts", repositoryRoot, actualSourceRoot], { cwd: repositoryRoot, timeout: 120_000 }); return {} as ReturnType<typeof Bun.spawnSync>; })();',
    ],
    [
      "source parameter for-of write",
      "  const child =",
      "  for (sourceRoot of [repositoryRoot]) {}\n  const child =",
    ],
    ["duplicate spawn call", "  const stdout =", "  Bun.spawnSync([unknown]);\n  const stdout ="],
    [
      "reordered spawn arguments",
      "repositoryRoot, actualSourceRoot]",
      "actualSourceRoot, repositoryRoot]",
    ],
    ["overridden cwd", "cwd: repositoryRoot", "cwd: actualSourceRoot"],
  ] as const;

  test.each(nativeWidthPhysicalMutations)(
    "NativeWidth AST route rejects physical mutation: %s",
    (_name, before, after) => {
      expect(
        sourceCode(nativeWidthFactsSource, {
          name: nativeWidthFactsCaller,
          read: nativeWidthFactsRead,
        }),
      ).not.toContain("!unresolved-executable");
      expect(nativeWidthFactsSource).toContain(before);
      const changed = nativeWidthFactsSource.replace(before, after);
      expect(changed).not.toBe(nativeWidthFactsSource);
      expect(
        sourceCode(changed, { name: nativeWidthFactsCaller, read: nativeWidthFactsRead }),
      ).toContain("!unresolved-executable");
    },
  );

  test("requires normal source registration for verifier and collector dependencies", () => {
    const snapshot = processRouteFixture();
    expect(checkHarnessContract(snapshot)).toEqual([]);
    for (const name of [
      "scripts/mermaid/native-render-provenance.ts",
      "scripts/mermaid/native-build-receipt.ts",
      "scripts/mermaid/browser-process-observe.py",
    ]) {
      expect(snapshot.graph.local.has(name)).toBe(true);
      expect(snapshot.graph.ci.has(name)).toBe(true);
      snapshot.tracked.delete(name);
      expect(checkHarnessContract(snapshot)).toContain(`untracked code: ${name}`);
      snapshot.tracked.add(name);
    }
    snapshot.biomeIncludes = ["scripts/html/**/*.ts", "scripts/quality/**/*.ts"];
    expect(checkHarnessContract(snapshot)).toContain(
      "lint scope missing: scripts/mermaid/native-render-provenance.ts",
    );
  });

  test("accepts multiple fixture calls when every call uses the same input/build binding", () => {
    const snapshot = processRouteFixture();
    const call = "for (const fixture of fixtures) await renderFixture(fixture, input, build);";
    replaceSources(snapshot, {
      "scripts/mermaid/native-render-provenance.ts": nativeRouteProducer.replace(
        call,
        `${call} ${call}`,
      ),
    });
    expect(checkHarnessContract(snapshot)).toEqual([]);
  });

  test("accepts the verified batch result rows projection", () => {
    const snapshot = processRouteFixture();
    replaceSources(snapshot, {
      "scripts/mermaid/native-render-provenance.ts": nativeRouteProducer.replace(
        "const rows = await renderFixtures([], input, prepared.build);",
        "const renderedResult = await renderFixtures([], input, prepared.build); const rows = renderedResult.rows;",
      ),
    });
    expect(checkHarnessContract(snapshot)).toEqual([]);
  });

  test.each([
    ["verified result rows", "const rows = renderedResult.rows;", true],
    ["unrelated result rows", "const rows = unrelatedResult.rows;", false],
    ["unverified result rows", "const rows = forged;", false],
  ])("validates native batch result binding: %s", (_label, extraction, accepted) => {
    const snapshot = processRouteFixture();
    replaceSources(snapshot, {
      "scripts/mermaid/native-render-provenance.ts": nativeRouteProducer.replace(
        "const rows = await renderFixtures([], input, prepared.build);",
        `const renderedResult = await renderFixtures([], input, prepared.build); ${extraction}`,
      ),
    });
    const issues = checkHarnessContract(snapshot);
    if (accepted) expect(issues).toEqual([]);
    else expect(issues).toContain("unresolved/external code: !unresolved-executable");
  });

  test.each([
    [
      "before guard missing",
      "const build = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime);",
      "const build = forged;",
    ],
    [
      "guard in false branch",
      "const build = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime);",
      "let build; if (false) { build = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime); }",
    ],
    [
      "guard in unrelated callback",
      "const build = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime);",
      "const build = callback(() => verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime));",
    ],
    [
      "additional wrapper call",
      "return child;",
      "spawnNativeCli(otherBinary, [], 'stdout', 'stderr'); return child;",
    ],
    [
      "additional fixture with unverified binary",
      "for (const fixture of fixtures) await renderFixture(fixture, input, build);",
      "for (const fixture of fixtures) { await renderFixture(fixture, input, build); await renderFixture(fixture, { ...input, binary: unverified }, build); }",
    ],
    [
      "additional fixture batch with unverified binary",
      "return { rows, afterBuild };",
      "await renderFixtures([], { ...input, binary: unverified }, prepared.build); return { rows, afterBuild };",
    ],
    [
      "aliased fixture batch",
      "return { rows, afterBuild };",
      "const other = renderFixtures; await other([], { ...input, binary: unverified }, prepared.build); return { rows, afterBuild };",
    ],
    [
      "additional fixture with unrelated build",
      "for (const fixture of fixtures) await renderFixture(fixture, input, build);",
      "for (const fixture of fixtures) { await renderFixture(fixture, input, build); await renderFixture(fixture, input, unrelatedBuild); }",
    ],
    [
      "additional fixture in another function",
      "function prepareRender(input: Options)",
      "function unverifiedRoute(row: Fixture, input: Options, build: Receipt) { return renderFixture(row, { ...input, binary: unverified }, build); } function prepareRender(input: Options)",
    ],
    [
      "aliased fixture call",
      "for (const fixture of fixtures) await renderFixture(fixture, input, build);",
      "for (const fixture of fixtures) { await renderFixture(fixture, input, build); const other = renderFixture; await other(fixture, { ...input, binary: unverified }, build); }",
    ],
    [
      "spread fixture call",
      "for (const fixture of fixtures) await renderFixture(fixture, input, build);",
      "for (const fixture of fixtures) { await renderFixture(fixture, input, build); await renderFixture(...unverifiedArguments); }",
    ],
    [
      "no fixture call",
      "for (const fixture of fixtures) await renderFixture(fixture, input, build);",
      "for (const fixture of fixtures) unverifiedRoute(fixture);",
    ],
    [
      "aliased wrapper",
      "return child;",
      "const rogue = spawnNativeCli; rogue(otherBinary, [], 'stdout', 'stderr'); return child;",
    ],
    [
      "guard unreachable",
      "const build = verifyNativeBuildReceipt",
      "return forged; const build = verifyNativeBuildReceipt",
    ],
    [
      "wrong binary guard",
      "input.buildReceipt, input.binary, input.runtime",
      "input.buildReceipt, otherBinary, input.runtime",
    ],
    [
      "after guard missing",
      "const afterBuild = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime);",
      "const afterBuild = forged;",
    ],
    [
      "wrong producer input",
      "renderFixtures([], input, prepared.build)",
      "renderFixtures([], otherInput, prepared.build)",
    ],
    ["unverified wrapper input", "spawnNativeCli(input.binary", "spawnNativeCli(otherBinary"],
    [
      "shadowed verifier",
      "function prepareRender(input: Options)",
      "function prepareRender(input: Options, verifyNativeBuildReceipt: Unknown)",
    ],
    [
      "mutated binary",
      "const child = await spawnNativeCli",
      "input.binary = replacement; const child = await spawnNativeCli",
    ],
  ])("rejects native artifact route %s", (_label, before, after) => {
    const snapshot = processRouteFixture();
    replaceSources(snapshot, {
      "scripts/mermaid/native-render-provenance.ts": nativeRouteProducer.replace(before, after),
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "unresolved/external code: !unresolved-executable",
    );
  });

  test.each([
    [
      "guard absent",
      "requireApprovedBrowserObservationRoute(request.observerPython);",
      "unguarded(request.observerPython);",
    ],
    [
      "tool identity absent",
      "physicalFileIdentity(observerPython);",
      "unverified(observerPython);",
    ],
    [
      "collector identity absent",
      "physicalFileIdentity(collectorPath());",
      "unverified(collectorPath());",
    ],
    ["collector replaced", "browser-process-observe.py", "different-collector.py"],
    [
      "identity in false branch",
      "physicalFileIdentity(observerPython);",
      "if (false) physicalFileIdentity(observerPython);",
    ],
    [
      "identity in unrelated callback",
      "physicalFileIdentity(observerPython);",
      "callback(() => physicalFileIdentity(observerPython));",
    ],
    [
      "guard returns before identity",
      "physicalFileIdentity(observerPython);",
      "return; physicalFileIdentity(observerPython);",
    ],
    [
      "spread collector is not script argv",
      '[collectorPath(), "1", "2"]',
      '[...collectorPath(), "1", "2"]',
    ],
    [
      "spread interpreter is not a fixed tool",
      "spawnSync(request.observerPython",
      "spawnSync(...request.observerPython",
    ],
    [
      "shadowed path",
      "export function collectorPath()",
      "export function collectorPath(path: Unknown)",
    ],
    [
      "shadowed identity",
      "function requireApprovedBrowserObservationRoute(observerPython: string)",
      "function requireApprovedBrowserObservationRoute(observerPython: string, physicalFileIdentity: Unknown)",
    ],
    ["unknown executable", "spawnSync(request.observerPython", "spawnSync(unknownBinary"],
    ["unknown interpreter argv", '[collectorPath(), "1", "2"]', "unknownArguments"],
    [
      "mutated request",
      "const result = spawnSync",
      "request.observerPython = replacement; const result = spawnSync",
    ],
  ])("rejects Python collector route %s", (_label, before, after) => {
    const snapshot = processRouteFixture();
    replaceSources(snapshot, {
      "scripts/mermaid/raster-browser-observation.ts": observerRouteSource.replace(before, after),
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "unresolved/external code: !unresolved-executable",
    );
  });

  test("retains Python static and local/CI contract failures", () => {
    const snapshot = processRouteFixture();
    replaceSources(snapshot, {
      Justfile: (snapshot.read("Justfile") ?? "").replace("mypy --strict scripts", "mypy scripts"),
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "Python Ruff + strict type check registration unavailable: scripts/mermaid/browser-process-observe.py",
    );
    snapshot.graph.local.delete("scripts/mermaid/browser-process-observe.py");
    snapshot.graph.ci.delete("scripts/mermaid/browser-process-observe.py");
    snapshot.codeFiles.push("scripts/mermaid/browser-process-observe.py");
    expect(checkHarnessContract(snapshot)).toContain(
      "local registration missing: scripts/mermaid/browser-process-observe.py",
    );
    expect(checkHarnessContract(snapshot)).toContain(
      "CI registration missing: scripts/mermaid/browser-process-observe.py",
    );
  });

  test("does not grant a domain route to identical code at an arbitrary module path", () => {
    expect(
      sourceCode(nativeRouteProcess, {
        name: "scripts/html/native-cli-process.ts",
        read: () => nativeRouteProducer,
      }),
    ).toContain("!unresolved-executable");
    expect(
      sourceCode(observerRouteSource, {
        name: "scripts/html/raster-browser-observation.ts",
        read: () => observerRouteSource,
      }),
    ).toContain("!unresolved-executable");
  });
  test("rejects an unverified caller even after a verified producer imported the wrapper", () => {
    const snapshot = processRouteFixture();
    replaceSources(snapshot, {
      "scripts/html/main.ts": 'import "../mermaid/native-render-provenance"; import "./rogue";',
      "scripts/html/rogue.ts":
        'import { spawnNativeCli } from "../mermaid/native-cli-process"; spawnNativeCli(binary, args, stdout, stderr);',
    });
    snapshot.tracked.add("scripts/html/rogue.ts");
    expect(checkHarnessContract(snapshot)).toContain(
      "unresolved/external code: !unresolved-executable",
    );
  });
});

function splitCiFixture(): HarnessSnapshot {
  const snapshot = moonFixture();
  if (!snapshot.policy.moon) throw new Error("fixture moon policy missing");
  Object.assign(snapshot.policy.moon, {
    ciJob: "static-quality",
    ciEntry: "source-static",
    downstreamJob: "test",
    ciRequiredStatic: [
      "biome",
      "typecheck",
      "python-lint",
      "python-typecheck",
      "duplication-check",
      "owned-validation-contract-check",
    ],
    ciBeforeHeavy: ["lint"],
  });
  replaceSources(snapshot, {
    [snapshot.policy.localBridge.source]:
      `preflight = ["bun", "scripts/html/harness-contract-cli.ts"]\nif preflight.returncode != 0:\n    return preflight.returncode\n${snapshot.read(snapshot.policy.localBridge.source)}`,
  });
  changeMoon(snapshot, (tasks) => {
    tasks["source-static"] = {
      command: "noop",
      deps: [
        "biome",
        "typecheck",
        "python-lint",
        "python-typecheck",
        "duplication-check",
        "owned-validation-contract-check",
      ],
    };
    if (tasks.lint) tasks.lint.deps = ["source-static"];
  });
  replaceSources(snapshot, {
    [snapshot.policy.ciWorkflow]: JSON.stringify({
      jobs: {
        "static-quality": {
          steps: [
            { run: "bun scripts/html/harness-contract-cli.ts" },
            {
              run: "bun run moon --concurrency 3 exec krr:source-static --on-failure continue --no-actions",
            },
          ],
        },
        test: { needs: "static-quality", steps: [{ run: "just lint" }, { run: "just html-test" }] },
      },
    }),
  });
  return snapshot;
}

function changeCi(
  snapshot: HarnessSnapshot,
  edit: (
    jobs: Record<
      string,
      {
        needs?: string;
        if?: string;
        steps: { run: string; if?: string; "continue-on-error"?: boolean }[];
      }
    >,
  ) => void,
): void {
  const workflow = JSON.parse(snapshot.read(snapshot.policy.ciWorkflow) ?? "") as {
    jobs: Parameters<typeof edit>[0];
  };
  edit(workflow.jobs);
  replaceSources(snapshot, { [snapshot.policy.ciWorkflow]: JSON.stringify(workflow) });
}

describe("HTML validation code ownership contract", () => {
  test("accepts registered tracked core and generated TMP output", () => {
    expect(checkHarnessContract(fixture())).toEqual([]);
    expect(
      declaredValidationCode({
        runnerPath: "scripts/html/main.ts",
        outputPath: "tmp/generated-output.ts",
        fixturePath: "tmp/private.html",
      }),
    ).toEqual(["scripts/html/main.ts"]);
  });
  test("rejects the actual TMP runner boundary instead of only checking registered core", () => {
    const snapshot = fixture();
    snapshot.declaredCode = declaredValidationCode({ runnerPath: "/workspace/tmp/runner.py" });
    expect(checkHarnessContract(snapshot)).toContain("misplaced code: /workspace/tmp/runner.py");
    expect(checkHarnessContract(snapshot)).toContain(
      "Python Ruff + strict type check registration unavailable: tmp/runner.py",
    );
  });
  test("rejects code omitted from Git", () => {
    const snapshot = fixture();
    snapshot.tracked.delete("scripts/html/helper.ts");
    expect(checkHarnessContract(snapshot)).toContain("untracked code: scripts/html/helper.ts");
  });
  test("rejects lint and type scope exclusion", () => {
    const snapshot = fixture();
    snapshot.biomeIncludes.push("!scripts/html/helper.ts");
    snapshot.typeExcludes.push("scripts/html/helper.ts");
    expect(checkHarnessContract(snapshot)).toContain("lint scope missing: scripts/html/helper.ts");
    expect(checkHarnessContract(snapshot)).toContain("type scope missing: scripts/html/helper.ts");
  });
  test.each([{ patterns: [] }, { patterns: ["crates/old-renderer/**/*.ts"] }])(
    "rejects absent or stale root static scopes: %j",
    ({ patterns }) => {
      const snapshot = fixture();
      snapshot.biomeIncludes = [...patterns];
      snapshot.typeIncludes = [...patterns];
      const errors = checkHarnessContract(snapshot);
      expect(errors).toContain("lint scope missing: scripts/html/main.ts");
      expect(errors).toContain("type scope missing: scripts/html/main.ts");
    },
  );
  test("accepts generated JSON imports as data rather than executable core", () => {
    const snapshot = fixture();
    replaceSources(snapshot, {
      "scripts/html/main.ts": 'import data from "../../tmp/generated.json"; import "./helper";',
    });
    expect(checkHarnessContract(snapshot)).toEqual([]);
  });
  test("rejects a detached local check bridge", () => {
    const snapshot = fixture();
    replaceSources(snapshot, { [snapshot.policy.localBridge.source]: "" });
    expect(checkHarnessContract(snapshot)).toContain("local bridge missing check-assets");
  });
  test("rejects an untracked policy authority", () => {
    const snapshot = fixture();
    snapshot.authorityFiles = ["scripts/html/harness-contract-policy.json"];
    expect(checkHarnessContract(snapshot)).toContain(
      "untracked contract authority: scripts/html/harness-contract-policy.json",
    );
  });
  test("rejects orphan code absent from both standard execution graphs", () => {
    const snapshot = fixture();
    snapshot.codeFiles.push("scripts/html/orphan.ts");
    snapshot.tracked.add("scripts/html/orphan.ts");
    expect(checkHarnessContract(snapshot)).toContain(
      "local registration missing: scripts/html/orphan.ts",
    );
    expect(checkHarnessContract(snapshot)).toContain(
      "CI registration missing: scripts/html/orphan.ts",
    );
  });
  test("rejects a TMP import and an executable TMP child, while ignoring comments/output", () => {
    const snapshot = fixture();
    replaceSources(snapshot, {
      "scripts/html/main.ts":
        'import "../../tmp/core.ts"; Bun.spawnSync(["python3", "tmp/helper.py"]); // import "tmp/comment.ts"',
    });
    const errors = checkHarnessContract(snapshot);
    expect(errors).toContain("misplaced code: tmp/core.ts");
    expect(errors).toContain("misplaced code: tmp/helper.py");
    expect(errors.some((error) => error.includes("comment.ts"))).toBe(false);
  });
  test("rejects unknown dynamic imports and external repository paths", () => {
    expect(sourceCode("import(variablePath)")).toContain("!unresolved-import");
    const snapshot = fixture();
    snapshot.declaredCode.push("/external/core.ts");
    expect(checkHarnessContract(snapshot)).toContain("misplaced code: /external/core.ts");
  });
  test.each([
    ["import(variablePath)", "unresolved/external code: !unresolved-import"],
    ['import "unregistered-package"', "unknown code dependency: unregistered-package"],
  ])("rejects unresolved executable import through the checker: %s", (source, expected) => {
    const snapshot = fixture();
    replaceSources(snapshot, { "scripts/html/main.ts": source });
    expect(checkHarnessContract(snapshot)).toContain(expected);
  });
  test("rejects missing local bridge and missing CI registration", () => {
    const snapshot = fixture();
    replaceSources(snapshot, { [snapshot.policy.ciWorkflow]: "run: just unknown" });
    expect(checkHarnessContract(snapshot)).toContain(
      "CI registration missing: scripts/html/main.ts",
    );
  });
  test("directory test registration derives files, not a fixed passing file allowlist", () => {
    const snapshot = fixture();
    snapshot.codeFiles.push("scripts/html/extra_test.ts");
    snapshot.tracked.add("scripts/html/extra_test.ts");
    snapshot.policy.localRecipes = ["directory-tests"];
    replaceSources(snapshot, {
      Justfile: `${snapshot.read("Justfile")}directory-tests arg=default:\n    bun test scripts/html\n`,
      [snapshot.policy.ciWorkflow]: 'run: just VERSION="1" directory-tests',
      "scripts/html/extra_test.ts": 'import "./main"',
      [snapshot.policy.localBridge.source]: 'Lane("assets", "directory-tests")',
    });
    expect(checkHarnessContract(snapshot)).toEqual([]);
  });
  test("an executable JSON-named child is code, unlike a JSON data import", () => {
    const snapshot = fixture();
    replaceSources(snapshot, {
      "scripts/html/main.ts": 'Bun.spawnSync(["python3", "tmp/runner.json"]);',
    });
    expect(checkHarnessContract(snapshot)).toContain("misplaced code: tmp/runner.json");
  });
  test("tracks a literal Just command wrapper through its Python script", () => {
    const source =
      'Bun.spawnSync(["just", "--command", "python3", "scripts/release/native_coverage_result_cli.py", "--repo", root, "local-validate", "--producer", "root"]);';
    const references = sourceCode(source);
    expect(references).toContain("!execute:scripts/release/native_coverage_result_cli.py");
    expect(references).not.toContain("!unresolved-executable");
  });
  test.each([
    ["missing script", 'Bun.spawnSync(["just", "--command", "python3"]);'],
    [
      "dynamic script path",
      'Bun.spawnSync(["just", "--command", "python3", path.join(root, script)]);',
    ],
    ["unsupported interpreter", 'Bun.spawnSync(["just", "--command", "ruby", "runner.py"]);'],
    ["spread payload", 'Bun.spawnSync(["just", "--command", ...command]);'],
  ])("rejects an unresolved Just child command: %s", (_name, source) => {
    expect(sourceCode(source)).toContain("!unresolved-executable");
  });
  test("rejects unmanaged declarations without reading their private contents", () => {
    const snapshot = fixture();
    snapshot.declaredCode = ["tmp/private-core.ts"];
    const prior = snapshot.read;
    snapshot.read = (name) => {
      if (name.startsWith("tmp/")) throw new Error("private content must not be read");
      return prior(name);
    };
    expect(checkHarnessContract(snapshot)).toContain("misplaced code: tmp/private-core.ts");
  });
  test.each(['import path from "node:path";', 'import * as path from "node:path";'])(
    "resolves fixed node:path join child with importer ownership: %s",
    (binding) => {
      const snapshot = fixture();
      replaceSources(snapshot, {
        "scripts/html/main.ts": `${binding} import "./helper"; Bun.spawnSync(["bun", path.join(import.meta.dir, "child.ts"), ...args]);`,
        "scripts/html/child.ts": 'import "./helper";',
      });
      snapshot.tracked.add("scripts/html/child.ts");
      expect(snapshot.graph.local.has("scripts/html/child.ts")).toBe(true);
      expect(snapshot.graph.ci.has("scripts/html/child.ts")).toBe(true);
      expect(checkHarnessContract(snapshot)).toEqual([]);
    },
  );
  test.each([
    'Bun.spawnSync(["bun", child]);',
    'Bun.spawnSync(["bun", "run", child]);',
    'Bun.spawnSync(["node", "-e", child]);',
    'import { execFile } from "node:child_process"; execFile("bun", [child]);',
    'import { execSync } from "node:child_process"; execSync(child);',
    'const path = fake; Bun.spawnSync(["bun", path.join(import.meta.dir, "helper.ts")]);',
    'import path from "node:path"; function run(path: unknown) { Bun.spawnSync(["bun", path.join(import.meta.dir, "helper.ts")]); }',
  ])("fails closed for unresolved code-child or false/shadowed path binding: %s", (source) => {
    const snapshot = fixture();
    replaceSources(snapshot, { "scripts/html/main.ts": `import "./helper"; ${source}` });
    expect(checkHarnessContract(snapshot)).toContain(
      "unresolved/external code: !unresolved-executable",
    );
  });
  test("ignores dynamic magick data arguments while rejecting importer-relative TMP/JSON/external executable locators", () => {
    const snapshot = fixture();
    replaceSources(snapshot, {
      "scripts/html/main.ts":
        'import "./helper"; Bun.spawnSync(["magick", "compare", input, output]);',
    });
    expect(checkHarnessContract(snapshot)).toEqual([]);
    replaceSources(snapshot, {
      "scripts/html/main.ts":
        'import path from "node:path"; import "./helper"; Bun.spawnSync(["bun", path.join(import.meta.dir, "../../tmp/child.json")]);',
    });
    expect(checkHarnessContract(snapshot)).toContain("misplaced code: tmp/child.json");
    replaceSources(snapshot, {
      "scripts/html/main.ts":
        'import path from "node:path"; import "./helper"; Bun.spawnSync(["bun", path.join(import.meta.dir, "../../../external/child.ts")]);',
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "unresolved/external code: ../../../external/child.ts",
    );
  });
});

describe("managed module resolution and process signatures", () => {
  test("uses Bun package imports resolution and rejects unknown/unmanaged targets", () => {
    withModulePackage((root) => {
      expect(
        resolveRepositoryCode(root, ["scripts/"], "#owned/helper", "scripts/html/main.ts"),
      ).toBe("scripts/html/helper.ts");
      for (const reference of ["#unknown", "#unmanaged"])
        expect(() =>
          resolveRepositoryCode(root, ["scripts/"], reference, "scripts/html/main.ts"),
        ).toThrow();
    });
  });
  test("rejects an alias whose realpath escapes the repository", () => {
    withModulePackage((root) => {
      const outside = fs.mkdtempSync(path.join(os.tmpdir(), "external-module-resolution-"));
      try {
        fs.writeFileSync(path.join(outside, "helper.ts"), "export const external = true;");
        fs.renameSync(path.join(root, "scripts/html"), path.join(root, "scripts/original-html"));
        fs.symlinkSync(outside, path.join(root, "scripts/html"), "junction");
        expect(() =>
          resolveRepositoryCode(
            root,
            ["scripts/"],
            "#owned/helper",
            "scripts/original-html/main.ts",
          ),
        ).toThrow("external code resolution");
      } finally {
        fs.rmSync(outside, { recursive: true, force: true });
      }
    });
  });
  test("resolves package imports to ordinary managed source with its importer", () => {
    const snapshot = fixture();
    replaceSources(snapshot, { "scripts/html/main.ts": 'import "#owned/helper";' });
    const resolutions: Array<[string, string | undefined]> = [];
    snapshot.graph = executionGraph(
      snapshot.policy,
      snapshot.read,
      {},
      new Set(),
      snapshot.codeFiles,
      (name, importer) => {
        resolutions.push([name, importer]);
        return "scripts/html/helper.ts";
      },
    );
    expect(resolutions).toEqual([
      ["#owned/helper", "scripts/html/main.ts"],
      ["#owned/helper", "scripts/html/main.ts"],
    ]);
    expect(checkHarnessContract(snapshot)).toEqual([]);
    snapshot.tracked.delete("scripts/html/helper.ts");
    expect(checkHarnessContract(snapshot)).toContain("untracked code: scripts/html/helper.ts");
  });

  test.each([
    'import { spawnSync } from "node:child_process"; spawnSync("bun", [path.join(import.meta.dir, "child.ts")]);',
    'import { spawn as run } from "node:child_process"; run("node", [path.join(import.meta.dir, "child.ts")]);',
    'import * as child from "node:child_process"; child.spawn("bun", [path.join(import.meta.dir, "child.ts")]);',
    'import { spawnSync } from "node:child_process"; spawnSync(process.execPath, [path.join(import.meta.dir, "child.ts")]);',
    'import { spawn as run } from "bun"; run(["bun", path.join(import.meta.dir, "child.ts")]);',
  ])("follows fixed child source using the actual Node or Bun API signature: %s", (invocation) => {
    const snapshot = fixture();
    replaceSources(snapshot, {
      "scripts/html/main.ts": `import path from "node:path"; import "./helper"; ${invocation}`,
      "scripts/html/child.ts": 'import "./helper";',
    });
    snapshot.tracked.add("scripts/html/child.ts");
    expect(snapshot.graph.local.has("scripts/html/child.ts")).toBe(true);
    expect(snapshot.graph.ci.has("scripts/html/child.ts")).toBe(true);
    expect(checkHarnessContract(snapshot)).toEqual([]);
  });

  test.each([
    'import { spawn } from "node:child_process"; spawn(binary, args);',
    'import { spawnSync } from "node:child_process"; spawnSync("bun", args);',
    'import { spawnSync } from "node:child_process"; function run(process: unknown) { spawnSync(process.execPath, ["scripts/html/helper.ts"]); }',
  ])(
    "keeps unknown executable/interpreter and shadowed process identity closed: %s",
    (invocation) => {
      const snapshot = fixture();
      replaceSources(snapshot, { "scripts/html/main.ts": `import "./helper"; ${invocation}` });
      expect(checkHarnessContract(snapshot)).toContain(
        "unresolved/external code: !unresolved-executable",
      );
    },
  );
});

describe("static Node eval child dependencies", () => {
  const interpolation = "$" + "{dependency}";
  test("inspects the real parser-generation child script and its builtin imports", () => {
    const source = fs.readFileSync(
      path.join(import.meta.dir, "../runtime-bundles/parser-body-onload-generation.test.ts"),
      "utf8",
    );
    const references = sourceCode(source);
    expect(references).toContain("!child-import:node:vm");
    expect(references).toContain("!child-import:node:fs");
    expect(references).not.toContain("!unresolved-executable");
  });

  test("routes an unknown static child import through the ordinary managed dependency check", () => {
    const snapshot = fixture();
    replaceSources(snapshot, {
      "scripts/html/main.ts":
        'import "./helper"; import { spawnSync } from "node:child_process"; const script = `require("unregistered-child")`; spawnSync("node", ["-e", script]);',
    });
    expect(checkHarnessContract(snapshot)).toContain("unknown code dependency: unregistered-child");
  });

  test.each([
    ["mutable let", 'let script = `require("node:vm")`;', "!unresolved-executable"],
    [
      "reassigned const",
      'const script = `require("node:vm")`; script = "changed";',
      "!unresolved-executable",
    ],
    [
      "interpolated template",
      `const dependency = "node:vm"; const script = \`require("${interpolation}")\`;`,
      "!unresolved-executable",
    ],
    ["unknown identifier", "const script = makeScript();", "!unresolved-executable"],
    [
      "shadowed identifier",
      'const script = `require("node:vm")`; function run(script: string) { spawnSync("node", ["-e", script]); }',
      "!unresolved-executable",
    ],
    [
      "duplicate declarator",
      "const script = 'require(\"node:vm\")', script = 'require(\"node:fs\")';",
      "!unresolved-executable",
    ],
    [
      "dynamic child require",
      "const script = `require(dependencyName)`;",
      "!child-import:!unresolved-import",
    ],
  ])("keeps non-static child source unresolved: %s", (_label, declaration, expected) => {
    const references = sourceCode(
      `import { spawnSync } from "node:child_process"; ${declaration} spawnSync("node", ["-e", script]);`,
    );
    expect(references).toContain(expected);
  });

  test("resolves a static StringLiteral payload as child source", () => {
    const references = sourceCode(
      'import { spawnSync } from "node:child_process"; const script = \'require("node:fs")\'; spawnSync("node", ["-e", script]);',
    );
    expect(references).toContain("!child-import:node:fs");
    expect(references).not.toContain("!unresolved-executable");
  });
  test.each([
    ["spread payload", 'spawnSync("node", ["-e", ...script]);', false],
    ["spread flag", 'spawnSync("node", [...["-e"], script]);', false],
    [
      "spread preload tail",
      'spawnSync("node", ["-e", script, ...["--require", "node:fs"]]);',
      false,
    ],
    ["unknown preload tail", 'spawnSync("node", ["-e", script, "--require", getModule()]);', false],
    ["known preload tail", 'spawnSync("node", ["-e", script, "--require", "node:fs"]);', false],
    ["outer argv spread", 'spawnSync("node", ...["-e", script]);', false],
    ["outer options spread", 'spawnSync("node", ["-e", script], ...options);', false],
    ["Bun tool spread", 'Bun.spawnSync([..."node", "-e", script]);', false],
    [
      "direct TDZ",
      'spawnSync("node", ["-e", script]); const script = `require("node:vm")`;',
      false,
    ],
    [
      "closure TDZ",
      'run(); const script = `require("node:vm")`; function run() { spawnSync("node", ["-e", script]); }',
      false,
    ],
    ["outer closure", 'function run() { spawnSync("node", ["-e", script]); } run();', false],
    ["arrow capture", '(() => spawnSync("node", ["-e", script]))();', false],
    [
      "default parameter capture",
      'run(); function run(value = spawnSync("node", ["-e", script])) {}',
      false,
    ],
    ["outer nested block", '{ spawnSync("node", ["-e", script]); }', false],
    [
      "cross switch cases",
      'switch (value) { case 0: const localScript = `require("node:vm")`; break; case 1: spawnSync("node", ["-e", localScript]); }',
      false,
    ],
    [
      "same callback block",
      'test("probe", () => { const localScript = `require("node:vm")`; spawnSync("node", ["-e", localScript]); });',
      true,
    ],
    [
      "same function block",
      'run(); function run() { const localScript = `require("node:vm")`; spawnSync("node", ["-e", localScript]); }',
      true,
    ],
    [
      "same nested block",
      'if (value) { const localScript = `require("node:vm")`; spawnSync("node", ["-e", localScript]); }',
      true,
    ],
    [
      "same switch case",
      'switch (value) { case 0: const localScript = `require("node:vm")`; spawnSync("node", ["-e", localScript]); break; }',
      true,
    ],
    [
      "ordered declarators",
      'const localScript = `require("node:vm")`, result = spawnSync("node", ["-e", localScript]);',
      true,
    ],
  ])(
    "requires exact eval argv and an initialized same execution block: %s",
    (label, code, accepted) => {
      const declaration =
        label === "direct TDZ" || label === "closure TDZ"
          ? ""
          : 'const script = `require("node:vm")`;';
      const source = `import "./helper"; import { spawnSync } from "node:child_process"; ${declaration} ${code}`;
      const references = sourceCode(source);
      const snapshot = fixture();
      replaceSources(snapshot, { "scripts/html/main.ts": source });
      if (accepted) {
        expect(references).toContain("!child-import:node:vm");
        expect(references).not.toContain("!unresolved-executable");
        expect(snapshot.graph.errors).toEqual([]);
      } else {
        expect(references).toContain("!unresolved-executable");
        expect(snapshot.graph.errors).toContain("unresolved/external code: !unresolved-executable");
      }
    },
  );
});

describe("Moon cheap static prerequisite for Cargo analysis", () => {
  test.each([
    ["missing ancestor", "moon cheap static barrier bypass: lint"],
    ["optional ancestor", "non-required moon dependency lint"],
    ["wait ancestor", "non-required moon dependency lint"],
    ["allow failure", "unsafe moon task options source-static"],
    ["expected failure", "unsafe moon task options source-static"],
    ["cycle", "moon dependency cycle lint"],
    ["self barrier", "moon cheap static barrier bypass: lint"],
  ] as const)("rejects %s", (kind, expected) => {
    const snapshot = splitCiFixture();
    changeMoon(snapshot, (tasks) => {
      const lint = tasks.lint;
      const cheap = tasks["source-static"];
      if (!lint || !cheap) throw new Error("cheap static fixture tasks missing");
      switch (kind) {
        case "missing ancestor":
          lint.deps = [];
          break;
        case "optional ancestor":
          lint.deps = [{ target: "source-static", type: "optional" }];
          break;
        case "wait ancestor":
          lint.deps = [{ target: "source-static", type: "wait" }];
          break;
        case "allow failure":
          cheap.options = { allowFailure: true };
          break;
        case "expected failure":
          cheap.options = { expectFailure: true };
          break;
        case "cycle":
          cheap.deps.push("lint");
          break;
        case "self barrier":
          if (!snapshot.policy.moon) throw new Error("Moon fixture policy missing");
          snapshot.policy.moon.ciEntry = "lint";
          break;
      }
    });
    expect(checkHarnessContract(snapshot).some((error) => error.includes(expected))).toBe(true);
  });

  test("accepts both prerequisites while format remains independent of cheap static", () => {
    const snapshot = splitCiFixture();
    changeMoon(snapshot, (tasks) => {
      tasks["fmt-check"] = { command: "noop", deps: [] };
      if (!tasks.lint) throw new Error("lint fixture task missing");
      tasks.lint.deps = ["fmt-check", "source-static"];
    });
    expect(checkHarnessContract(snapshot)).toEqual([]);
  });

  test("the actual Cargo analysis chain runs AST before Clippy after cheap static", () => {
    const config = z
      .object({
        taskOptions: z.object({
          cache: z.literal(false),
          runInCI: z.literal("always"),
          shell: z.literal(false),
        }),
        tasks: z.record(
          z.string(),
          z.object({
            command: z.union([z.string(), z.array(z.string())]),
            deps: z.array(z.string()).optional(),
          }),
        ),
      })
      .parse(Bun.YAML.parse(fs.readFileSync(path.join(import.meta.dir, "../../moon.yml"), "utf8")));
    expect(config.tasks.lint?.deps).toEqual(["ast-lint"]);
    expect(config.tasks["ast-lint"]?.deps).toEqual(["fmt-check", "source-static"]);
    expect(config.tasks["dependency-leak"]?.deps).toEqual(["lint"]);
    expect(config.tasks["fmt-check"]?.command).toEqual(["just", "fmt-check"]);
    expect(config.tasks["fmt-check"]?.deps).toBeUndefined();
    expect(config.tasks.static?.deps).toEqual(["source-static", "dependency-leak"]);
    expect(config.tasks["source-static"]?.command).toBe("noop");
    expect(config.taskOptions).toEqual({ cache: false, runInCI: "always", shell: false });
  });
});

describe("fixed moon quality graph and managed language scopes", () => {
  test.each([
    "scripts/quality/duplication-check.ts",
    "scripts/build/validate.ts",
    "crates/renderer/src/runtime.ts",
  ])("rejects an untracked invoked source under a managed code root: %s", (name) => {
    const snapshot = moonFixture();
    snapshot.tracked.delete(name);
    replaceSources(snapshot, {
      [name]: "export const validation = true;",
      Justfile:
        snapshot
          .read("Justfile")
          ?.replace("bun scripts/quality/duplication-check.ts", `bun ${name}`) ?? "",
    });
    expect(snapshot.graph.local.has(name)).toBe(true);
    expect(snapshot.graph.ci.has(name)).toBe(true);
    expect(checkHarnessContract(snapshot)).toContain(`untracked code: ${name}`);
  });
  test("checks static coverage for invoked managed code outside HTML", () => {
    const snapshot = moonFixture();
    snapshot.biomeIncludes = ["scripts/html/**/*.ts"];
    snapshot.typeIncludes = ["scripts/html/**/*.ts"];
    const errors = checkHarnessContract(snapshot);
    expect(errors).toContain("lint scope missing: scripts/quality/duplication-check.ts");
    expect(errors).toContain("type scope missing: scripts/quality/duplication-check.ts");
  });
  test.each(["redirected", "noop"])("rejects a required heavy leaf changed to %s", (change) => {
    const snapshot = moonFixture();
    changeMoon(snapshot, (tasks) => {
      if (tasks["html-test"])
        tasks["html-test"].command = change === "noop" ? "noop" : ["just", "lint"];
    });
    expect(checkHarnessContract(snapshot)).toContain("moon heavy leaf missing: html-test");
  });
  test("observes CI source-static job success dependency and actual downstream leaves", () => {
    expect(checkHarnessContract(splitCiFixture())).toEqual([]);
  });
  test.each(["needs", "entry", "preflight", "ordering", "always", "continue", "conditional"])(
    "rejects unsafe split CI registration: %s",
    (fault) => {
      const snapshot = splitCiFixture();
      changeCi(snapshot, (jobs) => {
        const upstream = jobs["static-quality"];
        const downstream = jobs.test;
        if (!upstream || !downstream) throw new Error("fixture CI jobs missing");
        const step = upstream.steps[1];
        if (!step) throw new Error("fixture static entry missing");
        switch (fault) {
          case "needs":
            downstream.needs = "unrelated";
            break;
          case "entry":
            step.run = `echo ${step.run}`;
            break;
          case "preflight":
            upstream.steps.reverse();
            break;
          case "ordering":
            downstream.steps.reverse();
            break;
          case "always":
            downstream.if = "always()";
            break;
          case "continue":
            step["continue-on-error"] = true;
            break;
          case "conditional":
            step.if = "false";
            break;
        }
      });
      expect(checkHarnessContract(snapshot).length).toBeGreaterThan(0);
    },
  );
  test("does not grant unexecuted CI heavy leaves from local moon reachability", () => {
    const snapshot = splitCiFixture();
    changeCi(snapshot, (jobs) => {
      if (jobs.test) jobs.test.steps = [{ run: "just lint" }, { run: "cargo build" }];
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "CI registration missing: scripts/html/main.ts",
    );
  });
  test("rejects missing CI source-static leaf", () => {
    const snapshot = splitCiFixture();
    changeMoon(snapshot, (tasks) => {
      if (tasks["source-static"]) tasks["source-static"].deps = ["biome"];
    });
    expect(checkHarnessContract(snapshot)).toContain("CI static leaf missing: typecheck");
  });
  test("rejects missing CI owned-validation static leaf", () => {
    const snapshot = splitCiFixture();
    changeMoon(snapshot, (tasks) => {
      if (tasks["source-static"])
        tasks["source-static"].deps = tasks["source-static"].deps.filter(
          (dependency) => dependency !== "owned-validation-contract-check",
        );
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "CI static leaf missing: owned-validation-contract-check",
    );
  });
  test.each(["local", "ci"])(
    "rejects the owned-validation gate redirected to a different leaf in %s",
    (registration) => {
      const snapshot = registration === "ci" ? splitCiFixture() : moonFixture();
      changeMoon(snapshot, (tasks) => {
        const task = tasks["owned-validation-contract-check"];
        if (task) task.command = ["just", "duplication-check"];
      });
      expect(checkHarnessContract(snapshot)).toContain(
        registration === "ci"
          ? "CI static leaf missing: owned-validation-contract-check"
          : "moon static leaf missing: owned-validation-contract-check",
      );
    },
  );
  test("rejects detached local preflight and admits non-authoritative CI diagnostics", () => {
    const snapshot = splitCiFixture();
    changeCi(snapshot, (jobs) => {
      jobs.test?.steps.push({ run: "df -h .", if: "always()" });
    });
    expect(checkHarnessContract(snapshot)).toEqual([]);
    replaceSources(snapshot, {
      [snapshot.policy.localBridge.source]:
        snapshot
          .read(snapshot.policy.localBridge.source)
          ?.replace("if preflight.returncode != 0:", "if False:") ?? "",
    });
    expect(checkHarnessContract(snapshot)).toContain("local contract preflight missing");
  });
  test.each(["wrong-version", "wrong-select", "wrong-scipy-env", "commented-scipy-env"])(
    "rejects weakened Python policy execution: %s",
    (fault) => {
      const snapshot = moonFixture();
      snapshot.declaredCode = ["scripts/html/managed.py"];
      snapshot.tracked.add("scripts/html/managed.py");
      replaceSources(snapshot, {
        Justfile: (() => {
          const justfile = snapshot.read("Justfile") ?? "";
          if (fault === "wrong-version") return justfile.replace("ruff==0.16.10", "ruff==0.15.0");
          if (fault === "wrong-select") return justfile.replace("F,E4,E7,E9,B,UP", "F");
          if (fault === "wrong-scipy-env")
            return justfile.replace(
              'HTML_PERFORMANCE_SCIPY_ENV := "KRR_HTML_PERFORMANCE_SCIPY_VERSION=" + HTML_PERFORMANCE_SCIPY_VERSION',
              'HTML_PERFORMANCE_SCIPY_ENV := "KRR_HTML_PERFORMANCE_SCIPY_VERSION=0.0.0"',
            );
          return justfile.replace(
            'HTML_PERFORMANCE_SCIPY_ENV := "KRR_HTML_PERFORMANCE_SCIPY_VERSION=" + HTML_PERFORMANCE_SCIPY_VERSION',
            '# HTML_PERFORMANCE_SCIPY_ENV := "KRR_HTML_PERFORMANCE_SCIPY_VERSION=" + HTML_PERFORMANCE_SCIPY_VERSION\nHTML_PERFORMANCE_SCIPY_ENV := "KRR_HTML_PERFORMANCE_SCIPY_VERSION=0.0.0"',
          );
        })(),
      });
      expect(checkHarnessContract(snapshot)).toContain(
        "Python Ruff + strict type check registration unavailable: scripts/html/managed.py",
      );
    },
  );
  test("rejects disabled automatic Rust library and accepts an explicit managed binary source", () => {
    const snapshot = moonFixture();
    snapshot.declaredCode = ["crates/renderer/src/lib.rs"];
    snapshot.tracked.add("crates/renderer/src/lib.rs");
    replaceSources(snapshot, {
      "crates/renderer/Cargo.toml": '[package]\nname="renderer"\nautolib=false\n',
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "Rust workspace/Clippy registration unavailable: crates/renderer/src/lib.rs",
    );
    snapshot.declaredCode = ["crates/renderer/custom/main.rs"];
    snapshot.tracked.add("crates/renderer/custom/main.rs");
    replaceSources(snapshot, {
      "crates/renderer/Cargo.toml":
        '[package]\nname="renderer"\n[[bin]]\nname="renderer"\npath="custom/main.rs"\n',
      "crates/renderer/custom/main.rs": "fn main() {}",
    });
    expect(checkHarnessContract(snapshot)).toEqual([]);
  });
  test("observes check bridge → fixed entry → required static barrier → HTML leaf", () => {
    expect(checkHarnessContract(moonFixture())).toEqual([]);
  });
  test.each([
    [
      "CAS read integrity missing",
      (source: string) => source.replace("    verifyIntegrity: true\n", ""),
    ],
    [
      "CAS read integrity disabled",
      (source: string) => source.replace("verifyIntegrity: true", "verifyIntegrity: false"),
    ],
    [
      "CAS output caching missing",
      (source: string) => source.replace("  casOutputsCache: true\n", ""),
    ],
    [
      "CAS output caching disabled",
      (source: string) => source.replace("casOutputsCache: true", "casOutputsCache: false"),
    ],
  ] as const)("rejects workspace %s", (_name, mutate) => {
    const snapshot = moonFixture();
    const source = snapshot.read(".moon/workspace.yml");
    if (source === undefined) throw new Error("Moon workspace fixture is missing");
    const changed = mutate(source);
    expect(changed).not.toBe(source);
    replaceSources(snapshot, { ".moon/workspace.yml": changed });
    expect(checkHarnessContract(snapshot)).toContain(
      "moon contract invalid: moon version, project, and verified CAS cache binding differs",
    );
  });
  test("rejects a heavy task without the static ancestor", () => {
    const snapshot = moonFixture();
    changeMoon(snapshot, (tasks) => {
      if (tasks["html-test"]) tasks["html-test"].deps = [];
    });
    expect(checkHarnessContract(snapshot)).toContain("moon static barrier bypass: html-test");
  });
  test("rejects using the owned validation gate as a substitute for the static barrier", () => {
    const snapshot = moonFixture();
    changeMoon(snapshot, (tasks) => {
      if (tasks["html-test"]) tasks["html-test"].deps = ["owned-validation-contract-check"];
    });
    expect(checkHarnessContract(snapshot)).toContain("moon static barrier bypass: html-test");
  });
  test.each([
    "python-lint",
    "python-typecheck",
    "duplication-check",
    "owned-validation-contract-check",
  ])("rejects omitted static leaf %s", (name) => {
    const snapshot = moonFixture();
    changeMoon(snapshot, (tasks) => {
      if (tasks.static) tasks.static.deps = tasks.static.deps.filter((dep) => dep !== name);
    });
    expect(checkHarnessContract(snapshot)).toContain(`moon static leaf missing: ${name}`);
  });
  test("rejects optional/wait dependency shapes and allow-failure options", () => {
    const snapshot = moonFixture();
    changeMoon(snapshot, (tasks) => {
      if (tasks["html-test"]) tasks["html-test"].deps = [{ target: "static", type: "wait" }];
    });
    expect(
      checkHarnessContract(snapshot).some((error) =>
        error.includes("non-required moon dependency"),
      ),
    ).toBe(true);
    changeMoon(snapshot, (tasks) => {
      if (tasks["html-test"]) {
        tasks["html-test"].deps = ["static"];
        tasks["html-test"].options = { allowFailure: true };
      }
    });
    expect(
      checkHarnessContract(snapshot).some((error) => error.includes("unsafe moon task options")),
    ).toBe(true);
  });
  test("rejects CI leaf-only registration rather than assuming same-entry execution", () => {
    const snapshot = moonFixture();
    replaceSources(snapshot, {
      [snapshot.policy.ciWorkflow]: "jobs:\n  test:\n    steps:\n      - run: just html-test\n",
    });
    expect(checkHarnessContract(snapshot)).toContain("CI moon entry registration missing");
  });
  test("accepts managed Python only with both fixed-version static recipes in the graph", () => {
    const snapshot = moonFixture();
    snapshot.declaredCode = ["scripts/html/managed.py"];
    snapshot.tracked.add("scripts/html/managed.py");
    expect(checkHarnessContract(snapshot)).toEqual([]);
    const currentJustfile = snapshot.read("Justfile") ?? "";
    replaceSources(snapshot, {
      Justfile: currentJustfile.replace(
        'HTML_PERFORMANCE_TYPING_PATH := "scripts/release/typings"',
        'HTML_PERFORMANCE_TYPING_PATH := "scripts/release/other-typings"',
      ),
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "Python Ruff + strict type check registration unavailable: scripts/html/managed.py",
    );
    replaceSources(snapshot, { Justfile: currentJustfile });
    replaceSources(snapshot, {
      Justfile: snapshot.read("Justfile")?.replace("mypy --strict scripts", "mypy scripts") ?? "",
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "Python Ruff + strict type check registration unavailable: scripts/html/managed.py",
    );
  });
  test("rejects Python lint/type scopes that do not include the declared script", () => {
    const snapshot = moonFixture();
    snapshot.declaredCode = ["scripts/html/managed.py"];
    snapshot.tracked.add("scripts/html/managed.py");
    replaceSources(snapshot, {
      Justfile: snapshot.read("Justfile")?.replace("--strict scripts", "--strict other") ?? "",
    });
    expect(checkHarnessContract(snapshot)).toContain(
      "Python Ruff + strict type check registration unavailable: scripts/html/managed.py",
    );
  });
  test("accepts a selected Cargo workspace target and rejects external/orphan Rust sources", () => {
    const snapshot = moonFixture();
    snapshot.declaredCode = ["crates/renderer/src/lib.rs"];
    snapshot.tracked.add("crates/renderer/src/lib.rs");
    expect(checkHarnessContract(snapshot)).toEqual([]);
    snapshot.declaredCode = ["crates/not-member/src/lib.rs", "crates/renderer/src/orphan.rs"];
    expect(checkHarnessContract(snapshot)).toContain(
      "Rust workspace/Clippy registration unavailable: crates/not-member/src/lib.rs",
    );
    expect(checkHarnessContract(snapshot)).toContain(
      "Rust workspace/Clippy registration unavailable: crates/renderer/src/orphan.rs",
    );
  });
  test("binary/outputs fields do not declare generated Rust executables as source", () => {
    expect(
      declaredValidationCode({
        nativeBinary: { path: "target/debug/runtime" },
        workerPath: "target/debug/runtime",
        outputPath: "tmp/report.rs",
      }),
    ).toEqual([]);
  });
  test("Q10 rejects a positive obsolete-crate include with zero metadata matches", () => {
    const snapshot = fixture();
    snapshot.scopeMatches = {
      biome: { "scripts/**/*.ts": 2, "crates/old/source/**/*.ts": 0 },
      types: { "scripts/**/*.ts": 2 },
    };
    expect(checkHarnessContract(snapshot)).toContain(
      "empty managed biome include: crates/old/source/**/*.ts",
    );
    snapshot.scopeMatches.biome = { "scripts/**/*.ts": 2 };
    snapshot.typeExcludes.push("tmp/not-generated-yet/**");
    expect(checkHarnessContract(snapshot)).toEqual([]);
  });
});

describe("closed process API references", () => {
  test.each([
    [
      "const alias",
      'import { spawnSync } from "node:child_process"; const launch = spawnSync; launch("node", ["-e", script]);',
      false,
    ],
    [
      "mutable alias",
      'import { spawnSync } from "node:child_process"; let launch; launch = spawnSync; launch("node", ["-e", script]);',
      false,
    ],
    [
      "argument escape",
      'import { spawnSync } from "node:child_process"; launch(spawnSync);',
      false,
    ],
    [
      "return escape",
      'import { spawnSync } from "node:child_process"; function launch() { return spawnSync; } launch()("node", ["-e", script]);',
      false,
    ],
    [
      "object escape",
      'import { spawnSync } from "node:child_process"; const api = { launch: spawnSync }; api.launch("node", ["-e", script]);',
      false,
    ],
    [
      "array escape",
      'import { spawnSync } from "node:child_process"; const api = [spawnSync]; api[0]("node", ["-e", script]);',
      false,
    ],
    [
      "bind escape",
      'import { spawnSync } from "node:child_process"; const launch = spawnSync.bind(null); launch("node", ["-e", script]);',
      false,
    ],
    [
      "apply escape",
      'import { spawnSync } from "node:child_process"; spawnSync.apply(null, ["node", ["-e", script]]);',
      false,
    ],
    [
      "namespace destructure",
      'import * as child from "node:child_process"; const { spawnSync: launch } = child; launch("node", ["-e", script]);',
      false,
    ],
    [
      "namespace alias",
      'import * as child from "node:child_process"; const api = child; api.spawnSync("node", ["-e", script]);',
      false,
    ],
    [
      "namespace method escape",
      'import * as child from "node:child_process"; const launch = child.spawnSync; launch("node", ["-e", script]);',
      false,
    ],
    [
      "computed literal",
      'import * as child from "node:child_process"; child["spawnSync"]("node", ["-e", script]);',
      false,
    ],
    [
      "computed variable",
      'import * as child from "node:child_process"; child[method]("node", ["-e", script]);',
      false,
    ],
    ["unsupported fork", 'import { fork } from "node:child_process"; fork("child.js");', false],
    [
      "CJS namespace",
      'const child = require("node:child_process"); child.spawnSync("node", ["-e", script]);',
      false,
    ],
    [
      "CJS destructure",
      'const { spawnSync } = require("child_process"); spawnSync("node", ["-e", script]);',
      false,
    ],
    ["CJS computed", 'require("node:child_process")["spawnSync"]("node", ["-e", script]);', false],
    ["CJS direct", 'require("node:child_process").spawnSync("node", ["-e", script]);', false],
    [
      "TS CJS import escape",
      'import child = require("node:child_process"); child.spawnSync("node", ["-e", script]);',
      false,
    ],
    [
      "dynamic import escape",
      'const child = await import("node:child_process"); child.spawnSync("node", ["-e", script]);',
      false,
    ],
    ["re-export escape", 'export { spawnSync } from "node:child_process";', false],
    [
      "Bun imported alias",
      'import { spawnSync } from "bun"; const launch = spawnSync; launch(["node", "-e", script]);',
      false,
    ],
    ["Bun method alias", 'const launch = Bun.spawnSync; launch(["node", "-e", script]);', false],
    ["Bun computed call", 'Bun["spawnSync"](["node", "-e", script]);', false],
    [
      "named import rename",
      'import { spawnSync as launch } from "node:child_process"; launch("node", ["-e", script]);',
      true,
    ],
    [
      "namespace direct",
      'import * as child from "node:child_process"; child.spawnSync("node", ["-e", script]);',
      true,
    ],
    [
      "default direct",
      'import child from "node:child_process"; child.spawnSync("node", ["-e", script]);',
      true,
    ],
    [
      "type-only import",
      'import type { ChildProcess } from "node:child_process"; function identity(value: ChildProcess) { return value; }',
      true,
    ],
    ["type-only export", 'export type { ChildProcess } from "node:child_process";', true],
    [
      "shadowed binding",
      'import { spawnSync } from "node:child_process"; function local(spawnSync) { const launch = spawnSync; return launch; }',
      true,
    ],
  ])("%s", (_name, code, supported) => {
    const source = `import "./helper"; const script = 'require("node:fs")'; ${code}`;
    const references = sourceCode(source);
    const snapshot = fixture();
    replaceSources(snapshot, { "scripts/html/main.ts": source });
    if (supported) {
      expect(references).not.toContain("!unresolved-executable");
      expect(snapshot.graph.errors).toEqual([]);
    } else {
      expect(references).toContain("!unresolved-executable");
      expect(snapshot.graph.errors).toContain("unresolved/external code: !unresolved-executable");
    }
  });

  test("source changes between local and CI are read again", () => {
    const snapshot = fixture();
    const prior = snapshot.read;
    let reads = 0;
    const read = (name: string): string | undefined => {
      if (name !== "scripts/html/main.ts") return prior(name);
      reads += 1;
      return reads === 1 ? 'import "./helper";' : 'import "unregistered-later-dependency";';
    };
    const graph = executionGraph(snapshot.policy, read, {}, new Set());
    expect(reads).toBe(2);
    expect(graph.errors).toContain("unknown code dependency: unregistered-later-dependency");
  });
});

describe("import-equals dependencies and Bun namespace boundaries", () => {
  test.each([
    [
      "local external module",
      'import helper = require("./helper"); helper.run();',
      "./helper",
      undefined,
    ],
    [
      "missing external module",
      'import helper = require("./missing-helper"); helper.run();',
      "./missing-helper",
      "missing code: scripts/html/missing-helper.ts",
    ],
    [
      "unregistered package",
      'import helper = require("unregistered-package"); helper.run();',
      "unregistered-package",
      "unknown code dependency: unregistered-package",
    ],
    [
      "builtin module",
      'import fs = require("node:fs"); fs.statSync("input");',
      "node:fs",
      undefined,
    ],
    [
      "type-only builtin",
      'import type fs = require("node:fs"); type Stat = ReturnType<typeof fs.statSync>;',
      "node:fs",
      undefined,
    ],
    [
      "type-only missing module",
      'import type helper = require("./missing-helper"); type Value = typeof helper;',
      "./missing-helper",
      "missing code: scripts/html/missing-helper.ts",
    ],
  ] as const)("%s retains its literal dependency", (_name, source, dependency, error) => {
    expect(sourceCode(source)).toContain(dependency);
    const snapshot = fixture();
    replaceSources(snapshot, { "scripts/html/main.ts": source });
    if (error) expect(snapshot.graph.errors).toContain(error);
    else expect(snapshot.graph.errors).toEqual([]);
    if (dependency === "./helper") {
      expect(snapshot.graph.local.has("scripts/html/helper.ts")).toBe(true);
      expect(snapshot.graph.ci.has("scripts/html/helper.ts")).toBe(true);
    }
  });

  test.each([
    ["global namespace alias", 'const api = Bun; api.spawnSync(["node", "-e", script]);', false],
    [
      "imported namespace alias",
      'import * as bun from "bun"; const api = bun; api.spawnSync(["node", "-e", script]);',
      false,
    ],
    [
      "default namespace alias",
      'import bun from "bun"; const api = bun; api.spawnSync(["node", "-e", script]);',
      false,
    ],
    ["global destructure", 'const { spawnSync } = Bun; spawnSync(["node", "-e", script]);', false],
    [
      "imported destructure",
      'import * as bun from "bun"; const { spawnSync } = bun; spawnSync(["node", "-e", script]);',
      false,
    ],
    ["namespace argument escape", "launch(Bun);", false],
    [
      "namespace return escape",
      'function api() { return Bun; } api().spawnSync(["node", "-e", script]);',
      false,
    ],
    [
      "namespace array escape",
      'const api = [Bun]; api[0].spawnSync(["node", "-e", script]);',
      false,
    ],
    [
      "globalThis Bun escape",
      'const api = globalThis.Bun; api.spawnSync(["node", "-e", script]);',
      false,
    ],
    [
      "globalThis computed Bun",
      'const api = globalThis["Bun"]; api.spawnSync(["node", "-e", script]);',
      false,
    ],
    ["globalThis unsupported direct", 'globalThis.Bun.spawnSync(["node", "-e", script]);', false],
    [
      "imported direct process",
      'import * as bun from "bun"; bun.spawnSync(["node", "-e", script]);',
      true,
    ],
    ["global environment member", "const environment = Bun.env;", true],
    ["imported ordinary member", 'import * as bun from "bun"; bun.YAML.parse("key: value");', true],
    [
      "global typeof guard",
      'if (typeof Bun !== "undefined") Bun.spawnSync(["node", "-e", script]);',
      true,
    ],
    ["globalThis typeof guard", 'if (typeof globalThis.Bun !== "undefined") {}', true],
    ["type-only qualified query", "type Spawn = ReturnType<typeof Bun.spawnSync>;", true],
    ["lexically shadowed Bun", "function local(Bun) { const api = Bun; return api; }", true],
  ] as const)("%s", (_name, code, supported) => {
    const source = `import "./helper"; const script = 'require("node:fs")'; ${code}`;
    const snapshot = fixture();
    replaceSources(snapshot, { "scripts/html/main.ts": source });
    if (supported) {
      expect(sourceCode(source)).not.toContain("!unresolved-executable");
      expect(snapshot.graph.errors).toEqual([]);
    } else {
      expect(sourceCode(source)).toContain("!unresolved-executable");
      expect(snapshot.graph.errors).toContain("unresolved/external code: !unresolved-executable");
    }
  });
});

const nativeUnitCaller = "scripts/mermaid/native-cli-process_test.ts";
const nativeUnitWrapper = "scripts/mermaid/native-cli-process.ts";

function actualNativeUnitSources() {
  return {
    caller: fs.readFileSync(nativeUnitCaller, "utf8"),
    wrapper: fs.readFileSync(nativeUnitWrapper, "utf8"),
  };
}

describe("reviewed native stream unit executable registry", () => {
  test("binds the actual finite unit program without claiming a KRR artifact", () => {
    const actual = actualNativeUnitSources();
    const bindings: UnitExecutableBinding[] = [];
    const references = sourceCode(actual.wrapper, {
      name: nativeUnitWrapper,
      importer: nativeUnitCaller,
      importerSource: actual.caller,
      read: (name) => (name === nativeUnitCaller ? actual.caller : undefined),
      onUnitBinding: (binding) => bindings.push(binding),
    });
    expect(references).not.toContain("!unresolved-executable");
    expect(bindings).toHaveLength(1);
    expect(bindings[0]?.proofKind).toBe("reviewed-canonical-unit-source");
    expect(bindings[0]?.artifactVerified).toBe(false);
    expect(bindings[0]?.caller).toBe(nativeUnitCaller);
    expect(bindings[0]?.wrapper).toBe(nativeUnitWrapper);
    expect(bindings[0]?.executions.slice(0, 2)).toEqual([
      { kind: "unit-self-runtime-version", executable: "process.execPath", argv: ["--version"] },
      {
        kind: "unit-known-absent-temp-path",
        executable: "streamFixture().missingBinary",
        argv: [[], ["--version"]],
      },
    ]);
    expect(bindings[0]?.executions[2]).toEqual({
      kind: "unit-self-runtime-temp-environment",
      executable: "process.execPath",
      argv: ["-e", "childProgram"],
      temporaryDirectory: "tempRoots.entries().tempRoot",
    });
    expect(bindings[0]?.executions).toHaveLength(3);
  });

  test("rechecks reader bytes when the same wrapper source is analyzed again", () => {
    const actual = actualNativeUnitSources();
    let callerBytes = actual.caller;
    let readerCalls = 0;
    const references = () =>
      sourceCode(actual.wrapper, {
        name: nativeUnitWrapper,
        importer: nativeUnitCaller,
        importerSource: actual.caller,
        read: (name) => {
          if (name !== nativeUnitCaller) return undefined;
          readerCalls += 1;
          return callerBytes;
        },
      });

    expect(references()).not.toContain("!unresolved-executable");
    const priorReaderCalls = readerCalls;
    callerBytes = actual.caller.replace("process.execPath", "unknownBinary");
    expect(callerBytes).not.toBe(actual.caller);
    expect(references()).toContain("!unresolved-executable");
    expect(readerCalls).toBeGreaterThan(priorReaderCalls);
  });

  test.each([
    ["caller unknown target", "caller", "process.execPath", "unknownBinary"],
    ["caller eval argv", "caller", '"--version"', '"-e"'],
    ["caller preload argv", "caller", '"--version"', '"--require"'],
    [
      "caller executable fixture write",
      "caller",
      "  return {\n",
      '  fs.writeFileSync(path.join(root, "missing-binary"), "executable fixture");\n  return {\n',
    ],
    [
      "caller namespace alias",
      "caller",
      'import { spawnNativeCli } from "./native-cli-process";',
      'import { spawnNativeCli } from "./native-cli-process"; const launch = spawnNativeCli;',
    ],
    [
      "caller extra process call",
      "caller",
      "const root = fs.mkdtempSync",
      'spawnNativeCli(process.execPath, [process.execPath, "-e", "hidden"], "out", "err");\n  const root = fs.mkdtempSync',
    ],
    [
      "caller changed temp expression",
      "caller",
      "        tempRoot,",
      "        process.env.TMPDIR,",
    ],
    [
      "caller general Node eval",
      "caller",
      'const os = require("node:os");',
      'const os = require("node:child_process");',
    ],
    [
      "wrapper changed temp override",
      "wrapper",
      "environment.TMPDIR = temporaryDirectory",
      "environment.PATH = temporaryDirectory",
    ],
    [
      "wrapper changed no-temp behavior",
      "wrapper",
      "temporaryDirectory === undefined ? undefined",
      "temporaryDirectory === undefined ? {}",
    ],
    ["wrapper changed argv", "wrapper", "argv.slice(1)", '["-e", "hidden"]'],
    ["wrapper changed target", "wrapper", "spawn(binary,", "spawn(unknownBinary,"],
    [
      "wrapper extra call",
      "wrapper",
      "const child = spawn(",
      'spawn("node", ["-e", "hidden"]); const child = spawn(',
    ],
  ] as const)(
    "%s rejects instead of recalculating a candidate baseline",
    (_name, target, original, replacement) => {
      const actual = actualNativeUnitSources();
      expect(actual[target]).toContain(original);
      const changed = { ...actual, [target]: actual[target].replace(original, replacement) };
      const bindings: UnitExecutableBinding[] = [];
      const references = sourceCode(changed.wrapper, {
        name: nativeUnitWrapper,
        importer: nativeUnitCaller,
        importerSource: changed.caller,
        read: (name) => (name === nativeUnitCaller ? changed.caller : undefined),
        onUnitBinding: (binding) => bindings.push(binding),
      });
      expect(references).toContain("!unresolved-executable");
      expect(bindings).toEqual([]);
    },
  );

  test.each([
    {
      name: "unknown caller",
      importer: "scripts/mermaid/unknown_test.ts",
      source: "actual",
      reader: "actual",
    },
    {
      name: "missing caller snapshot",
      importer: nativeUnitCaller,
      source: "missing",
      reader: "actual",
    },
    {
      name: "missing caller reader",
      importer: nativeUnitCaller,
      source: "actual",
      reader: "missing",
    },
    {
      name: "changed caller after analysis",
      importer: nativeUnitCaller,
      source: "actual",
      reader: "changed",
    },
    {
      name: "earlier unknown caller cannot use later canonical bytes",
      importer: nativeUnitCaller,
      source: "changed",
      reader: "actual",
    },
  ])("%j keeps the target unresolved", ({ importer, source, reader }) => {
    const actual = actualNativeUnitSources();
    const changed = actual.caller.replace("process.execPath", "unknownBinary");
    const references = sourceCode(actual.wrapper, {
      name: nativeUnitWrapper,
      importer,
      ...(source === "missing"
        ? {}
        : { importerSource: source === "actual" ? actual.caller : changed }),
      read: () =>
        reader === "missing" ? undefined : reader === "actual" ? actual.caller : changed,
    });
    expect(references).toContain("!unresolved-executable");
  });

  test("wrapper raw source cannot be replaced by a later reader value", () => {
    const actual = actualNativeUnitSources();
    const references = sourceCode(`${actual.wrapper}\n// changed raw source\n`, {
      name: nativeUnitWrapper,
      importer: nativeUnitCaller,
      importerSource: actual.caller,
      read: (name) => (name === nativeUnitCaller ? actual.caller : actual.wrapper),
    });
    expect(references).toContain("!unresolved-executable");
  });

  test("both lanes record the actual unit proof and caller bytes remain live", () => {
    const actual = actualNativeUnitSources();
    const snapshot = fixture();
    replaceSources(snapshot, {
      "scripts/html/main.ts": 'import "../mermaid/native-cli-process_test";',
      [nativeUnitCaller]: actual.caller,
      [nativeUnitWrapper]: actual.wrapper,
    });
    expect(snapshot.graph.errors).toEqual([]);
    expect(snapshot.graph.unitExecutableBindings?.map((binding) => binding.lane)).toEqual([
      "local",
      "ci",
    ]);
    expect(snapshot.graph.local.has(nativeUnitCaller)).toBe(true);
    expect(snapshot.graph.ci.has(nativeUnitWrapper)).toBe(true);
    const prior = snapshot.read;
    let reads = 0;
    const graph = executionGraph(
      snapshot.policy,
      (name) => {
        if (name !== nativeUnitCaller) return prior(name);
        reads += 1;
        return reads === 1
          ? actual.caller.replace("process.execPath", "unknownBinary")
          : actual.caller;
      },
      {},
      new Set(),
    );
    expect(graph.errors).toContain("unresolved/external code: !unresolved-executable");
    expect(graph.unitExecutableBindings?.map((binding) => binding.lane)).toEqual(["ci"]);
  });
});

describe("reviewed Mermaid CLI source route and runtime compressor unit", () => {
  const nativeCliProducer = "scripts/mermaid/native-render-provenance.ts";
  const compressorCaller = "scripts/runtime-assets/runtime-package-asset-compressor_test.ts";
  const compressorScript = "scripts/runtime-assets/runtime-package-asset-compressor.ts";

  test.each([
    ["canonical", "caller", null, null, true],
    ["different binary", "caller", "process.execPath", "otherBinary", false],
    ["different child program", "caller", "process.stdout.write", "process.stderr.write", false],
    ["different wrapper launch", "wrapper", "spawn(binary", "spawn(otherBinary", false],
  ] as const)(
    "native batch reviewed fixture boundary: %s",
    (_label, role, before, after, accepted) => {
      const caller = "scripts/mermaid/native-render-batch_test.ts";
      const wrapper = "scripts/mermaid/native-cli-process.ts";
      const sources = {
        caller: fs.readFileSync(
          path.join(import.meta.dir, "../mermaid/native-render-batch_test.ts"),
          "utf8",
        ),
        wrapper: fs.readFileSync(
          path.join(import.meta.dir, "../mermaid/native-cli-process.ts"),
          "utf8",
        ),
      };
      if (before !== null && after !== null) sources[role] = sources[role].replace(before, after);
      const references = sourceCode(sources.wrapper, {
        name: wrapper,
        importer: caller,
        importerSource: sources.caller,
        read: (name) =>
          name === caller ? sources.caller : name === wrapper ? sources.wrapper : undefined,
      });
      expect(references.includes("!unresolved-executable")).toBe(!accepted);
    },
  );

  test("classifies the actual native helper only with a matching production or pinned unit context", () => {
    const wrapper = fs.readFileSync(
      path.join(import.meta.dir, "../mermaid/native-cli-process.ts"),
      "utf8",
    );
    const producer = fs.readFileSync(
      path.join(import.meta.dir, "../mermaid/native-render-provenance.ts"),
      "utf8",
    );
    const unit = fs.readFileSync(
      path.join(import.meta.dir, "../mermaid/native-cli-process_test.ts"),
      "utf8",
    );
    const read = (name: string) =>
      name === nativeCliProducer ? producer : name === nativeUnitCaller ? unit : undefined;
    const production = sourceCode(wrapper, {
      name: nativeUnitWrapper,
      importer: nativeCliProducer,
      read,
    });
    const unitContext = sourceCode(wrapper, {
      name: nativeUnitWrapper,
      importer: nativeUnitCaller,
      importerSource: unit,
      read,
    });
    const missingCallerBytes = sourceCode(wrapper, {
      name: nativeUnitWrapper,
      importer: nativeUnitCaller,
      read,
    });
    expect(producer).toContain('import { spawnNativeCli } from "./native-cli-process"');
    expect(production).not.toContain("!unresolved-executable");
    expect(unitContext).not.toContain("!unresolved-executable");
    expect(missingCallerBytes).toContain("!unresolved-executable");
    expect(sourceCode(wrapper, { name: nativeUnitWrapper, read })).toContain(
      "!unresolved-executable",
    );
  });

  test("binds only the canonical compressor CLI and its three finite argument vectors", () => {
    const caller = fs.readFileSync(
      path.join(import.meta.dir, "../runtime-assets/runtime-package-asset-compressor_test.ts"),
      "utf8",
    );
    const script = fs.readFileSync(
      path.join(import.meta.dir, "../runtime-assets/runtime-package-asset-compressor.ts"),
      "utf8",
    );
    const bindings: UnitExecutableBinding[] = [];
    const references = sourceCode(caller, {
      name: compressorCaller,
      read: (name) => (name === compressorScript ? script : undefined),
      onUnitBinding: (binding) => bindings.push(binding),
    });
    expect(references).not.toContain("!unresolved-executable");
    expect(bindings).toHaveLength(1);
    expect(bindings[0]).toMatchObject({
      proofKind: "reviewed-canonical-unit-source",
      artifactVerified: false,
      caller: compressorCaller,
      wrapper: compressorScript,
      swcVersion,
      executions: [
        {
          kind: "unit-runtime-asset-compressor-cli",
          executable: "process.execPath",
          script: compressorScript,
          argv: [
            ["--write", "--zenuml-only"],
            ["--zenuml-only", "--check"],
            ["--check", "--zenuml-only"],
          ],
          cwd: "temporaryRoot()",
          stdio: { stdout: "pipe", stderr: "pipe" },
        },
      ],
    });
  });

  test.each([
    ["executable", "process.execPath", "process.env.KRR_TEST_RUNTIME"],
    ["canonical script", "./runtime-package-asset-compressor.ts", "./other.ts"],
    ["temporary cwd", "cwd: root", "cwd: process.cwd()"],
    ["stdio capture", 'stdout: "pipe"', 'stdout: "inherit"'],
    ["argument vector", '["--write", "--zenuml-only"]', '["--write", "--check"]'],
    [
      "additional spawn",
      "const cli = Bun.spawn(",
      'Bun.spawn(["node"], {}); const cli = Bun.spawn(',
    ],
  ])("rejects compressor unit route changes to %s", (_name, before, after) => {
    const actual = fs.readFileSync(
      path.join(import.meta.dir, "../runtime-assets/runtime-package-asset-compressor_test.ts"),
      "utf8",
    );
    expect(actual).toContain(before);
    const changed = actual.replace(before, after);
    const bindings: UnitExecutableBinding[] = [];
    const references = sourceCode(changed, {
      name: compressorCaller,
      read: (name) =>
        name === compressorScript
          ? fs.readFileSync(
              path.join(import.meta.dir, "../runtime-assets/runtime-package-asset-compressor.ts"),
              "utf8",
            )
          : undefined,
      onUnitBinding: (binding) => bindings.push(binding),
    });
    expect(references).toContain("!unresolved-executable");
    expect(bindings).toEqual([]);
  });

  test.each(["unknown caller", "missing script source", "changed script source"])(
    "%s cannot borrow the canonical compressor binding",
    (condition) => {
      const caller = fs.readFileSync(
        path.join(import.meta.dir, "../runtime-assets/runtime-package-asset-compressor_test.ts"),
        "utf8",
      );
      const script = fs.readFileSync(
        path.join(import.meta.dir, "../runtime-assets/runtime-package-asset-compressor.ts"),
        "utf8",
      );
      const bindings: UnitExecutableBinding[] = [];
      const references = sourceCode(caller, {
        name: compressorCaller,
        ...(condition === "unknown caller"
          ? { importer: "scripts/runtime-assets/other_test.ts" }
          : {}),
        read: (name) =>
          condition === "missing script source"
            ? undefined
            : name === compressorScript
              ? condition === "changed script source"
                ? `${script}\n// changed source`
                : script
              : undefined,
        onUnitBinding: (binding) => bindings.push(binding),
      });
      expect(references).toContain("!unresolved-executable");
      expect(bindings).toEqual([]);
    },
  );
});

describe("reviewed Chrome font identity probe process route", () => {
  const probePath = "scripts/html/visual-state-fidelity.ts";
  const productionImporter = "scripts/html/fidelity-acceptance-cli.ts";
  const importerPaths = [
    productionImporter,
    "scripts/html/visual-state-fidelity_test.ts",
    "scripts/html/dynamic-fidelity_test.ts",
    "scripts/html/feature-catalog_test.ts",
  ];
  const probeSource = fs.readFileSync(
    path.join(import.meta.dir, "visual-state-fidelity.ts"),
    "utf8",
  );
  const importerSources = new Map<string, string>(
    importerPaths.map((name): [string, string] => [
      name,
      fs.readFileSync(path.join(import.meta.dir, path.basename(name)), "utf8"),
    ]),
  );
  const read = (name: string) =>
    importerSources.get(name) ?? (name === probePath ? probeSource : undefined);
  const probeContext = (importer?: string, importerSource?: string) => ({
    name: probePath,
    ...(importer === undefined ? {} : { importer }),
    ...(importerSource === undefined ? {} : { importerSource }),
    read,
  });

  test("accepts only the actual synchronous probe in its production caller context", () => {
    const importerSource = importerSources.get(importerPaths[0] ?? "");
    expect(importerSource).toMatch(
      /await runCatalogStage90Captures\(\s*binary,\s*plan\.conditionPlan,\s*plan\.output,\s*plan\.resume,\s*plan\.chromeExecutable,\s*binaryProof,\s*currentBuild,\s*\);/,
    );
    for (const importer of importerPaths) {
      const importerSourceForRoute = importerSources.get(importer);
      if (importerSourceForRoute === undefined)
        throw new Error(`Expected importer source is missing: ${importer}`);
      expect(sourceCode(probeSource, probeContext(importer, importerSourceForRoute))).not.toContain(
        "!unresolved-executable",
      );
    }
  });

  test("keeps the reviewed probe route stable when source prefixes move its AST positions", () => {
    const importer = importerPaths[0];
    if (importer === undefined) throw new Error("Production importer path is missing");
    const importerSource = importerSources.get(importer);
    if (importerSource === undefined) throw new Error("Production importer source is missing");
    expect(
      sourceCode(
        `// position-independent-probe-fixture\n${probeSource}`,
        probeContext(importer, importerSource),
      ),
    ).not.toContain("!unresolved-executable");
  });

  test("resolves the catalog inspector import without changing the pinned probe", () => {
    const importer = "scripts/html/feature-catalog_test.ts";
    const importerSource = importerSources.get(importer);
    if (importerSource === undefined) throw new Error("Catalog importer source is missing");
    expect(importerSource).toContain(
      'import { inspectChromeCatalogBoundary } from "./visual-state-fidelity"',
    );
    expect(importerSource).not.toContain("runChromeFontIdentityProbe(");
    expect(sourceCode(probeSource, probeContext(importer, importerSource))).not.toContain(
      "!unresolved-executable",
    );
  });

  test.each([
    ["missing caller context", undefined, undefined],
    ["wrong importer", "scripts/html/other.ts", "scripts/html/fidelity-acceptance-cli.ts"],
  ])("keeps the probe unresolved with %s", (_label, importer, contextSource) => {
    const importerSource = contextSource ? importerSources.get(contextSource) : undefined;
    expect(sourceCode(probeSource, probeContext(importer, importerSource))).toContain(
      "!unresolved-executable",
    );
  });

  test.each([
    ["argv change", '"--nocapture",', '"--test-threads=1",'],
    ["binary change", "spawnSync(absoluteBinary, args, {", "spawnSync(untrustedBinary, args, {"],
    [
      "environment key change",
      "KRR_CHROME_FONT_IDENTITY_OUTPUT: absoluteResponse",
      "KRR_CHROME_FONT_IDENTITY_OUTPUT: untrustedResponse",
    ],
    [
      "second process call",
      "  if (result.error)\n",
      '  spawnSync(absoluteBinary, args, { encoding: "buffer" });\n  if (result.error)\n',
    ],
  ])("rejects a changed reviewed probe: %s", (_label, before, after) => {
    expect(probeSource).toContain(before);
    const changed = probeSource.replace(before, after);
    const productionImporterSource = importerSources.get(productionImporter);
    if (productionImporterSource === undefined)
      throw new Error("Production importer source is missing");
    expect(changed).not.toBe(probeSource);
    expect(
      sourceCode(changed, {
        name: probePath,
        importer: productionImporter,
        importerSource: productionImporterSource,
        read,
      }),
    ).toContain("!unresolved-executable");
  });
});

describe("auxiliary JavaScript type scope", () => {
  test.each([true, false, undefined])(
    "uses actual strict project membership %s rather than root globs",
    (typed) => {
      const snapshot = fixture();
      const name = "scripts/html/helper.js";
      snapshot.codeFiles.push(name);
      snapshot.tracked.add(name);
      snapshot.biomeIncludes.push("scripts/**/*.js");
      snapshot.typeIncludes.push("scripts/**/*.js");
      snapshot.javaScriptTyped = () => typed === true;
      expect(checkHarnessContract(snapshot).includes(`type scope missing: ${name}`)).toBe(
        typed !== true,
      );
    },
  );
  test("reports failed compiler authority without accepting JavaScript scope", () => {
    const snapshot = fixture();
    snapshot.declaredCode.push("scripts/html/helper.js");
    snapshot.javaScriptTyped = () => {
      throw new Error("changed compiler input");
    };
    expect(checkHarnessContract(snapshot)).toContain(
      "TypeScript project authority unavailable: scripts/html/helper.js: Error: changed compiler input",
    );
  });
  test("retains TypeScript include/exclude rejection despite auxiliary JavaScript reader", () => {
    const snapshot = fixture();
    snapshot.javaScriptTyped = () => true;
    snapshot.typeExcludes.push("scripts/html/helper.ts");
    expect(checkHarnessContract(snapshot)).toContain("type scope missing: scripts/html/helper.ts");
  });
});

function coverageFixtureJustfile(base: string): string {
  const actualJustfile = fs.readFileSync(path.join(import.meta.dir, "../../Justfile"), "utf8");
  const actualRecipes = justRecipes(actualJustfile);
  const actualLines = actualJustfile.split(/\r?\n/u);
  const factsDefinitionKeys = [
    "set shell",
    "REPO_ROOT",
    "RTK",
    "RTK_PROXY_CMD",
    "CARGO",
    "CARGO_BUILD",
  ] as const;
  const isDefinition = (line: string, key: (typeof factsDefinitionKeys)[number]): boolean => {
    if (key === "set shell") return line.startsWith("set shell :=");
    if (key === "CARGO") return line.startsWith("CARGO :=") || line.startsWith("export CARGO :=");
    return line.startsWith(`${key} :=`);
  };
  const factsDefinitions = factsDefinitionKeys.map((key) => {
    const matching = actualLines.filter((line) => isDefinition(line, key));
    const [line] = matching;
    if (
      matching.length !== 1 ||
      line === undefined ||
      (key === "CARGO" && !line.startsWith("export CARGO :="))
    )
      throw new Error(`current facts definition missing: ${key}`);
    return line;
  });
  const baseLines = base.split(/\r?\n/u);
  for (const [index, key] of factsDefinitionKeys.entries()) {
    const matching = baseLines.filter((line) => isDefinition(line, key));
    if (matching.length > 1 || (matching.length === 1 && matching[0] !== factsDefinitions[index]))
      throw new Error(`coverage fixture facts definition mismatch: ${key}`);
  }
  const missingFactsDefinitions = factsDefinitions.filter((line) => !baseLines.includes(line));
  const actualHeaders = new Map(
    [...actualJustfile.matchAll(/^([\w-]+(?:\s+[^:]*)?):/gm)].map((match) => [
      match[1]?.split(/\s+/, 1)[0] ?? "",
      match[1] ?? "",
    ]),
  );
  const runtimeRecipeNames = [
    "runtime-asset-script-aggregate-run",
    "runtime-asset-script-test",
    "capacity-admission",
    "rust-facts-host-build",
    "rust-facts-host-metadata",
    "mathjax-owned-evidence",
    "rust-facts-host-prebuild",
    "runtime-asset-script-preflight-run",
    "runtime-classic-generation-evidence",
    "runtime-asset-script-other-run",
    "runtime-unused-script-regression-run",
    "runtime-unused-script-final-verify-run",
    "runtime-script-cas-diagnostic",
    "runtime-script-cas-diagnostic-run",
    "runtime-unused-script-producer-run",
    "runtime-unused-script-verify-run",
    "runtime-unused-script-cache-key",
    "native-width-hash-facts",
    "native-width-classic-facts",
    "mermaid-classic-script-facts",
    "mathjax-consumer-facts",
    ...RUNTIME_UNUSED_LEAF_IDS.map((id) => `${id}-run`),
  ];
  const runtimeRecipes = runtimeRecipeNames.map((name) => {
    const body = actualRecipes.get(name);
    const header = actualHeaders.get(name);
    if (body === undefined || header === undefined)
      throw new Error(`current runtime recipe missing: ${name}`);
    return `${header}:\n${body
      .split("\n")
      .map((line) => `    ${line}`)
      .join("\n")}`;
  });
  return `${missingFactsDefinitions.join("\n")}${missingFactsDefinitions.length > 0 ? "\n" : ""}${base}
COVERAGE_MIN_LINES := env_var_or_default("COVERAGE_MIN_LINES", "100")
COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default("COVERAGE_MAX_UNCOVERED_LINES", "0")
COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX := "(^|[/\\\\])scripts[/\\\\]html[/\\\\]performance-harness[/\\\\]src[/\\\\]([^/\\\\]+[/\\\\])*[^/\\\\]+(_tests|_test_support)\\.rs$"
${runtimeRecipes.join("\n")}
native-coverage-cache-key producer:
    CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-key --producer {{quote(producer)}} --moon-fingerprint
root-test-and-coverage: plantuml-install
    CARGO={{quote(CARGO)}} CARGO_BUILD_JOBS={{quote(JOBS)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/release/native_nextest_root_coverage.py --repo "{{REPO_ROOT}}"
performance-harness-coverage:
    CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/hooks/run_locked_command.py --performance-coverage
[private]
performance-harness-coverage-run:
    mkdir -p "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage"
    python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-begin --producer performance
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-baseline" python3 scripts/build/cargo_capacity_guard.py --coverage-domain performance-baseline -- {{CARGO}} llvm-cov clean --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --locked
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-baseline" python3 scripts/build/cargo_capacity_guard.py --coverage-collection-domain performance-baseline -- {{CARGO}} llvm-cov --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --all-targets --no-default-features --features baseline --locked --profile krr-coverage --ignore-filename-regex {{quote(COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX)}} --json --output-path "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage/baseline.json"{{TEST_THREAD_ARGS}}
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-candidate" python3 scripts/build/cargo_capacity_guard.py --coverage-domain performance-candidate -- {{CARGO}} llvm-cov clean --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --locked
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-candidate" python3 scripts/build/cargo_capacity_guard.py --coverage-collection-domain performance-candidate -- {{CARGO}} llvm-cov --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --all-targets --no-default-features --features candidate --locked --profile krr-coverage --ignore-filename-regex {{quote(COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX)}} --json --output-path "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage/candidate.json"{{TEST_THREAD_ARGS}}
    python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-collect-finish --producer performance --producer-exit 0
    python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-collection-validate --producer performance
coverage-result-check:
    python3 scripts/release/native_coverage_local_aggregate.py --repo "{{REPO_ROOT}}"
`;
}

function localCoverageMoonFixture(): HarnessSnapshot {
  const snapshot = moonFixture();
  const root = path.resolve(import.meta.dir, "../..");
  const runtimeImportSources = runtimeUnusedImportInputs(root).sources.map((file) =>
    path.relative(root, file),
  );
  const canonicalRecipeSourcePaths = canonicalBunTestSourcePaths(root);
  const fixtureCrateScriptPaths = [
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/drawio/runtime_definition.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid/runtime_definition.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid/runtime_entrypoint.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/shared/dom_fragments.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/shared/runtime_bundle.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/zenuml/runtime_definition.ts",
  ];
  const pinnedSourcePaths = new Set([
    ...runtimeImportSources.filter((name) => !canonicalRecipeSourcePaths.has(name)),
    ...NATIVE_WIDTH_HASH_INPUTS,
    ...MATHJAX_UNUSED_INPUTS,
    "scripts/quality/duplication-check_test.ts",
    ...fixtureCrateScriptPaths,
    "scripts/quality/moon_cache_contract_test.py",
    "scripts/mermaid/browser-process-observe.py",
    "scripts/quality/duplication-schema.ts",
    "scripts/quality/owned-source-inventory.ts",
    "scripts/release/runtime_script_cache_cli.py",
    "scripts/runtime-bundles/bundle-runtime.ts",
    "scripts/runtime-bundles/runtime-bundle-cli.ts",
    "scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts",
    "scripts/runtime-bundles/runtime-rust-facts-host.ts",
    "scripts/runtime-bundles/runtime-rust-facts-host-inputs.ts",
    "scripts/runtime-bundles/runtime-rust-facts-host-protocol.ts",
    "scripts/runtime-bundles/runtime-source-provenance.ts",
    "scripts/runtime-bundles/runtime-unused-semantic-node.ts",
    "scripts/runtime-bundles/runtime-unused-check.ts",
    "scripts/mermaid/official-renderer.ts",
  ]);
  const syntheticSourcePaths = new Set(
    [...canonicalRecipeSourcePaths].filter((name) => !pinnedSourcePaths.has(name)),
  );
  for (const name of finitePinnedImportClosure(root, pinnedSourcePaths, syntheticSourcePaths))
    pinnedSourcePaths.add(name);
  for (const name of [...pinnedSourcePaths, ...canonicalRecipeSourcePaths])
    snapshot.tracked.add(name);
  const packageJson = z
    .record(z.string(), z.unknown())
    .parse(JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")));
  const packageDependencies = new Set<string>([
    ...Object.keys(z.record(z.string(), z.unknown()).parse(packageJson.dependencies ?? {})),
    ...Object.keys(z.record(z.string(), z.unknown()).parse(packageJson.devDependencies ?? {})),
  ]);
  coverageFixtureContexts.set(snapshot, {
    dependencies: packageDependencies,
    resolveCode: (reference, importer) =>
      resolveRepositoryCode(root, snapshot.policy.codeRoots, reference, importer),
  });
  const fixtureSourcePaths = new Set([
    ...Object.keys(moonRunContract(root).inputSha256),
    ...MOON_QUALITY_INPUTS,
    "scripts/build/cargo_capacity_guard.py",
    "scripts/hooks/local_review_lock.py",
    "scripts/hooks/run_locked_command.py",
    "scripts/html/helper.ts",
    "scripts/html/main.ts",
    "scripts/quality/duplication-check.ts",
    "scripts/quality/owned-validation-contract.ts",
    "scripts/release/native_coverage_local_aggregate.py",
    "scripts/release/native_coverage_result_cli.py",
    "scripts/release/native_nextest_root_coverage.py",
  ]);
  const fixtureRead = snapshot.read;
  snapshot.read = (name) => {
    const fixtureSource = fixtureRead(name);
    if (fixtureSource !== undefined) return fixtureSource;
    if (pinnedSourcePaths.has(name)) {
      const absolute = path.join(root, name);
      return fs.existsSync(absolute) && fs.statSync(absolute).isFile()
        ? fs.readFileSync(absolute, "utf8")
        : undefined;
    }
    if (canonicalRecipeSourcePaths.has(name))
      return 'import { test } from "bun:test"; test("coverage unit fixture", () => {});\n';
    if (!fixtureSourcePaths.has(name)) return undefined;
    const absolute = path.join(root, name);
    return fs.existsSync(absolute) && fs.statSync(absolute).isFile()
      ? fs.readFileSync(absolute, "utf8")
      : undefined;
  };
  if (!snapshot.policy.moon) throw new Error("fixture moon policy missing");
  snapshot.biomeIncludes.push(...fixtureCrateScriptPaths);
  snapshot.typeIncludes.push(...fixtureCrateScriptPaths);
  snapshot.javaScriptTyped = (name) =>
    name ===
    "crates/katana-render-runtime/src/renderer/backends/html_runtime/dom_bootstrap_test.js";
  snapshot.biomeIncludes.push(
    "crates/katana-render-runtime/src/renderer/backends/html_runtime/dom_bootstrap_test.js",
  );
  snapshot.typeIncludes.push(
    "crates/katana-render-runtime/src/renderer/backends/html_runtime/dom_bootstrap_test.js",
  );
  snapshot.policy.moon.entry = "coverage";
  snapshot.policy.moon.requiredHeavy.push("coverage", "unit-test", "runtime-asset-script-test");
  const actualTasks = z.record(z.string(), z.unknown()).parse(currentMoonConfig().tasks);
  const currentRuntimeTaskNames = [
    "unit-test",
    "runtime-asset-script-test",
    "runtime-classic-generation-evidence",
    "runtime-asset-script-preflight",
    "runtime-asset-script-other",
    "runtime-unused-script-regression",
    "runtime-unused-fragment-loader-producer",
    "runtime-unused-bundle-definitions-producer",
    "runtime-unused-check-producer",
    "runtime-unused-tool-fingerprint-producer",
    "runtime-unused-mathjax-producer",
    "runtime-unused-native-width-producer",
    "runtime-unused-rust-classic-producer",
    "performance-coverage",
    "coverage",
  ];
  changeMoon(snapshot, (tasks) => {
    const format = actualTasks["fmt-check"];
    if (!format || typeof format !== "object") throw new Error("current Moon fmt-check missing");
    const qualityInputs = z.object({ inputs: z.array(z.string()) }).parse(format).inputs;
    tasks["fmt-check"] = JSON.parse(JSON.stringify(format)) as TestMoonTask;
    tasks["heavy-native"] = {
      command: "noop",
      deps: ["runtime-asset-script-test"],
      inputs: [...qualityInputs],
    };
    tasks["html-test"] = {
      command: ["just", "html-test"],
      deps: ["static"],
      inputs: [...qualityInputs],
    };
    tasks.check = {
      command: "noop",
      deps: ["heavy-native", "html-test"],
      inputs: [...qualityInputs],
    };
    tasks["source-static-foundation"] = {
      command: "noop",
      deps: [
        "lint",
        "python-lint",
        "python-typecheck",
        "biome",
        "typecheck",
        "duplication-check",
        "owned-validation-contract-check",
      ],
      inputs: [...qualityInputs],
    };
    tasks["capacity-admission"] = {
      command: ["just", "capacity-admission"],
      deps: ["source-static-foundation"],
      inputs: [...qualityInputs],
    };
    tasks["rust-facts-host-prebuild"] = {
      command: ["just", "rust-facts-host-prebuild"],
      deps: ["capacity-admission"],
      inputs: [...qualityInputs],
    };
    tasks["mathjax-owned-evidence"] = {
      command: ["just", "mathjax-owned-evidence"],
      deps: ["static", "rust-facts-host-prebuild"],
      inputs: [...qualityInputs],
    };
    for (const name of currentRuntimeTaskNames) {
      const task = actualTasks[name];
      if (!task || typeof task !== "object") throw new Error(`current Moon task missing: ${name}`);
      tasks[name] = JSON.parse(JSON.stringify(task)) as TestMoonTask;
    }
  });
  replaceSources(snapshot, {
    [snapshot.policy.localBridge.source]: fs.readFileSync(
      path.join(import.meta.dir, "../hooks/run_parallel_checks.py"),
      "utf8",
    ),
    "scripts/html/harness-contract-cli.ts": fs.readFileSync(
      path.join(import.meta.dir, "harness-contract-cli.ts"),
      "utf8",
    ),
    [snapshot.policy.ciWorkflow]:
      "jobs:\n  test:\n    steps:\n      - run: bun run moon --concurrency 3 exec krr:coverage --on-failure continue --no-actions\n",
    Justfile: coverageFixtureJustfile(snapshot.read("Justfile") ?? ""),
    "scripts/build/cargo_capacity_guard.py": fs.readFileSync(
      path.join(root, "scripts/build/cargo_capacity_guard.py"),
      "utf8",
    ),
  });
  snapshot.tracked.add("scripts/build/cargo_capacity_guard.py");
  snapshot.tracked.add("scripts/release/native_coverage_result_cli.py");
  snapshot.tracked.add("scripts/release/native_coverage_local_aggregate.py");
  snapshot.tracked.add("scripts/release/native_coverage_local_anchor.py");
  snapshot.tracked.add("scripts/release/native_coverage_local_result.py");
  snapshot.tracked.add("scripts/release/native_nextest_root_coverage.py");
  for (const name of [...snapshot.graph.local, ...snapshot.graph.ci]) snapshot.tracked.add(name);
  return snapshot;
}

function canonicalBunTestSourcePaths(root: string): Set<string> {
  const paths = new Set<string>();
  const recipes = justRecipes(fs.readFileSync(path.join(root, "Justfile"), "utf8"));
  for (const body of recipes.values()) {
    for (const line of body.split("\n")) {
      const words = shellWords(line);
      const bun = words.indexOf("bun");
      if (bun < 0 || words[bun + 1] !== "test") continue;
      for (const word of words.slice(bun + 2)) {
        if (/^\.?\/?(?:scripts|crates)\/.*\.[cm]?[jt]sx?$/.test(word))
          paths.add(word.replace(/^\.\//, ""));
      }
    }
  }
  return paths;
}

function finitePinnedImportClosure(
  root: string,
  roots: Iterable<string>,
  synthetic: ReadonlySet<string>,
): Set<string> {
  const pending = [...roots];
  const closure = new Set<string>();
  while (pending.length > 0) {
    const name = pending.pop();
    if (!name || closure.has(name) || synthetic.has(name)) continue;
    const file = path.join(root, name);
    if (!fs.existsSync(file)) throw new Error(`pinned source missing: ${name}`);
    closure.add(name);
    for (const target of pinnedImportTargets(root, name, synthetic)) pending.push(target);
  }
  return closure;
}

function pinnedImportTargets(root: string, name: string, synthetic: ReadonlySet<string>): string[] {
  if (!/\.[cm]?[jt]sx?$/.test(name)) return [];
  const loader = name.endsWith(".tsx")
    ? "tsx"
    : name.endsWith(".jsx")
      ? "jsx"
      : /\.[cm]?ts$/.test(name)
        ? "ts"
        : "js";
  const file = path.join(root, name);
  const scanner = new Bun.Transpiler({ loader });
  const source = fs.readFileSync(file, "utf8");
  return pinnedImportReferences(source, scanner).flatMap((reference) =>
    resolvePinnedImport(root, file, reference, synthetic),
  );
}

function pinnedImportReferences(
  source: string,
  scanner: InstanceType<typeof Bun.Transpiler>,
): string[] {
  const references = new Set(scanner.scanImports(source).map((imported) => imported.path));
  const parsed = parseSync(source, { syntax: "typescript", tsx: true });
  for (const raw of parsed.body) {
    const node = raw as {
      type?: string;
      source?: { value?: unknown };
    };
    if (
      !["ImportDeclaration", "ExportNamedDeclaration", "ExportAllDeclaration"].includes(
        node.type ?? "",
      )
    )
      continue;
    if (typeof node.source?.value === "string") references.add(node.source.value);
  }
  return [...references];
}

function resolvePinnedImport(
  root: string,
  file: string,
  reference: string,
  synthetic: ReadonlySet<string>,
): string[] {
  if (!reference.startsWith(".") && !reference.startsWith("#")) return [];
  const resolved = fs.realpathSync(Bun.resolveSync(reference, path.dirname(file)));
  if (!resolved.startsWith(`${root}${path.sep}`))
    throw new Error(`pinned import escaped repository: ${reference}`);
  const target = path.relative(root, resolved).replaceAll(path.sep, "/");
  if (!target.startsWith("scripts/") && !target.startsWith("crates/"))
    throw new Error(`pinned import outside code roots: ${target}`);
  return synthetic.has(target) ? [] : [target];
}

function combinedCoverageMoonFixture(): HarnessSnapshot {
  const snapshot = localCoverageMoonFixture();
  return snapshot;
}

test("combined coverage producer binds the root and performance arms", () => {
  expect(checkHarnessContract(combinedCoverageMoonFixture())).toEqual([]);
});

type CoverageFixtureMutation = (snapshot: HarnessSnapshot) => void;

function replaceCoverageJustfile(
  snapshot: HarnessSnapshot,
  mutate: (source: string) => string,
): void {
  const before = snapshot.read("Justfile");
  if (before === undefined) throw new Error("Justfile fixture is missing");
  const after = mutate(before);
  expect(after).not.toBe(before);
  replaceSources(snapshot, { Justfile: after });
}

const combinedCoverageFaults: Array<[string, CoverageFixtureMutation]> = [
  ["old unit leaf", (snapshot) => changeUnitCoverageLeaf(snapshot)],
  ["coverage mask", (snapshot) => replaceCoverageJustfile(snapshot, maskRootCoverage)],
  ["missing PlantUML dependency", (snapshot) => replaceCoverageJustfile(snapshot, removePlantUml)],
  ["coverage threshold", (snapshot) => replaceCoverageJustfile(snapshot, lowerCoverageThreshold)],
  ["missing asset dependency", (snapshot) => removeAssetDependency(snapshot)],
  ["performance profile missing", (snapshot) => changePerformanceProfile(snapshot, "missing")],
  ["performance profile wrong", (snapshot) => changePerformanceProfile(snapshot, "wrong")],
  ["performance wrapper bypass", (snapshot) => bypassPerformanceCoverageWrapper(snapshot)],
];

function changeUnitCoverageLeaf(snapshot: HarnessSnapshot): void {
  changeMoon(snapshot, (tasks) => {
    const unit = tasks["unit-test"];
    if (!unit) throw new Error("fixture unit-test task missing");
    unit.command = ["just", "unit-test"];
  });
}

function removeAssetDependency(snapshot: HarnessSnapshot): void {
  changeMoon(snapshot, (tasks) => {
    const unit = tasks["unit-test"];
    if (!unit) throw new Error("fixture unit-test task missing");
    unit.deps = [];
  });
}

function maskRootCoverage(source: string): string {
  return source.replace(
    "python3 scripts/release/native_nextest_root_coverage.py",
    "echo python3 scripts/release/native_nextest_root_coverage.py",
  );
}

function removePlantUml(source: string): string {
  return source.replace("root-test-and-coverage: plantuml-install", "root-test-and-coverage:");
}

function lowerCoverageThreshold(source: string): string {
  return source.replace('"COVERAGE_MIN_LINES", "100"', '"COVERAGE_MIN_LINES", "99"');
}

function changePerformanceProfile(snapshot: HarnessSnapshot, profile: "missing" | "wrong"): void {
  replaceCoverageJustfile(snapshot, (source) =>
    source.replace(
      "--features baseline --locked --profile krr-coverage --ignore-filename-regex {{quote(COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX)}} --json",
      profile === "missing"
        ? "--features baseline --locked --ignore-filename-regex {{quote(COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX)}} --json"
        : "--features baseline --locked --profile wrong-profile --ignore-filename-regex {{quote(COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX)}} --json",
    ),
  );
}

function bypassPerformanceCoverageWrapper(snapshot: HarnessSnapshot): void {
  replaceCoverageJustfile(snapshot, (source) =>
    source.replace(
      "python3 scripts/hooks/run_locked_command.py --performance-coverage",
      "just performance-harness-coverage-run",
    ),
  );
}

test.each(combinedCoverageFaults)("combined coverage contract rejects %s", (_fault, mutate) => {
  const snapshot = combinedCoverageMoonFixture();
  mutate(snapshot);
  expect(checkHarnessContract(snapshot).length).toBeGreaterThan(0);
});

test.each([
  ["CARGO override", "CARGO={{quote(CARGO)}}", "CARGO=cargo"],
  ["bounded build jobs missing", "CARGO_BUILD_JOBS={{quote(JOBS)}} ", ""],
  ["bounded build jobs changed", "CARGO_BUILD_JOBS={{quote(JOBS)}}", "CARGO_BUILD_JOBS=1"],
  ["thread override", "TEST_THREADS={{quote(TEST_THREADS)}}", "TEST_THREADS=1"],
  ["repo override", '--repo "{{REPO_ROOT}}"', '--repo "tmp/repo"'],
])("root coverage delegation rejects %s", (_name, before, after) => {
  const snapshot = combinedCoverageMoonFixture();
  const justfile = snapshot.read("Justfile") ?? "";
  const recipe = justRecipes(justfile).get("root-test-and-coverage") ?? "";
  const bodyStart = recipe.indexOf("\n");
  expect(bodyStart).toBeGreaterThanOrEqual(0);
  const recipeBody = recipe.slice(bodyStart + 1);
  expect(recipeBody).toContain(before);
  expect(justfile).toContain(recipeBody);
  const changedBody = recipeBody.replace(before, after);
  expect(changedBody).not.toBe(recipeBody);
  const changedJustfile = justfile.replace(recipeBody, changedBody);
  expect(changedJustfile).not.toBe(justfile);
  expect(justRecipes(changedJustfile).get("root-test-and-coverage")).toBe(
    recipe.replace(before, after),
  );
  replaceSources(snapshot, { Justfile: changedJustfile });
  expect(checkHarnessContract(snapshot).length).toBeGreaterThan(0);
});

test("local coverage entry preserves static barrier and required heavy closure", () => {
  const snapshot = localCoverageMoonFixture();
  expect(checkHarnessContract(snapshot)).toEqual([]);
  const projection = moonStaticTaskProjection(snapshot.read("moon.yml") ?? "", "krr", "coverage");
  expect(moonProjectionClosure("performance-coverage", projection).has("unit-test")).toBe(false);
  expect(moonProjectionClosure("coverage", projection).has("performance-coverage")).toBe(true);
  expect(snapshot.graph.localRecipes?.has("performance-harness-coverage")).toBe(true);
  expect(snapshot.graph.localRecipes?.has("html-test")).toBe(true);
  expect(snapshot.graph.local.has("scripts/html/main.ts")).toBe(true);
  expect(snapshot.graph.local.has("scripts/quality/owned-validation-contract.ts")).toBe(true);
  const justfile = snapshot.read("Justfile") ?? "";
  expect(justfile).toContain(
    "performance-harness-coverage:\n    CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/hooks/run_locked_command.py --performance-coverage",
  );
  expect(justfile).toMatch(/\[private\]\s*\nperformance-harness-coverage-run:/);
});

test.each(RUNTIME_UNUSED_LEAF_IDS)(
  "runtime unused leaf rejects incorrect runner selection: %s",
  (id) => {
    const snapshot = localCoverageMoonFixture();
    changeMoon(snapshot, (tasks) => {
      const task = tasks[`${id}-producer`];
      if (!task) throw new Error(`runtime unused task missing: ${id}`);
      task.command = ["just", "runtime-unused-script-producer-run", id];
    });
    expect(checkHarnessContract(snapshot).length).toBeGreaterThan(0);
  },
);

test.each(["missing prerequisite", "single leaf runner", "cached batch"])(
  "runtime unused batch rejects %s",
  (mutation) => {
    const snapshot = localCoverageMoonFixture();
    changeMoon(snapshot, (tasks) => {
      const task = tasks["runtime-unused-script-regression"];
      if (!task) throw new Error("runtime unused batch missing");
      if (mutation === "missing prerequisite") task.deps = [];
      else if (mutation === "single leaf runner")
        task.command = ["just", "runtime-unused-script-verify-run", RUNTIME_UNUSED_LEAF_IDS[0]];
      else task.options = { ...task.options, cache: true };
    });
    expect(checkHarnessContract(snapshot)).toContain(
      mutation === "cached batch"
        ? "moon contract invalid: unsafe moon task options runtime-unused-script-regression"
        : "runtime unused final verification differs",
    );
  },
);

test.each(["omitted", "ignored", "reordered"])(
  "runtime script CAS outer recipe rejects %s prerequisite",
  (mutation) => {
    const snapshot = localCoverageMoonFixture();
    const justfile = snapshot.read("Justfile");
    if (justfile === undefined) throw new Error("Justfile fixture is missing");
    const body = justRecipes(justfile).get("runtime-script-cas-diagnostic");
    if (body === undefined) throw new Error("CAS outer recipe missing");
    const [prerequisite, diagnostic] = body.split("\n").filter((line) => line.trim() !== "");
    if (prerequisite === undefined || diagnostic === undefined)
      throw new Error("CAS outer recipe commands missing");
    expect(prerequisite.trim()).toBe("bun run moon run krr:runtime-unused-script-regression");
    expect(diagnostic.trim()).toBe("just runtime-script-cas-diagnostic-run");
    const expected = `${prerequisite}\n${diagnostic}\n`;
    expect(justfile).toContain(expected);
    const replacement =
      mutation === "omitted"
        ? `${diagnostic}\n`
        : mutation === "ignored"
          ? `${prerequisite} || true\n${diagnostic}\n`
          : `${diagnostic}\n${prerequisite}\n`;
    replaceSources(snapshot, { Justfile: justfile.replace(expected, replacement) });
    expect(checkHarnessContract(snapshot)).toContain("runtime script CAS outer recipe differs");
  },
);

test.each(["missing", "ignored"])(
  "local coverage route rejects %s collection authentication",
  (kind) => {
    const snapshot = localCoverageMoonFixture();
    const justfile = snapshot.read("Justfile");
    if (justfile === undefined) throw new Error("Justfile fixture is missing");
    const command =
      'python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-collection-validate --producer performance';
    expect(justfile).toContain(command);
    const replacement = kind === "missing" ? "" : `${command} || true`;
    replaceSources(snapshot, { Justfile: justfile.replace(command, replacement) });
    expect(checkHarnessContract(snapshot)).toContain(
      "performance collection authentication differs",
    );
  },
);

test.each(["root before performance", "artifact producer after root"])(
  "local coverage route rejects %s ordering",
  (mutation) => {
    const snapshot = localCoverageMoonFixture();
    changeMoon(snapshot, (tasks) => {
      if (mutation === "root before performance") {
        const root = tasks["unit-test"];
        if (!root) throw new Error("fixture root coverage task missing");
        root.deps = ["runtime-asset-script-test"];
        return;
      }
      const coverage = tasks.coverage;
      if (!coverage) throw new Error("fixture coverage task missing");
      tasks["late-artifact-cleanup"] = { command: ["just", "capacity-clean"], deps: ["unit-test"] };
      coverage.deps = ["unit-test", "late-artifact-cleanup"];
    });
    if (mutation === "root before performance") {
      expect(() =>
        moonStaticTaskProjection(snapshot.read("moon.yml") ?? "", "krr", "coverage"),
      ).toThrow();
      return;
    }
    const errors = checkHarnessContract(snapshot);
    expect(errors).toContain("moon coverage aggregation must follow root coverage directly");
  },
);

test.each(["old bridge", "old entry", "noop coverage", "redirected coverage", "barrier bypass"])(
  "local coverage route rejects %s",
  (kind) => {
    const snapshot = localCoverageMoonFixture();
    if (kind === "old bridge")
      replaceSources(snapshot, {
        [snapshot.policy.localBridge.source]:
          snapshot.read(snapshot.policy.localBridge.source)?.replace("krr:coverage", "krr:check") ??
          "",
      });
    else if (kind === "old entry") {
      if (!snapshot.policy.moon) throw new Error("fixture moon policy missing");
      snapshot.policy.moon.entry = "check";
      replaceSources(snapshot, {});
    } else
      changeMoon(snapshot, (tasks) => {
        const coverage = tasks.coverage;
        if (!coverage) throw new Error("fixture coverage task missing");
        if (kind === "barrier bypass") coverage.deps = ["static"];
        else coverage.command = kind === "noop coverage" ? "noop" : ["just", "html-test"];
      });
    expect(checkHarnessContract(snapshot).length).toBeGreaterThan(0);
  },
);

test("actual local bridge and policy select coverage after all shared Cargo consumers", () => {
  const root = path.resolve(import.meta.dir, "../..");
  const policy = z
    .object({
      moon: z.object({ entry: z.literal("coverage"), requiredHeavy: z.array(z.string()) }),
    })
    .parse(
      JSON.parse(
        fs.readFileSync(path.join(root, "scripts/html/harness-contract-policy.json"), "utf8"),
      ),
    );
  const config = z
    .object({
      tasks: z.object({
        coverage: z.object({
          command: z.tuple([z.literal("just"), z.literal("coverage-result-check")]),
          deps: z.tuple([z.literal("unit-test")]),
        }),
        "unit-test": z.object({ deps: z.tuple([z.literal("performance-coverage")]) }),
        check: z.object({ deps: z.array(z.string()) }),
      }),
    })
    .parse(Bun.YAML.parse(fs.readFileSync(path.join(root, "moon.yml"), "utf8")));
  expect(policy.moon.requiredHeavy).toContain("coverage");
  expect(config.tasks.coverage).toEqual({
    command: ["just", "coverage-result-check"],
    deps: ["unit-test"],
  });
  expect(config.tasks["unit-test"]?.deps).toEqual(["performance-coverage"]);
  expect(config.tasks.check?.deps).toContain("heavy-native");
  const bridge = fs.readFileSync(path.join(root, "scripts/hooks/run_parallel_checks.py"), "utf8");
  expect(bridge).toContain('"krr:coverage"');
  expect(bridge).not.toContain('"krr:check"');
  const justfile = fs.readFileSync(path.join(root, "Justfile"), "utf8");
  expect(justfile).toContain("just performance-harness-coverage");
  expect(justfile).toContain("python3 scripts/release/native_coverage_local_aggregate.py");
  expect(justfile).toContain("--coverage-domain performance-baseline");
  expect(justfile).toContain("--coverage-domain performance-candidate");
});

const terminalSourceMutations: Array<[string, string, (source: string) => string]> = [
  [
    "preflight bypass",
    "bridge",
    (source) => source.replace("if preflight.returncode != 0:", "if False:"),
  ],
  [
    "engine failure ignored",
    "bridge",
    (source) => source.replace("if result.returncode != 0:", "if False:"),
  ],
  [
    "verifier failure ignored",
    "bridge",
    (source) => source.replace("if verification.returncode != 0:", "if False:"),
  ],
  [
    "early success",
    "bridge",
    (source) => source.replace("        lock_path =", "        return 0\n        lock_path ="),
  ],
  [
    "dead verifier",
    "bridge",
    (source) =>
      source.replace(
        "            verification = subprocess.run(",
        "            return 0\n            verification = subprocess.run(",
      ),
  ],
  [
    "wrong stdin",
    "bridge",
    (source) => source.replace("input=verification_request", "input=preflight.stdout"),
  ],
  ["wrong verifier mode", "bridge", (source) => source.replace("--verify-moon-run", "--code-path")],
  [
    "wrong verifier source",
    "bridge",
    (source) =>
      source.replace("scripts/html/harness-contract-cli.ts", "scripts/html/fake-verifier.ts"),
  ],
  [
    "unconsumed proof",
    "bridge",
    (source) =>
      source.replace(
        "            _validate_success_proof(",
        "            return 0\n            _validate_success_proof(",
      ),
  ],
  [
    "proof helper overwritten",
    "bridge",
    (source) =>
      source.replace(
        "def run_checks(jobs: int)",
        "_validate_success_proof = lambda *args: None\n\ndef run_checks(jobs: int)",
      ),
  ],
  [
    "wrong report forwarded",
    "bridge",
    (source) => source.replace("                report_text,", "                preflight.stdout,"),
  ],
  [
    "Moon lock check removed",
    "cli",
    (source) => source.replace('  assertMoonLock(text("bun.lock"));', ""),
  ],
  [
    "fake report validator",
    "cli",
    (source) =>
      source.replace(
        "  validateCompletedReport(report, contract, repositoryIdentity(root).root, startedNs, finishedNs);",
        "  void contract;",
      ),
  ],
  [
    "fake success publisher",
    "cli",
    (source) =>
      source.replace(
        "    console.log(JSON.stringify(verifyMoonRun(root, request)));",
        '    const forged = verificationRequest.parse(request);\n    console.log(JSON.stringify({ schema: 1, status: "verified", reportSha256: forged.reportSha256, inputSha256: forged.inputSha256Before, head: forged.headBefore, startedNs: forged.startedNs, finishedNs: forged.finishedNs, reportMtimeNs: forged.reportMtimeNs }));',
      ),
  ],
];

test.each(terminalSourceMutations)("coverage source binding rejects %s", (_name, owner, mutate) => {
  const snapshot = localCoverageMoonFixture();
  expect(checkHarnessContract(snapshot)).toEqual([]);
  const name =
    owner === "bridge"
      ? snapshot.policy.localBridge.source
      : "scripts/html/harness-contract-cli.ts";
  const original = snapshot.read(name);
  if (!original) throw new Error("canonical Moon producer source is missing");
  const changed = mutate(original);
  expect(changed).not.toBe(original);
  replaceSources(snapshot, { [name]: changed });
  expect(checkHarnessContract(snapshot)).toContain(
    "local terminal report producer binding differs",
  );
});

test("coverage binding preserves CRLF and rejects a missing real CLI producer", () => {
  const snapshot = localCoverageMoonFixture();
  const bridge = snapshot.read(snapshot.policy.localBridge.source);
  const cli = snapshot.read("scripts/html/harness-contract-cli.ts");
  if (!bridge || !cli) throw new Error("canonical Moon sources are missing");
  replaceSources(snapshot, {
    [snapshot.policy.localBridge.source]: bridge.replaceAll("\n", "\r\n"),
    "scripts/html/harness-contract-cli.ts": cli.replaceAll("\n", "\r\n"),
  });
  expect(checkHarnessContract(snapshot)).toEqual([]);
  replaceSources(snapshot, { "scripts/html/harness-contract-cli.ts": "" });
  expect(checkHarnessContract(snapshot)).toContain(
    "local terminal report producer binding differs",
  );
});

type TerminalAction = {
  status: string;
  allowFailure: boolean;
  expectFailure: boolean;
  nodeIndex: number;
  node: {
    action: string;
    params: {
      target: string;
      args: string[];
      env: Record<string, string>;
      interactive: boolean;
      persistent: boolean;
    };
  };
  startedAt: string;
  finishedAt: string;
  operations: Array<{ status: string; meta: { type: string; exitCode?: number; hash?: string } }>;
};
type TerminalReport = {
  status: string;
  context: {
    initialTargets: string[];
    primaryTargets: string[];
    affected: null;
    passthroughArgs: string[];
    targetStates: Record<string, { state: string; hash?: string }>;
  };
  actions: TerminalAction[];
};
type TerminalFixture = {
  root: string;
  contract: ReturnType<typeof moonRunContract>;
  report: TerminalReport;
  request: {
    schema: number;
    entry?: "coverage" | "runtime-bundle-check";
    preflightText: string;
    reportText: string;
    reportSha256: string;
    inputSha256Before: ReturnType<typeof moonRunContract>["inputSha256"];
    inputSha256After: ReturnType<typeof moonRunContract>["inputSha256"];
    headBefore: string;
    headAfter: string;
    startedNs: string;
    finishedNs: string;
    reportMtimeNs: string;
  };
};

function terminalTimestamp(ns: bigint): string {
  return `${new Date(Number(ns / 1_000_000_000n) * 1000).toISOString().slice(0, 19)}.${String(ns % 1_000_000_000n).padStart(9, "0")}`;
}
function terminalAction(fixture: TerminalFixture, target?: string): TerminalAction {
  const action = target
    ? fixture.report.actions.find((action) => action.node.params.target === target)
    : fixture.report.actions[0];
  if (!action) throw new Error("terminal fixture action is missing");
  return action;
}
function withTerminalFixture(
  action: (fixture: TerminalFixture) => void,
  entry: "coverage" | "runtime-bundle-check" = "coverage",
): void {
  withModulePackage((root) => {
    const repository = path.resolve(import.meta.dir, "../..");
    const authority = moonRunContract(repository).inputSha256;
    for (const name of Object.keys(authority)) {
      const destination = path.join(root, name);
      fs.mkdirSync(path.dirname(destination), { recursive: true });
      fs.writeFileSync(destination, fs.readFileSync(path.join(repository, name)));
    }
    expect(Bun.spawnSync(["git", "init", "--quiet"], { cwd: root }).exitCode).toBe(0);
    // 隔離fixtureの履歴に、ユーザーの署名・hook設定を持ち込まない。
    expect(
      Bun.spawnSync(
        [
          "git",
          "-c",
          "user.name=Moon Contract Unit",
          "-c",
          "user.email=unit@example.invalid",
          "-c",
          "commit.gpgsign=false",
          "-c",
          "core.hooksPath=",
          "commit",
          "--allow-empty",
          "--quiet",
          "-m",
          "Moon contract fixture",
        ],
        { cwd: root },
      ).exitCode,
    ).toBe(0);
    const git = Bun.spawnSync(["git", "--no-replace-objects", "rev-parse", "HEAD"], { cwd: root });
    expect(git.exitCode).toBe(0);
    const head = git.stdout.toString().trim();
    const contract = moonRunContract(root, entry);
    const order: string[] = [];
    const visit = (name: string): void => {
      if (order.includes(name)) return;
      const task = contract.tasks[name];
      if (!task) throw new Error("terminal fixture task is missing");
      task.deps.forEach(visit);
      order.push(name);
    };
    visit(entry);
    const base = BigInt(Date.now()) * 1_000_000n;
    const report: TerminalReport = {
      status: "completed",
      context: {
        initialTargets: [`krr:${entry}`],
        primaryTargets: [`krr:${entry}`],
        affected: null,
        passthroughArgs: [],
        targetStates: Object.fromEntries(
          order.map((name) => [`krr:${name}`, { state: "passed", hash: "1".repeat(64) }]),
        ),
      },
      actions: order.map((name, index) => ({
        status: "passed",
        allowFailure: false,
        expectFailure: false,
        nodeIndex: index,
        node: {
          action: "run-task",
          params: {
            target: `krr:${name}`,
            args: [],
            env: {},
            interactive: false,
            persistent: false,
          },
        },
        startedAt: terminalTimestamp(base + BigInt(index + 1) * 10_000n),
        finishedAt: terminalTimestamp(base + BigInt(index + 1) * 10_000n + 1000n),
        operations: [
          {
            status: "passed",
            meta:
              contract.tasks[name]?.leaf === null
                ? { type: "no-operation" }
                : { type: "task-execution", exitCode: 0 },
          },
        ],
      })),
    };
    action({
      root,
      contract,
      report,
      request: {
        schema: 1,
        entry,
        preflightText: JSON.stringify({
          schema: 1,
          staticContractOnly: true,
          errors: [],
          moonRunContract: contract,
        }),
        reportText: "",
        reportSha256: "",
        inputSha256Before: { ...contract.inputSha256 },
        inputSha256After: { ...contract.inputSha256 },
        headBefore: head,
        headAfter: head,
        startedNs: String(base),
        finishedNs: String(base + 1_000_000n),
        reportMtimeNs: String(base + 999999n),
      },
    });
  });
}
function terminalRequest(fixture: TerminalFixture) {
  const reportText = fixture.request.reportText || JSON.stringify(fixture.report);
  return {
    ...fixture.request,
    reportText,
    reportSha256:
      fixture.request.reportSha256 || createHash("sha256").update(reportText).digest("hex"),
  };
}

const terminalValidationCases: Array<[string, (fixture: TerminalFixture) => void, boolean]> = [
  ["synthetic completed exact closure", () => {}, true],
  ...["failed", "skipped", "cached", "cached-from-remote"].map(
    (status): [string, (fixture: TerminalFixture) => void, boolean] => [
      `dependency ${status}`,
      (fixture) => {
        terminalAction(fixture).status = status;
      },
      false,
    ],
  ),
  [
    "expected failure exit one",
    (fixture) => {
      const action = terminalAction(fixture);
      action.expectFailure = true;
      action.operations = [{ status: "passed", meta: { type: "task-execution", exitCode: 1 } }];
    },
    false,
  ],
  [
    "target state skipped",
    (fixture) => {
      fixture.report.context.targetStates[terminalAction(fixture).node.params.target] = {
        state: "skipped",
      };
    },
    false,
  ],
  [
    "duplicate action",
    (fixture) => {
      fixture.report.actions.push(structuredClone(terminalAction(fixture)));
    },
    false,
  ],
  [
    "partial actions",
    (fixture) => {
      fixture.report.actions.pop();
    },
    false,
  ],
  [
    "hash operation only",
    (fixture) => {
      terminalAction(fixture).operations = [
        { status: "passed", meta: { type: "hash-generation" } },
      ];
    },
    false,
  ],
  [
    "non-approved leaf cached",
    (fixture) => {
      const action = terminalAction(fixture, "krr:coverage");
      action.status = "cached";
      action.operations = [
        { status: "passed", meta: { type: "hash-generation", hash: "1".repeat(64) } },
        { status: "cached", meta: { type: "task-execution" } },
      ];
    },
    false,
  ],
  [
    "noop is execution",
    (fixture) => {
      const action = fixture.report.actions.find(
        (action) => fixture.contract.tasks[action.node.params.target.slice(4)]?.leaf === null,
      );
      if (!action) throw new Error("noop fixture is missing");
      action.operations = [{ status: "passed", meta: { type: "task-execution", exitCode: 0 } }];
    },
    false,
  ],
  [
    "auxiliary operation tail",
    (fixture) => {
      terminalAction(fixture).operations.push({
        status: "skipped",
        meta: { type: "hash-generation" },
      });
    },
    true,
  ],
  [
    "dependency order one nanosecond",
    (fixture) => {
      const consumer = terminalAction(fixture, "krr:source-static");
      const deps = fixture.contract.tasks["source-static"]?.deps;
      if (!deps) throw new Error("source-static fixture is missing");
      const last = fixture.report.actions
        .filter((action) => deps.includes(action.node.params.target.slice(4)))
        .sort((left, right) => left.finishedAt.localeCompare(right.finishedAt))
        .at(-1);
      if (!last) throw new Error("dependency fixture is missing");
      consumer.startedAt = `${last.finishedAt.slice(0, -9)}${String(BigInt(last.finishedAt.slice(-9)) - 1n).padStart(9, "0")}`;
    },
    false,
  ],
  [
    "invalid calendar day",
    (fixture) => {
      terminalAction(fixture).startedAt = "2026-02-30T00:00:00.000000000";
    },
    false,
  ],
  [
    "interrupted zero engine simulation",
    (fixture) => {
      fixture.report.status = "interrupted";
    },
    false,
  ],
  [
    "R22 interrupted skipped excerpt",
    (fixture) => {
      fixture.report.status = "interrupted";
      fixture.report.context.initialTargets = ["krr:source-static", "krr:fmt-check"];
      fixture.report.context.primaryTargets = ["krr:source-static", "krr:fmt-check"];
      fixture.report.actions = fixture.report.actions
        .slice(0, 8)
        .map((action) => ({ ...action, status: "skipped", operations: [] }));
    },
    false,
  ],
  [
    "wrong report SHA",
    (fixture) => {
      fixture.request.reportSha256 = "0".repeat(64);
    },
    false,
  ],
  [
    "wrong current HEAD",
    (fixture) => {
      fixture.request.headBefore = "0".repeat(40);
      fixture.request.headAfter = "0".repeat(40);
    },
    false,
  ],
  [
    "wrong current authority",
    (fixture) => {
      fixture.request.inputSha256Before["moon.yml"] = "0".repeat(64);
      fixture.request.inputSha256After["moon.yml"] = "0".repeat(64);
    },
    false,
  ],
  [
    "unknown request field",
    (fixture) => {
      Object.assign(fixture.request, { reportPath: "private.html" });
    },
    false,
  ],
  [
    "negative nanoseconds",
    (fixture) => {
      fixture.request.startedNs = "-1";
    },
    false,
  ],
  [
    "forged projection",
    (fixture) => {
      const changed = structuredClone(fixture.contract);
      delete changed.tasks[terminalAction(fixture).node.params.target.slice(4)];
      fixture.request.preflightText = JSON.stringify({
        schema: 1,
        staticContractOnly: true,
        errors: [],
        moonRunContract: changed,
      });
    },
    false,
  ],
];

test("native local cache hit requires the uncached aggregate and a physical current receipt", () => {
  withTerminalFixture((fixture) => {
    const action = terminalAction(fixture, "krr:unit-test");
    action.status = "cached";
    const targetHash = fixture.report.context.targetStates[action.node.params.target]?.hash;
    if (!targetHash) throw new Error("native cached target hash is missing");
    action.operations = [
      { status: "cached", meta: { type: "hash-generation", hash: targetHash } },
      { status: "cached", meta: { type: "output-hydration" } },
    ];
    expect(() => verifyMoonRun(fixture.root, terminalRequest(fixture))).toThrow(
      "local native coverage receipt validation failed: root",
    );
  });
});

test("native local cache hit is rejected when aggregate validation is cached", () => {
  withTerminalFixture((fixture) => {
    const producer = terminalAction(fixture, "krr:unit-test");
    producer.status = "cached";
    const targetHash = fixture.report.context.targetStates[producer.node.params.target]?.hash;
    if (!targetHash) throw new Error("native cached target hash is missing");
    producer.operations = [
      { status: "cached", meta: { type: "hash-generation", hash: targetHash } },
      { status: "cached", meta: { type: "output-hydration" } },
    ];
    const aggregate = terminalAction(fixture, "krr:coverage");
    aggregate.status = "cached";
    aggregate.operations = [
      { status: "cached", meta: { type: "hash-generation", hash: "1".repeat(64) } },
      { status: "cached", meta: { type: "output-hydration" } },
    ];
    expect(() => verifyMoonRun(fixture.root, terminalRequest(fixture))).toThrow(
      "cached Moon action is not authorized by the selected leaf contract",
    );
  });
});

test("runtime bundle named entry binds the one cache-enabled fingerprint leaf", () => {
  const source = fs.readFileSync(path.join(import.meta.dir, "../../moon.yml"), "utf8");
  const config = Bun.YAML.parse(source) as {
    tasks: Record<string, Record<string, unknown>>;
  };
  const projection = moonStaticTaskProjection(source, "krr", "runtime-bundle-check");
  expect(Object.keys(projection)).toEqual(["runtime-bundle-check"]);
  expect(projection["runtime-bundle-check"]).toMatchObject({
    deps: [],
    leaf: "runtime-bundle-check",
    options: {
      cache: true,
      shell: false,
      allowFailure: false,
      expectFailure: false,
      runInCI: "always",
    },
  });
  expect(config.tasks["runtime-bundle-check"]?.inputs).toEqual([
    "**/*",
    "!/docs/quality-gate-improvement-tasks.md",
    "!/tmp/**",
    "!/.moon/cache/**",
    "!/target/**",
    "!/crates/katana-render-runtime/target/**",
    "!**/.ruff_cache/**",
    "!**/.mypy_cache/**",
    "!**/.pytest_cache/**",
    "!/tests/fixtures/quality-cache/.moon/cache/**",
    "!/tests/fixtures/quality-cache/.outputs/**",
  ]);

  const candidate = structuredClone(config);
  const runtimeTask = candidate.tasks["runtime-bundle-check"];
  if (!runtimeTask) throw new Error("runtime bundle task missing");
  runtimeTask.checks = [
    {
      check: "fingerprint",
      script: "bun scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts",
      hash: "stdout",
    },
  ];
  runtimeTask.options = { cache: false };
  expect(() =>
    moonStaticTaskProjection(JSON.stringify(candidate), "krr", "runtime-bundle-check"),
  ).toThrow("runtime bundle cache task options changed");

  runtimeTask.options = { cache: true };
  runtimeTask.inputs = ["**/*", "!/tmp/**", "!/.moon/cache/**"];
  expect(() =>
    moonStaticTaskProjection(JSON.stringify(candidate), "krr", "runtime-bundle-check"),
  ).toThrow("runtime bundle cache inputs changed");
  const ledgerExclusion = "!/docs/quality-gate-improvement-tasks.md";
  const outputExclusion = "!/tests/fixtures/quality-cache/.outputs/**";
  const invalidInputProjections = [
    MOON_QUALITY_INPUTS.filter((input) => input !== ledgerExclusion),
    [
      MOON_QUALITY_INPUTS[0],
      "!/docs/**",
      ...MOON_QUALITY_INPUTS.slice(1).filter((input) => input !== ledgerExclusion),
    ],
    MOON_QUALITY_INPUTS.filter((input) => input !== outputExclusion),
  ];
  for (const inputs of invalidInputProjections) {
    runtimeTask.inputs = inputs;
    expect(() =>
      moonStaticTaskProjection(JSON.stringify(candidate), "krr", "runtime-bundle-check"),
    ).toThrow("runtime bundle cache inputs changed");
  }
  runtimeTask.inputs = config.tasks["runtime-bundle-check"]?.inputs;
  runtimeTask.checks = [{ check: "unknown", script: "true", hash: "stdout" }];
  expect(() =>
    moonStaticTaskProjection(JSON.stringify(candidate), "krr", "runtime-bundle-check"),
  ).toThrow("runtime bundle cache fingerprint check changed");

  const otherTask = candidate.tasks.lint;
  if (!otherTask) throw new Error("lint task missing");
  otherTask.options = { cache: true };
  expect(() =>
    moonStaticTaskProjection(JSON.stringify(candidate), "krr", "runtime-bundle-check"),
  ).toThrow("unsafe moon task options lint");
});

test("Moon cache manifest binds the observed string version", () => {
  const fingerprint = {
    schema: "krr-runtime-bundle-tool-fingerprint-v1",
    platform: "darwin",
    arch: "arm64",
    tools: {
      bun: { version: "1.4.4", executableSha256: "1".repeat(64) },
      just: { version: "1.46.0", executableSha256: "2".repeat(64) },
      node: { version: "v24.0.0", executableSha256: "3".repeat(64) },
      bash: { version: "5.2", executableSha256: "4".repeat(64) },
      moon: {
        version: "2.6.5",
        executableSha256: "5".repeat(64),
        wrapperSha256: "6".repeat(64),
        utilsSha256: "7".repeat(64),
        packageSha256: "8".repeat(64),
      },
    },
    environment: [],
  };
  const manifest = (version: unknown) => [
    {
      args: ["runtime-bundle-check"],
      command: "just",
      deps: {},
      env: {},
      inputEnv: {},
      inputs: {},
      outputs: [],
      projectDeps: [],
      target: "krr:runtime-bundle-check",
      toolchains: ["system"],
      version,
    },
    {
      checks: {
        "bun scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts": {
          stdout: JSON.stringify(fingerprint),
        },
      },
    },
  ];
  expect(() => validateRuntimeBundleCacheArray(manifest("3"))).not.toThrow();
  expect(() => validateRuntimeBundleCacheArray(manifest(3))).toThrow(
    "Moon hash manifest task binding changed",
  );
  expect(() => validateRuntimeBundleCacheArray(manifest("3").slice(0, 1))).toThrow(
    "unknown Moon hash manifest shape",
  );
  const extraFingerprint = manifest("3");
  const checkEntry = extraFingerprint[1] as { checks: Record<string, unknown> } | undefined;
  if (!checkEntry) throw new Error("check fingerprint missing");
  checkEntry.checks["another-script"] = { stdout: "{}" };
  expect(() => validateRuntimeBundleCacheArray(extraFingerprint)).toThrow(
    "Moon task check fingerprint binding changed",
  );
  const extraOutput = manifest("3");
  const outputEntry = extraOutput[1] as
    | { checks: Record<string, Record<string, unknown>> }
    | undefined;
  if (!outputEntry) throw new Error("check fingerprint missing");
  const fingerprintOutput =
    outputEntry.checks["bun scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts"];
  if (!fingerprintOutput) throw new Error("fingerprint output missing");
  fingerprintOutput.stderr = "unexpected";
  expect(() => validateRuntimeBundleCacheArray(extraOutput)).toThrow(
    "Moon task check fingerprint output changed",
  );
  const invalidFingerprint = manifest("3");
  const invalidEntry = invalidFingerprint[1] as
    | { checks: Record<string, { stdout: string }> }
    | undefined;
  if (!invalidEntry) throw new Error("check fingerprint missing");
  invalidEntry.checks["bun scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts"] = {
    stdout: "{}",
  };
  expect(() => validateRuntimeBundleCacheArray(invalidFingerprint)).toThrow();
  const blob = Buffer.from(JSON.stringify(manifest("3")));
  const taskHash = createHash("sha256").update(blob).digest("hex");
  expect(() => validateRuntimeBundleCacheBlob(blob, taskHash)).not.toThrow();
  expect(() =>
    validateRuntimeBundleCacheBlob(Buffer.concat([blob, Buffer.from(" ")]), taskHash),
  ).toThrow("Moon hash manifest content hash differs from task hash");
});

test.each(terminalValidationCases)("private Moon CLI validates %s", (_name, mutate, succeeds) => {
  withTerminalFixture((fixture) => {
    mutate(fixture);
    const request = terminalRequest(fixture);
    const child = Bun.spawnSync(
      ["bun", path.join(import.meta.dir, "harness-contract-cli.ts"), "--verify-moon-run"],
      { cwd: fixture.root, stdin: Buffer.from(JSON.stringify(request)), timeout: 4000 },
    );
    expect(child.exitCode).toBe(succeeds ? 0 : 1);
    if (succeeds) {
      const proof: unknown = JSON.parse(child.stdout.toString());
      expect(proof).toEqual({
        schema: 1,
        status: "verified",
        reportSha256: request.reportSha256,
        inputSha256: request.inputSha256Before,
        head: request.headBefore,
        startedNs: request.startedNs,
        finishedNs: request.finishedNs,
        reportMtimeNs: request.reportMtimeNs,
      });
    } else {
      expect(child.stdout.toString()).toBe("");
      expect(child.stderr.toString()).toMatch(/Error/);
    }
  });
});

test("runtime bundle cache leaf is authorized in the coverage closure but synthetic proof is rejected", () => {
  withTerminalFixture((fixture) => {
    expect(runtimeBundleCacheActionAuthorized(fixture.contract, "krr:runtime-bundle-check")).toBe(
      true,
    );
    const action = terminalAction(fixture, "krr:runtime-bundle-check");
    action.status = "cached";
    action.operations = [
      { status: "passed", meta: { type: "hash-generation", hash: "1".repeat(64) } },
      { status: "cached", meta: { type: "task-execution" } },
    ];
    const request = terminalRequest(fixture);
    expect(() => verifyMoonRun(fixture.root, request)).toThrow(
      "cached Moon report is not the current on-disk run report",
    );
  });
});

test.each([
  ["workspace version", ".moon/workspace.yml", "=2.6.5", "=2.6.4"],
  ["manifest version", "package.json", '"@moonrepo/cli": "2.6.5"', '"@moonrepo/cli": "2.6.4"'],
  ["lock workspace version", "bun.lock", '"@moonrepo/cli": "2.6.5"', '"@moonrepo/cli": "2.6.4"'],
  ["lock executable version", "bun.lock", "@moonrepo/cli@2.6.5", "@moonrepo/cli@2.6.4"],
  [
    "lock platform version",
    "bun.lock",
    "@moonrepo/core-macos-arm64@2.6.5",
    "@moonrepo/core-macos-arm64@2.6.4",
  ],
] as const)("Moon run contract rejects mismatched %s", (_label, name, original, replacement) => {
  withTerminalFixture((fixture) => {
    const file = path.join(fixture.root, name);
    const source = fs.readFileSync(file, "utf8");
    const changed = source.replace(original, replacement);
    expect(changed).not.toBe(source);
    fs.writeFileSync(file, changed);
    expect(() => moonRunContract(fixture.root)).toThrow();
  });
});

test.each([
  [
    "additional exclusion",
    (source: string) =>
      source.replace('    - "!/.moon/cache/**"', '    - "!/.moon/cache/**"\n    - "!/target/**"'),
  ],
  ["broader cache exclusion", (source: string) => source.replace("!/.moon/cache/**", "!/.moon/**")],
  [
    "additional hasher setting",
    (source: string) =>
      source.replace(
        "hasher:\n  ignorePatterns:",
        "hasher:\n  cacheDirectory: .moon/cache\n  ignorePatterns:",
      ),
  ],
] as const)("Moon run contract rejects %s", (_mutation, mutate) => {
  withTerminalFixture((fixture) => {
    const workspacePath = path.join(fixture.root, ".moon/workspace.yml");
    const source = fs.readFileSync(workspacePath, "utf8");
    const changed = mutate(source);
    expect(changed).not.toBe(source);
    fs.writeFileSync(workspacePath, changed);
    expect(() => moonRunContract(fixture.root)).toThrow();
  });
});

test("private Moon CLI defaults omitted request entry to coverage", () => {
  withTerminalFixture((fixture) => {
    const workspacePath = path.join(fixture.root, ".moon/workspace.yml");
    const workspaceSha = createHash("sha256").update(fs.readFileSync(workspacePath)).digest("hex");
    expect(fixture.contract.inputSha256[".moon/workspace.yml"]).toBe(workspaceSha);
    delete fixture.request.entry;
    expect(verifyMoonRun(fixture.root, terminalRequest(fixture)).status).toBe("verified");
  });
});

test("private Moon CLI rejects unknown arguments and a repository subdirectory", () => {
  withTerminalFixture((fixture) => {
    const request = terminalRequest(fixture);
    expect(() => verifyMoonRun(fixture.root, request)).not.toThrow();
    const unknown = Bun.spawnSync(
      [
        "bun",
        path.join(import.meta.dir, "harness-contract-cli.ts"),
        "--verify-moon-run",
        "--report-path",
        "private.html",
      ],
      { cwd: fixture.root, stdin: Buffer.from(JSON.stringify(request)), timeout: 4000 },
    );
    expect(unknown.exitCode).toBe(1);
    expect(unknown.stdout.toString()).toBe("");
    const subdirectory = Bun.spawnSync(
      ["bun", path.join(import.meta.dir, "harness-contract-cli.ts"), "--verify-moon-run"],
      {
        cwd: path.join(fixture.root, "scripts"),
        stdin: Buffer.from(JSON.stringify(request)),
        timeout: 4000,
      },
    );
    expect(subdirectory.exitCode).toBe(1);
    expect(subdirectory.stdout.toString()).toBe("");
    expect(subdirectory.stderr.toString()).toContain("physical Git repository root");
  });
});

test.each([
  ["unknown command string", 'Bun.spawnSync("unknown-tool --version");', false],
  [
    "unknown exec command",
    'import { execSync } from "node:child_process"; execSync("unknown-tool --version");',
    false,
  ],
  ["unknown head", 'Bun.spawnSync([getBinary(), "--version"]);', false],
  ["unknown tool", 'Bun.spawnSync(["unknown-tool", "--version"]);', false],
  ["external spread", 'Bun.spawnSync(["bunx", ...getArgs()]);', false],
  ["external tool missing", 'Bun.spawnSync(["bunx", "--no-install", getTool()]);', false],
  ["different package", 'Bun.spawnSync(["bunx", "--no-install", "other-tool"]);', false],
  ["assignment compound", 'let argv; Bun.spawnSync(argv += ["git", "status"]);', false],
  ["assignment member", 'const state = {}; Bun.spawnSync(state.argv = ["git", "status"]);', false],
  [
    "assignment nested unknown",
    'let argv, other; Bun.spawnSync(argv = other = ["git", "status"]);',
    false,
  ],
  ["assignment spread", 'let argv; Bun.spawnSync(argv = ["bunx", ...args]);', false],
  ["fixed Git", 'Bun.spawnSync(["git", "ls-files", "--cached", "-z"]);', true],
  ["fixed image tool data", 'Bun.spawnSync(["magick", "compare", input, output]);', true],
  ["fixed Biome", 'Bun.spawnSync(["bunx", "--no-install", "biome", "lint", file]);', true],
  [
    "fixed TypeScript",
    'Bun.spawnSync(["bunx", "--no-install", "tsc", "--showConfig", config]);',
    true,
  ],
  ["fixed Just version probe", 'Bun.spawnSync(["just", "--version"]);', true],
  ["fixed Bash version probe", 'Bun.spawnSync(["bash", "--version"]);', true],
  ["fixed Node version probe", 'Bun.spawnSync(["node", "--version"]);', true],
  [
    "fixed Moon wrapper version probe",
    'Bun.spawnSync(["node", "node_modules/@moonrepo/cli/moon.js", "--version"]);',
    true,
  ],
  [
    "fixed Playwright Test runner",
    'Bun.spawnSync(["node", "node_modules/playwright/cli.js", "test", "--config", "scripts/html/playwright.config.ts"]);',
    true,
  ],
  ["Just version extra argument", 'Bun.spawnSync(["just", "--version", "--verbose"]);', false],
  ["Bash version wrong flag", 'Bun.spawnSync(["bash", "-version"]);', false],
  ["Node version extra argument", 'Bun.spawnSync(["node", "--version", "--json"]);', false],
  [
    "Moon wrapper path drift",
    'Bun.spawnSync(["node", "node_modules/other/moon.js", "--version"]);',
    false,
  ],
  [
    "Moon wrapper extra argument",
    'Bun.spawnSync(["node", "node_modules/@moonrepo/cli/moon.js", "--version", "--json"]);',
    false,
  ],
  [
    "Moon wrapper spread argument",
    'Bun.spawnSync(["node", "node_modules/@moonrepo/cli/moon.js", ...args]);',
    false,
  ],
  [
    "Playwright runner extra argument",
    'Bun.spawnSync(["node", "node_modules/playwright/cli.js", "test", "--config", "scripts/html/playwright.config.ts", "--grep", "catalog"]);',
    false,
  ],
  [
    "Playwright runner config drift",
    'Bun.spawnSync(["node", "node_modules/playwright/cli.js", "test", "--config", "scripts/html/other.config.ts"]);',
    false,
  ],
  ["dynamic metadata executable", 'Bun.spawnSync([executable, "--version"]);', false],
  [
    "unsupported argv assignment",
    'let argv; Bun.spawnSync(argv = ["bunx", "--no-install", "biome", "lint", file]);',
    false,
  ],
])("external command boundary %s", (_name, source, accepted) => {
  expect(sourceCode(source).includes("!unresolved-executable")).toBe(!accepted);
});

test("Playwright Test boundary returns only owned runner inputs", () => {
  const references = sourceCode(
    'Bun.spawnSync(["node", "node_modules/playwright/cli.js", "test", "--config", "scripts/html/playwright.config.ts"]);',
  );
  expect(references).toEqual([
    "!execute-local:scripts/html/playwright.config.ts",
    "!execute-local:scripts/html/feature-catalog_test.ts",
    "!execute-local:scripts/html/chrome_badge_metrics_test.ts",
    "!execute-local:scripts/html/reference-capture_browser_test.ts",
  ]);
});

test("root Just recipe registers the exact Playwright runner inputs", () => {
  const snapshot = fixture();
  const prior = snapshot.read;
  snapshot.read = (name) =>
    name === "Justfile"
      ? (prior(name) ?? "").replace(
          "bun scripts/html/main.ts --output tmp/report.json",
          "node node_modules/playwright/cli.js test --config scripts/html/playwright.config.ts",
        )
      : prior(name);
  snapshot.graph = executionGraph(snapshot.policy, snapshot.read, {}, new Set());
  expect(snapshot.graph.local).toEqual(
    new Set([
      "scripts/html/playwright.config.ts",
      "scripts/html/feature-catalog_test.ts",
      "scripts/html/chrome_badge_metrics_test.ts",
      "scripts/html/reference-capture_browser_test.ts",
    ]),
  );
  expect(snapshot.graph.errors).not.toContain("unresolved/external code: !unresolved-executable");
});

const macosBrowserDefinition =
  'MACOS_BROWSER_CONTRACT_RUN := if os() == "macos" { "node node_modules/playwright/cli.js test --config scripts/html/playwright.macos.config.ts" } else { "true" }';

function macosBrowserGraph(definition: string, body: string) {
  const snapshot = fixture();
  const justfile = `${definition}\ncheck:\n    python3 scripts/hooks/run_parallel_checks.py\ncheck-assets:\n    just macos-browser-contract-test\nmacos-browser-contract-test:\n    ${body}\n`;
  const prior = snapshot.read;
  const read = (name: string) => {
    if (name === "Justfile") return justfile;
    if (name === ".github/workflows/test.yml") return "run: just macos-browser-contract-test";
    if (name === "scripts/html/playwright.macos.config.ts") return "export default {};";
    return prior(name);
  };
  return executionGraph(snapshot.policy, read, {}, new Set());
}

test("macOS conditional browser recipe registers its actual config in both lanes", () => {
  const graph = macosBrowserGraph(macosBrowserDefinition, "{{MACOS_BROWSER_CONTRACT_RUN}}");
  expect(graph.local.has("scripts/html/playwright.macos.config.ts")).toBe(true);
  expect(graph.ci.has("scripts/html/playwright.macos.config.ts")).toBe(true);
  expect(graph.errors).not.toContain("unverified macOS browser conditional recipe");
});

test.each([
  [
    "platform guard",
    macosBrowserDefinition.replace('== "macos"', '!= "macos"'),
    "{{MACOS_BROWSER_CONTRACT_RUN}}",
  ],
  [
    "runner command",
    macosBrowserDefinition.replace("playwright/cli.js test", "playwright/cli.js install"),
    "{{MACOS_BROWSER_CONTRACT_RUN}}",
  ],
  [
    "config path",
    macosBrowserDefinition.replace("playwright.macos.config.ts", "other.config.ts"),
    "{{MACOS_BROWSER_CONTRACT_RUN}}",
  ],
  [
    "else branch",
    macosBrowserDefinition.replace('else { "true" }', 'else { "false" }'),
    "{{MACOS_BROWSER_CONTRACT_RUN}}",
  ],
  [
    "duplicate assignment",
    `${macosBrowserDefinition}\n${macosBrowserDefinition}`,
    "{{MACOS_BROWSER_CONTRACT_RUN}}",
  ],
  ["recipe command", macosBrowserDefinition, "{{MACOS_BROWSER_CONTRACT_RUN}} --grep anything"],
] as const)("macOS conditional browser recipe rejects %s drift", (_label, definition, body) => {
  const graph = macosBrowserGraph(definition, body);
  expect(graph.local.has("scripts/html/playwright.macos.config.ts")).toBe(false);
  expect(graph.ci.has("scripts/html/playwright.macos.config.ts")).toBe(false);
  expect(graph.errors).toContain("unverified macOS browser conditional recipe");
});

function observedUnitTool(request: ExternalToolRequest): ExternalToolBinding {
  return {
    ...request,
    packageName: "typescript",
    packageVersion: "7.0.2",
    verification: "observed-local-pins",
    artifactVerified: false,
    inputPins: [{ name: "unit-observation", physicalPath: "/unit/tool", sha256: "0".repeat(64) }],
    runtime: { kind: "bun", version: "1.4.4", path: "/unit/bun", sha256: "1".repeat(64) },
    lockIdentities: [
      { name: "typescript", version: "7.0.2", defaultRegistry: true, integrity: "sha512-unit" },
    ],
  };
}

function typeScriptGraphFixture(source?: string): HarnessSnapshot {
  const snapshot = fixture();
  const caller = "scripts/quality/owned-type-projects.ts";
  const actual = fs.readFileSync(
    path.join(import.meta.dir, "../quality/owned-type-projects.ts"),
    "utf8",
  );
  const files = new Map([
    [caller, source ?? actual],
    ["scripts/html/harness-contract-graph.ts", "export const graph = true;"],
    ["scripts/quality/mathjax-declaration-contract.ts", "export const contract = true;"],
  ]);
  const prior = snapshot.read;
  snapshot.read = (name) =>
    files.get(name) ??
    (name === "scripts/html/main.ts"
      ? `import "../quality/owned-type-projects"; import "./helper";`
      : prior(name));
  snapshot.codeFiles.push(...files.keys());
  for (const file of files.keys()) snapshot.tracked.add(file);
  snapshot.externalToolResolver = observedUnitTool;
  snapshot.graph = executionGraph(
    snapshot.policy,
    snapshot.read,
    {},
    new Set(["zod", "@swc/core"]),
    [],
    undefined,
    snapshot.externalToolResolver,
  );
  return snapshot;
}

test("external tool binding is separate from owned sources and retains both lane edges", () => {
  const snapshot = typeScriptGraphFixture();
  expect(checkHarnessContract(snapshot)).toEqual([]);
  expect(snapshot.graph.externalToolBindings?.map(({ lane, target }) => [lane, target])).toEqual([
    ["local", "node_modules/typescript/bin/tsc"],
    ["ci", "node_modules/typescript/bin/tsc"],
  ]);
  expect(snapshot.graph.local.has("node_modules/typescript/bin/tsc")).toBe(false);
  expect(snapshot.graph.ci.has("node_modules/typescript/bin/tsc")).toBe(false);
});

test("actual repository keeps the public compiler resolver fresh after graph inspection", () => {
  const root = path.resolve(import.meta.dir, "../..");
  const snapshot = repositorySnapshot(root, []);
  const request = snapshot.graph.externalToolBindings?.[0];
  if (!request) throw new Error("actual TypeScript bootstrap request missing");
  const resolve = snapshot.externalToolResolver;
  if (!resolve) throw new Error("actual TypeScript compiler resolver missing");
  expect(resolve(request)).toBeDefined();
  const read = changedCompilerInputRead(root, spyOn);
  try {
    expect(() => resolve(request)).toThrow(
      "TypeScript compiler input changed: node_modules/typescript/lib/tsc.js",
    );
  } finally {
    read.mockRestore();
  }
});

test("external tool reports a post-graph resolver failure as unavailable", () => {
  const snapshot = typeScriptGraphFixture();
  snapshot.externalToolResolver = () => {
    throw new Error("TypeScript compiler input changed: node_modules/typescript/lib/tsc.js");
  };
  expect(checkHarnessContract(snapshot)).toContain(
    "external tool binding unavailable: node_modules/typescript/bin/tsc",
  );
});

test("executionGraph keeps pure source analysis and resolver observations in both lanes", () => {
  const snapshot = typeScriptGraphFixture();
  const caller = "scripts/quality/owned-type-projects.ts";
  const prior = snapshot.read;
  let callerReads = 0;
  let resolverCalls = 0;
  const graph = executionGraph(
    snapshot.policy,
    (name) => {
      if (name === caller) callerReads += 1;
      return prior(name);
    },
    {},
    new Set(["zod", "@swc/core"]),
    [],
    undefined,
    (request) => {
      resolverCalls += 1;
      return observedUnitTool(request);
    },
  );
  expect(callerReads).toBe(2);
  expect(resolverCalls).toBe(2);
  expect(graph.externalToolBindings?.map(({ lane, target }) => [lane, target])).toEqual([
    ["local", "node_modules/typescript/bin/tsc"],
    ["ci", "node_modules/typescript/bin/tsc"],
  ]);
});

test("executionGraph does not reuse a parsed source after a changed invocation", () => {
  const caller = "scripts/quality/owned-type-projects.ts";
  const actual = fs.readFileSync(
    path.join(import.meta.dir, "../quality/owned-type-projects.ts"),
    "utf8",
  );
  const original = typeScriptGraphFixture(actual);
  const changedSource = actual.replace(
    "const authorities = typeAuthorities(root, name);",
    'execFileSync(process.execPath, ["unknown.ts"]); const authorities = typeAuthorities(root, name);',
  );
  expect(changedSource).not.toBe(actual);
  const changed = typeScriptGraphFixture(changedSource);
  expect(original.graph.externalToolBindings).not.toEqual([]);
  expect(changed.graph.externalToolBindings).toEqual([]);
  expect(checkHarnessContract(changed).length).toBeGreaterThan(0);
  expect(changed.graph.errors).not.toContain(`missing code: ${caller}`);
});

test("executionGraph walkers do not mutate the AST reused across lanes", () => {
  const bindings = typeScriptGraphFixture().graph.externalToolBindings ?? [];
  expect(bindings.map(({ lane }) => lane)).toEqual(["local", "ci"]);
  const withoutLane = bindings.map(({ lane: _lane, ...binding }) => binding);
  expect(withoutLane[0]).toEqual(withoutLane[1]);
  expect(withoutLane[0]).not.toBe(withoutLane[1]);
});

test.each([
  ["other bootstrap", "../../node_modules/typescript/bin/tsc", "../../node_modules/other/bin/tsc"],
  ["different method", "execFileSync(", "spawnSync("],
  ["unsafe configuration", "const config = safeFile(root, name);", "const config = name;"],
  ["preload tail", '"--showConfig",', '"--require", "unknown-preload", "--showConfig",'],
  ["spread array", '"--showConfig",', '...flags, "--showConfig",'],
  ["weak path check", "fs.lstatSync(current).isSymbolicLink()", "false"],
  [
    "overridden path check",
    "export function safeFile(",
    "export function safeFile(root: string, name: string) { return name; } export function safeFile(",
  ],
  [
    "new launch",
    "const authorities = typeAuthorities(root, name);",
    'execFileSync(process.execPath, ["unknown.ts"]); const authorities = typeAuthorities(root, name);',
  ],
  [
    "compiler runtime pin drift",
    'process.versions.bun !== "1.4.4"',
    'process.versions.bun !== "1.4.5"',
  ],
])("external tool rejects changed actual route %s", (_name, before, after) => {
  const actual = fs.readFileSync(
    path.join(import.meta.dir, "../quality/owned-type-projects.ts"),
    "utf8",
  );
  expect(actual).toContain(before);
  const snapshot = typeScriptGraphFixture(actual.replace(before, after));
  expect(snapshot.graph.externalToolBindings).toEqual([]);
  expect(checkHarnessContract(snapshot).length).toBeGreaterThan(0);
});

test.each([
  "missing resolver",
  "undefined record",
  "changed pins",
  "changed runtime version",
  "changed caller",
  "untracked caller",
])("external tool rejects post-graph %s", (kind) => {
  const snapshot = typeScriptGraphFixture();
  if (kind === "missing resolver") delete snapshot.externalToolResolver;
  else if (kind === "undefined record") snapshot.externalToolResolver = () => undefined;
  else if (kind === "changed pins")
    snapshot.externalToolResolver = (request) => ({
      ...observedUnitTool(request),
      runtime: { kind: "bun", version: "1.4.4", path: "/different/bun", sha256: "2".repeat(64) },
    });
  else if (kind === "changed runtime version")
    snapshot.externalToolResolver = (request) => {
      const binding = observedUnitTool(request);
      Reflect.set(binding.runtime, "version", "1.4.5");
      return binding;
    };
  else if (kind === "changed caller") {
    const prior = snapshot.read;
    snapshot.read = (name) =>
      name === "scripts/quality/owned-type-projects.ts"
        ? `${prior(name)}\n// changed bytes\n`
        : prior(name);
  } else snapshot.tracked.delete("scripts/quality/owned-type-projects.ts");
  expect(checkHarnessContract(snapshot).length).toBeGreaterThan(0);
});

test("keeps an unverified TypeScript executable external while rejecting the failed binding", () => {
  const snapshot = typeScriptGraphFixture();
  snapshot.externalToolResolver = (request) => {
    const binding = observedUnitTool(request);
    Reflect.set(binding.runtime, "version", "1.4.5");
    return binding;
  };
  expect(snapshot.graph.local.has("node_modules/typescript/bin/tsc")).toBe(false);
  expect(snapshot.graph.ci.has("node_modules/typescript/bin/tsc")).toBe(false);
  expect(checkHarnessContract(snapshot)).toEqual(
    expect.arrayContaining([
      "external tool binding changed/unverified: node_modules/typescript/bin/tsc",
    ]),
  );
});

function actualBiomeContext(): string {
  return fs.readFileSync(
    path.join(import.meta.dir, "../runtime-bundles/runtime-unused-check.ts"),
    "utf8",
  );
}

function biomeReferences(
  source: string,
  caller = "scripts/runtime-bundles/runtime-unused-check.ts",
): string[] {
  return sourceCode(source, {
    name: caller,
    read: (name) =>
      name === "package.json"
        ? fs.readFileSync(path.join(import.meta.dir, "../../package.json"), "utf8")
        : undefined,
  });
}

test("actual fixed Biome context has no dynamic executable and preserves executed argv object", () => {
  const source = actualBiomeContext();
  expect(biomeReferences(source)).not.toContain("!unresolved-executable");
  const module = parseSync(source, { syntax: "typescript" });
  const fn = module.body.find(
    (node) => node.type === "FunctionDeclaration" && node.identifier.value === "contextDiagnostics",
  );
  if (!fn) throw new Error("actual contextDiagnostics missing");
  const body = Buffer.from(source)
    .subarray(fn.span.start - module.span.start, fn.span.end - module.span.start)
    .toString();
  const emitted = transformSync(body, {
    jsc: { parser: { syntax: "typescript" }, target: "es2024" },
  }).code;
  const bytes = new Map<string, Buffer>();
  let executed: unknown;
  const invoke: unknown = runInNewContext(`${emitted}\ncontextDiagnostics`, {
    path,
    Buffer,
    createHash,
    fs: {
      writeFileSync: (file: string, value: Uint8Array) => bytes.set(file, Buffer.from(value)),
      lstatSync: () => ({ isFile: () => true, isSymbolicLink: () => false }),
      readFileSync: (file: string) => bytes.get(file) ?? Buffer.from("{}"),
    },
    Bun: {
      spawnSync: (argv: unknown) => {
        executed = argv;
        return { stdout: Buffer.from("stdout"), stderr: Buffer.from("stderr"), exitCode: 0 };
      },
    },
    inspectUnusedBiomeReport: () => [],
  });
  if (typeof invoke !== "function") throw new Error("actual function not returned");
  const result: unknown = invoke(
    "/unit",
    "/unit/data",
    "/unit/biome.json",
    "context",
    "const value = 1;",
  );
  const record = z.record(z.string(), z.unknown()).parse(result);
  expect(record.argv).toBe(executed);
  expect(z.array(z.string()).parse(executed)).toEqual([
    "bunx",
    "--no-install",
    "biome",
    "lint",
    "--config-path=/unit/biome.json",
    "--only=correctness/noUnusedVariables",
    "--only=correctness/noUnusedFunctionParameters",
    "--error-on-warnings",
    "--max-diagnostics=none",
    "--reporter=json",
    "--reporter-file=/unit/data/context.json",
    "/unit/data/context.js",
  ]);
  expect(bytes.get("/unit/data/context.stdout")?.toString()).toBe("stdout");
  expect(bytes.get("/unit/data/context.stderr")?.toString()).toBe("stderr");
});

test.each([
  ["different caller", "", ""],
  ["different function", "function contextDiagnostics(", "function otherDiagnostics("],
  ["mutable declaration", "const argv = [", "let argv = ["],
  ["unknown head", '"bunx",', "getBinary(),"],
  ["spread argv", '"bunx",', "...getArgs(),"],
  [
    "array mutation",
    "const child = Bun.spawnSync(argv",
    'argv.push("--require", "unknown"); const child = Bun.spawnSync(argv',
  ],
  [
    "reassignment",
    "const child = Bun.spawnSync(argv",
    "argv = replacement; const child = Bun.spawnSync(argv",
  ],
  [
    "shadowed launcher",
    "const child = Bun.spawnSync(argv",
    "const Bun = replacement; const child = Bun.spawnSync(argv",
  ],
  ["different logged argv", "    argv,", "    argv: [],"],
  [
    "overridden function",
    "function contextDiagnostics(",
    "function contextDiagnostics() { return {}; } function contextDiagnostics(",
  ],
])("fixed Biome context rejects %s", (name, before, after) => {
  const source = actualBiomeContext();
  if (before) expect(source).toContain(before);
  const changed = before ? source.replace(before, after) : source;
  expect(
    biomeReferences(changed, name === "different caller" ? "scripts/html/other.ts" : undefined),
  ).toContain("!unresolved-executable");
});

test.each([
  "undefined",
  "exception",
  "wrong caller",
  "wrong target",
  "empty pins",
  "official claim",
])("external tool rejects supplied record %s", (kind) => {
  const snapshot = typeScriptGraphFixture();
  snapshot.externalToolResolver = (request) => {
    if (kind === "undefined") return undefined;
    if (kind === "exception") throw new Error("binding unavailable");
    const binding = observedUnitTool(request);
    if (kind === "wrong caller") binding.caller = "scripts/html/other.ts";
    if (kind === "wrong target") binding.target = "node_modules/other/bin/tsc";
    if (kind === "empty pins") binding.inputPins = [];
    if (kind === "official claim") Reflect.set(binding, "artifactVerified", true);
    return binding;
  };
  snapshot.graph = executionGraph(
    snapshot.policy,
    snapshot.read,
    {},
    new Set(["zod", "@swc/core"]),
    [],
    undefined,
    snapshot.externalToolResolver,
  );
  expect(snapshot.graph.externalToolBindings).toEqual([]);
  expect(checkHarnessContract(snapshot).length).toBeGreaterThan(0);
});

test("external tool canonical query under another caller remains unresolved owned execution", () => {
  const source = fs.readFileSync(
    path.join(import.meta.dir, "../quality/owned-type-projects.ts"),
    "utf8",
  );
  const captured: ExternalToolBinding[] = [];
  const references = sourceCode(source, {
    name: "scripts/html/copied-query.ts",
    read: () => undefined,
    resolveExternalTool: observedUnitTool,
    onExternalToolBinding: (binding) => captured.push(binding),
  });
  expect(captured).toEqual([]);
  expect(references).toContain("!execute-local:../../node_modules/typescript/bin/tsc");
});

test("actual three RTK TypeScript calls retain finite external tool classification", () => {
  const caller = path.join(import.meta.dir, "../runtime-bundles/runtime-bundle-checks_test.ts");
  const source = fs.readFileSync(caller, "utf8");
  const module = parseSync(source, { syntax: "typescript" });
  const calls: Array<{ span: { start: number; end: number } }> = [];
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const child of value) visit(child);
      return;
    }
    const node = z.record(z.string(), z.unknown()).safeParse(value);
    if (!node.success) return;
    const call = z
      .object({
        type: z.literal("CallExpression"),
        callee: z.object({ type: z.literal("Identifier"), value: z.literal("spawnSync") }),
        arguments: z.array(z.object({ expression: z.unknown() })),
        span: z.object({ start: z.number(), end: z.number() }),
      })
      .safeParse(node.data);
    if (
      call.success &&
      z
        .object({ type: z.literal("StringLiteral"), value: z.literal("rtk") })
        .safeParse(call.data.arguments[0]?.expression).success
    )
      calls.push(call.data);
    for (const child of Object.values(node.data)) visit(child);
  };
  visit(module);
  expect(calls.length).toBe(3);
  for (const call of calls) {
    const actualCall = Buffer.from(source)
      .subarray(call.span.start - module.span.start, call.span.end - module.span.start)
      .toString();
    const references = sourceCode(`import { spawnSync } from "node:child_process"; ${actualCall};`);
    expect(references).not.toContain("!unresolved-executable");
  }
  expect(sourceCode(source)).not.toContain("!unresolved-executable");
});

test.each([
  ["unknown proxy", 'Bun.spawnSync(["rtk", "other", "bunx", "--no-install", "tsc"]);'],
  ["unknown tool", 'Bun.spawnSync(["rtk", "proxy", "other-tool"]);'],
  ["unknown package", 'Bun.spawnSync(["rtk", "proxy", "bunx", "--no-install", "other-package"]);'],
  ["unknown head", 'Bun.spawnSync([getRtk(), "proxy", "bunx", "--no-install", "tsc"]);'],
  ["head spread", 'Bun.spawnSync([...wrappers, "bunx", "--no-install", "tsc"]);'],
  ["tail spread", 'Bun.spawnSync(["rtk", "proxy", "bunx", "--no-install", "tsc", ...args]);'],
  [
    "compound wrappers",
    'Bun.spawnSync(["rtk", "proxy", "rtk", "proxy", "bunx", "--no-install", "tsc"]);',
  ],
  ["computed proxy", 'Bun.spawnSync(["rtk", getProxy(), "bunx", "--no-install", "tsc"]);'],
  ["missing target", 'Bun.spawnSync(["rtk", "proxy"]);'],
  ["unknown argv", 'import { spawnSync } from "node:child_process"; spawnSync("rtk", args);'],
  [
    "nonliteral wrapper",
    'const wrapper = "rtk"; Bun.spawnSync([wrapper, "proxy", "bunx", "--no-install", "tsc"]);',
  ],
  [
    "shell wrapper",
    'import { execSync } from "node:child_process"; execSync("rtk proxy bunx --no-install tsc && unknown-tool");',
  ],
])("finite RTK wrapper rejects %s", (_name, source) => {
  expect(sourceCode(source)).toContain("!unresolved-executable");
});

test.each([
  ["semicolon", "bunx --no-install biome check scripts; bun scripts/hidden.ts"],
  ["and", "bunx --no-install biome check scripts && python3 scripts/hidden.py"],
  ["or", "git status --short || bun scripts/hidden.ts"],
  ["pipe", "git status --short | bun scripts/hidden.ts"],
  ["background", "git status --short & bun scripts/hidden.ts"],
  ["newline", "git status --short\nbun scripts/hidden.ts"],
  ["CRLF", "git status --short\r\nbun scripts/hidden.ts"],
  ["redirect", "git status --short > tmp/output.txt"],
  ["escaped quote", 'git status \\"; bun scripts/hidden.ts #\\"'],
  ["substitution", 'git status "$(bun scripts/hidden.ts)"'],
  ["backtick", 'git status "`bun scripts/hidden.ts`"'],
])("external shell command rejects %s before losing owned followup edges", (_name, command) => {
  const source = `import { execSync } from "node:child_process"; execSync(${JSON.stringify(command)});`;
  expect(sourceCode(source)).toContain("!unresolved-executable");
});

test.each([
  "git log '--format=semi;and&&or||pipe|redirect<>'",
  'bunx --no-install biome check "semi;and&&or||pipe|redirect<>"',
  "git status --short",
])("external shell literal quoted data remains data: %s", (command) => {
  expect(
    sourceCode(
      `import { execSync } from "node:child_process"; execSync(${JSON.stringify(command)});`,
    ),
  ).not.toContain("!unresolved-executable");
});

test.each([";", "&&", "||", "|", "\n"])(
  "existing owned interpreter shell chain retains managed edges: %s",
  (operator) => {
    const command = `bun scripts/main.ts ${operator} python3 scripts/hidden.py`;
    const references = sourceCode(
      `import { execSync } from "node:child_process"; execSync(${JSON.stringify(command)});`,
    );
    expect(references).toContain("!execute:scripts/main.ts");
    expect(references).toContain("!execute:scripts/hidden.py");
    expect(references).not.toContain("!unresolved-executable");
  },
);

test.each([
  [
    "unzip member",
    'import { spawnSync } from "node:child_process"; spawnSync("unzip", ["-p", archive, member]);',
  ],
  [
    "zip fixture",
    'import { spawnSync } from "node:child_process"; spawnSync("zip", ["-q", archive, "js/viewer.min.js"]);',
  ],
  [
    "GitHub token data query",
    'Bun.spawnSync(["gh", "auth", "token", "--hostname", "github.com"]);',
  ],
  [
    "Git inventory flags",
    'import { execFileSync } from "node:child_process"; execFileSync("git", ["ls-files", ...flags, "-z"]);',
  ],
])("finite external data command accepts %s", (_name, source) => {
  expect(sourceCode(source)).not.toContain("!unresolved-executable");
});

test.each([
  ["unknown command", 'Bun.spawnSync([tool, "-p", archive, member]);'],
  ["unzip operation", 'Bun.spawnSync(["unzip", "-x", archive, member]);'],
  ["unzip prefix spread", 'Bun.spawnSync(["unzip", ...prefix, archive, member]);'],
  ["unzip extra", 'Bun.spawnSync(["unzip", "-p", archive, member, extra]);'],
  ["zip spread", 'Bun.spawnSync(["zip", "-q", ...data]);'],
  ["GitHub unknown host", 'Bun.spawnSync(["gh", "auth", "token", "--hostname", host]);'],
  ["GitHub extra", 'Bun.spawnSync(["gh", "auth", "token", "--hostname", "github.com", extra]);'],
  ["Git prefix spread", 'Bun.spawnSync(["git", ...prefix, "-z"]);'],
  ["Git unknown subcommand", 'Bun.spawnSync(["git", command, ...flags, "-z"]);'],
  ["Git noninventory spread", 'Bun.spawnSync(["git", "status", ...flags, "-z"]);'],
  ["Git missing suffix", 'Bun.spawnSync(["git", "ls-files", ...flags]);'],
])("finite external data command rejects %s", (_name, source) => {
  expect(sourceCode(source)).toContain("!unresolved-executable");
});

test.each([
  ["Bun.spawnSync([process.execPath, script, ...args]);", "!execute-local:duplication-check.ts"],
  ['Bun.spawnSync(["bun", script, ...args]);', "!execute-local:duplication-check.ts"],
  [
    'Bun.spawnSync(["bun", "scripts/runtime-assets/update.ts", ...args]);',
    "!execute:scripts/runtime-assets/update.ts",
  ],
  [
    'Bun.spawn(["bun", "run", "scripts/runtime-assets/update.ts", ...args.slice(2)]);',
    "!execute:scripts/runtime-assets/update.ts",
  ],
])("selected managed source retains its edge before data spread: %s", (call, expected) => {
  const source = `import path from "node:path"; function run(args: string[]) { const script = path.join(import.meta.dir, "duplication-check.ts"); ${call} }`;
  const references = sourceCode(source);
  expect(references).toContain(expected);
  expect(references).not.toContain("!unresolved-executable");
});

test.each([
  [
    "path TDZ",
    'Bun.spawnSync([process.execPath, script, ...args]); const script = path.join(import.meta.dir, "child.ts");',
  ],
  [
    "cross closure",
    'const script = path.join(import.meta.dir, "child.ts"); function run() { Bun.spawnSync([process.execPath, script, ...args]); }',
  ],
  [
    "mutable path",
    'let script = path.join(import.meta.dir, "child.ts"); Bun.spawnSync([process.execPath, script, ...args]);',
  ],
  [
    "reassigned path",
    'const script = path.join(import.meta.dir, "child.ts"); script = getPath(); Bun.spawnSync([process.execPath, script, ...args]);',
  ],
  [
    "reassigned provider",
    'path = getPath(); const script = path.join(import.meta.dir, "child.ts"); Bun.spawnSync([process.execPath, script, ...args]);',
  ],
  [
    "shadowed path",
    'function run(path: unknown) { const script = path.join(import.meta.dir, "child.ts"); Bun.spawnSync([process.execPath, script, ...args]); }',
  ],
  [
    "shadowed runtime",
    'function run(process: unknown) { const script = path.join(import.meta.dir, "child.ts"); Bun.spawnSync([process.execPath, script, ...args]); }',
  ],
  [
    "computed runtime",
    'const script = path.join(import.meta.dir, "child.ts"); Bun.spawnSync([process["execPath"], script, ...args]);',
  ],
  ["unknown child", 'Bun.spawnSync(["bun", child, ...args]);'],
  ["prefix spread", 'Bun.spawnSync(["bun", ...prefix, "scripts/child.ts", ...args]);'],
  ["package route spread", 'Bun.spawnSync(["bun", "run", "package-task", ...args]);'],
  ["eval tail", 'Bun.spawnSync(["node", "-e", "void 0", ...args]);'],
])("managed source selection rejects %s", (_name, body) => {
  expect(sourceCode(`import path from "node:path"; ${body}`)).toContain("!unresolved-executable");
});

test.each([
  ["scripts/quality/owned-source-inventory.ts", undefined],
  ["scripts/runtime-assets/depends-update-all.ts", "!execute:scripts/runtime-assets/update.ts"],
  ["scripts/runtime-assets/drawio-war-extractor.ts", undefined],
  ["scripts/runtime-assets/latest-check.ts", undefined],
  ["scripts/runtime-assets/update_test.ts", undefined],
])("actual bounded data or managed-source caller remains classified: %s", (name, child) => {
  const source = fs.readFileSync(path.join(import.meta.dir, "../..", name), "utf8");
  const references = sourceCode(source, { name, read: () => undefined });
  expect(references).not.toContain("!unresolved-executable");
  if (child) expect(references).toContain(child);
});

const nativeHtmlCallers = [
  ["scripts/html/dynamic-fidelity.ts", "src/lib.rs", "html-feature-lib-test"],
  ["scripts/html/toc-fidelity.ts", "src/lib.rs", "html-toc-lib-test"],
  ["scripts/html/fidelity-suite.ts", "examples/html_fidelity_probe.rs", "html-fidelity-example"],
] as const;

function nativeHtmlSourceFixture(caller: string) {
  const member = "crates/katana-render-runtime";
  const paths = [
    "Cargo.toml",
    `${member}/Cargo.toml`,
    `${member}/src/lib.rs`,
    `${member}/examples/html_fidelity_probe.rs`,
    `${member}/src/renderer/backends/html_interactive/tests/feature_diagnostic.rs`,
    `${member}/src/renderer/backends/html_interactive/tests/toc_diagnostic.rs`,
    `${member}/examples/probe/frame.rs`,
  ];
  const files = new Map(paths.map((name) => [name, fs.readFileSync(name, "utf8")]));
  const source = fs.readFileSync(caller, "utf8");
  const records: NativeConsumerBinding[] = [];
  const inspect = (text = source, name = caller) =>
    sourceCode(text, {
      name,
      read: (path) => files.get(path),
      onNativeConsumerBinding: (record) => records.push(record),
    });
  return { files, source, records, inspect };
}

test.each(nativeHtmlCallers)(
  "native consumer %s registers only source ownership",
  (caller, entrypoint, argvClass) => {
    const actual = nativeHtmlSourceFixture(caller);
    const references = actual.inspect();
    expect(references).not.toContain("!unresolved-executable");
    expect(references).toContain(`crates/katana-render-runtime/${entrypoint}`);
    expect(actual.records).toHaveLength(1);
    expect(actual.records[0]).toMatchObject({
      caller,
      argvClass,
      proofKind: "source-ownership-classification",
      artifactVerified: false,
      executionVerified: false,
    });
    expect(actual.records[0]?.callerSourceSha256).toBe(
      createHash("sha256").update(actual.source).digest("hex"),
    );
    expect(actual.records[0]?.expectedProducerSources).toHaveLength(1);
    expect(actual.files.has("binary")).toBe(false);
  },
);

const nativeConsumerMutations = [
  { name: "wrong caller", caller: "scripts/html/unknown.ts", needle: "", replacement: "" },
  { name: "shadow Bun", needle: "Bun.spawnSync(", replacement: "((Bun) => Bun.spawnSync)(" },
  { name: "wrong API", needle: "Bun.spawnSync(", replacement: "Bun.spawn(" },
  { name: "unknown additional call", needle: "", replacement: "\nBun.spawnSync([unbound]);" },
  {
    name: "duplicate native call",
    needle: "",
    replacement:
      "\nfunction extra(binary:string){Bun.spawnSync([binary, 'writes_fixture_feature_state_interactive', '--nocapture']);}",
  },
  {
    name: "wrong selector",
    needle: "writes_fixture_feature_state_interactive",
    replacement: "unverified_test",
  },
  { name: "wrong flags", needle: '"--nocapture"', replacement: '"--exact"' },
  {
    name: "head spread",
    needle: '[binary, "writes_fixture',
    replacement: '[...binary, "writes_fixture',
  },
];

test.each(nativeConsumerMutations)("native consumer rejects %j", (mutation) => {
  const actual = nativeHtmlSourceFixture("scripts/html/dynamic-fidelity.ts");
  if (mutation.needle) expect(actual.source).toContain(mutation.needle);
  const changed = mutation.needle
    ? actual.source.replace(mutation.needle, mutation.replacement)
    : actual.source + mutation.replacement;
  expect(actual.inspect(changed, mutation.caller)).toContain("!unresolved-executable");
  expect(actual.records).toEqual([]);
});

test.each([
  [
    "binary argv",
    '[binary, "writes_fixture_feature_state_interactive"',
    '[fixture, "writes_fixture_feature_state_interactive"',
  ],
  ["environment source", "...process.env", "...process.argv"],
  ["nonce binding", "KRR_HTML_FEATURE_NONCE: nonce", "KRR_HTML_FEATURE_NONCE: binarySha256"],
  [
    "binary hash binding",
    "KRR_HTML_FEATURE_BINARY_SHA256: binarySha256",
    "KRR_HTML_FEATURE_BINARY_SHA256: sourceSha256",
  ],
  [
    "catalog operation JSON binding",
    "KRR_HTML_FEATURE_OPERATION_JSON: JSON.stringify(operation)",
    "KRR_HTML_FEATURE_OPERATION_JSON: JSON.stringify(request)",
  ],
  ["function moved", "function captureKrrCatalogCondition(", "function movedCatalogCondition("],
])("dynamic native consumer rejects changed %s binding", (_name, needle, replacement) => {
  const actual = nativeHtmlSourceFixture("scripts/html/dynamic-fidelity.ts");
  expect(actual.source).toContain(needle);
  expect(actual.inspect(actual.source.replace(needle, replacement))).toContain(
    "!unresolved-executable",
  );
  expect(actual.records).toEqual([]);
});

test("dynamic native consumer rejects a missing named spawn", () => {
  const actual = nativeHtmlSourceFixture("scripts/html/dynamic-fidelity.ts");
  const call = "Bun.spawnSync";
  const offset = actual.source.lastIndexOf(call);
  expect(offset).toBeGreaterThan(0);
  const changed = [
    actual.source.slice(0, offset),
    "JSON.stringify",
    actual.source.slice(offset + call.length),
  ].join("");
  expect(actual.inspect(changed)).toContain("!unresolved-executable");
  expect(actual.records).toEqual([]);
});

test("dynamic native consumer rejects a wrong catalog selector", () => {
  const actual = nativeHtmlSourceFixture("scripts/html/dynamic-fidelity.ts");
  const selector = "writes_fixture_feature_state_interactive";
  const offset = actual.source.lastIndexOf(selector);
  expect(offset).toBeGreaterThan(0);
  const changed =
    actual.source.slice(0, offset) +
    "unverified_catalog_test" +
    actual.source.slice(offset + selector.length);
  expect(actual.inspect(changed)).toContain("!unresolved-executable");
  expect(actual.records).toEqual([]);
});

test.each([
  ["options head", "options.binary,", "options.other,"],
  ["unknown flag", '"--width",', '"--hidden",'],
  ["unbound viewport", "String(viewport.width)", "String(other.width)"],
  [
    "unknown tail",
    '...scrolls.flatMap(({ requested }) => ["--scroll-y", String(requested)])',
    "...unbound",
  ],
  ["wrong scroll flag", '"--scroll-y", String(requested)', '"--hidden", String(requested)'],
  ["unknown scroll data", "String(requested)", "String(unbound)"],
])("native fidelity argv rejects %s", (_name, needle, replacement) => {
  const actual = nativeHtmlSourceFixture("scripts/html/fidelity-suite.ts");
  expect(actual.source).toContain(needle);
  expect(actual.inspect(actual.source.replace(needle, replacement))).toContain(
    "!unresolved-executable",
  );
  expect(actual.records).toEqual([]);
});

test.each([
  ["member missing", "Cargo.toml", ""],
  ["unknown package", "crates/katana-render-runtime/Cargo.toml", '[package]\nname="other"'],
  [
    "wrong lib",
    "crates/katana-render-runtime/Cargo.toml",
    '[package]\nname="katana-render-runtime"\n[lib]\npath="src/other.rs"',
  ],
  ["entrypoint missing", "crates/katana-render-runtime/src/lib.rs", undefined],
  [
    "producer missing",
    "crates/katana-render-runtime/src/renderer/backends/html_interactive/tests/feature_diagnostic.rs",
    undefined,
  ],
])("native Cargo registration rejects %s", (_name, path, text) => {
  const actual = nativeHtmlSourceFixture("scripts/html/dynamic-fidelity.ts");
  if (text === undefined) actual.files.delete(path);
  else actual.files.set(path, text);
  expect(actual.inspect()).toContain("!unresolved-executable");
  expect(actual.records).toEqual([]);
});

test("native example target override cannot silently choose a different source", () => {
  const actual = nativeHtmlSourceFixture("scripts/html/fidelity-suite.ts");
  actual.files.set(
    "crates/katana-render-runtime/Cargo.toml",
    '[package]\nname="katana-render-runtime"\n[[example]]\nname="html_fidelity_probe"\npath="examples/other.rs"',
  );
  expect(actual.inspect()).toContain("!unresolved-executable");
  expect(actual.records).toEqual([]);
});

test("native classification does not certify nonce or hash guards", () => {
  const actual = nativeHtmlSourceFixture("scripts/html/dynamic-fidelity.ts");
  const changed = actual.source.replace("value.executionNonce !== nonce", "false");
  expect(changed).not.toBe(actual.source);
  expect(actual.inspect(changed)).not.toContain("!unresolved-executable");
  expect(actual.records[0]?.executionVerified).toBe(false);
});

test("native source records and Cargo entrypoints remain separate in both lanes", () => {
  const actual = nativeHtmlSourceFixture("scripts/html/dynamic-fidelity.ts");
  const snapshot = fixture();
  const source =
    'import { randomUUID } from "node:crypto"; import { sha256File } from "./reference-capture"; ' +
    "export function captureKrrState(binary:string, fixture:string, expression:string, output:string, caseId:string, viewport:{width:number;height:number}) {" +
    "const sourceSha256=sha256File(fixture); const binarySha256=sha256File(binary); const nonce=randomUUID(); " +
    'Bun.spawnSync([binary,"writes_fixture_feature_state_interactive","--nocapture"],{env:{...process.env,KRR_HTML_FEATURE_INPUT:fixture,KRR_HTML_FEATURE_EXPRESSION:expression,KRR_HTML_FEATURE_NONCE:nonce,KRR_HTML_FEATURE_CASE:caseId,KRR_HTML_FEATURE_BINARY_SHA256:binarySha256,KRR_HTML_FEATURE_WIDTH:String(viewport.width),KRR_HTML_FEATURE_HEIGHT:String(viewport.height),KRR_HTML_FEATURE_OUTPUT:output}}); } ' +
    "export function captureKrrCatalogCondition(binary:string, fixture:string, expression:string, output:string, request:{caseId:string;viewport:{width:number;height:number}}) {" +
    'const sourceSha256=sha256File(fixture); const binarySha256=sha256File(binary); const nonce=randomUUID(); const operation={schema:"krr-html84-operation-v1"}; ' +
    'Bun.spawnSync([binary,"writes_fixture_feature_state_interactive","--nocapture"],{env:{...process.env,KRR_HTML_FEATURE_INPUT:fixture,KRR_HTML_FEATURE_EXPRESSION:expression,KRR_HTML_FEATURE_NONCE:nonce,KRR_HTML_FEATURE_CASE:request.caseId,KRR_HTML_FEATURE_BINARY_SHA256:binarySha256,KRR_HTML_FEATURE_WIDTH:String(request.viewport.width),KRR_HTML_FEATURE_HEIGHT:String(request.viewport.height),KRR_HTML_FEATURE_OUTPUT:output,KRR_HTML_FEATURE_OPERATION_JSON:JSON.stringify(operation)}}); }';
  actual.files.set(
    "scripts/html/reference-capture.ts",
    "export function sha256File(value:string): string { return value; }",
  );
  actual.files.set("scripts/html/main.ts", 'import "./dynamic-fidelity";');
  actual.files.set("scripts/html/dynamic-fidelity.ts", source);
  const graph = executionGraph(
    { ...snapshot.policy, codeRoots: ["scripts/", "crates/"] },
    (name) => actual.files.get(name) ?? snapshot.read(name),
    {},
    new Set(),
  );
  expect(graph.errors).toEqual([]);
  const entrypoint = "crates/katana-render-runtime/src/lib.rs";
  expect(graph.local.has(entrypoint)).toBe(true);
  expect(graph.ci.has(entrypoint)).toBe(true);
  expect(
    graph.nativeConsumerBindings?.map(({ lane, artifactVerified, executionVerified }) => ({
      lane,
      artifactVerified,
      executionVerified,
    })),
  ).toEqual([
    { lane: "local", artifactVerified: false, executionVerified: false },
    { lane: "ci", artifactVerified: false, executionVerified: false },
  ]);
  expect(
    graph.local.has(
      "crates/katana-render-runtime/src/renderer/backends/html_interactive/tests/feature_diagnostic.rs",
    ),
  ).toBe(false);
});

const bunSpyFixture = `import { spyOn } from "bun:test";
function fixture() {
  const spy = spyOn(Bun, "spawn").mockImplementation(() => { throw new Error("fixture"); });
  spy.mockRestore();
}`;

test.each([
  ["qualified import", bunSpyFixture],
  [
    "import alias",
    bunSpyFixture.replace("{ spyOn }", "{ spyOn as observe }").replace("spyOn(Bun", "observe(Bun"),
  ],
])("standard Bun spy is setup rather than process execution: %s", (_name, source) => {
  expect(sourceCode(source)).not.toContain("!unresolved-executable");
});

test.each([
  ["spy invocation", "spy.mockRestore();", 'spy(["bun", "unbound"]); spy.mockRestore();'],
  ["spy alias", "spy.mockRestore();", "const alias = spy; spy.mockRestore();"],
  ["spy return", "spy.mockRestore();", "spy.mockRestore(); return spy;"],
  ["spy argument escape", "spy.mockRestore();", "consume(spy); spy.mockRestore();"],
  ["restore alias", "spy.mockRestore();", "const restore = spy.mockRestore; restore();"],
  ["restore args", "spy.mockRestore();", "spy.mockRestore(unbound);"],
  ["unknown method", "spy.mockRestore();", "spy.getMockName();"],
  ["mutable spy", "const spy =", "let spy ="],
  ["spy assignment", "spy.mockRestore();", "spy = unbound; spy.mockRestore();"],
  ["wrong module", 'from "bun:test"', 'from "./fake"'],
  ["type-only import", "import { spyOn }", "import { type spyOn }"],
  ["shadow import", "function fixture()", "function fixture(spyOn)"],
  ["unknown target", '"spawn")', '"spawnSync")'],
  ["variable target", '"spawn")', "method)"],
  ["spread args", 'spyOn(Bun, "spawn")', 'spyOn(Bun, ...["spawn"])'],
  ["computed namespace", "spyOn(Bun,", 'spyOn(globalThis["Bun"],'],
])("standard Bun spy keeps unknown use failclosed: %s", (_name, needle, replacement) => {
  expect(bunSpyFixture).toContain(needle);
  expect(sourceCode(bunSpyFixture.replace(needle, replacement))).toContain(
    "!unresolved-executable",
  );
});

test("standard spy recognition preserves actual mock callback process edges", () => {
  const source = bunSpyFixture.replace(
    '() => { throw new Error("fixture"); }',
    '() => Bun.spawn(["bun", "scripts/hidden.ts"])',
  );
  expect(sourceCode(source)).toContain("!execute:scripts/hidden.ts");
  const changed = source.replace('["bun", "scripts/hidden.ts"]', "[unbound]");
  expect(sourceCode(changed)).toContain("!unresolved-executable");
});

test("B default runner unit only uses Bun.spawn as a compile-time type", () => {
  const source = fs.readFileSync("scripts/runtime-assets/depends-update-all_test.ts", "utf8");
  expect(source).toContain("new Proxy<typeof Bun.spawn>(");
  expect(sourceCode(source)).not.toContain("!unresolved-executable");
  expect(
    sourceCode('const target: typeof Bun.spawn = () => {throw new Error("fixture");};'),
  ).not.toContain("!unresolved-executable");
  expect(sourceCode("new Proxy(Bun.spawn, {});")).toContain("!unresolved-executable");
});

const invalidPolicyShapes: Array<[string, Record<string, unknown>]> = [
  ["dispatch array", { recipeDispatch: [] }],
  ["Python array", { python: [] }],
  ["Rust array", { rust: [] }],
  ["Moon null", { moon: null }],
  ["missing local recipes", { localRecipes: undefined }],
  ["missing bridge", { localBridge: undefined }],
  ["missing workflow", { ciWorkflow: undefined }],
  ["missing dispatch", { recipeDispatch: undefined }],
  ["missing roots", { codeRoots: undefined }],
  ["roots wrong type", { codeRoots: "scripts/" }],
  ["root member wrong type", { codeRoots: [1] }],
  ["schema mismatch", { schema: 2 }],
];

for (const [name, changes] of invalidPolicyShapes) {
  test(`contract policy boundary rejects ${name} before repository setup`, () => {
    const raw = { ...fixture().policy, ...changes };
    expect(policyContract.safeParse(raw).success).toBe(false);
    withModulePackage((root) => {
      fs.writeFileSync(
        path.join(root, "scripts/html/harness-contract-policy.json"),
        JSON.stringify(raw),
      );
      expect(() => repositorySnapshot(root, [])).toThrow(z.ZodError);
    });
  });
}

test("contract policy boundary preserves actual policy and existing fixture graphs", () => {
  const raw = JSON.parse(fs.readFileSync("scripts/html/harness-contract-policy.json", "utf8"));
  expect(policyContract.parse(raw)).toEqual(raw);
  for (const snapshot of [fixture(), moonFixture()]) {
    const parsed = policyContract.parse(snapshot.policy);
    expect(parsed).toEqual(snapshot.policy);
    const graph = executionGraph(parsed, snapshot.read, {}, new Set(), snapshot.codeFiles);
    expect(graph).toEqual(snapshot.graph);
    expect(checkHarnessContract({ ...snapshot, policy: parsed, graph })).toEqual(
      checkHarnessContract(snapshot),
    );
  }
});

test("contract policy boundary normalizes unknown fields with shared policy schema", () => {
  const snapshot = fixture();
  const raw = {
    ...snapshot.policy,
    unknown: true,
    localBridge: { ...snapshot.policy.localBridge, unknown: true },
    recipeDispatch: { sample: { source: "scripts/sample.py", recipes: [], unknown: true } },
  };
  const parsed = policyContract.parse(raw);
  expect(parsed).toEqual({
    ...snapshot.policy,
    recipeDispatch: { sample: { source: "scripts/sample.py", recipes: [] } },
  });
});

test("contract policy boundary preserves roots semantics and supported-root rejection", () => {
  const snapshot = fixture();
  const duplicate = policyContract.parse({
    ...snapshot.policy,
    codeRoots: ["scripts/", "scripts/"],
  });
  expect(executionGraph(duplicate, snapshot.read, {}, new Set())).toEqual(snapshot.graph);
  const empty = policyContract.parse({ ...snapshot.policy, codeRoots: [] });
  const graph = executionGraph(empty, snapshot.read, {}, new Set());
  expect(checkHarnessContract({ ...snapshot, policy: empty, graph })).toContain(
    "misplaced code: scripts/html/main.ts",
  );
  withModulePackage((root) => {
    fs.writeFileSync(
      path.join(root, "scripts/html/harness-contract-policy.json"),
      JSON.stringify({ ...snapshot.policy, codeRoots: ["other/"] }),
    );
    expect(() => repositorySnapshot(root, [])).toThrow("invalid contract policy");
  });
});

describe("finite standard gate process and scratch import bindings", () => {
  const root = path.resolve(import.meta.dir, "../..");
  const gate = "scripts/runtime-bundles/runtime-unused-check.ts";
  const adapterTest = "scripts/runtime-bundles/runtime-unused-semantic-adapter_test.ts";
  const read = (name: string) => fs.readFileSync(path.join(root, name), "utf8");
  const references = (
    name: string,
    source = read(name),
    changedRead: (name: string) => string | undefined = read,
  ) => sourceCode(source, { name, read: changedRead });
  test("recognizes the current native source inventory calls and stays fail-closed for dynamic argv", () => {
    const name = "scripts/mermaid/native-build-receipt.ts";
    const source = read(name);
    expect(references(name, source)).not.toContain("!unresolved-executable");
    const changed = source.replace('["ls-files", "-z"]', '["ls-files", dynamicArg]');
    expect(changed).not.toBe(source);
    expect(references(name, changed)).toContain("!unresolved-executable");
  });
  test("binds current full raw Biome and library queries without suppressing unknown routes", () => {
    for (const name of [
      gate,
      "scripts/runtime-bundles/runtime-unused-semantic-adapter.ts",
      adapterTest,
    ]) {
      const actual = references(name);
      expect(actual).not.toContain("!unresolved-executable");
      expect(actual).not.toContain("!unresolved-import");
    }
    expect(references(adapterTest)).toContain(
      "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid/runtime_definition.ts",
    );
    expect(references(gate)).toContain("!execute-local:runtime-unused-semantic-node.ts");
  });
  test("accepts the reviewed MathJax preflight sources and rejects changed routes", () => {
    const cli = "scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts";
    const inputs = "scripts/runtime-bundles/runtime-mathjax-audit-evidence-inputs.ts";
    for (const name of [cli, inputs]) {
      expect(references(name)).not.toContain("!unresolved-executable");
    }
    const changedCli = read(cli).replace(
      'process.argv[2] === "preflight"',
      'process.argv[2] === "preflight-alt"',
    );
    expect(changedCli).not.toBe(read(cli));
    expect(references(cli, changedCli)).toContain("!unresolved-executable");
    const changedInputs = read(inputs).replace("process.env.BIOME_BINARY ||", "false ||");
    expect(changedInputs).not.toBe(read(inputs));
    expect(references(inputs, changedInputs)).toContain("!unresolved-executable");
  });
  test("binds only the hermetic duplication CLI that copies and hashes current sources", () => {
    const testPath = "scripts/quality/duplication-check_test.ts";
    const actual = references(testPath);
    expect(actual).toContain("!execute-local:duplication-check.ts");
    expect(actual).not.toContain("!unresolved-executable");
    const snapshot = repositorySnapshot(root, []);
    expect(
      snapshot.graph.errors.filter((error) => error.includes("!unresolved-executable")),
    ).toEqual([]);
  });
  test("keeps duplication CLI origin unresolved without its current source closure", () => {
    const testPath = "scripts/quality/duplication-check_test.ts";
    expect(references(testPath, read(testPath), () => undefined)).toContain(
      "!unresolved-executable",
    );
  });
  test.each([
    ["additional caller invocation", "", "\nrunDuplicationCli('/untrusted', ['--help']);\n"],
    ["global runtime alias", "", "\nconst childRuntime = Bun;\n"],
    ["copy source", '"owned-source-inventory.ts"', '"unreviewed-source.ts"'],
    ["copy destination", '"scripts/quality"', '"scripts/unreviewed"'],
    ["import alias", "  copyFileSync,", "  copyFileSync as copySource,"],
    [
      "source hash assertion",
      "sha256(readFileSync(path.join(import.meta.dir, source))),",
      "sha256(readFileSync(path.join(import.meta.dir, source))).toUpperCase(),",
    ],
    [
      "spawn substitution",
      "Bun.spawnSync([process.execPath, script, ...args], { cwd: repo });",
      'Bun.spawnSync(["bun", script, ...args], { cwd: repo });',
    ],
    [
      "additional command",
      "return Bun.spawnSync([process.execPath, script, ...args], { cwd: repo });",
      'Bun.spawnSync(["bun", "scripts/child.ts"]); return Bun.spawnSync([process.execPath, script, ...args], { cwd: repo });',
    ],
    [
      "shadowed runtime",
      "function runDuplicationCli(repo: string, args: string[]):",
      "function runDuplicationCli(repo: string, args: string[], Bun: unknown):",
    ],
  ])("rejects duplication CLI origin after %s mutation", (_label, before, after) => {
    const testPath = "scripts/quality/duplication-check_test.ts";
    const source = read(testPath);
    if (before) expect(source).toContain(before);
    const changed = before ? source.replace(before, after) : source + after;
    const importedAlias =
      _label === "import alias" ? changed.replaceAll("copyFileSync(", "copySource(") : changed;
    expect(importedAlias).not.toBe(source);
    expect(references(testPath, importedAlias)).toContain("!unresolved-executable");
  });
  test.each([
    ["raw diagnostic cap", "--max-diagnostics=none", "--max-diagnostics=1"],
    [
      "argv mutation",
      "const started = performance.now(),",
      'argv.push("--skip-errors"); const started = performance.now(),',
    ],
    [
      "Node query",
      "platform:process.platform,arch:process.arch",
      "platform:process.platform,arch:process.arch,extra:process.env",
    ],
    ["Node helper flag", '"--experimental-strip-types"', '"--eval"'],
  ])("rejects %s rather than accepting an unknown process", (_label, before, after) => {
    const source = read(gate).replace(before, after);
    expect(source).not.toBe(read(gate));
    expect(references(gate, source)).toContain("!unresolved-executable");
  });
  test("binds only the existing native receipt Git inventory tuples", () => {
    const name = "scripts/mermaid/native-build-receipt.ts",
      source = read(name);
    expect(references(name)).not.toContain("!unresolved-executable");
    for (const [before, after] of [
      ['executable = "git"', 'executable = "node"'],
      ['["ls-files", "-z"]', '["ls-files", "--recurse-submodules", "-z"]'],
      ['["ls-files", "--others", "--exclude-standard", "-z"]', '["ls-files", "--others", "-z"]'],
    ] as const) {
      expect(source.replace(before, after)).not.toBe(source);
      expect(references(name, source.replace(before, after))).toContain("!unresolved-executable");
    }
  });
  test("Git inventory rejects native/default and managed/pinned executable consumer drift", () => {
    const name = "scripts/mermaid/native-build-receipt.ts",
      source = read(name);
    expect(
      references(
        name,
        source.replace(
          "assertCurrentGitSourceSet(workingDirectory, manifest.files)",
          "assertCurrentGitSourceSet(workingDirectory, manifest.files, process.execPath)",
        ),
      ),
    ).toContain("!unresolved-executable");
    expect(
      references(name, source, (path) =>
        path === "scripts/mermaid/native-cli-managed-inputs.ts"
          ? read(path).replace(
              "snapshot.tools.git.path, snapshot.environment",
              "process.execPath, snapshot.environment",
            )
          : read(path),
      ),
    ).toContain("!unresolved-executable");
  });
  test("rejects a weakened package Biome command", () => {
    expect(
      references(gate, read(gate), (name) =>
        name === "package.json"
          ? read(name).replace("biome check --error-on-warnings", "biome check")
          : read(name),
      ),
    ).toContain("!unresolved-executable");
  });
  test.each([
    [
      "factory path",
      "await import(path.join(selectedRoot, entrypointOwnerPaths[1]))",
      "await import(path.join(selectedRoot, entrypointOwnerPaths[0]))",
    ],
    [
      "producer copy",
      "fs.copyFileSync(path.join(root, dom), path.join(selectedRoot, dom))",
      "fs.copyFileSync(path.join(root, dom), path.join(selectedRoot, dom + '.js'))",
    ],
    ["fake SDK output", '()=>()=>"FAKE"', '() => () => import("node:child_process")'],
  ])("rejects changed %s scratch consumer ownership", (_label, before, after) => {
    const source = read(adapterTest).replace(before, after);
    expect(source).not.toBe(read(adapterTest));
    expect(references(adapterTest, source)).toContain("!unresolved-import");
  });
  test("rejects catalog drift and an extra dynamic import", () => {
    expect(
      references(adapterTest, read(adapterTest), (name) =>
        read(name).replace("/mermaid/runtime_definition.ts", "/mermaid/unowned.ts"),
      ),
    ).toContain("!unresolved-import");
    expect(
      references(adapterTest, `${read(adapterTest)}\nawait import(process.env.UNKNOWN_MODULE);`),
    ).toContain("!unresolved-import");
  });
});
