import { afterAll, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { getLineInfo } from "acorn";
import { SourceMapGenerator } from "source-map-js";
import { z } from "zod";
import { RuntimeBundleDefinition, type RuntimeFragmentTransform } from "#shared/runtime_bundle";
import { MathJaxRuntimeBundleDefinition } from "../../crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition";
import { resolveOfficialRendererAssetInputs } from "../mermaid/official-renderer-asset-inputs";
import { RuntimeBundleChecks } from "./runtime-bundle-checks";
import { RuntimeBundleCommand } from "./runtime-bundle-command";
import { RuntimeBundlePaths } from "./runtime-bundle-paths";
import type { GeneratedBundle } from "./runtime-bundle-types";
import { RuntimeFragmentLoader } from "./runtime-fragment-loader";
import { RuntimeRollupBundler, type RuntimeRollupInspection } from "./runtime-rollup-bundler";
import {
  auditRuntimeUnused,
  collectStandardUnusedRawBiome,
  inspectMathJaxDefaultStaticClosure,
  inspectStandardUnusedGate,
  inspectStandardUnusedRawReport,
  inspectUnusedBiomeReport,
  MATHJAX_GENERATED_OUTPUT_PATH,
  runStandardUnusedGate,
  unusedProgramMetadata,
} from "./runtime-unused-check";
import {
  incompleteUnusedConsumers,
  inspectNativeWidthResultLiteral,
  inspectNodeFunctionPrograms,
  inspectOfficialStrictPrograms,
  inspectOfficialUnusedCallbacks,
  nativeWidthUnusedContext,
  nodeFunctionUnusedContext,
  officialMermaidUnusedContext,
  officialStrictConsumerRoute,
  officialStrictUnusedContext,
  readUnusedPhysicalSource,
  runtimeUnusedContexts,
  sequenceVmUnusedContext,
  sharedSvgCaptureUnusedContext,
  verifyOfficialStrictExternalInputs,
  verifyUnusedSources,
} from "./runtime-unused-contexts";
import {
  createMathJaxModuleInspector,
  inspectMathJaxRustFacts,
  MATHJAX_SOURCE_PATH,
  type MathJaxModuleInspectionInput,
  type MathJaxModuleInspector,
  validateMathJaxRustFacts,
} from "./runtime-unused-mathjax";
import { NATIVE_WIDTH_HASH_INPUT_PATHS } from "./runtime-unused-native-width-hash";
import {
  BrowserUnusedOrigin,
  inspectSharedSvgCaptureCallback,
  inspectSharedSvgCaptureCallers,
  inspectSharedSvgCaptureProgram,
  physicalUnusedSource,
  RuntimeUnusedOrigin,
  sequenceVmPaths,
  sharedSvgCaptureRoute,
  sharedSvgProgramSchema,
  type UnusedDiagnostic,
} from "./runtime-unused-origin";
import { RUST_CLASSIC_INPUT_PATHS } from "./runtime-unused-rust-classic";

const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "unused-origin-")));
let next = 0;
afterAll(() => fs.rmSync(directory, { recursive: true, force: true }));

test("standard unused context entry rejects malformed explicit classic input before inspection", () => {
  const root = fs.realpathSync(process.cwd());
  const inputs = { argv: ["--classic-normal-input", "{}"], env: {}, cwd: root, home: directory };
  expect(() => runtimeUnusedContexts(root, inputs)).toThrow();
});

test("standard unused audit entry never converts broken specified capture into missing success", async () => {
  const root = fs.realpathSync(process.cwd());
  const inputs = { argv: ["--classic-zenuml-input", "{}"], env: {}, cwd: root, home: directory };
  await expect(auditRuntimeUnused(root, directory, inputs)).rejects.toThrow();
});

test("MathJax static materializer proof leaves the other three consumers incomplete", () => {
  expect(incompleteUnusedConsumers).toHaveLength(3);
  expect(incompleteUnusedConsumers).not.toContain(
    "MathJax ESM consumer (Rust materializer and runtime route are not yet closed)",
  );
});
function fixture(
  source: string,
  transform: RuntimeFragmentTransform = "verbatim",
  entrypoint = "",
  occurrences = 1,
) {
  const sourcePath = `raw-${next++}.js`;
  fs.writeFileSync(path.join(directory, sourcePath), source);
  const definition = new RuntimeBundleDefinition(
    "drawio",
    "test.js",
    Array.from({ length: occurrences }, () => ({ label: "fixture.js", sourcePath, transform })),
    entrypoint,
  );
  const provenance = new RuntimeFragmentLoader(directory).loadDefinitionWithProvenance(definition);
  const raw = new Map([
    [sourcePath, physicalUnusedSource(fs.readFileSync(path.join(directory, sourcePath)))],
  ]);
  const rawSource = raw.get(sourcePath);
  if (!rawSource) throw new Error("missing fixture raw source");
  return {
    sourcePath,
    provenance,
    raw,
    rawSource,
    origin: new RuntimeUnusedOrigin(provenance, raw),
  };
}
function diagnosticAt(code: string, start: number, length = 12): UnusedDiagnostic {
  const point = (offset: number) => {
    const prefix = code.slice(0, offset);
    return {
      line: prefix.split("\n").length,
      column: Array.from(prefix.slice(prefix.lastIndexOf("\n") + 1)).length + 1,
    };
  };
  return {
    severity: "warning",
    category: "lint/correctness/noUnusedVariables",
    message: "unused",
    location: { path: "context.js", start: point(start), end: point(start + length) },
  };
}

const repositoryRoot = path.resolve(import.meta.dir, "../..");
// 実Rollupで全4 bundleを生成しguarded CLIも動かすintegrationは15.26秒の実測だった。これはテスト用の有限予算で、製品性能の受入値ではない。
const MATHJAX_PRODUCER_INTEGRATION_TIMEOUT_MS = 30_000;
let mathJaxInspection: Promise<RuntimeRollupInspection> | undefined;
let mathJaxInspector: Promise<MathJaxModuleInspector> | undefined;
let checkedBundlesPromise: ReturnType<RuntimeBundleCommand["generateCheckedBundles"]> | undefined;
function canonicalMathJaxInspection(): Promise<RuntimeRollupInspection> {
  if (mathJaxInspection === undefined)
    mathJaxInspection = new RuntimeRollupBundler(repositoryRoot).inspect(
      MathJaxRuntimeBundleDefinition.create(),
    );
  return mathJaxInspection;
}

function checkedBundles(): ReturnType<RuntimeBundleCommand["generateCheckedBundles"]> {
  if (checkedBundlesPromise === undefined)
    checkedBundlesPromise = new RuntimeBundleCommand(
      repositoryRoot,
      "check",
    ).generateCheckedBundles();
  return checkedBundlesPromise;
}

async function mathJaxInspectionCopy(): Promise<MathJaxModuleInspectionInput> {
  return structuredClone(await canonicalMathJaxInspection()) as MathJaxModuleInspectionInput;
}

async function isolatedMathJaxRoot(): Promise<string> {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(directory, "mathjax-isolated-")));
  const inspection = await (await canonicalMathJaxInspector()).inspectCanonical();
  const copy = (sourcePath: string) => {
    const target = path.resolve(root, sourcePath);
    if (target !== root && !target.startsWith(`${root}${path.sep}`))
      throw new Error(`MathJax isolated source escapes fixture root: ${sourcePath}`);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repositoryRoot, sourcePath), target);
  };
  for (const { sourcePath } of inspection.physicalSources) copy(sourcePath);
  for (const sourcePath of [
    MATHJAX_SOURCE_PATH,
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition.ts",
    "package.json",
    "bun.lock",
    "node_modules/@mathjax/src/package.json",
  ])
    copy(sourcePath);
  return root;
}

function canonicalMathJaxInspector(): Promise<MathJaxModuleInspector> {
  if (mathJaxInspector === undefined)
    mathJaxInspector = createMathJaxModuleInspector(repositoryRoot);
  return mathJaxInspector;
}
function secondMathJaxModule(value: MathJaxModuleInspectionInput) {
  const module = value.modules[1];
  if (!module) throw new Error("fixture second module missing");
  return module;
}

test("MathJax adapter binds the canonical Rollup graph, map, and global", async () => {
  const result = (await canonicalMathJaxInspector()).inspect(await mathJaxInspectionCopy());
  expect(result).toMatchObject({
    acceptedConsumerClosure: false,
    auditOnly: true,
    input: "\0krr-runtime-bundle:mathjax.js",
    moduleCount: 220,
    staticImportCount: 5,
    sourceMapSourceCount: 219,
    externalNodeCount: 0,
    dynamicNodeCount: 0,
    canonicalInspectionGlobalVerified: true,
    rustCallerVerified: false,
    overrideRouteVerified: false,
  });
  expect(result.physicalSources).toHaveLength(219);
  expect(result.physicalInputPins).toHaveLength(3);
  expect(result.global.sourcePath).toBe(
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime/render_mathjax.js",
  );
});

test("MathJax trusted origin is immutable across repeated inspection", async () => {
  const inspector = await canonicalMathJaxInspector();
  const origin = inspector.inspect(await mathJaxInspectionCopy());
  expect(Object.isFrozen(origin)).toBe(true);
  expect(Object.isFrozen(origin.physicalSources)).toBe(true);
  expect(Object.isFrozen(origin.physicalSources[0])).toBe(true);
  expect(Object.isFrozen(origin.physicalInputPins)).toBe(true);
  expect(Object.isFrozen(origin.physicalInputPins[0])).toBe(true);
  expect(Object.isFrozen(origin.global)).toBe(true);
  expect(Reflect.set(origin as object, "auditOnly", false)).toBe(false);
  expect(inspector.inspectCanonical().auditOnly).toBe(true);
});

test(
  "checked production bundles expose immutable bytes only after the full canonical check",
  async () => {
    const bundles = await checkedBundles();
    expect(Object.isFrozen(bundles)).toBe(true);
    expect(bundles.map(({ definition }) => definition.name)).toEqual([
      "mermaid",
      "drawio",
      "zenuml",
      "mathjax",
    ]);
    const mathjax = bundles.find(({ definition }) => definition.name === "mathjax");
    if (!mathjax) throw new Error("checked MathJax bundle is missing");
    expect(mathjax.definition.outputFile).toBe("mathjax-runtime.min.js");
    expect(mathjax.definition.fragments).toHaveLength(0);
    expect(mathjax.definition.bundledEntryPath).toBe(
      "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime/render_mathjax.js",
    );
    expect(Object.isFrozen(mathjax)).toBe(true);
    expect(Object.isFrozen(mathjax.definition)).toBe(true);
    expect(Object.isFrozen(mathjax.definition.fragments)).toBe(true);
    expect(fs.readFileSync(mathjax.outputPath, "utf8")).toBe(mathjax.content);
    expect(Reflect.set(mathjax as object, "content", "forged")).toBe(false);
    expect(Reflect.set(mathjax.definition as object, "bundledEntryPath", "forged.js")).toBe(false);
  },
  MATHJAX_PRODUCER_INTEGRATION_TIMEOUT_MS,
);

test(
  "MathJax default static closure is collected from canonical repository inputs",
  async () => {
    const proof = await inspectMathJaxDefaultStaticClosure(repositoryRoot);
    expect(proof).toEqual({
      defaultStaticClosed: true,
      incompleteConsumers: [...incompleteUnusedConsumers],
      acceptedConsumerClosure: false,
      generatedGlobalVerified: false,
      externalOverrideVerified: false,
      nativeRuntimeVerified: false,
    });
  },
  MATHJAX_PRODUCER_INTEGRATION_TIMEOUT_MS,
);

test("checked MathJax bundle boundary rejects content, checksum, and definition drift", async () => {
  const bundles = await checkedBundles();
  const mathjaxIndex = bundles.findIndex(({ definition }) => definition.name === "mathjax");
  const bundle = bundles[mathjaxIndex];
  if (bundle === undefined) throw new Error("checked MathJax bundle is missing");
  const checks = new RuntimeBundleChecks(new RuntimeBundlePaths(repositoryRoot));
  const assertRejected = (candidate: GeneratedBundle) => {
    const changed = [...bundles];
    changed[mathjaxIndex] = candidate;
    expect(() => checks.checkGeneratedBundles(changed)).toThrow();
  };

  assertRejected({ ...bundle, content: `${bundle.content}\n` });
  assertRejected({ ...bundle, checksum: "0".repeat(64) });
  assertRejected({
    ...bundle,
    definition: { ...bundle.definition, outputFile: "not-mathjax.min.js" },
  });
});

test("Rust facts validator rejects a mutated generated MathJax source pin", () => {
  const facts = structuredClone(inspectMathJaxRustFacts(repositoryRoot));
  facts.inputSha256[MATHJAX_GENERATED_OUTPUT_PATH] = "0".repeat(64);
  expect(() => validateMathJaxRustFacts(repositoryRoot, facts)).toThrow(
    `MathJax Rust facts input hash drift: ${MATHJAX_GENERATED_OUTPUT_PATH}`,
  );
});

test("isolated real MathJax fixture rejects source and generation-input drift", async () => {
  const root = await isolatedMathJaxRoot();
  const previousCwd = process.cwd();
  process.chdir(root);
  let inspector: MathJaxModuleInspector;
  let candidate: MathJaxModuleInspectionInput;
  try {
    inspector = await createMathJaxModuleInspector(root);
    candidate = structuredClone(
      await new RuntimeRollupBundler(root).inspect(MathJaxRuntimeBundleDefinition.create()),
    ) as MathJaxModuleInspectionInput;
  } finally {
    process.chdir(previousCwd);
  }
  const drift = (sourcePath: string, change: (source: Buffer) => Buffer, message: string) => {
    const target = path.join(root, sourcePath);
    const original = fs.readFileSync(target);
    try {
      fs.writeFileSync(target, change(original));
      expect(() => inspector.inspectCanonical(), message).toThrow();
      expect(() => inspector.inspect(candidate), message).toThrow();
    } finally {
      fs.writeFileSync(target, original);
    }
  };
  drift(
    MATHJAX_SOURCE_PATH,
    (source) => Buffer.concat([source, Buffer.from("\n")]),
    "mapped render source drift",
  );
  drift(
    "package.json",
    (source) => Buffer.concat([source, Buffer.from("\n")]),
    "generation input drift",
  );
});

test.each([
  ["wrong entry", (value: MathJaxModuleInspectionInput) => (value.input = "forged-entry")],
  [
    "missing entry",
    (value: MathJaxModuleInspectionInput) => {
      const entry = value.modules.find((module) => module.isEntry);
      if (!entry) throw new Error("fixture entry missing");
      entry.isEntry = false;
    },
  ],
  [
    "dynamic edge",
    (value: MathJaxModuleInspectionInput) =>
      secondMathJaxModule(value).dynamicallyImportedIds.push("forged"),
  ],
  [
    "external edge",
    (value: MathJaxModuleInspectionInput) => (secondMathJaxModule(value).isExternal = true),
  ],
  [
    "global drift",
    (value: MathJaxModuleInspectionInput) => {
      value.output.code = value.output.code.replace(
        "katanaRunMathJaxRuntime",
        "katanaRunMathJaxRuntimX",
      );
    },
  ],
  [
    "source content drift",
    (value: MathJaxModuleInspectionInput) => {
      const map = value.output.map as { sources: string[]; sourcesContent: string[] };
      const index = map.sources.findIndex((source) => source.endsWith("render_mathjax.js"));
      if (index < 0) throw new Error("fixture render source missing");
      map.sourcesContent[index] += "\nforged";
    },
  ],
  [
    "asset map drift",
    (value: MathJaxModuleInspectionInput) => {
      const source = value.sourceMapAsset.source;
      if (typeof source !== "string") throw new Error("fixture source asset is not text");
      const map = JSON.parse(source) as Record<string, unknown>;
      map.file = "forged.js";
      value.sourceMapAsset.source = JSON.stringify(map);
    },
  ],
  [
    "same-length output body drift",
    (value: MathJaxModuleInspectionInput) => {
      const index = value.output.code.length - 1;
      const current = value.output.code[index];
      if (current === undefined) throw new Error("fixture output code missing");
      value.output.code = `${value.output.code.slice(0, index)}${current === ";" ? ":" : ";"}`;
    },
  ],
  [
    "vendor module code drift",
    (value: MathJaxModuleInspectionInput) => {
      const module = secondMathJaxModule(value);
      if (module.code === null) throw new Error("fixture vendor module code missing");
      module.code = `${module.code.slice(0, -1)}${module.code.endsWith(";") ? ":" : ";"}`;
    },
  ],
  [
    "valid vendor static edge drift",
    (value: MathJaxModuleInspectionInput) => {
      const module = value.modules.find((candidate) => candidate.importedIds.length > 0);
      if (!module) throw new Error("fixture imported module missing");
      const replacement = value.modules.find((candidate) => candidate.id !== module.importedIds[0]);
      if (!replacement) throw new Error("fixture replacement module missing");
      module.importedIds[0] = replacement.id;
    },
  ],
  [
    "reverse importer drift",
    (value: MathJaxModuleInspectionInput) => {
      const module = value.modules.find((candidate) => candidate.importers.length > 0);
      if (!module) throw new Error("fixture importer missing");
      const importers = [...module.importers].reverse();
      module.importers =
        importers.length > 1 ? importers : [...importers, importers[0] ?? "forged"];
    },
  ],
  [
    "output module metadata drift",
    (value: MathJaxModuleInspectionInput) => {
      const id = Object.keys(value.output.modules)[0];
      if (id === undefined) throw new Error("fixture output metadata missing");
      value.output.modules[id] = { forged: true };
    },
  ],
  [
    "module count drift",
    (value: MathJaxModuleInspectionInput) => {
      value.output.modules = Object.fromEntries(Object.entries(value.output.modules).slice(1));
    },
  ],
  [
    "compatible duplicate map source",
    (value: MathJaxModuleInspectionInput) => {
      const map = value.output.map as { sources: string[]; sourcesContent: string[] };
      const source = map.sources[0];
      const content = map.sourcesContent[0];
      if (source === undefined || content === undefined)
        throw new Error("fixture map source missing");
      map.sources[1] = source;
      map.sourcesContent[1] = content;
    },
  ],
])("MathJax adapter rejects %s", async (_name, mutate) => {
  const value = await mathJaxInspectionCopy();
  mutate(value);
  await expect(
    canonicalMathJaxInspector().then((inspector) => inspector.inspect(value)),
  ).rejects.toThrow();
});

test("MathJax adapter compares source-map metadata semantically across key order", async () => {
  const value = await mathJaxInspectionCopy();
  const source = value.sourceMapAsset.source;
  if (typeof source !== "string") throw new Error("fixture source asset is not text");
  const map = JSON.parse(source) as Record<string, unknown>;
  value.sourceMapAsset.source = JSON.stringify(Object.fromEntries(Object.entries(map).reverse()));
  await expect(
    canonicalMathJaxInspector().then((inspector) => inspector.inspect(value)),
  ).resolves.toMatchObject({
    moduleCount: 220,
  });
});

test.each([
  "/*😀𐐀e\u0301*/ const unusedMarker=1;\n",
  "// header\r\n/*😀𐐀e\u0301*/ const unusedMarker=1;\r\n",
  "/*😀𐐀🧪*/ const unusedMarker=1;",
  "/*e\u0301a\u0308*/ const unusedMarker=1;",
  "\ufeffconst unusedMarker=1;",
])("exact Unicode/CRLF/BOM diagnostic span retains physical origin", (source) => {
  const f = fixture(source);
  for (const stage of ["assembled", "normalized"] as const) {
    const code = stage === "assembled" ? f.origin.assembled : f.provenance.code;
    const mapped = f.origin.resolve(diagnosticAt(code, code.indexOf("unusedMarker")), stage);
    expect(mapped).toEqual({
      sourcePath: f.sourcePath,
      sourceSha256: f.rawSource.sha256,
      occurrence: 0,
      start: source.indexOf("unusedMarker"),
      end: source.indexOf("unusedMarker") + 12,
    });
  }
});

test.each(["label", "separator", "entrypoint", "replacement"])(
  "%s never becomes raw origin",
  (kind) => {
    const source =
      kind === "replacement"
        ? "function same(){return 1;}\nfunction same(){return 2;}"
        : "const unusedMarker=1;";
    const f = fixture(source, "verbatim", "virtualEntry();");
    const label = f.provenance.generatedSpans.find((span) => span.kind === "label");
    if (!label) throw new Error("missing label");
    const separator = f.provenance.generatedSpans.find((span) => span.kind === "separator");
    if (!separator) throw new Error("missing separator");
    const offsets = {
      label: f.provenance.code.indexOf("fixture.js"),
      separator: separator.start,
      entrypoint: f.provenance.code.indexOf("virtualEntry"),
      replacement: label.end,
    };
    expect(() =>
      f.origin.resolve(diagnosticAt(f.provenance.code, offsets[kind], 1), "normalized"),
    ).toThrow();
  },
);

test("same raw path keeps real fragment occurrence rather than filename-only attribution", () => {
  const f = fixture("function unusedMarker(){}", "verbatim", "", 2);
  expect(
    f.origin.resolve(
      diagnosticAt(f.origin.assembled, f.origin.assembled.lastIndexOf("unusedMarker")),
      "assembled",
    ).occurrence,
  ).toBe(1);
  expect(
    f.origin.resolve(
      diagnosticAt(f.provenance.code, f.provenance.code.indexOf("unusedMarker")),
      "normalized",
    ).occurrence,
  ).toBe(1);
});

test.each(["gap", "transform"])("exact mapping %s cannot borrow a nearby raw anchor", (kind) => {
  const f = fixture(
    kind === "transform"
      ? "katanaInstallDrawioRuntimeAdapter();\nconst unusedMarker=1;"
      : "const unusedMarker=1;",
    kind === "transform" ? "drawio-runtime-adapter-without-installer" : "verbatim",
  );
  const map = new SourceMapGenerator();
  const code = f.origin.assembled;
  const raw = f.raw.get(f.sourcePath);
  if (!raw) throw new Error("missing fixture raw");
  map.setSourceContent(f.sourcePath, raw.code);
  map.addMapping({
    generated: getLineInfo(code, code.indexOf("unusedMarker")),
    original: getLineInfo(raw.code, kind === "transform" ? 0 : raw.code.indexOf("unusedMarker")),
    source: f.sourcePath,
  });
  const origin = new RuntimeUnusedOrigin({ ...f.provenance, assemblyMap: map.toString() }, f.raw);
  expect(() =>
    origin.resolve(diagnosticAt(code, code.indexOf("unusedMarker")), "assembled"),
  ).toThrow();
});

test.each(["source", "sha", "frame", "map-source"])("changed %s rejects provenance", (kind) => {
  const f = fixture("const unusedMarker=1;");
  const raw = f.raw.get(f.sourcePath);
  if (!raw) throw new Error("missing fixture raw");
  if (kind === "source") f.raw.set(f.sourcePath, { ...raw, code: "const changed=2;" });
  if (kind === "sha") f.raw.set(f.sourcePath, { ...raw, sha256: "0".repeat(64) });
  const map = z.record(z.string(), z.unknown()).parse(JSON.parse(f.provenance.assemblyMap));
  if (kind === "map-source") map.sources = ["unknown.js"];
  const provenance = {
    ...f.provenance,
    assemblyMap: JSON.stringify(map),
    rawInputs:
      kind === "frame"
        ? f.provenance.rawInputs.map((input) => ({ ...input, occurrence: 2 }))
        : f.provenance.rawInputs,
  };
  expect(() => new RuntimeUnusedOrigin(provenance, f.raw)).toThrow();
});

test.each([0, -1, 100000, 1.5])("invalid Biome column %s fails without coercion", (column) => {
  const f = fixture("const unusedMarker=1;");
  const diagnostic = diagnosticAt(f.provenance.code, f.provenance.code.indexOf("unusedMarker"));
  diagnostic.location.start.column = column;
  expect(() => f.origin.resolve(diagnostic, "normalized")).toThrow();
});

test("raw coverage retains transformed/deleted and unmapped spans as incomplete", () => {
  const f = fixture(
    "katanaInstallDrawioRuntimeAdapter();\nconst unusedMarker=1;",
    "drawio-runtime-adapter-without-installer",
  );
  const coverage = f.origin.coverage("normalized")[0];
  expect(coverage?.uncoveredUtf16).toBeGreaterThan(0);
  expect(coverage?.transformOut).toHaveLength(1);
});

test("physical source drift/symlink/unsafe path and malformed UTF8 reject", () => {
  const f = fixture("const unusedMarker=1;");
  fs.appendFileSync(path.join(directory, f.sourcePath), " ");
  expect(() => verifyUnusedSources(directory, f.raw)).toThrow("physical raw source changed");
  expect(() => readUnusedPhysicalSource(directory, "../outside.js")).toThrow();
  const linked = path.join(directory, `link-${next++}.js`);
  fs.symlinkSync(path.join(directory, f.sourcePath), linked);
  expect(() => readUnusedPhysicalSource(directory, path.basename(linked))).toThrow("symlink");
  for (const byte of [0xff, 0xfe])
    expect(() => physicalUnusedSource(Uint8Array.of(byte))).toThrow();
});

test.each(["truncated", "unknown-category", "count", "exit", "missing"])(
  "Biome %s report remains failclosed",
  (kind) => {
    const diagnostic = diagnosticAt("unusedMarker", 0);
    const report = {
      summary: {
        warnings: kind === "count" ? 2 : 1,
        errors: 0,
        diagnosticsNotPrinted: kind === "truncated" ? 1 : 0,
      },
      diagnostics: kind === "missing" ? [] : [diagnostic],
    };
    if (kind === "unknown-category") diagnostic.category = "lint/unknown";
    expect(() => inspectUnusedBiomeReport(report, kind === "exit" ? 0 : 1)).toThrow();
  },
);

test("real warning report remains a warning and empty success has complete summary", () => {
  const diagnostic = diagnosticAt("unusedMarker", 0);
  expect(
    inspectUnusedBiomeReport(
      { summary: { warnings: 1, errors: 0, diagnosticsNotPrinted: 0 }, diagnostics: [diagnostic] },
      1,
    ),
  ).toHaveLength(1);
  expect(
    inspectUnusedBiomeReport(
      { summary: { warnings: 0, errors: 0, diagnosticsNotPrinted: 0 }, diagnostics: [] },
      0,
    ),
  ).toEqual([]);
});

test("conflicting exact mappings are ambiguous even at the same generated point", () => {
  const f = fixture("const unusedMarker=1;");
  const map = new SourceMapGenerator();
  const raw = f.rawSource.code;
  const generated = getLineInfo(f.origin.assembled, f.origin.assembled.indexOf("unusedMarker"));
  map.setSourceContent(f.sourcePath, raw);
  for (const offset of [raw.indexOf("unusedMarker"), raw.indexOf("unusedMarker") + 1])
    map.addMapping({ generated, original: getLineInfo(raw, offset), source: f.sourcePath });
  expect(
    () => new RuntimeUnusedOrigin({ ...f.provenance, assemblyMap: map.toString() }, f.raw),
  ).toThrow("ambiguous exact mapping");
});

test("generated metadata inside raw-looking code still cannot become raw origin", () => {
  const f = fixture("const unusedMarker=1;");
  const start = f.origin.assembled.indexOf("unusedMarker");
  const origin = new RuntimeUnusedOrigin(
    {
      ...f.provenance,
      generatedSpans: [
        ...f.provenance.generatedSpans,
        {
          kind: "separator",
          start,
          end: start + 12,
          startPosition: getLineInfo(f.origin.assembled, start),
          endPosition: getLineInfo(f.origin.assembled, start + 12),
        },
      ],
    },
    f.raw,
  );
  expect(() =>
    origin.resolve(
      diagnosticAt(f.provenance.code, f.provenance.code.indexOf("unusedMarker")),
      "normalized",
    ),
  ).toThrow();
});

test.each(["\r", "\u2028", "\u2029"])("unobserved line terminator remains unknown", (separator) => {
  expect(() => fixture(`const first=1;${separator}const unusedMarker=1;`)).toThrow(
    "unsupported line terminator",
  );
});

test("boolean position and empty reversed spans are rejected", () => {
  const f = fixture("const unusedMarker=1;");
  const diagnostic = diagnosticAt(f.provenance.code, f.provenance.code.indexOf("unusedMarker"));
  expect(() =>
    inspectUnusedBiomeReport(
      {
        summary: { warnings: 1, errors: 0, diagnosticsNotPrinted: 0 },
        diagnostics: [
          { ...diagnostic, location: { ...diagnostic.location, start: { line: true, column: 1 } } },
        ],
      },
      1,
    ),
  ).toThrow();
  diagnostic.location.end = diagnostic.location.start;
  expect(() => f.origin.resolve(diagnostic, "normalized")).toThrow("invalid diagnostic span");
});

test.each(["assembly", "normalization"])(
  "changed %s mapping cannot borrow an identical later token",
  (stage) => {
    const f = fixture("const unusedMarker=1; function later(){return unusedMarker;}");
    const source = stage === "assembly" ? f.rawSource.code : f.origin.assembled;
    const generatedCode = stage === "assembly" ? f.origin.assembled : f.provenance.code;
    const name = stage === "assembly" ? f.sourcePath : "@runtime/assembled";
    const map = new SourceMapGenerator();
    map.setSourceContent(name, source);
    for (let index = 0; index < 12; index++)
      map.addMapping({
        generated: getLineInfo(generatedCode, generatedCode.indexOf("unusedMarker") + index),
        original: getLineInfo(source, source.lastIndexOf("unusedMarker") + index),
        source: name,
      });
    const provenance = {
      ...f.provenance,
      [stage === "assembly" ? "assemblyMap" : "normalizationMap"]: map.toString(),
    };
    const origin = new RuntimeUnusedOrigin(provenance, f.raw);
    expect(() =>
      origin.resolve(
        diagnosticAt(generatedCode, generatedCode.indexOf("unusedMarker")),
        stage === "assembly" ? "assembled" : "normalized",
      ),
    ).toThrow();
  },
);

const browserRoot = fs.realpathSync(path.resolve(import.meta.dir, "../.."));
const browserRaw = new Map<string, ReturnType<typeof physicalUnusedSource>>();
const browserContext = officialMermaidUnusedContext(browserRoot, browserRaw);
const browserPrograms = browserContext.programs;
if (!browserPrograms) throw new Error("missing browser program metadata");

test("official browser provider order and callback modes retain actual producer programs", () => {
  const providers = browserPrograms.filter((program) => program.role === "provider");
  expect(providers).toHaveLength(25);
  expect(providers[0]?.sourcePath.endsWith("/dom_text_metrics.js")).toBe(true);
  expect(providers[1]?.sourcePath.endsWith("/svg_bounds_attrs.js")).toBe(true);
  expect(providers[24]?.sourcePath.endsWith("/mermaid_zenuml_bridge.js")).toBe(true);
  const physicalProviders = [...browserRaw].filter(([sourcePath]) =>
    sourcePath.startsWith("crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/"),
  );
  expect(physicalProviders).toHaveLength(31);
  expect(physicalProviders.filter(([, source]) => source.code.length === 0)).toHaveLength(6);
  expect(providers.every((program) => program.start < program.end)).toBe(true);
  expect(providers.map((program) => program.sourcePath)).toEqual(
    physicalProviders
      .filter(([, source]) => source.code.length > 0)
      .map(([sourcePath]) => sourcePath),
  );
  const callbacks = browserPrograms.filter((program) => program.role === "callback");
  expect(callbacks).toHaveLength(7);
  expect(callbacks.map((program) => program.mode)).toEqual([
    "both",
    "owned-reference",
    "both",
    "both",
    "owned-reference",
    "strictVendorOriginal",
    "owned-reference",
  ]);
  expect(
    callbacks.every(
      (program) => program.runtimeAstSha256?.length === 64 && program.callAstSha256?.length === 64,
    ),
  ).toBe(true);
  expect(browserContext.stages).toEqual(["assembled"]);
  expect(browserContext.codeBytesEqual).toBe(false);
  expect(browserContext.origin.coverage("assembled")).toHaveLength(31);
  verifyUnusedSources(browserRoot, browserRaw);
});

test.each(["katanaSvgTextBox", "katanaNormalizeMermaidSvg"])(
  "browser whole diagnostic %s retains actual physical provider",
  (name) => {
    const code = browserContext.origin.assembled;
    const at = code.lastIndexOf(name);
    const origin = browserContext.origin.resolve(diagnosticAt(code, at, name.length), "assembled");
    const raw = browserRaw.get(origin.sourcePath);
    if (!raw) throw new Error("missing mapped browser raw source");
    expect(raw?.code.slice(origin.start, origin.end)).toBe(name);
    expect(origin.sourceSha256).toBe(raw?.sha256);
  },
);

test("actual callback identifier retains original physical TS span after type erasure", () => {
  const code = browserContext.origin.assembled;
  const marker = "__katanaOfficialReference";
  const at = code.lastIndexOf(marker);
  const origin = browserContext.origin.resolve(diagnosticAt(code, at, marker.length), "assembled");
  expect(origin.sourcePath).toBe("scripts/mermaid/official-renderer.ts");
  expect(browserRaw.get(origin.sourcePath)?.code.slice(origin.start, origin.end)).toBe(marker);
  expect(() =>
    browserContext.origin.resolve(diagnosticAt(code, at, marker.length), "normalized"),
  ).toThrow("not normalized");
  const label = code.indexOf("/* browser callback:");
  expect(() => browserContext.origin.resolve(diagnosticAt(code, label, 4), "assembled")).toThrow();
});

test.each([
  ["unknown producer", "scripts/mermaid/not-the-producer.ts", (code: string) => code],
  [
    "missing callback",
    "scripts/mermaid/official-renderer.ts",
    (code: string) => code.replace("page.evaluate", "page.other"),
  ],
  [
    "computed API",
    "scripts/mermaid/official-renderer.ts",
    (code: string) => code.replace("page.evaluate", 'page["evaluate"]'),
  ],
  [
    "escaped API",
    "scripts/mermaid/official-renderer.ts",
    (code: string) =>
      code.replace(
        "await page.evaluate(() => {",
        "const escaped=page.evaluate; await escaped(() => {",
      ),
  ],
  [
    "spread callback",
    "scripts/mermaid/official-renderer.ts",
    (code: string) => code.replace("page.evaluate(() =>", "page.evaluate(...() =>"),
  ],
])("browser callback boundary rejects %s", (label, sourcePath, change) => {
  expect(label.length).toBeGreaterThan(0);
  const code = browserRaw.get("scripts/mermaid/official-renderer.ts")?.code;
  if (!code) throw new Error("missing actual producer");
  expect(() => inspectOfficialUnusedCallbacks(sourcePath, change(code))).toThrow();
});

test.each(["sha", "order", "map-source", "tail"])(
  "browser projection rejects changed %s",
  (kind) => {
    const origin = browserContext.origin;
    if (!(origin instanceof BrowserUnusedOrigin)) throw new Error("missing browser origin");
    const raw = new Map(browserRaw);
    const programs = [...origin.programs];
    let code = origin.assembled,
      map = origin.map;
    const first = programs[0];
    if (!first) throw new Error("missing browser provider");
    if (kind === "sha") {
      const source = raw.get(first.sourcePath);
      if (!source) throw new Error("missing browser physical source");
      raw.set(first.sourcePath, { ...source, sha256: "0".repeat(64) });
    }
    if (kind === "order") programs.reverse();
    if (kind === "map-source") {
      const value = z
        .object({ sourcesContent: z.array(z.string()) })
        .passthrough()
        .parse(JSON.parse(map));
      value.sourcesContent[0] = "different physical source";
      map = JSON.stringify(value);
    }
    if (kind === "tail") code += "unknownCallback();";
    expect(() => new BrowserUnusedOrigin(code, map, programs, raw)).toThrow();
  },
);

test("browser diagnostic spanning erased type cannot borrow an adjacent raw mapping", () => {
  const code = browserContext.origin.assembled;
  const marker = "__katanaOfficialReference";
  const end = code.lastIndexOf(marker) + marker.length;
  const start = code.lastIndexOf("window", end);
  expect(() =>
    browserContext.origin.resolve(diagnosticAt(code, start, end - start), "assembled"),
  ).toThrow("discontinuous");
});

const determinismFile = "scripts/mermaid/official-renderer-determinism.ts";
const determinismCode = browserRaw.get(determinismFile)?.code;
if (!determinismCode) throw new Error("missing actual determinism producer");
const fakePage = "{ evaluate: () => Promise.resolve() }";
test.each([
  {
    kind: "reviewed fake same-name receiver",
    change: (code: string) =>
      code
        .replace("install(page:", "install(_inputPage:")
        .replace("    return page.evaluate", `    const page=${fakePage}; return page.evaluate`),
  },
  {
    kind: "nested local shadow",
    change: (code: string) =>
      code
        .replace("    return page.evaluate", `    { const page=${fakePage}; return page.evaluate`)
        .replace("    }, CONFIG);", "    }, CONFIG); }"),
  },
  {
    kind: "same-binding reassignment",
    change: (code: string) =>
      code.replace("    return page.evaluate", `    page=${fakePage}; return page.evaluate`),
  },
  {
    kind: "same-binding compound assignment",
    change: (code: string) =>
      code.replace("    return page.evaluate", `    page ||= ${fakePage}; return page.evaluate`),
  },
  {
    kind: "same-binding update",
    change: (code: string) =>
      code.replace("    return page.evaluate", "    page++; return page.evaluate"),
  },
  {
    kind: "receiver alias",
    change: (code: string) =>
      code.replace("    return page.evaluate", "    const receiver=page; return receiver.evaluate"),
  },
  {
    kind: "type-name-only impostor",
    change: (code: string) =>
      code.replace(
        'import type { PageHandle } from "./official-renderer-types";',
        "type PageHandle = unknown;",
      ),
  },
])("official callback receiver rejects $kind", ({ change }) => {
  expect(() => inspectOfficialUnusedCallbacks(determinismFile, change(determinismCode))).toThrow();
});

test("actual imported PageHandle receiver keeps surviving callback AST identities", () => {
  for (const sourcePath of [determinismFile, "scripts/mermaid/official-renderer.ts"]) {
    const code = browserRaw.get(sourcePath)?.code;
    if (!code) throw new Error("missing actual browser producer");
    const callbacks = inspectOfficialUnusedCallbacks(sourcePath, code);
    const previous = browserPrograms.filter(
      (program) => program.sourcePath === sourcePath && program.role === "callback",
    );
    for (const key of ["runtimeAstSha256", "callAstSha256"] as const) {
      const actual = callbacks.map((callback) => callback[key]);
      const expected = previous.map((program) => {
        const value = program[key];
        if (typeof value !== "string") throw new Error("missing actual callback AST identity");
        return value;
      });
      expect(actual).toEqual(expected);
    }
  }
});

const nativeBase = "crates/katana-render-runtime/src/markdown/mermaid_renderer/";
const nativeHelper = `${nativeBase}js_runtime_native_width_helpers.rs`;
const nativeTrace = `${nativeBase}js_runtime_native_width_trace.js`;
const nativeScripts = `${nativeBase}js_runtime_scripts.rs`;
const nativeRoute = [
  nativeHelper,
  `${nativeBase}js_runtime_native_width_tests.rs`,
  nativeTrace,
  ...NATIVE_WIDTH_HASH_INPUT_PATHS,
  "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
  "crates/katana-render-runtime/src/lib.rs",
  "crates/katana-render-runtime/src/v8_script.rs",
  ...RUST_CLASSIC_INPUT_PATHS,
].filter((sourcePath, index, route) => route.indexOf(sourcePath) === index);
interface NativeWidthFixture {
  nativeRoot: string;
  nativeRaw: Map<string, ReturnType<typeof physicalUnusedSource>>;
  nativeContext: ReturnType<typeof nativeWidthUnusedContext>;
  nativeOrigin: BrowserUnusedOrigin;
}

let nativeWidthFixtureValue: NativeWidthFixture | undefined;
function nativeWidthFixture(): NativeWidthFixture {
  if (nativeWidthFixtureValue !== undefined) return nativeWidthFixtureValue;
  const nativeRoot = fs.mkdtempSync(path.join(directory, "native-width-"));
  for (const name of new Set([...nativeRoute, ...RUST_CLASSIC_INPUT_PATHS])) {
    const target = path.join(nativeRoot, name);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(browserRoot, name), target);
  }
  const nativeRaw = new Map<string, ReturnType<typeof physicalUnusedSource>>();
  const nativeContext = nativeWidthUnusedContext(nativeRoot, nativeRaw);
  const nativeOrigin = nativeContext.origin;
  if (!(nativeOrigin instanceof BrowserUnusedOrigin))
    throw new Error("missing native width origin");
  nativeWidthFixtureValue = { nativeRoot, nativeRaw, nativeContext, nativeOrigin };
  return nativeWidthFixtureValue;
}

test("native width projection consumes actual trace and Rust result literal with source-only route", () => {
  const { nativeRoot, nativeRaw, nativeContext, nativeOrigin } = nativeWidthFixture();
  expect(nativeOrigin.programs.map((program) => program.role)).toEqual([
    "native-test-trace",
    "rust-result-literal",
  ]);
  expect(nativeContext.codeBytesEqual).toBe(false);
  const program = nativeOrigin.programs[1];
  const helper = nativeRaw.get(nativeHelper);
  if (!program || !helper) throw new Error("missing Rust result metadata");
  expect(helper.code.slice(program.start, program.end)).toBe(
    "JSON.stringify({ calls: globalThis.__krrActualWidths, probes: nativeWidthProbeResult() })",
  );
  expect(program.consumerRoute?.map(({ sourcePath }) => sourcePath)).toEqual(nativeRoute);
  expect(
    program.consumerRoute?.map(({ sourcePath, sourceSha256 }) => ({ sourcePath, sourceSha256 })),
  ).toEqual(
    nativeRoute.map((sourcePath) => {
      const source = nativeRaw.get(sourcePath);
      if (!source) throw new Error(`missing native route source: ${sourcePath}`);
      return { sourcePath, sourceSha256: source.sha256 };
    }),
  );
  expect(program.sourceSha256).toBe(helper.sha256);
  const metadata = unusedProgramMetadata(nativeContext.name, nativeOrigin.programs);
  expect(metadata.realm).toBe("Rust native-width test, shared V8 classic-script context");
  expect(metadata.externalProgramsUnmapped).not.toContain("options.mermaidJs");
  expect("executionVerified" in metadata && metadata.executionVerified).toBe(false);
  verifyUnusedSources(nativeRoot, nativeRaw);
});

test("native width keeps live source binding but ignores unrelated script test edits", () => {
  const { nativeRoot } = nativeWidthFixture();
  const target = path.join(nativeRoot, nativeScripts);
  const before = fs.readFileSync(target);
  const original = before.toString();
  const changed = original.replace(
    'assert!(scripts.iter().any(|it| it.name == "render-mermaid.js"));',
    'assert!(scripts.iter().any(|it| it.name == "mermaid-runtime.min.js"));',
  );
  expect(changed).not.toBe(original);
  try {
    fs.writeFileSync(target, changed);
    const raw = new Map<string, ReturnType<typeof physicalUnusedSource>>();
    const context = nativeWidthUnusedContext(nativeRoot, raw);
    const source = raw.get(nativeScripts);
    expect(source?.sha256).toBe(physicalUnusedSource(fs.readFileSync(target)).sha256);
    expect(source?.sha256).not.toBe(physicalUnusedSource(before).sha256);
    const program =
      context.origin instanceof BrowserUnusedOrigin
        ? context.origin.programs.find((entry) => entry.role === "rust-result-literal")
        : undefined;
    expect(
      program?.consumerRoute?.find((entry) => entry.sourcePath === nativeScripts)?.sourceSha256,
    ).toBe(source?.sha256);
  } finally {
    fs.writeFileSync(target, before);
  }
});

const nativeScriptRouteMutations = [
  [
    "constructor",
    "mermaid-scripts:entry",
    (source: string) =>
      source.replace(
        'DiagramRuntimeScript::borrowed("mermaid-runtime.min.js", MERMAID_RUNTIME)',
        'DiagramRuntimeScript::owned("mermaid-runtime.min.js", MERMAID_RUNTIME)',
      ),
  ],
  [
    "call",
    "mermaid-scripts:forward",
    (source: string) =>
      source.replace(
        "diagram_type == MermaidDiagramType::Zenuml",
        "diagram_type != MermaidDiagramType::Zenuml",
      ),
  ],
  [
    "bundle",
    "mermaid-scripts:entry-code",
    (source: string) =>
      source.replace('"mermaid.min.js", bundle', '"mermaid.min.js", zenuml_bundle'),
  ],
  [
    "entrypoint",
    "mermaid-scripts:format",
    (source: string) => source.replace("katanaRunMermaidRuntime", "changedMermaidRuntime"),
  ],
] as const;

test.each(nativeScriptRouteMutations)(
  "native width rejects Mermaid script %s tampering through existing Rust facts",
  (_kind, expectedFactError, mutate) => {
    const { nativeRoot } = nativeWidthFixture();
    const target = path.join(nativeRoot, nativeScripts);
    const before = fs.readFileSync(target);
    const original = before.toString();
    const changed = mutate(original);
    expect(changed).not.toBe(original);
    try {
      fs.writeFileSync(target, changed);
      expect(() => nativeWidthUnusedContext(nativeRoot, new Map())).toThrow(expectedFactError);
    } finally {
      fs.writeFileSync(target, before);
    }
  },
);

test.each(["trace declaration", "Rust result call"])(
  "native width mapping restores %s full raw span",
  (site) => {
    const { nativeRaw, nativeOrigin } = nativeWidthFixture();
    const code = nativeOrigin.assembled;
    const marker = "nativeWidthProbeResult";
    const start = site === "trace declaration" ? code.indexOf(marker) : code.lastIndexOf(marker);
    const origin = nativeOrigin.resolve(diagnosticAt(code, start, marker.length), "assembled");
    expect(origin.sourcePath).toBe(site === "trace declaration" ? nativeTrace : nativeHelper);
    expect(nativeRaw.get(origin.sourcePath)?.code.slice(origin.start, origin.end)).toBe(marker);
    expect(() =>
      nativeOrigin.resolve(diagnosticAt(code, start, marker.length), "normalized"),
    ).toThrow();
  },
);

const nativeRouteMutations = new Map<string, (source: string) => string>([
  [
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/mod.rs",
    (source) => source.replace("source: &str", "source: &String"),
  ],
  [
    `${nativeBase}js_runtime.rs`,
    (source) => source.replace('"mermaid-svg-id-v1"', '"changed-mermaid-svg-id"'),
  ],
  [
    `${nativeBase}render.rs`,
    (source) =>
      source.replace(
        "MERMAID_RENDER_CACHE_SCHEMA.hash(&mut hasher);",
        "CHANGED_RENDER_CACHE_SCHEMA.hash(&mut hasher);",
      ),
  ],
  [
    "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
    (source) =>
      source.replace(
        "rendered = evaluate(scope, script)?;",
        "let separate_context = v8::Context::new(scope, Default::default()); let mut separate_scope = v8::ContextScope::new(scope, separate_context); rendered = evaluate(&mut separate_scope, script)?;",
      ),
  ],
  [
    `${nativeBase}js_runtime_native_width_helpers.rs`,
    (source) => source.replace("scripts.push", "other_scripts.push"),
  ],
  [
    "crates/katana-render-runtime/src/lib.rs",
    (source) => source.replace("mod v8_script;", "pub mod v8_script;"),
  ],
  [
    "crates/katana-render-runtime/src/v8_script.rs",
    (source) =>
      source.replace(
        "v8::Script::compile(scope, source, Some(&origin))",
        "v8::Script::compile(scope, source, None)",
      ),
  ],
]);

const nativeDiagramRouteMutations = new Map<string, (source: string) => string>([
  [
    "trace target",
    (source) =>
      source.replace(
        "katana_render_runtime::mermaid_classic_capture",
        "changed::mermaid_classic_capture",
      ),
  ],
  ["trace phase", (source) => source.replace('phase = "script"', 'phase = "changed"')],
  ["source", (source) => source.replace("code = script_code", "code = script.name")],
  ["name", (source) => source.replace("name = script.name", "name = script_code")],
  [
    "compile target",
    (source) =>
      source.replace(
        "compile_with_origin(scope, source, origin_name)",
        "compile_with_origin(scope, source, source)",
      ),
  ],
  [
    "complete before resolve",
    (source) =>
      source.replace(
        '    let resolved = resolve_value(scope, value)?;\n    tracing::event!(\n        target: "katana_render_runtime::mermaid_classic_capture",\n        tracing::Level::TRACE,\n        phase = "complete",\n    );',
        '    tracing::event!(\n        target: "katana_render_runtime::mermaid_classic_capture",\n        tracing::Level::TRACE,\n        phase = "complete",\n    );\n    let resolved = resolve_value(scope, value)?;',
      ),
  ],
  [
    "skip resolve",
    (source) =>
      source.replace(
        "let resolved = resolve_value(scope, value)?;",
        "let resolved = value.to_rust_string_lossy(scope);",
      ),
  ],
]);

test.each([...nativeDiagramRouteMutations])(
  "native width rejects changed diagram route: %s",
  (_kind, mutate) => {
    const { nativeRoot } = nativeWidthFixture();
    const target = path.join(
      nativeRoot,
      "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
    );
    const before = fs.readFileSync(target);
    try {
      const original = before.toString();
      const changed = mutate(original);
      expect(changed).not.toBe(original);
      fs.writeFileSync(target, changed);
      expect(() => nativeWidthUnusedContext(nativeRoot, new Map())).toThrow(
        "changed native width consumer",
      );
    } finally {
      fs.writeFileSync(target, before);
    }
  },
);

test.each(nativeRoute.filter((name) => name !== nativeScripts))(
  "native width rejects changed consumer/trace %s",
  (name) => {
    const { nativeRoot } = nativeWidthFixture();
    const target = path.join(nativeRoot, name);
    const before = fs.readFileSync(target);
    try {
      const original = before.toString();
      const changed =
        nativeRouteMutations.get(name)?.(original) ?? `${original}\n// changed consumer\n`;
      fs.writeFileSync(target, changed);
      expect(() => nativeWidthUnusedContext(nativeRoot, new Map())).toThrow(
        "changed native width consumer",
      );
    } finally {
      fs.writeFileSync(target, before);
    }
  },
);

test.each(["unknown producer", "missing", "duplicate", "different callee", "comment", "escape"])(
  "native width result rejects %s",
  (kind) => {
    const { nativeRaw } = nativeWidthFixture();
    const raw = nativeRaw.get(nativeHelper);
    if (!raw) throw new Error("missing Rust helper");
    let code = raw.code;
    if (kind === "missing") code = code.replace("fn result_script()", "fn other_script()");
    if (kind === "duplicate") code += code;
    if (kind === "different callee")
      code = code.replace("probes: nativeWidthProbeResult()", "probes: otherResult()");
    if (kind === "comment")
      code = code.replace(
        "probes: nativeWidthProbeResult()",
        "probes: /* nativeWidthProbeResult() */ null",
      );
    if (kind === "escape")
      code = code.replace("JSON.stringify({ calls:", "JSON.stringify({\\n calls:");
    expect(() =>
      inspectNativeWidthResultLiteral(
        kind === "unknown producer" ? "other.rs" : nativeHelper,
        code,
      ),
    ).toThrow();
  },
);

test("native width input drift and generated labels remain unmapped", () => {
  const { nativeRoot, nativeRaw, nativeOrigin } = nativeWidthFixture();
  const trace = nativeRaw.get(nativeTrace);
  if (!trace) throw new Error("missing trace");
  const target = path.join(nativeRoot, nativeTrace);
  try {
    fs.appendFileSync(target, "\n// drift");
    expect(() => verifyUnusedSources(nativeRoot, nativeRaw)).toThrow();
  } finally {
    fs.writeFileSync(target, trace.code);
  }
  expect(() =>
    nativeOrigin.resolve(diagnosticAt(nativeOrigin.assembled, 0, 4), "assembled"),
  ).toThrow();
});

test.each(["metadata-browser", "metadata-native", "metadata-unknown", "origin-browser"])(
  "native/browser realm swap rejects %s",
  (kind) => {
    const { nativeRaw, nativeOrigin } = nativeWidthFixture();
    expect(() => {
      if (kind === "metadata-browser")
        return unusedProgramMetadata("official-mermaid-browser", nativeOrigin.programs);
      if (kind === "metadata-native")
        return unusedProgramMetadata("rust-native-width-test-result", browserPrograms);
      if (kind === "metadata-unknown")
        return unusedProgramMetadata("unknown", nativeOrigin.programs);
      return new BrowserUnusedOrigin(
        nativeOrigin.assembled,
        nativeOrigin.map,
        nativeOrigin.programs,
        nativeRaw,
      );
    }).toThrow();
  },
);

test.each(["source-hash", "map-content", "map-gap"])(
  "native width source-map rejects %s",
  (kind) => {
    const { nativeRaw, nativeOrigin } = nativeWidthFixture();
    const raw = new Map(nativeRaw);
    let map = nativeOrigin.map;
    if (kind === "source-hash") {
      const source = raw.get(nativeTrace);
      if (!source) throw new Error("missing trace source");
      raw.set(nativeTrace, { ...source, sha256: "0".repeat(64) });
    } else {
      const value = z
        .object({ sourcesContent: z.array(z.string()), mappings: z.string() })
        .passthrough()
        .parse(JSON.parse(map));
      if (kind === "map-content") value.sourcesContent[0] = "different raw input";
      else value.mappings = "";
      map = JSON.stringify(value);
    }
    expect(() => {
      const origin = new BrowserUnusedOrigin(
        nativeOrigin.assembled,
        map,
        nativeOrigin.programs,
        raw,
        "rust-native-width",
      );
      return origin.resolve(
        diagnosticAt(origin.assembled, origin.assembled.lastIndexOf("nativeWidthProbeResult"), 22),
        "assembled",
      );
    }).toThrow();
  },
);

const functionBase = "crates/katana-render-runtime/src/renderer/backends/html_runtime/";
const functionRoute = [`${functionBase}dom_bootstrap_test.js`, `${functionBase}dom_bootstrap.js`];
const functionRoot = fs.mkdtempSync(path.join(directory, "node-function-"));
for (const name of functionRoute) {
  const target = path.join(functionRoot, name);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.copyFileSync(path.join(browserRoot, name), target);
}
const functionRaw = new Map<string, ReturnType<typeof physicalUnusedSource>>();
const functionContext = nodeFunctionUnusedContext(functionRoot, functionRaw);
const functionOrigin = functionContext.origin;
if (!(functionOrigin instanceof BrowserUnusedOrigin)) throw new Error("missing Function origin");

test("actual node Function constructor preserves parameter/body/return spans and real caller metadata", () => {
  expect(functionOrigin.programs.map((program) => program.role)).toEqual([
    "node-function-parameters",
    "node-function-body",
    "node-function-body",
    "node-function-return",
  ]);
  expect(functionContext.codeBytesEqual).toBe(false);
  const metadata = unusedProgramMetadata(functionContext.name, functionOrigin.programs);
  expect(metadata.realm).toBe("Node/Bun unit new Function, installStaticLoadHandler constructor");
  expect("fullSourceClosure" in metadata && metadata.fullSourceClosure).toBe(false);
  expect("executionVerified" in metadata && metadata.executionVerified).toBe(false);
  for (const program of functionOrigin.programs) {
    expect(program.mode).toBe("node-function-test");
    expect(program.runtimeAstSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(program.callAstSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(program.callerRange?.sourcePath).toBe(functionRoute[0] ?? "");
    expect(program.callerRange?.sourceSha256).toBe(functionRaw.get(functionRoute[0] ?? "")?.sha256);
    expect(program.consumerRoute).toHaveLength(2);
  }
  verifyUnusedSources(functionRoot, functionRaw);
});

test.each(functionRoute)(
  "node Function accepts unrelated suffix changes while retaining the live %s hash",
  (name) => {
    const file = path.join(functionRoot, name);
    const before = fs.readFileSync(file);
    try {
      fs.writeFileSync(file, Buffer.concat([before, Buffer.from("\n// unrelated suffix\n")]));
      const current = physicalUnusedSource(fs.readFileSync(file));
      const raw = new Map<string, ReturnType<typeof physicalUnusedSource>>();
      const context = nodeFunctionUnusedContext(functionRoot, raw);
      const origin = context.origin;
      if (!(origin instanceof BrowserUnusedOrigin)) throw new Error("missing Function origin");
      expect(raw.get(name)?.sha256).toBe(current.sha256);
      for (const program of origin.programs) {
        expect(
          program.consumerRoute?.find((entry) => entry.sourcePath === name)?.sourceSha256,
        ).toBe(current.sha256);
        if (name === functionRoute[0])
          expect(program.callerRange?.sourceSha256).toBe(current.sha256);
      }
    } finally {
      fs.writeFileSync(file, before);
    }
  },
);

test.each(["parameter", "body", "return"])("node Function maps actual %s source span", (site) => {
  const marker =
    site === "parameter"
      ? "__krrStoreEventHandler"
      : site === "body"
        ? "__krrInstallBodyLoadHandler"
        : "__krrInstallStaticBodyLoadHandler";
  const code = functionOrigin.assembled;
  const start = site === "return" ? code.lastIndexOf(marker) : code.indexOf(marker);
  const origin = functionOrigin.resolve(diagnosticAt(code, start, marker.length), "assembled");
  expect(origin.sourcePath).toBe(functionRoute[site === "body" ? 1 : 0] ?? "");
  expect(functionRaw.get(origin.sourcePath)?.code.slice(origin.start, origin.end)).toBe(marker);
  expect(() =>
    functionOrigin.resolve(diagnosticAt(code, start, marker.length), "normalized"),
  ).toThrow();
});

const nodeFunctionDrifts = [
  ["missing", "producer", "const installStaticLoadHandler = (", "const missing = ("],
  [
    "rebind",
    "producer",
    'const install = new Function(\n    "globalThis",\n    "window",',
    'const Function = () => {}; const install = new Function(\n    "globalThis",\n    "window",',
  ],
  [
    "body template",
    "producer",
    "const bodyLoadHandler = source.match(",
    "const changedBodyLoadHandler = source.match(",
  ],
  ["caller", "producer", "  install();", "  otherInstall();"],
  [
    "selected resource suffix",
    "resource",
    'const handler = Function("event", String(source));',
    'const handler = Function("event", String(source + ""));',
  ],
  [
    "body-load-handler selected span",
    "resource",
    'if (source === null || source === undefined) {\n    __krrStoreEventHandler(window, "load", null);',
    "if (source === null)",
  ],
  [
    "static-body-load-handler selected span",
    "resource",
    "if (laterToken) {\n    const nodeId =",
    "if (!laterToken) {\n    const nodeId =",
  ],
] as const;

function replaceNodeFunctionSource(source: string, before: string, after: string): string {
  if (!before) return `${source}${after}`;
  const occurrences = source.split(before).length - 1;
  if (occurrences !== 1) throw new Error(`expected one node Function mutation target: ${before}`);
  return source.replace(before, after);
}

test.each(nodeFunctionDrifts)(
  "node Function refuses actual %s drift",
  (_kind, target, before, after) => {
    const producer = functionRaw.get(functionRoute[0] ?? "");
    const resource = functionRaw.get(functionRoute[1] ?? "");
    if (!producer || !resource) throw new Error("missing Function fixtures");
    const changedProducer =
      target === "producer"
        ? physicalUnusedSource(Buffer.from(replaceNodeFunctionSource(producer.code, before, after)))
        : producer;
    const changedResource =
      target === "resource"
        ? physicalUnusedSource(Buffer.from(replaceNodeFunctionSource(resource.code, before, after)))
        : resource;
    expect(() => inspectNodeFunctionPrograms(changedProducer, changedResource)).toThrow(
      /node Function/,
    );
  },
);
test.each(functionRoute)("node Function refuses missing/drifting physical %s", (name) => {
  const file = path.join(functionRoot, name);
  const before = fs.readFileSync(file);
  try {
    fs.writeFileSync(file, `${before.toString()}\n// changed`);
    expect(() => verifyUnusedSources(functionRoot, functionRaw)).toThrow();
    expect(() => nodeFunctionUnusedContext(functionRoot, functionRaw)).not.toThrow();
  } finally {
    fs.writeFileSync(file, before);
  }
});

test.each(["generated", "source-hash", "body-code", "map-content", "map-gap", "realm"])(
  "node Function mapping fails closed on %s",
  (kind) => {
    let code = functionOrigin.assembled;
    let map = functionOrigin.map;
    const raw = new Map(functionRaw);
    if (kind === "body-code")
      code = code.replace("__krrInstallBodyLoadHandler", "__krrChangedBodyLoadHandler");
    if (kind === "source-hash") {
      const name = functionRoute[1] ?? "";
      const source = raw.get(name);
      if (!source) throw new Error("missing Function resource");
      raw.set(name, { ...source, sha256: "0".repeat(64) });
    }
    if (kind === "map-content" || kind === "map-gap") {
      const value = z
        .object({ sourcesContent: z.array(z.string()), mappings: z.string() })
        .passthrough()
        .parse(JSON.parse(map));
      if (kind === "map-content") value.sourcesContent[0] = "changed raw source";
      else value.mappings = "";
      map = JSON.stringify(value);
    }
    expect(() => {
      const origin = new BrowserUnusedOrigin(
        code,
        map,
        functionOrigin.programs,
        raw,
        kind === "realm" ? "browser" : "node-function",
      );
      return origin.resolve(
        diagnosticAt(
          code,
          kind === "generated" ? 0 : code.indexOf("__krrInstallBodyLoadHandler"),
          kind === "generated" ? 1 : 26,
        ),
        "assembled",
      );
    }).toThrow();
  },
);

test.each(functionRoute)("node Function refuses absent physical %s", (name) => {
  const file = path.join(functionRoot, name);
  const backup = `${file}.missing`;
  fs.renameSync(file, backup);
  try {
    expect(() => nodeFunctionUnusedContext(functionRoot, new Map())).toThrow();
  } finally {
    fs.renameSync(backup, file);
  }
});

test.each(["callerRange", "callAstSha256", "runtimeAstSha256", "consumerRoute"] as const)(
  "node Function Origin rejects stale %s provenance independently of valid code/map",
  (field) => {
    const changed = functionOrigin.programs.map((program) => {
      if (field === "callerRange")
        return {
          ...program,
          callerRange: {
            sourcePath: functionRoute[0] ?? "",
            sourceSha256: "0".repeat(64),
            start: 0,
            end: 1,
          },
        };
      if (field === "consumerRoute") return { ...program, consumerRoute: [] };
      return { ...program, [field]: "0".repeat(64) };
    });
    expect(
      () =>
        new BrowserUnusedOrigin(
          functionOrigin.assembled,
          functionOrigin.map,
          changed,
          functionRaw,
          "node-function",
        ),
    ).toThrow();
  },
);

test.each(["callerRange", "callAstSha256", "runtimeAstSha256", "consumerRoute"] as const)(
  "node Function Origin rejects missing %s provenance",
  (field) => {
    const changed = functionOrigin.programs.map((program) => ({ ...program }));
    const first = changed[0];
    if (!first) throw new Error("missing Function program");
    Reflect.deleteProperty(first, field);
    expect(
      () =>
        new BrowserUnusedOrigin(
          functionOrigin.assembled,
          functionOrigin.map,
          changed,
          functionRaw,
          "node-function",
        ),
    ).toThrow();
  },
);

test.each(["missing-source", "caller-code", "shadow-code", "resource-code"])(
  "node Function Origin rejects %s even with stale source SHA/metadata",
  (kind) => {
    const raw = new Map(functionRaw);
    const name = functionRoute[kind === "resource-code" ? 1 : 0] ?? "";
    const source = raw.get(name);
    if (!source) throw new Error("missing Function source");
    const changed =
      kind === "caller-code"
        ? source.code.replace("  install();", "  changed();")
        : kind === "shadow-code"
          ? source.code.replace(
              "  const install = new Function(",
              "  const Function = null;\n  const install = new Function(",
            )
          : `${source.code}\n// source changed`;
    if (kind === "missing-source") raw.delete(name);
    else raw.set(name, { ...source, code: changed });
    const value = z
      .object({ sources: z.array(z.string()), sourcesContent: z.array(z.string()) })
      .passthrough()
      .parse(JSON.parse(functionOrigin.map));
    const index = value.sources.indexOf(name);
    if (index < 0) throw new Error("missing mapped physical source");
    value.sourcesContent[index] = changed;
    expect(
      () =>
        new BrowserUnusedOrigin(
          functionOrigin.assembled,
          JSON.stringify(value),
          functionOrigin.programs,
          raw,
          "node-function",
        ),
    ).toThrow();
  },
);

const sizingRoot = fs.mkdtempSync(path.join(directory, "shared-svg-"));
for (const binding of sharedSvgCaptureRoute) {
  const target = path.join(sizingRoot, binding.sourcePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.copyFileSync(path.join(browserRoot, binding.sourcePath), target);
}
const sizingRaw = new Map<string, ReturnType<typeof physicalUnusedSource>>();
const sizingContext = sharedSvgCaptureUnusedContext(sizingRoot, sizingRaw);
const sizingOrigin = sizingContext.origin;
if (!(sizingOrigin instanceof BrowserUnusedOrigin)) throw new Error("missing shared sizing origin");
const sizingProgram = sizingOrigin.programs[0];
const sizingHelper = sizingRaw.get(sharedSvgCaptureRoute[2].sourcePath);
if (!sizingProgram || !sizingHelper) throw new Error("missing actual shared sizing callback");

function liveSharedSvgRoute(raw: ReadonlyMap<string, ReturnType<typeof physicalUnusedSource>>) {
  return sharedSvgCaptureRoute.map(({ sourcePath }) => {
    const source = raw.get(sourcePath);
    if (!source) throw new Error(`missing shared SVG source: ${sourcePath}`);
    const sourceSha256 = physicalUnusedSource(Buffer.from(source.code)).sha256;
    if (source.sha256 !== sourceSha256)
      throw new Error(`changed shared SVG raw digest: ${sourcePath}`);
    return { sourcePath, sourceSha256 };
  });
}

test("shared sizing binds one real exported callback to both actual mode callers and Page ABI", () => {
  expect(sizingOrigin.programs).toHaveLength(1);
  const liveRoute = liveSharedSvgRoute(sizingRaw);
  expect(sizingProgram.consumerRoute).toEqual(liveRoute);
  expect(sizingProgram.captureCallers?.map((caller) => caller.mode)).toEqual([
    "officialFallback",
    "strictViewBox",
  ]);
  expect(sizingProgram.captureCallers?.map((caller) => caller.strictSource)).toEqual([
    "strict",
    'this.options.referenceMode === "strictVendorOriginal"',
  ]);
  expect(sizingProgram.data).toBe("{ strict, mode }");
  expect(sizingProgram.mode).toBe("shared-svg-capture");
  const metadata = unusedProgramMetadata(sizingContext.name, sizingOrigin.programs);
  expect("executionVerified" in metadata && metadata.executionVerified).toBe(false);
  expect("fullSourceClosure" in metadata && metadata.fullSourceClosure).toBe(false);
  expect(sizingContext.codeBytesEqual).toBe(false);
  expect(sizingOrigin.coverage("assembled")[0]?.sourceClosureComplete).toBe(false);
  expect(
    browserPrograms.filter((program) => program.role === "callback").length +
      sizingOrigin.programs.length,
  ).toBe(8);
  verifyUnusedSources(sizingRoot, sizingRaw);
});

test.each([sharedSvgCaptureRoute[1], sharedSvgCaptureRoute[3]])(
  "shared sizing accepts unrelated suffix changes while retaining live hash for %s",
  ({ sourcePath }) => {
    const file = path.join(sizingRoot, sourcePath);
    const before = fs.readFileSync(file);
    try {
      fs.writeFileSync(file, Buffer.concat([before, Buffer.from("\n// unrelated suffix\n")]));
      const raw = new Map<string, ReturnType<typeof physicalUnusedSource>>();
      const context = sharedSvgCaptureUnusedContext(sizingRoot, raw);
      const origin = context.origin;
      if (!(origin instanceof BrowserUnusedOrigin)) throw new Error("missing shared sizing origin");
      const program = origin.programs[0];
      const source = raw.get(sourcePath);
      if (!program || !source) throw new Error("missing live shared sizing provenance");
      const expectedRoute = liveSharedSvgRoute(raw);
      const expectedSource = expectedRoute.find((entry) => entry.sourcePath === sourcePath);
      if (!expectedSource) throw new Error("missing shared sizing route entry");
      expect(program.consumerRoute).toEqual(expectedRoute);
      expect(
        program.consumerRoute?.find((entry) => entry.sourcePath === sourcePath)?.sourceSha256,
      ).toBe(physicalUnusedSource(fs.readFileSync(file)).sha256);
      expect(source.sha256).toBe(expectedSource.sourceSha256);
    } finally {
      fs.writeFileSync(file, before);
    }
  },
);

test.each(["dimensionAt", "getBoundingClientRect", "strict", "mode"])(
  "shared callback maps full actual %s span",
  (marker) => {
    const code = sizingOrigin.assembled,
      start = code.indexOf(marker);
    const mapped = sizingOrigin.resolve(diagnosticAt(code, start, marker.length), "assembled");
    expect(mapped.sourcePath).toBe(sharedSvgCaptureRoute[2].sourcePath);
    expect(sizingHelper.code.slice(mapped.start, mapped.end)).toBe(marker);
    expect(() =>
      sizingOrigin.resolve(diagnosticAt(code, start, marker.length), "normalized"),
    ).toThrow();
  },
);

test.each([
  "missing-export",
  "alias-receiver",
  "shadow",
  "reassign",
  "update",
  "fake-Pick",
  "fake-PageHandle",
  "computed",
  "closure",
  "data-order",
])(
  "actual sizing inspector rejects %s callback binding drift before physical admission",
  (kind) => {
    let code = sizingHelper.code;
    if (kind === "missing-export")
      code = code.replace("export function resizeSvgCapture", "function resizeSvgCapture");
    if (kind === "alias-receiver")
      code = code.replace(
        "  return page.evaluate(",
        "  const receiver = page; return receiver.evaluate(",
      );
    if (kind === "shadow")
      code = code
        .replace("  return page.evaluate(", "  { const page = null; return page.evaluate(")
        .replace("    { strict, mode },\n  );", "    { strict, mode },\n  ); }");
    if (kind === "reassign")
      code = code.replace("  return page.evaluate(", "  page = page; return page.evaluate(");
    if (kind === "update")
      code = code.replace("  return page.evaluate(", "  page++; return page.evaluate(");
    if (kind === "fake-Pick")
      code = code.replace(
        "export type SvgCaptureSizingMode",
        "type Pick<T, K> = T; export type SvgCaptureSizingMode",
      );
    if (kind === "fake-PageHandle")
      code = code.replace(
        'import type { PageHandle } from "./official-renderer-types";',
        "type PageHandle = unknown;",
      );
    if (kind === "computed")
      code = code.replace("  return page.evaluate(", '  return page["evaluate"](');
    if (kind === "closure")
      code = code.replace("const width = Math.ceil(", "const width = rasterSha256(");
    if (kind === "data-order") code = code.replace("{ strict, mode },", "{ mode, strict },");
    expect(() => inspectSharedSvgCaptureCallback(code)).toThrow();
  },
);

const sharedSvgSelectedSourceDrifts = [
  [
    sharedSvgCaptureRoute[1],
    "baseHtml selected method",
    "  private baseHtml() {",
    "  private baseHtml(changed: boolean) {",
  ],
  [
    sharedSvgCaptureRoute[1],
    "rasterCaptureHtml selected function",
    "export function rasterCaptureHtml(theme: DiagramTheme): string {",
    "export function rasterCaptureHtml(theme: unknown): string {",
  ],
  [
    sharedSvgCaptureRoute[3],
    "BrowserLauncher selected ABI",
    "launch(options: { headless: boolean }): Promise<BrowserHandle>;",
    "launch(options: { headless: number }): Promise<BrowserHandle>;",
  ],
] as const;

test.each(sharedSvgSelectedSourceDrifts)(
  "shared sizing rejects changed %s selected source span",
  (binding, _kind, beforeText, afterText) => {
    const file = path.join(sizingRoot, binding.sourcePath);
    const before = fs.readFileSync(file);
    try {
      const changed = replaceNodeFunctionSource(before.toString(), beforeText, afterText);
      expect(changed).not.toBe(before.toString());
      fs.writeFileSync(file, changed);
      expect(() => sharedSvgCaptureUnusedContext(sizingRoot, new Map())).toThrow();
    } finally {
      fs.writeFileSync(file, before);
    }
  },
);

test.each(["import", "callee-shadow", "reassign-import", "mode", "strict", "PageHandle"])(
  "actual sizing caller inspector rejects %s import/facade drift",
  (kind) => {
    const official = sizingRaw.get(sharedSvgCaptureRoute[0].sourcePath),
      raster = sizingRaw.get(sharedSvgCaptureRoute[1].sourcePath);
    if (!official || !raster) throw new Error("missing sizing actual callers");
    let code = official.code;
    if (kind === "import")
      code = code.replace('from "./raster-capture-observation"', 'from "./other-helper"');
    if (kind === "callee-shadow")
      code = code.replace(
        'return resizeSvgCapture(page, strict, "officialFallback");',
        'const resizeSvgCapture = () => {}; return resizeSvgCapture(page, strict, "officialFallback");',
      );
    if (kind === "reassign-import")
      code = code.replace(
        'return resizeSvgCapture(page, strict, "officialFallback");',
        'resizeSvgCapture = null; return resizeSvgCapture(page, strict, "officialFallback");',
      );
    if (kind === "mode")
      code = code.replace('page, strict, "officialFallback"', 'page, strict, "strictViewBox"');
    if (kind === "strict")
      code = code.replace('page, strict, "officialFallback"', 'page, false, "officialFallback"');
    if (kind === "PageHandle")
      code = code.replace("resizeCapture(page: PageHandle", "resizeCapture(page: unknown");
    expect(() =>
      inspectSharedSvgCaptureCallers(physicalUnusedSource(Buffer.from(code)), raster),
    ).toThrow();
  },
);

test.each(sharedSvgCaptureRoute)(
  "shared sizing rejects changed/missing physical $sourcePath",
  (binding) => {
    const file = path.join(sizingRoot, binding.sourcePath),
      before = fs.readFileSync(file);
    try {
      fs.writeFileSync(file, `${before.toString()}\n// consumer drift`);
      if (
        binding.sourcePath === sharedSvgCaptureRoute[1].sourcePath ||
        binding.sourcePath === sharedSvgCaptureRoute[3].sourcePath
      ) {
        expect(() => sharedSvgCaptureUnusedContext(sizingRoot, new Map())).not.toThrow();
      } else {
        expect(() => sharedSvgCaptureUnusedContext(sizingRoot, new Map())).toThrow();
      }
      expect(() => verifyUnusedSources(sizingRoot, sizingRaw)).toThrow();
    } finally {
      fs.writeFileSync(file, before);
    }
    const missing = new Map(sizingRaw);
    missing.delete(binding.sourcePath);
    expect(() => inspectSharedSvgCaptureProgram(missing)).toThrow();
  },
);

test.each([
  "caller-mode",
  "caller-hash",
  "caller-range",
  "caller-order",
  "runtime-hash",
  "call-hash",
  "route",
  "data",
  "range",
])("Origin recomputes shared sizing provenance and rejects %s", (kind) => {
  const actualCallers = sizingProgram.captureCallers;
  if (!actualCallers) throw new Error("missing actual sizing callers");
  const changed = {
    ...sizingProgram,
    captureCallers: actualCallers.map((caller) => ({ ...caller })),
  };
  const callers = changed.captureCallers;
  if (!callers[0]) throw new Error("missing actual sizing caller metadata");
  if (kind === "caller-mode") callers[0].mode = "strictViewBox";
  if (kind === "caller-hash") callers[0].callAstSha256 = "0".repeat(64);
  if (kind === "caller-range") callers[0].start++;
  if (kind === "caller-order") callers.reverse();
  if (kind === "runtime-hash") changed.runtimeAstSha256 = "0".repeat(64);
  if (kind === "call-hash") changed.callAstSha256 = "0".repeat(64);
  if (kind === "route") changed.consumerRoute = [];
  if (kind === "data") changed.data = "{ mode, strict }";
  if (kind === "range") changed.start++;
  expect(
    () =>
      new BrowserUnusedOrigin(
        sizingOrigin.assembled,
        sizingOrigin.map,
        [changed],
        sizingRaw,
        "shared-svg-capture",
      ),
  ).toThrow();
});

test.each([
  "key-order",
  "key-order-forged",
  "missing-metadata",
  "map-gap",
  "map-content",
  "generated",
  "realm",
])("shared sizing handles %s without trusting metadata or generated spans", (kind) => {
  const changed = { ...sizingProgram };
  const reordered = Object.fromEntries(Object.entries(changed).reverse());
  const parsed = sharedSvgProgramSchema.parse(reordered);
  if (kind === "key-order-forged") parsed.runtimeAstSha256 = "0".repeat(64);
  if (kind === "missing-metadata") Reflect.deleteProperty(parsed, "captureCallers");
  let map = sizingOrigin.map;
  if (kind === "map-gap" || kind === "map-content") {
    const value = z
      .object({ mappings: z.string(), sourcesContent: z.array(z.string()) })
      .passthrough()
      .parse(JSON.parse(map));
    if (kind === "map-gap") value.mappings = "";
    else value.sourcesContent[0] = "changed physical input";
    map = JSON.stringify(value);
  }
  const construct = () => {
    const origin = new BrowserUnusedOrigin(
      sizingOrigin.assembled,
      map,
      [parsed],
      sizingRaw,
      kind === "realm" ? "browser" : "shared-svg-capture",
    );
    const at = kind === "generated" ? 0 : origin.assembled.indexOf("dimensionAt");
    return origin.resolve(
      diagnosticAt(origin.assembled, at, kind === "generated" ? 1 : 11),
      "assembled",
    );
  };
  if (kind === "key-order")
    expect(construct().sourcePath).toBe(sharedSvgCaptureRoute[2].sourcePath);
  else expect(construct).toThrow();
});

test.each([0, 1, 2, 3])("shared sizing ignores forged extra raw path on route %s", (routeIndex) => {
  const binding = sharedSvgCaptureRoute[routeIndex];
  if (!binding) throw new Error("missing shared sizing route");
  const source = sizingRaw.get(binding.sourcePath);
  if (!source) throw new Error("missing shared sizing raw source");
  const forgedPath = "forged.js";
  const poisoned = { ...source, sourcePath: forgedPath };
  const raw = new Map(sizingRaw);
  raw.set(binding.sourcePath, poisoned);
  raw.set(forgedPath, poisoned);

  const program = inspectSharedSvgCaptureProgram(raw);
  const helperRoute = liveSharedSvgRoute(sizingRaw)[2];
  if (!helperRoute) throw new Error("missing shared sizing helper route");
  expect(program.sourcePath).toBe(sharedSvgCaptureRoute[2].sourcePath);
  expect(program.sourceSha256).toBe(helperRoute.sourceSha256);
  expect(program.consumerRoute).toEqual(liveSharedSvgRoute(sizingRaw));
  expect(
    program.captureCallers?.map(({ sourcePath, sourceSha256 }) => ({
      sourcePath,
      sourceSha256,
    })),
  ).toEqual(
    liveSharedSvgRoute(sizingRaw)
      .slice(0, 2)
      .map(({ sourcePath, sourceSha256 }) => ({
        sourcePath,
        sourceSha256,
      })),
  );

  const origin = new BrowserUnusedOrigin(
    sizingOrigin.assembled,
    sizingOrigin.map,
    [program],
    raw,
    "shared-svg-capture",
  );
  const marker = "dimensionAt";
  const start = origin.assembled.indexOf(marker);
  expect(
    origin.resolve(diagnosticAt(origin.assembled, start, marker.length), "assembled").sourcePath,
  ).toBe(sharedSvgCaptureRoute[2].sourcePath);
});

test.each([0, 1, 2, 3])("shared sizing rejects forged raw SHA on route %s", (routeIndex) => {
  const binding = sharedSvgCaptureRoute[routeIndex];
  if (!binding) throw new Error("missing shared sizing route");
  const source = sizingRaw.get(binding.sourcePath);
  if (!source) throw new Error("missing shared sizing raw source");
  const raw = new Map(sizingRaw);
  raw.set(binding.sourcePath, { ...source, sha256: "f".repeat(64) });
  expect(() => inspectSharedSvgCaptureProgram(raw)).toThrow();
});

test("shared sizing explicitly selects canonical code/hash despite unknown raw properties", () => {
  const binding = sharedSvgCaptureRoute[2];
  if (!binding) throw new Error("missing shared sizing callback route");
  const source = sizingRaw.get(binding.sourcePath);
  if (!source) throw new Error("missing shared sizing callback source");
  const raw = new Map(sizingRaw);
  const withUnknownExtra = {
    ...source,
    diagnostics: { sourcePath: "forged.js", sourceSha256: "f".repeat(64) },
  };
  raw.set(binding.sourcePath, withUnknownExtra);
  const program = inspectSharedSvgCaptureProgram(raw);
  const helperRoute = liveSharedSvgRoute(sizingRaw)[2];
  if (!helperRoute) throw new Error("missing shared sizing helper route");
  expect(program.sourcePath).toBe(binding.sourcePath);
  expect(program.sourceSha256).toBe(helperRoute.sourceSha256);
  expect(program.runtimeAstSha256).toBe(sizingProgram.runtimeAstSha256);
});

function strictPhysicalFixture() {
  const root = fs.mkdtempSync(path.join(directory, "official-strict-"));
  const fixtureSources = new Set([
    ...sharedSvgCaptureRoute.map(({ sourcePath }) => sourcePath),
    ...officialStrictConsumerRoute.map(({ sourcePath }) => sourcePath),
  ]);
  for (const sourcePath of fixtureSources) {
    const target = path.join(root, sourcePath);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(browserRoot, sourcePath), target);
  }
  const mermaid = path.join(root, "chosen-mermaid.js"),
    zenuml = path.join(root, "chosen-zenuml.js");
  fs.writeFileSync(mermaid, "window.mermaid = {};\n");
  fs.writeFileSync(zenuml, 'window["mermaid-zenuml"] = {};\n');
  const inputs = {
    argv: ["--mermaid-js", mermaid, "--mermaid-zenuml-js", zenuml],
    env: { MERMAID_JS: "wrong-env-mermaid.js", MERMAID_ZENUML_JS: "wrong-env-zenuml.js" },
    cwd: root,
    home: path.join(root, "home"),
  };
  const raw = new Map<string, ReturnType<typeof physicalUnusedSource>>();
  const context = officialStrictUnusedContext(root, raw, inputs);
  return { root, mermaid, zenuml, inputs, raw, context };
}

test("official strict resolver preserves direct and Justfile defaults and explicit home", () => {
  const cwd = path.join(directory, "resolver-cwd"),
    home = path.join(directory, "resolver-home");
  const direct = resolveOfficialRendererAssetInputs([], {}, cwd, home);
  expect(direct).toEqual({
    mermaidJs: path.join(
      cwd,
      "crates/katana-render-runtime/vendor/mermaid/12.2.0-krr.pr6823.1/mermaid.min.js",
    ),
    mermaidZenumlJs: path.join(
      cwd,
      "crates/katana-render-runtime/vendor/mermaid-zenuml/1.0.1/mermaid-zenuml.min.js",
    ),
  });
  const tracked = "explicit-mermaid.js";
  expect(
    resolveOfficialRendererAssetInputs(["--mermaid-js", tracked], {}, cwd, home).mermaidJs,
  ).toBe(path.join(cwd, tracked));
  expect(direct.mermaidJs).not.toBe(path.join(cwd, tracked));
  expect(resolveOfficialRendererAssetInputs([], {}, cwd, home).mermaidJs).toBe(
    resolveOfficialRendererAssetInputs([], {}, cwd, `${home}-other`).mermaidJs,
  );
});

test.each(["mermaidJs", "mermaidZenumlJs"] as const)(
  "official strict resolver applies CLI/env/default precedence for %s",
  (option) => {
    const flag = option === "mermaidJs" ? "--mermaid-js" : "--mermaid-zenuml-js";
    const key = option === "mermaidJs" ? "MERMAID_JS" : "MERMAID_ZENUML_JS";
    const cwd = path.join(directory, "cwd"),
      home = path.join(directory, "home");
    expect(
      resolveOfficialRendererAssetInputs([flag, "~/cli.js"], { [key]: "env.js" }, cwd, home)[
        option
      ],
    ).toBe(path.join(home, "cli.js"));
    expect(
      resolveOfficialRendererAssetInputs([`${flag}=cli.js`], { [key]: "env.js" }, cwd, home)[
        option
      ],
    ).toBe(path.join(cwd, "env.js"));
    expect(resolveOfficialRendererAssetInputs([], { [key]: "env.js" }, cwd, home)[option]).toBe(
      path.join(cwd, "env.js"),
    );
    expect(resolveOfficialRendererAssetInputs([], { [key]: "" }, cwd, home)[option]).toBe(
      resolveOfficialRendererAssetInputs([], {}, cwd, home)[option],
    );
  },
);

test("official strict physical producer binds five callbacks and external bytes without vendor ownership", () => {
  const { raw, context, mermaid, zenuml } = strictPhysicalFixture();
  const programs = context.programs,
    inputs = context.externalInputs;
  if (!programs || !inputs || !(context.origin instanceof BrowserUnusedOrigin))
    throw new Error("missing strict fixture metadata");
  expect(programs).toHaveLength(5);
  expect(programs.map((program) => program.sourcePath)).toEqual([
    officialStrictConsumerRoute[0].sourcePath,
    officialStrictConsumerRoute[2].sourcePath,
    officialStrictConsumerRoute[2].sourcePath,
    officialStrictConsumerRoute[4].sourcePath,
    officialStrictConsumerRoute[4].sourcePath,
  ]);
  expect(
    programs.every(
      (program) => program.role === "callback" && program.mode === "strictVendorOriginal",
    ),
  ).toBe(true);
  expect(inputs.map((input) => input.absolutePath)).toEqual([mermaid, zenuml]);
  for (const input of inputs) {
    const bytes = fs.readFileSync(input.absolutePath);
    expect(input.sha256).toBe(physicalUnusedSource(bytes).sha256);
    expect(input.byteLength).toBe(bytes.length);
    expect(raw.has(input.absolutePath)).toBe(false);
  }
  expect(context.origin.assembled).not.toContain("dimensionAt");
  expect(inspectOfficialStrictPrograms(raw)).toEqual([...programs]);
  expect(() => verifyOfficialStrictExternalInputs(inputs)).not.toThrow();
  const metadata = unusedProgramMetadata(context.name, programs, inputs);
  expect(metadata).toMatchObject({
    projectionOnly: true,
    acceptedConsumerClosure: false,
    externalRuntimeConfigObserved: false,
    nativeRuntimeVerified: false,
    browserExecutionVerified: false,
    executionVerified: false,
    artifactVerified: false,
    fullSourceClosure: false,
  });
  expect(() => unusedProgramMetadata(context.name, [...programs].reverse(), inputs)).toThrow(
    "wrong official strict",
  );
  expect(incompleteUnusedConsumers).toHaveLength(3);
});

test.each(["same-length", "longer", "missing", "directory", "symlink"])(
  "official strict external post-diagnostics boundary rejects %s",
  (kind) => {
    const { context, mermaid, root } = strictPhysicalFixture();
    const inputs = context.externalInputs;
    if (!inputs) throw new Error("missing external inputs");
    expect(() => verifyOfficialStrictExternalInputs(inputs)).not.toThrow();
    if (kind === "same-length")
      fs.writeFileSync(mermaid, fs.readFileSync(mermaid, "utf8").replace("mermaid", "mermaix"));
    else if (kind === "longer") fs.appendFileSync(mermaid, "changed");
    else {
      fs.renameSync(mermaid, path.join(root, "saved-mermaid.js"));
      if (kind === "directory") fs.mkdirSync(mermaid);
      if (kind === "symlink") fs.symlinkSync(path.join(root, "saved-mermaid.js"), mermaid);
    }
    expect(() => verifyOfficialStrictExternalInputs(inputs)).toThrow(
      kind === "missing"
        ? "ENOENT"
        : kind === "directory" || kind === "symlink"
          ? "not a regular file"
          : "external input changed",
    );
  },
);

test.each(["missing", "directory", "symlink"])(
  "official strict external before-projection boundary rejects %s",
  (kind) => {
    const { root, inputs, mermaid } = strictPhysicalFixture();
    fs.renameSync(mermaid, path.join(root, "saved-mermaid.js"));
    if (kind === "directory") fs.mkdirSync(mermaid);
    if (kind === "symlink") fs.symlinkSync(path.join(root, "saved-mermaid.js"), mermaid);
    expect(() => officialStrictUnusedContext(root, new Map(), inputs)).toThrow(
      kind === "missing" ? "ENOENT" : "not a regular file",
    );
  },
);

const strictBoundaryMutations = [
  {
    kind: "missing callback",
    pathIndex: 4,
    change: (code: string) => code.replace("page.evaluate", "page.other"),
    error: "callback inventory/order changed",
  },
  {
    kind: "computed callback",
    pathIndex: 4,
    change: (code: string) => code.replace("page.evaluate", 'page["evaluate"]'),
    error: "computed official strict page API",
  },
  {
    kind: "extra global callback",
    pathIndex: 4,
    change: (code: string) => `${code}\npage.evaluate(() => document.title);`,
    error: "callback inventory/order changed",
  },
  {
    kind: "escaped callback",
    pathIndex: 4,
    change: (code: string) =>
      code.replace(
        "const references = await page.evaluate(() => {",
        "const escaped = page.evaluate; const references = await escaped(() => {",
      ),
    error: "escaped official strict page.evaluate API",
  },
  {
    kind: "helper branch",
    pathIndex: 4,
    change: (code: string) =>
      code.replace("if (!image.complete || decoding)", "if (image.complete || decoding)"),
    error: "helper call/branch changed",
  },
  {
    kind: "image callback order",
    pathIndex: 4,
    change: (code: string) =>
      code.replace(
        "const urls = strictSvgExternalImageUrls(references);",
        "page.evaluate(() => document.title); const urls = strictSvgExternalImageUrls(references);",
      ),
    error: "callback inventory/order changed",
  },
  {
    kind: "page shadow",
    pathIndex: 4,
    change: (code: string) =>
      code.replace("  const references =", "  const page = null; const references ="),
    error: "page callback parameter shadow/reassignment",
  },
  {
    kind: "page reassignment",
    pathIndex: 4,
    change: (code: string) =>
      code.replace("  const references =", "  page = page; const references ="),
    error: "page callback parameter shadow/reassignment",
  },
  {
    kind: "page alias",
    pathIndex: 4,
    change: (code: string) =>
      code.replace(
        "  const references = await page.evaluate",
        "  const receiver = page; const references = await receiver.evaluate",
      ),
    error: "callback inventory/order changed",
  },
  {
    kind: "strict caller branch",
    pathIndex: 0,
    change: (code: string) =>
      code.replace("if (strict) {\n      loadedSvg", "if (!strict) {\n      loadedSvg"),
    error: "caller call/branch changed",
  },
  {
    kind: "strict caller helper",
    pathIndex: 0,
    change: (code: string) =>
      code.replace(
        "loadedSvg = await assignStrictSvg(page, primarySvg)",
        "loadedSvg = await observeCaptureSnapshot(page)",
      ),
    error: "caller call/branch changed",
  },
  {
    kind: "strict asset loading",
    pathIndex: 0,
    change: (code: string) =>
      code.replace("path: this.options.mermaidJs", "path: this.options.mermaidZenumlJs"),
    error: "caller call/branch changed",
  },
  {
    kind: "raster snapshot caller",
    pathIndex: 1,
    change: (code: string) =>
      code.replace(
        "strict ? await observeCaptureSnapshot(page) : null",
        "!strict ? await observeCaptureSnapshot(page) : null",
      ),
    error: "caller call/branch changed",
  },
  {
    kind: "global helper reassignment",
    pathIndex: 0,
    change: (code: string) => `${code}\nassignStrictSvg = null;`,
    error: "owned source pin changed",
  },
  {
    kind: "caller import alias",
    pathIndex: 0,
    change: (code: string) =>
      code.replace("  assignStrictSvg,", "  assignStrictSvg as hiddenAssign,"),
    error: "owned source pin changed",
  },
  {
    kind: "source parser position drift",
    pathIndex: 4,
    change: (code: string) => `// shifted physical parser positions\n${code}`,
    error: "parser source position changed",
  },
] as const;

test.each(strictBoundaryMutations)(
  "official strict producer rejects $kind before admitting closure",
  ({ pathIndex, change, error }) => {
    const { raw } = strictPhysicalFixture();
    expect(inspectOfficialStrictPrograms(raw)).toHaveLength(5);
    const binding = officialStrictConsumerRoute[pathIndex],
      source = raw.get(binding.sourcePath);
    if (!source) throw new Error("missing mutation source");
    const changed = change(source.code);
    expect(changed).not.toBe(source.code);
    const mutated = new Map(raw);
    mutated.set(binding.sourcePath, physicalUnusedSource(Buffer.from(changed)));
    expect(() => inspectOfficialStrictPrograms(mutated)).toThrow(
      new RegExp(
        `${error}|changed/missing shared SVG consumer|missing/stale official strict physical source`,
      ),
    );
  },
);

test.each(officialStrictConsumerRoute)(
  "official strict physical fixture rejects missing/stale $sourcePath",
  (binding) => {
    const { root, raw, inputs } = strictPhysicalFixture();
    expect(inspectOfficialStrictPrograms(raw)).toHaveLength(5);
    const missing = new Map(raw);
    missing.delete(binding.sourcePath);
    expect(() => inspectOfficialStrictPrograms(missing)).toThrow(
      sharedSvgCaptureRoute.some(({ sourcePath }) => sourcePath === binding.sourcePath)
        ? "changed/missing shared SVG consumer"
        : "missing/stale official strict physical source",
    );
    const source = raw.get(binding.sourcePath);
    if (!source) throw new Error("missing stale fixture source");
    const stale = new Map(raw);
    stale.set(binding.sourcePath, { ...source, sha256: "0".repeat(64) });
    expect(() => inspectOfficialStrictPrograms(stale)).toThrow(
      sharedSvgCaptureRoute.some(({ sourcePath }) => sourcePath === binding.sourcePath)
        ? "changed/missing shared SVG consumer"
        : "missing/stale official strict physical source",
    );
    fs.appendFileSync(path.join(root, binding.sourcePath), "\n// physical drift\n");
    expect(() => officialStrictUnusedContext(root, raw, inputs)).toThrow(
      "changed official strict owned input",
    );
  },
);

test("official strict callback mappings preserve exact physical identifiers and reject map position tampering", () => {
  const { context, raw } = strictPhysicalFixture();
  if (!(context.origin instanceof BrowserUnusedOrigin) || !context.programs)
    throw new Error("missing strict mapped context");
  for (const marker of ["mermaid", "expectedDocument", "capture", "namespace", "loadAndDecode"]) {
    const generated = context.origin.assembled.indexOf(`const ${marker}`) + 6;
    const origin = context.origin.resolve(
      diagnosticAt(context.origin.assembled, generated, marker.length),
      "assembled",
    );
    expect(raw.get(origin.sourcePath)?.code.slice(origin.start, origin.end)).toBe(marker);
  }
  const strictOrigin = context.origin,
    strictPrograms = context.programs;
  const program = strictPrograms[0];
  if (!program) throw new Error("missing strict first program");
  expect(
    () =>
      new BrowserUnusedOrigin(
        strictOrigin.assembled,
        strictOrigin.map,
        [{ ...program, start: program.start + 1 }, ...strictPrograms.slice(1)],
        raw,
      ),
  ).toThrow("changed browser projection/program order");
});

test.each([
  "runtimeAstSha256",
  "callAstSha256",
  "consumerRoute",
  "start",
  "end",
  "role",
  "mode",
] as const)("official strict metadata schema rejects changed %s", (field) => {
  const { context } = strictPhysicalFixture();
  const programs = context.programs;
  if (!programs || !context.externalInputs) throw new Error("missing strict schema fixture");
  expect(() => unusedProgramMetadata(context.name, programs, context.externalInputs)).not.toThrow();
  const changed: Record<string, unknown> = { ...programs[0] };
  if (field === "consumerRoute") changed[field] = [];
  else if (field === "start") changed[field] = -1;
  else if (field === "end") changed[field] = true;
  else delete changed[field];
  expect(() =>
    unusedProgramMetadata(
      context.name,
      [changed as unknown as (typeof programs)[number], ...programs.slice(1)],
      context.externalInputs,
    ),
  ).toThrow();
});

test("official strict pre-projection snapshot rejects same-length asset mutation", () => {
  const { context, zenuml } = strictPhysicalFixture();
  if (!context.externalInputs) throw new Error("missing pre-projection snapshot");
  const externalInputs = context.externalInputs;
  expect(() => verifyOfficialStrictExternalInputs(externalInputs)).not.toThrow();
  const previous = fs.readFileSync(zenuml);
  const changed = Buffer.from(previous);
  changed[0] = changed[0] === 119 ? 118 : 119;
  fs.writeFileSync(zenuml, changed);
  expect(changed.length).toBe(previous.length);
  expect(() => verifyOfficialStrictExternalInputs(externalInputs)).toThrow(
    "official strict external input changed",
  );
});

test.each([1, 3])("official strict metadata rejects same-module callback swap at %s", (index) => {
  const { context } = strictPhysicalFixture();
  const programs = context.programs;
  if (!programs) throw new Error("missing strict order fixture");
  const changed = [...programs];
  const first = changed[index],
    second = changed[index + 1];
  if (!first || !second) throw new Error("missing strict adjacent callbacks");
  changed[index] = second;
  changed[index + 1] = first;
  expect(() => unusedProgramMetadata(context.name, changed, context.externalInputs)).toThrow(
    "callback manifest order",
  );
});

const standardGateBase = path.join(
  repositoryRoot,
  "tmp/quality/runtime-unused-standard-gate-fix-r1",
);
fs.mkdirSync(standardGateBase, { recursive: true });
const standardGateSuite = fs.mkdtempSync(path.join(standardGateBase, "unit-"));
afterAll(() => fs.rmSync(standardGateSuite, { recursive: true, force: true }));
function isolatedStandardGateTools(modules: string) {
  fs.mkdirSync(path.join(modules, "@biomejs/biome/bin"), { recursive: true });
  for (const name of fs.readdirSync(path.join(repositoryRoot, "node_modules")))
    if (name !== ".bin" && name !== "@biomejs")
      fs.symlinkSync(path.join(repositoryRoot, "node_modules", name), path.join(modules, name));
  for (const name of fs.readdirSync(path.join(repositoryRoot, "node_modules/@biomejs")))
    if (name !== "biome")
      fs.symlinkSync(
        path.join(repositoryRoot, "node_modules/@biomejs", name),
        path.join(modules, "@biomejs", name),
      );
  for (const name of ["bin/biome", "package.json"])
    fs.copyFileSync(
      path.join(repositoryRoot, "node_modules/@biomejs/biome", name),
      path.join(modules, "@biomejs/biome", name),
    );
  fs.mkdirSync(path.join(modules, ".bin"));
  fs.symlinkSync("../@biomejs/biome/bin/biome", path.join(modules, ".bin/biome"));
}
function standardGateFixture(
  consumer = "candidate();\n",
  producer = "function candidate() {\n  return 1;\n}\n",
  isolatedTools = false,
) {
  const root = fs.mkdtempSync(path.join(standardGateSuite, "project-")),
    prefix = "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime",
    names = [`${prefix}/gate_producer.js`, `${prefix}/gate_consumer.js`];
  fs.mkdirSync(path.join(root, prefix), { recursive: true });
  fs.writeFileSync(path.join(root, names[0] ?? ""), producer);
  fs.writeFileSync(path.join(root, names[1] ?? ""), consumer);
  fs.writeFileSync(
    path.join(root, "biome.json"),
    fs.readFileSync(path.join(repositoryRoot, "biome.json")),
  );
  fs.writeFileSync(
    path.join(root, "package.json"),
    JSON.stringify({ scripts: { biome: "biome check --error-on-warnings" } }),
  );
  const modules = path.join(root, "node_modules");
  if (!isolatedTools) fs.symlinkSync(path.join(repositoryRoot, "node_modules"), modules);
  else isolatedStandardGateTools(modules);
  const raw = new Map(names.map((name) => [name, readUnusedPhysicalSource(root, name)])),
    definition = new RuntimeBundleDefinition(
      "mermaid",
      "fixture.js",
      names.map((sourcePath) => ({ sourcePath, label: sourcePath, transform: "verbatim" })),
      "",
    ),
    provenance = new RuntimeFragmentLoader(root).loadDefinitionWithProvenance(definition),
    context = {
      name: "mermaid",
      origin: new RuntimeUnusedOrigin(provenance, raw),
      stages: ["assembled"] as const,
      codeBytesEqual: true,
    };
  const directory = path.join(root, "tmp/gate"),
    collect = () => collectStandardUnusedRawBiome(root, directory);
  return {
    root,
    names,
    context,
    directory,
    collect,
    inspect: (run: ReturnType<typeof collect>) =>
      inspectStandardUnusedGate(root, path.join(root, "tmp/semantic"), run, [context]),
  };
}

test("standard unused gate resolves actual full Biome cross-fragment diagnostic in one batched pair", () => {
  const item = standardGateFixture(),
    run = item.collect(),
    result = item.inspect(run);
  expect(run.exitCode).toBe(1);
  expect(run.argv).toEqual([
    "bun",
    "run",
    "biome",
    "--reporter=json",
    `--reporter-file=${run.reportPath}`,
    "--max-diagnostics=none",
  ]);
  expect(result.lintAccepted).toBe(true);
  expect(result.counts.resolved).toBe(1);
  expect(result.groups.length).toBe(1);
  expect(result.acceptedConsumerClosure).toBe(false);
  expect(result.fullRuntimeAcceptance).toBe(false);
});

test("standard unused gate resolves the actual finite VM literal consumer", () => {
  const item = standardGateFixture("", "console.log(1);\n");
  for (const sourcePath of sequenceVmPaths) {
    const target = path.join(item.root, sourcePath);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repositoryRoot, sourcePath), target);
  }
  const context = sequenceVmUnusedContext(item.root),
    run = item.collect(),
    result = inspectStandardUnusedGate(item.root, path.join(item.root, "tmp/semantic"), run, [
      context,
    ]);
  expect(result.counts.resolved).toBe(1);
  expect(result.lintAccepted).toBe(true);
  expect(result.rows[0]?.candidate?.symbol).toBe("katanaNormalizeSequenceActivationSvg");
  expect(result.rows[0]?.proofs[0]?.provenLive).toBe(true);
});

test.each(["missing alias", "replaced alias", "changed wrapper", "Bun lookup", "binary override"])(
  "standard unused gate rejects actual installed tool drift: %s",
  (kind) => {
    const item = standardGateFixture(undefined, undefined, true),
      run = item.collect(),
      alias = path.join(item.root, "node_modules/.bin/biome"),
      originalPath = process.env.PATH,
      originalOverride = process.env.BIOME_BINARY;
    try {
      if (kind === "missing alias" || kind === "replaced alias")
        fs.renameSync(alias, `${alias}.old`);
      if (kind === "replaced alias") fs.writeFileSync(alias, "changed executable\n");
      if (kind === "changed wrapper")
        fs.appendFileSync(
          path.join(item.root, "node_modules/@biomejs/biome/bin/biome"),
          "\n// drift\n",
        );
      if (kind === "Bun lookup") {
        const selected = path.join(item.root, "bin");
        fs.mkdirSync(selected);
        fs.symlinkSync(process.execPath, path.join(selected, "bun"));
        process.env.PATH = `${selected}${path.delimiter}${originalPath ?? ""}`;
      }
      if (kind === "binary override") process.env.BIOME_BINARY = "unknown-biome";
      expect(() => item.inspect(run)).toThrow();
    } finally {
      if (originalPath === undefined) delete process.env.PATH;
      else process.env.PATH = originalPath;
      if (originalOverride === undefined) delete process.env.BIOME_BINARY;
      else process.env.BIOME_BINARY = originalOverride;
    }
  },
);

for (const [name, consumer, producer] of [
  ["declaration only", "", "function candidate() {\n  return 1;\n}\n"],
  ["write only", "candidate = 1;\n", "function candidate() {\n  return 1;\n}\n"],
  [
    "shadow",
    "function shadow(candidate) {\n  return candidate();\n}\n",
    "function candidate() {\n  return 1;\n}\n",
  ],
  ["self only", "", "function candidate() {\n  return candidate();\n}\n"],
] as const)
  test(`standard unused gate keeps ${name} warnings`, () => {
    const item = standardGateFixture(consumer, producer),
      result = item.inspect(item.collect());
    expect(result.lintAccepted).toBe(false);
    expect(result.counts.resolved).toBe(0);
    expect(result.counts["declaration-only"]).toBeGreaterThan(0);
  });

test("standard unused gate passes actual zero raw diagnostics without requiring consumer closure", () => {
  const item = standardGateFixture("", "console.log(1);\n"),
    result = runStandardUnusedGate(item.root, item.directory);
  expect(result.lintAccepted).toBe(true);
  expect(result.rawExitCode).toBe(0);
  expect(result.rows).toHaveLength(0);
  expect(result.groups).toHaveLength(0);
  expect(result.fullRuntimeAcceptance).toBe(false);
});

test("standard unused gate never resolves actual nonunused formatting/error diagnostics", () => {
  const item = standardGateFixture("candidate();\n", "function candidate(){return 1}\n"),
    run = item.collect(),
    result = item.inspect(run);
  expect(run.exitCode).toBe(1);
  expect(result.lintAccepted).toBe(false);
  expect(result.counts["other-diagnostic"]).toBeGreaterThan(0);
});

test("standard unused gate keeps unsupported current source warning unknown", () => {
  const item = standardGateFixture(),
    run = item.collect(),
    result = inspectStandardUnusedGate(item.root, path.join(item.root, "tmp/semantic"), run, []);
  expect(result.lintAccepted).toBe(false);
  expect(result.counts.unknown).toBe(1);
});

for (const kind of [
  "clone execution",
  "missing raw",
  "raw drift",
  "source drift",
  "config drift",
  "script drift",
  "stale context map",
] as const)
  test(`standard unused gate rejects ${kind}`, () => {
    const item = standardGateFixture(),
      run = item.collect();
    if (kind === "clone execution")
      expect(() => item.inspect({ ...run })).toThrow("missing/changed actual");
    else {
      if (kind === "missing raw") fs.renameSync(run.reportPath, `${run.reportPath}.old`);
      if (kind === "raw drift") fs.appendFileSync(run.reportPath, " ");
      if (kind === "source drift")
        fs.appendFileSync(path.join(item.root, item.names[0] ?? ""), "// drift\n");
      if (kind === "config drift") fs.appendFileSync(path.join(item.root, "biome.json"), " ");
      if (kind === "script drift")
        fs.writeFileSync(
          path.join(item.root, "package.json"),
          JSON.stringify({ scripts: { biome: "biome check" } }),
        );
      if (kind === "stale context map") {
        Object.assign(item.context.origin.provenance, {
          assemblyMap: "{}",
          normalizationMap: "{}",
        });
        const result = item.inspect(run);
        expect(result.lintAccepted).toBe(false);
        return;
      }
      expect(() => item.inspect(run)).toThrow();
    }
  });

for (const kind of [
  "missing",
  "count",
  "truncated",
  "exit",
  "outside scan",
  "unknown severity",
] as const)
  test(`standard unused gate raw boundary rejects ${kind}`, () => {
    const item = standardGateFixture(),
      run = item.collect(),
      report: unknown = JSON.parse(fs.readFileSync(run.reportPath, "utf8")),
      parsed = z
        .object({
          summary: z.record(z.string(), z.unknown()),
          diagnostics: z.array(z.record(z.string(), z.unknown())),
        })
        .parse(report);
    if (kind === "missing") delete parsed.summary.warnings;
    if (kind === "count") parsed.summary.warnings = 10;
    if (kind === "truncated") parsed.summary.diagnosticsNotPrinted = 1;
    if (kind === "outside scan") parsed.summary.unchanged = run.snapshot.sources.length + 1;
    if (kind === "unknown severity" && parsed.diagnostics[0])
      parsed.diagnostics[0].severity = "unknown";
    expect(() =>
      inspectStandardUnusedRawReport(
        parsed,
        kind === "exit" ? 0 : run.exitCode,
        run.snapshot.sources.length,
      ),
    ).toThrow();
  });

test("standard raw Biome info severity preserves summary and other diagnostics", () => {
  const report: {
      summary: {
        errors: number;
        warnings: number;
        infos: number;
        changed: number;
        unchanged: number;
        skipped: number;
        diagnosticsNotPrinted: number;
      };
      diagnostics: { severity: "error" | "warning" | "info"; category: string; message: string }[];
    } = {
      summary: {
        errors: 1,
        warnings: 1,
        infos: 1,
        changed: 0,
        unchanged: 3,
        skipped: 0,
        diagnosticsNotPrinted: 0,
      },
      diagnostics: [
        { severity: "error", category: "format", message: "format error" },
        { severity: "warning", category: "lint/correctness/noUnusedVariables", message: "unused" },
        { severity: "info", category: "lint/style/useTemplate", message: "style information" },
      ],
    },
    inspected = inspectStandardUnusedRawReport(report, 1, 3);
  expect(inspected.diagnostics).toEqual(report.diagnostics);

  const countMismatch = { ...report, summary: { ...report.summary, infos: 0 } };
  expect(() => inspectStandardUnusedRawReport(countMismatch, 1, 3)).toThrow();

  const unknownSeverity = {
    ...report,
    diagnostics: report.diagnostics.map((diagnostic, index) =>
      index === 2 ? { ...diagnostic, severity: "notice" } : diagnostic,
    ),
  };
  expect(() => inspectStandardUnusedRawReport(unknownSeverity, 1, 3)).toThrow();
});
