import { afterAll, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { createContext, runInContext } from "node:vm";
import { parseSync } from "@swc/core";
import { getLineInfo } from "acorn";
import MagicString, { Bundle } from "magic-string";
import { z } from "zod";
import type { RuntimeBundleDefinition } from "#shared/runtime_bundle";
import { runtimeBundleDefinitions } from "./runtime-bundle-definitions";
import { RuntimeFragmentLoader } from "./runtime-fragment-loader";
import {
  createUnusedBiomeConfiguration,
  runUnusedContextDiagnostics,
} from "./runtime-unused-check";
import {
  type RuntimeUnusedContext,
  readUnusedPhysicalSource,
  sequenceVmUnusedContext,
} from "./runtime-unused-contexts";
import {
  entrypointOwnerPaths,
  entrypointReads,
  readEntrypointOwner,
} from "./runtime-unused-entrypoint-owner";
import {
  astObject,
  astSpan,
  BrowserUnusedOrigin,
  type BrowserUnusedProgram,
  browserAstNodes,
  physicalUnusedSource,
  RuntimeUnusedOrigin,
  sequenceVmPaths,
  type UnusedDiagnostic,
  unusedProgramAffixes,
} from "./runtime-unused-origin";
import {
  inspectSharedUnusedSemantics,
  type SharedUnusedSemanticManifest,
  semanticContextPin,
  sharedUnusedSemanticPoint,
} from "./runtime-unused-semantic-adapter";

const root = fs.realpathSync(process.cwd());
const evidence = path.join(
  root,
  "tmp/quality/shared-classic-unused-semantic-adapter-implementation-r1",
);
fs.mkdirSync(evidence, { recursive: true });
const suiteDirectory = fs.mkdtempSync(path.join(evidence, "unit-"));
afterAll(() => fs.rmSync(suiteDirectory, { recursive: true, force: true }));

function position(code: string, at: number) {
  const info = getLineInfo(code, at);
  return { line: info.line, column: Array.from(code.slice(at - info.column, at)).length + 1 };
}

function fixture(
  consumer = "candidate();",
  producer = "/*😀*/ function candidate() { return 1; }",
) {
  const directory = fs.mkdtempSync(path.join(suiteDirectory, "case-"));
  const sourcePaths = ["producer.js", "consumer.js"].map((name) =>
    path.relative(root, path.join(directory, name)).split(path.sep).join("/"),
  );
  const raw = new Map(
    sourcePaths.map((name, index) => {
      const code = index === 0 ? producer : consumer;
      fs.writeFileSync(path.join(root, name), code);
      return [name, physicalUnusedSource(Buffer.from(code))] as const;
    }),
  );
  const definition: RuntimeBundleDefinition = {
    name: "mermaid",
    outputFile: "fixture.js",
    bundledEntryPath: null,
    fragments: sourcePaths.map((sourcePath) => ({
      sourcePath,
      label: sourcePath,
      transform: "verbatim",
    })),
    entrypoint: "",
  };
  const provenance = new RuntimeFragmentLoader(root).loadDefinitionWithProvenance(definition);
  const context: RuntimeUnusedContext = {
    name: "mermaid",
    stages: ["assembled"],
    origin: new RuntimeUnusedOrigin(provenance, raw),
    codeBytesEqual: true,
  };
  const code = context.origin.assembled;
  const producerPath = sourcePaths[0];
  if (!producerPath) throw new Error("missing fixture producer");
  const source = raw.get(producerPath);
  if (!source) throw new Error("missing fixture source");
  const start = source.code.indexOf("candidate"),
    end = start + 9;
  const location = (at: number) => position(source.code, at);
  const diagnostic: UnusedDiagnostic = {
    severity: "warning",
    category: "lint/correctness/noUnusedVariables",
    message: "unused",
    location: { path: producerPath, start: location(start), end: location(end) },
  };
  const binding = (generatedStart: number, length = 9) => ({
    kind: "raw" as const,
    ...context.origin.resolve(
      {
        ...diagnostic,
        location: {
          path: "@context",
          start: position(code, generatedStart),
          end: position(code, generatedStart + length),
        },
      },
      "assembled",
    ),
    generatedStart,
    generatedEnd: generatedStart + length,
  });
  const declaration = binding(code.indexOf("candidate"));
  const reads = consumer === "candidate();" ? [binding(code.lastIndexOf("candidate"))] : [];
  const manifest: SharedUnusedSemanticManifest = {
    typescriptVersion: "7.0.2",
    api: "typescript/unstable/sync",
    sources: [...raw].map(([sourcePath, item]) => ({ sourcePath, sourceSha256: item.sha256 })),
    contexts: [semanticContextPin(context, "assembled")],
    candidates: [
      {
        diagnosticIndex: 0,
        symbol: "candidate",
        raw: { sourcePath: producerPath, sourceSha256: source.sha256, start, end },
        bindings: [{ realm: "mermaid", stage: "assembled", declaration, reads }],
      },
    ],
  };
  const report = {
    summary: { errors: 0, warnings: 1, diagnosticsNotPrinted: 0 },
    diagnostics: [diagnostic],
  };
  const run = (input: unknown = manifest, rawReport: unknown = report, exit = 1) =>
    inspectSharedUnusedSemantics(
      root,
      path.join(directory, "semantic"),
      [context],
      rawReport,
      exit,
      input,
    );
  return { run, manifest, report, context, directory, sourcePaths, binding };
}

function actualGeneratedFixture() {
  const directory = fs.mkdtempSync(path.join(suiteDirectory, "generated-"));
  const definition = runtimeBundleDefinitions().find((item) => item.name === "mermaid");
  if (!definition) throw new Error("missing actual Mermaid definition");
  const raw = new Map(
    definition.fragments.map((fragment) => [
      fragment.sourcePath,
      readUnusedPhysicalSource(root, fragment.sourcePath),
    ]),
  );
  const origin = new RuntimeUnusedOrigin(
    new RuntimeFragmentLoader(root).loadDefinitionWithProvenance(definition),
    raw,
  );
  const context: RuntimeUnusedContext = {
    name: "mermaid",
    stages: ["normalized"],
    origin,
    codeBytesEqual: true,
  };
  const code = origin.provenance.code,
    bytes = Buffer.from(code),
    nodes = browserAstNodes(parseSync(code, { syntax: "ecmascript" }));
  const offset = (at: number) =>
    new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, at - 1)).length;
  const symbols = [
    "katanaMermaidDiagramType",
    "katanaRestoreMermaidI18nText",
    "katanaNormalizeMermaidSourceI18n",
    "katanaPrepareMermaidSource",
    "katanaNormalizeMermaidSvg",
  ];
  const readEntrypoint = entrypointReads(root, context, "normalized");
  const diagnostics: UnusedDiagnostic[] = [];
  const candidates = symbols.map((symbol, diagnosticIndex) => {
    const node = nodes.find(
      ({ node }) =>
        node.type === "FunctionDeclaration" && astObject.parse(node.identifier).value === symbol,
    )?.node;
    if (!node) throw new Error("missing actual generated callee declaration");
    const range = astSpan.parse(astObject.parse(node.identifier).span);
    const declaration = sharedUnusedSemanticPoint(
      context,
      "normalized",
      offset(range.start),
      offset(range.end),
    );
    const source = raw.get(declaration.sourcePath);
    if (!source) throw new Error("missing generated callee physical source");
    diagnostics.push({
      severity: "warning",
      category: "lint/correctness/noUnusedVariables",
      message: "bounded generated-caller regression",
      location: {
        path: declaration.sourcePath,
        start: position(source.code, declaration.start),
        end: position(source.code, declaration.end),
      },
    });
    const reads = nodes
      .filter(({ node }) => node.type === "Identifier" && node.value === symbol)
      .flatMap(
        ({
          node,
        }): SharedUnusedSemanticManifest["candidates"][number]["bindings"][number]["reads"] => {
          const span = astSpan.parse(node.span),
            start = offset(span.start),
            end = offset(span.end);
          if (start === declaration.generatedStart && end === declaration.generatedEnd) return [];
          try {
            return [sharedUnusedSemanticPoint(context, "normalized", start, end)];
          } catch {
            return [readEntrypoint(start, end)];
          }
        },
      );
    const {
      kind: _kind,
      occurrence: _occurrence,
      generatedStart: _start,
      generatedEnd: _end,
      ...rawPoint
    } = declaration;
    return {
      diagnosticIndex,
      symbol,
      raw: rawPoint,
      bindings: [{ realm: "mermaid" as const, stage: "normalized" as const, declaration, reads }],
    };
  });
  const manifest: SharedUnusedSemanticManifest = {
    typescriptVersion: "7.0.2",
    api: "typescript/unstable/sync",
    sources: [...raw].map(([sourcePath, source]) => ({ sourcePath, sourceSha256: source.sha256 })),
    contexts: [semanticContextPin(context, "normalized")],
    candidates,
  };
  const report = {
    summary: { errors: 0, warnings: diagnostics.length, diagnosticsNotPrinted: 0 },
    diagnostics,
  };
  const run = (input: unknown = manifest, selectedRoot = root) =>
    inspectSharedUnusedSemantics(
      selectedRoot,
      path.join(directory, selectedRoot === root ? "semantic" : "fixture-root/tmp/semantic"),
      [context],
      report,
      1,
      input,
    );
  const physicalOwnerFixture = () => {
    const selectedRoot = path.join(directory, "fixture-root");
    for (const name of [...raw.keys(), ...entrypointOwnerPaths]) {
      const file = path.join(selectedRoot, name);
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, fs.readFileSync(path.join(root, name)));
    }
    return selectedRoot;
  };
  const refreshOwnerPins = (selectedRoot: string) => {
    for (const candidate of manifest.candidates)
      for (const binding of candidate.bindings)
        for (const read of binding.reads)
          if (read.kind === "generated-entrypoint")
            read.owner.pins = read.owner.pins.map((pin) => ({
              ...pin,
              sourceSha256: readUnusedPhysicalSource(selectedRoot, pin.sourcePath).sha256,
            }));
  };
  const actualPhysicalFactory = async (selectedRoot: string) => {
    const dom =
      "crates/katana-render-runtime/src/markdown/diagram_runtime/source/shared/dom_fragments.ts";
    fs.copyFileSync(path.join(root, dom), path.join(selectedRoot, dom));
    const loaded: unknown = await import(path.join(selectedRoot, entrypointOwnerPaths[1]));
    const create = z
      .object({
        MermaidRuntimeBundleDefinition: z.object({
          create: z.custom<() => RuntimeBundleDefinition>((value) => typeof value === "function"),
        }),
      })
      .parse(loaded).MermaidRuntimeBundleDefinition.create;
    return create();
  };
  return {
    manifest,
    context,
    report,
    run,
    directory,
    physicalOwnerFixture,
    refreshOwnerPins,
    actualPhysicalFactory,
  };
}

test("actual generated Mermaid caller five stays generated and verifies real TS owner aliases in one group", () => {
  const item = actualGeneratedFixture(),
    result = item.run();
  expect(result.dispositions.map((row) => row.provenLive)).toEqual([true, true, true, true, true]);
  expect(result.semanticProcessCount).toBe(1);
  expect([
    result.accepted,
    result.scopeComplete,
    result.actualNative,
    result.actualBrowser,
  ]).toEqual([false, false, false, false]);
  const reads = item.manifest.candidates
    .flatMap((candidate) => candidate.bindings.flatMap((binding) => binding.reads))
    .filter((read) => read.kind === "generated-entrypoint");
  expect(reads.length).toBe(5);
  expect(reads.map((read) => read.rawSourcePath)).toEqual([null, null, null, null, null]);
});

for (const key of ["cookedSha256", "generatedValueSha256"] as const)
  test(`generated owner rejects tampered ${key}`, () => {
    const item = actualGeneratedFixture(),
      read = item.manifest.candidates[0]?.bindings[0]?.reads.find(
        (read) => read.kind === "generated-entrypoint",
      );
    if (read?.kind !== "generated-entrypoint") throw new Error("missing generated tamper read");
    read.owner[key] = "0".repeat(64);
    expect(() => item.run()).toThrow("changed generated entrypoint owner/span");
  });

test("generated caller outside current entrypoint range fails closed", () => {
  const item = actualGeneratedFixture(),
    declaration = item.manifest.candidates[0]?.bindings[0]?.declaration;
  if (!declaration) throw new Error("missing actual generated declaration");
  expect(() =>
    entrypointReads(
      root,
      item.context,
      "normalized",
    )(declaration.generatedStart, declaration.generatedEnd),
  ).toThrow("outside entrypoint range");
});

test("physical owned literal source drift fails with the previous generated owner pin", () => {
  const item = actualGeneratedFixture(),
    selectedRoot = item.physicalOwnerFixture();
  fs.appendFileSync(path.join(selectedRoot, entrypointOwnerPaths[0]), "\n");
  expect(() => item.run(undefined, selectedRoot)).toThrow(
    "changed generated entrypoint owner/span",
  );
});

for (const [name, transform] of [
  [
    "wrong constructor arg",
    (code: string) => code.replace("      MERMAID_RUNTIME_ENTRYPOINT,", '      "wrong",'),
  ],
  [
    "wrong constructor export",
    (code: string) => code.replace("new RuntimeBundleDefinition(", "new MissingRuntimeDefinition("),
  ],
] as const)
  test(`owned definition rejects ${name}`, () => {
    const item = actualGeneratedFixture(),
      selectedRoot = item.physicalOwnerFixture(),
      file = path.join(selectedRoot, entrypointOwnerPaths[1]);
    fs.writeFileSync(file, transform(fs.readFileSync(file, "utf8")));
    expect(() => readEntrypointOwner(selectedRoot)).toThrow();
  });

test("real TS imported export shadow is rejected even with recomputed matching owner metadata", () => {
  const item = actualGeneratedFixture(),
    selectedRoot = item.physicalOwnerFixture(),
    file = path.join(selectedRoot, entrypointOwnerPaths[1]);
  fs.writeFileSync(
    file,
    fs
      .readFileSync(file, "utf8")
      .replace(
        "create(): RuntimeBundleDefinition",
        "create(MERMAID_RUNTIME_ENTRYPOINT: string): RuntimeBundleDefinition",
      ),
  );
  item.refreshOwnerPins(selectedRoot);
  expect(() => item.run(undefined, selectedRoot)).toThrow();
});

for (const [name, transform] of [
  [
    "spread create override",
    (code: string) =>
      code.replace("  },\n};", '  },\n  ...{ create() { return { entrypoint: "" }; } },\n};'),
  ],
  [
    "post declaration create override",
    (code: string) =>
      `${code}\nMermaidRuntimeBundleDefinition.create = () => new RuntimeBundleDefinition("mermaid", "mermaid-runtime.min.js", [], "");\n`,
  ],
  [
    "duplicate create override",
    (code: string) =>
      code.replace("  },\n};", '  },\n  create() { return { entrypoint: "" }; },\n};'),
  ],
] as const)
  test(`physical factory ${name} fails closed with current physical owner SHA`, async () => {
    const item = actualGeneratedFixture(),
      selectedRoot = item.physicalOwnerFixture();
    const file = path.join(selectedRoot, entrypointOwnerPaths[1]);
    fs.writeFileSync(file, transform(fs.readFileSync(file, "utf8")));
    expect(
      z.object({ entrypoint: z.string() }).parse(await item.actualPhysicalFactory(selectedRoot))
        .entrypoint,
    ).toBe("");
    item.refreshOwnerPins(selectedRoot);
    expect(() => item.run(undefined, selectedRoot)).toThrow("unknown owned definition");
  });

test("physical constructor non-null bundled entry default fails with current owner SHA", async () => {
  const item = actualGeneratedFixture(),
    selectedRoot = item.physicalOwnerFixture();
  const file = path.join(selectedRoot, entrypointOwnerPaths[2]);
  fs.writeFileSync(
    file,
    fs
      .readFileSync(file, "utf8")
      .replace(
        "readonly bundledEntryPath: string | null = null",
        'readonly bundledEntryPath: string | null = "alternate.js"',
      ),
  );
  expect(
    z.object({ bundledEntryPath: z.string() }).parse(await item.actualPhysicalFactory(selectedRoot))
      .bundledEntryPath,
  ).toBe("alternate.js");
  item.refreshOwnerPins(selectedRoot);
  expect(() => item.run(undefined, selectedRoot)).toThrow(
    "owned constructor classic default differs",
  );
});

const constructorFields = [
  "name",
  "outputFile",
  "fragments",
  "entrypoint",
  "bundledEntryPath",
] as const;
const constructorCases: { name: string; change: (code: string) => string }[] =
  constructorFields.flatMap((field) => [
    {
      name: `rename ${field}`,
      change: (code: string) => code.replace(`readonly ${field}:`, `readonly ignored_${field}:`),
    },
    {
      name: `missing property ${field}`,
      change: (code: string) => code.replace(`readonly ${field}:`, `${field}:`),
    },
  ]);
for (let index = 0; index < constructorFields.length - 1; index++) {
  const left = constructorFields[index],
    right = constructorFields[index + 1];
  constructorCases.push({
    name: `order ${left}/${right}`,
    change: (code: string) => {
      const lines = code.split("\n");
      const from = lines.findIndex((line) => line.includes(`readonly ${left}:`));
      const to = lines.findIndex((line) => line.includes(`readonly ${right}:`));
      const first = lines[from],
        second = lines[to];
      if (!first || !second) throw new Error("missing constructor order fixture");
      lines[from] = second;
      lines[to] = first;
      return lines.join("\n");
    },
  });
}
constructorCases.push(
  {
    name: "missing classic default",
    change: (code) =>
      code.replace(
        "readonly bundledEntryPath: string | null = null",
        "readonly bundledEntryPath: string | null",
      ),
  },
  {
    name: "extra constructor mutation",
    change: (code) => code.replace(") {}", ') { this.entrypoint = ""; }'),
  },
  {
    name: "entrypoint wrong type",
    change: (code) => code.replace("readonly entrypoint: string", "readonly entrypoint: number"),
  },
);
for (const countercase of constructorCases)
  test(`finite constructor mapping rejects ${countercase.name} with current physical owner SHA`, async () => {
    const item = actualGeneratedFixture(),
      selectedRoot = item.physicalOwnerFixture();
    const file = path.join(selectedRoot, entrypointOwnerPaths[2]);
    fs.writeFileSync(file, countercase.change(fs.readFileSync(file, "utf8")));
    const actual = await item.actualPhysicalFactory(selectedRoot);
    let loaderResult: { loaded: boolean; generatedValueSha256?: string; error?: string };
    try {
      loaderResult = {
        loaded: true,
        generatedValueSha256: new RuntimeFragmentLoader(selectedRoot).loadDefinitionWithProvenance(
          actual,
        ).entrypoint.sourceSha256,
      };
    } catch (error) {
      loaderResult = { loaded: false, error: String(error) };
    }
    item.refreshOwnerPins(selectedRoot);
    let admissionError = "";
    try {
      item.run(undefined, selectedRoot);
    } catch (error) {
      admissionError = String(error);
    }
    expect(admissionError.length).toBeGreaterThan(0);
    console.log(
      "finiteConstructorCase",
      JSON.stringify({
        name: countercase.name,
        currentConstructorSha256: readUnusedPhysicalSource(selectedRoot, entrypointOwnerPaths[2])
          .sha256,
        fields: {
          name: actual.name ?? null,
          outputFile:
            typeof actual.outputFile === "string"
              ? actual.outputFile
              : { kind: typeof actual.outputFile },
          fragments: Array.isArray(actual.fragments)
            ? { kind: "array", length: actual.fragments.length }
            : { kind: typeof actual.fragments },
          entrypoint:
            typeof actual.entrypoint === "string"
              ? { kind: "string", length: actual.entrypoint.length }
              : { kind: typeof actual.entrypoint },
          bundledEntryPath:
            actual.bundledEntryPath === null
              ? null
              : {
                  kind: typeof actual.bundledEntryPath,
                  length:
                    typeof actual.bundledEntryPath === "string"
                      ? actual.bundledEntryPath.length
                      : 0,
                },
        },
        presentProperties: constructorFields.filter((name) => Object.hasOwn(actual, name)),
        loaderResult,
        admissionError,
        accepted: false,
      }),
    );
  });

test("real TypeScript 7 reference engine proves one exact UTF16 cross-fragment read without gate acceptance", () => {
  const result = fixture().run();
  expect(result.dispositions[0]?.provenLive).toBe(true);
  expect(result.dispositions[0]?.bindings[0]?.readCount).toBe(1);
  expect([
    result.accepted,
    result.scopeComplete,
    result.actualNative,
    result.actualBrowser,
    result.dispositions[0]?.deadProved,
  ]).toEqual([false, false, false, false, false]);
  expect(result.typescriptVersion).toBe("7.0.2");
});

test("real Biome nonBMP diagnostic binds its codepoint column to TS UTF16 span", () => {
  const item = fixture();
  const config = createUnusedBiomeConfiguration(root, item.directory);
  const result = runUnusedContextDiagnostics(
    root,
    item.directory,
    config,
    "producer",
    "/*😀*/ function candidate() { return 1; }",
  );
  expect(result.diagnostics.length).toBe(1);
  const actualReport = {
    summary: { errors: 0, warnings: 1, diagnosticsNotPrinted: 0 },
    diagnostics: result.diagnostics,
  };
  expect(item.run(item.manifest, actualReport, result.exit).dispositions[0]?.provenLive).toBe(true);
});

test("two real symbols in one realm use one Node semantic process", () => {
  const producer = "function candidate() { return 1; } function second() { return 2; }";
  const item = fixture("candidate(); second();", producer),
    code = item.context.origin.assembled;
  const first = item.manifest.candidates[0]?.bindings[0];
  if (!first) throw new Error("missing first grouped binding");
  first.reads.push(item.binding(code.lastIndexOf("candidate")));
  const declaration = item.binding(code.indexOf("second"), 6),
    read = item.binding(code.lastIndexOf("second"), 6);
  const {
    kind: _kind,
    occurrence: _occurrence,
    generatedStart: _start,
    generatedEnd: _end,
    ...raw
  } = declaration;
  item.manifest.candidates.push({
    diagnosticIndex: 1,
    symbol: "second",
    raw,
    bindings: [{ realm: "mermaid", stage: "assembled", declaration, reads: [read] }],
  });
  const result = runUnusedContextDiagnostics(
    root,
    item.directory,
    createUnusedBiomeConfiguration(root, item.directory),
    "producer",
    producer,
  );
  const inspected = item.run(
    item.manifest,
    {
      summary: { errors: 0, warnings: result.diagnostics.length, diagnosticsNotPrinted: 0 },
      diagnostics: result.diagnostics,
    },
    result.exit,
  );
  expect(inspected.semanticProcessCount).toBe(1);
  expect(inspected.dispositions.map((entry) => entry.provenLive)).toEqual([true, true]);
  const requests = fs
    .readdirSync(path.join(item.directory, "semantic"))
    .filter((name) => name.endsWith("-request.json"));
  expect(requests.length).toBe(1);
  const request = requests[0];
  if (!request) throw new Error("missing grouped request");
  expect(
    JSON.parse(fs.readFileSync(path.join(item.directory, "semantic", request), "utf8")).declarations
      .length,
  ).toBe(2);
});

test("output symlink fails before writing semantic evidence", () => {
  const item = fixture(),
    target = path.join(item.directory, "other");
  fs.mkdirSync(target);
  fs.symlinkSync(target, path.join(item.directory, "semantic"));
  expect(() => item.run()).toThrow("destination symlink");
  expect(fs.readdirSync(target)).toEqual([]);
});

test("current map mutation with recomputed manifest cannot reuse cached old Origin mapping", () => {
  const item = fixture();
  if (!(item.context.origin instanceof RuntimeUnusedOrigin))
    throw new Error("wrong regression origin");
  const before = semanticContextPin(item.context, "assembled");
  Object.assign(item.context.origin.provenance, { assemblyMap: "{}", normalizationMap: "{}" });
  const after = semanticContextPin(item.context, "assembled");
  expect(after.originSha256).not.toBe(before.originSha256);
  item.manifest.contexts = [after];
  expect(() => item.run()).toThrow();
});

test("current browser map mutation with recomputed manifest cannot reuse cached old points", () => {
  const item = fixture(),
    raw = item.context.origin.raw,
    bundle = new Bundle({ separator: "\n;\n" });
  const programs: BrowserUnusedProgram[] = [...raw].map(([sourcePath, source]) => ({
    sourcePath,
    sourceSha256: source.sha256,
    role: "provider",
    mode: "owned-reference",
    start: 0,
    end: source.code.length,
    removals: [],
    data: null,
  }));
  for (const [occurrence, program] of programs.entries()) {
    const source = raw.get(program.sourcePath);
    if (!source) throw new Error("missing browser regression source");
    const { prefix, suffix } = unusedProgramAffixes(program, "browser", occurrence);
    bundle.addSource({
      content: new MagicString(source.code).prepend(prefix).append(suffix),
      filename: program.sourcePath,
    });
  }
  const code = bundle.toString(),
    origin = new BrowserUnusedOrigin(
      code,
      bundle.generateMap({ hires: true, includeContent: true }).toString(),
      programs,
      raw,
    );
  const context: RuntimeUnusedContext = {
    name: "official-mermaid-browser",
    stages: ["assembled"],
    origin,
    codeBytesEqual: false,
  };
  const candidate = item.manifest.candidates[0];
  if (!candidate) throw new Error("missing browser candidate");
  const declarationAt = code.indexOf("candidate"),
    readAt = code.lastIndexOf("candidate");
  candidate.bindings = [
    {
      realm: "official-mermaid-browser",
      stage: "assembled",
      declaration: sharedUnusedSemanticPoint(
        context,
        "assembled",
        declarationAt,
        declarationAt + 9,
      ),
      reads: [sharedUnusedSemanticPoint(context, "assembled", readAt, readAt + 9)],
    },
  ];
  item.manifest.contexts = [semanticContextPin(context, "assembled")];
  expect(
    inspectSharedUnusedSemantics(
      root,
      path.join(item.directory, "browser-good"),
      [context],
      item.report,
      1,
      item.manifest,
    ).dispositions[0]?.provenLive,
  ).toBe(true);
  const before = item.manifest.contexts[0];
  Object.assign(origin, { map: "{}" });
  const after = semanticContextPin(context, "assembled");
  expect(after.originSha256).not.toBe(before?.originSha256);
  item.manifest.contexts = [after];
  expect(() =>
    inspectSharedUnusedSemantics(
      root,
      path.join(item.directory, "browser-corrupt"),
      [context],
      item.report,
      1,
      item.manifest,
    ),
  ).toThrow();
});

for (const consumer of ["", "function shadow(candidate) { return candidate(); }", "candidate = 3;"])
  test(`real semantic references keep declaration-only candidate: ${consumer || "deleted consumer"}`, () => {
    const result = fixture(consumer).run();
    expect(result.dispositions[0]?.provenLive).toBe(false);
    expect(result.dispositions[0]?.deadProved).toBe(false);
  });

test("self-recursion does not prove external use", () => {
  expect(
    fixture("", "function candidate() { return candidate(); }").run().dispositions[0]?.provenLive,
  ).toBe(false);
});

const expressionCases: readonly [string, string, string][] = [
  ["typeof direct operand", "typeof candidate;", "const candidate = 1;"],
  ["const array direct element", "const selected = [candidate];", "const candidate = 1;"],
  ["class constructor", "new candidate();", "class candidate {}"],
  ["conditional condition", "const selected = candidate ? 1 : 2;", "const candidate = true;"],
  ["conditional branch", "const selected = true ? candidate : null;", "const candidate = 1;"],
  ["property receiver", "const selected = candidate.value;", "const candidate = { value: 1 };"],
  ["element receiver", "const selected = candidate[0];", "const candidate = [1];"],
  ["property mutation receiver", "candidate.value = 2;", "const candidate = { value: 1 };"],
  ["element mutation receiver", "candidate[0] = 2;", "const candidate = [1];"],
  ["element mutation index", "const object = {}; object[candidate] = 1;", "const candidate = 0;"],
  ["constructor argument", "new Box(candidate);", "const candidate = 1;"],
];
for (const [name, consumer, producer] of expressionCases)
  test(`real bounded expression read: ${name}`, () => {
    const item = fixture(consumer, producer),
      binding = item.manifest.candidates[0]?.bindings[0];
    if (!binding) throw new Error("missing expression regression binding");
    binding.reads.push(item.binding(item.context.origin.assembled.lastIndexOf("candidate")));
    const result = item.run();
    expect(result.dispositions[0]?.provenLive).toBe(true);
    expect(result.dispositions[0]?.bindings[0]?.readCount).toBe(1);
  });

for (const [name, consumer, producer] of [
  ["typeof self only", "", "function candidate() { return typeof candidate; }"],
  [
    "array self only",
    "",
    "function candidate() { const selected = [candidate]; return selected; }",
  ],
  [
    "typeof local shadow",
    "function shadow(candidate) { return typeof candidate; }",
    "const candidate = 1;",
  ],
  [
    "array local shadow",
    "function shadow(candidate) { const selected = [candidate]; return selected; }",
    "const candidate = 1;",
  ],
  ["class self only", "", "class candidate { static create() { return new candidate(); } }"],
  [
    "constructor local shadow",
    "function shadow(candidate) { return new candidate(); }",
    "class candidate {}",
  ],
  [
    "same-name property",
    "const object = { candidate: 1 }; object.candidate;",
    "const candidate = 1;",
  ],
  [
    "conditional local shadow",
    "function shadow(candidate) { return candidate ? 1 : 2; }",
    "const candidate = true;",
  ],
] as const)
  test(`real expression has no global read: ${name}`, () => {
    const result = fixture(consumer, producer).run();
    expect(result.dispositions[0]?.provenLive).toBe(false);
    expect(result.dispositions[0]?.deadProved).toBe(false);
  });

test("unregistered void expression remains a strict semantic failure", () => {
  expect(() => fixture("void candidate;").run()).toThrow();
});

for (const [name, consumer] of [
  ["array assignment target", "[candidate] = [1];"],
  ["for-of array target", "for ([candidate] of [[1]]) {}"],
  ["spread element", "const selected = [...candidate];"],
  ["parenthesized typeof operand", "typeof (candidate);"],
  ["nonconst array initializer", "let selected = [candidate];"],
] as const)
  test(`unbounded browser read fails closed: ${name}`, () => {
    expect(() => fixture(consumer, "let candidate = 0;").run()).toThrow();
  });

type Fixture = ReturnType<typeof fixture>;
const rejectionCases: readonly [string, (item: Fixture) => void][] = [
  [
    "fabricated context raw bytes",
    (item) => {
      const source = [...item.context.origin.raw.values()][0];
      if (source) Object.assign(source, { code: `${source.code}\n` });
    },
  ],
  [
    "source SHA",
    (item) => {
      const pin = item.manifest.sources[0];
      if (pin) pin.sourceSha256 = "0".repeat(64);
    },
  ],
  [
    "source inventory",
    (item) => {
      item.manifest.sources.pop();
    },
  ],
  [
    "map pin",
    (item) => {
      const pin = item.manifest.contexts[0];
      if (pin) pin.originSha256 = "0".repeat(64);
    },
  ],
  [
    "code pin",
    (item) => {
      const pin = item.manifest.contexts[0];
      if (pin) pin.codeSha256 = "0".repeat(64);
    },
  ],
  [
    "missing candidate",
    (item) => {
      item.manifest.candidates.pop();
    },
  ],
  [
    "stale source",
    (item) => {
      const name = item.sourcePaths[1];
      if (name) fs.appendFileSync(path.join(root, name), "\n");
    },
  ],
  [
    "unknown API",
    (item) => {
      Object.assign(item.manifest, { api: "typescript" });
    },
  ],
  [
    "different realm",
    (item) => {
      const binding = item.manifest.candidates[0]?.bindings[0];
      if (binding) binding.realm = "drawio";
    },
  ],
  [
    "UTF16 span",
    (item) => {
      const candidate = item.manifest.candidates[0];
      if (candidate) candidate.raw.start++;
    },
  ],
  [
    "generated span",
    (item) => {
      const declaration = item.manifest.candidates[0]?.bindings[0]?.declaration;
      if (declaration) declaration.generatedStart++;
    },
  ],
  [
    "missing read",
    (item) => {
      item.manifest.candidates[0]?.bindings[0]?.reads.pop();
    },
  ],
  [
    "raw category",
    (item) => {
      const diagnostic = item.report.diagnostics[0];
      if (diagnostic) diagnostic.category = "lint/correctness/noUnusedFunctionParameters";
    },
  ],
  [
    "truncated report",
    (item) => {
      item.report.summary.diagnosticsNotPrinted = 1;
    },
  ],
  [
    "wrong severity",
    (item) => {
      const diagnostic = item.report.diagnostics[0];
      if (diagnostic) diagnostic.severity = "error";
    },
  ],
  [
    "non-unused warning",
    (item) => {
      const diagnostic = item.report.diagnostics[0];
      if (diagnostic) diagnostic.category = "lint/style/noVar";
    },
  ],
];
for (const [name, mutate] of rejectionCases)
  test(`fails closed for ${name}`, () => {
    const item = fixture();
    mutate(item);
    expect(() => item.run()).toThrow();
  });

for (const [shape, consumer] of [
  ["typeof operand", "typeof candidate;"],
  ["const array element", "const selected = [candidate];"],
] as const)
  for (const [name, mutate] of rejectionCases.filter(([name]) =>
    ["source SHA", "map pin", "UTF16 span", "different realm", "missing read"].includes(name),
  ))
    test(`browser ${shape} fails closed for ${name}`, () => {
      const item = fixture(consumer),
        binding = item.manifest.candidates[0]?.bindings[0];
      if (!binding) throw new Error("missing browser read regression binding");
      binding.reads.push(item.binding(item.context.origin.assembled.lastIndexOf("candidate")));
      mutate(item);
      expect(() => item.run()).toThrow();
    });

test("summary/exit mismatch and malformed input fail closed", () => {
  for (const [report, exit] of [
    [{}, 1],
    [fixture().report, 0],
    ["{", 1],
  ] as const)
    expect(() => fixture().run(undefined, report, exit)).toThrow();
});

test("declared read in a local shadow cannot be attached to global symbol", () => {
  const item = fixture("function shadow(candidate) { return candidate(); }");
  const binding = item.manifest.candidates[0]?.bindings[0];
  if (!binding) throw new Error("missing test binding");
  const code = item.context.origin.assembled,
    at = code.lastIndexOf("candidate");
  const startInfo = getLineInfo(code, at),
    endInfo = getLineInfo(code, at + 9);
  const diagnostic = item.report.diagnostics[0];
  if (!diagnostic) throw new Error("missing test diagnostic");
  binding.reads.push({
    kind: "raw",
    ...item.context.origin.resolve(
      {
        ...diagnostic,
        location: {
          path: "@context",
          start: { line: startInfo.line, column: startInfo.column + 1 },
          end: { line: endInfo.line, column: endInfo.column + 1 },
        },
      },
      "assembled",
    ),
    generatedStart: at,
    generatedEnd: at + 9,
  });
  expect(() => item.run()).toThrow("unregistered/changed semantic read/tool");
});

test("different duplicate declaration bodies fail closed", () => {
  expect(() => fixture("function candidate() { return 2; }").run()).toThrow();
});

function sequenceVmFixture(rewrite: (code: string) => string = (code) => code) {
  const directory = fs.mkdtempSync(path.join(suiteDirectory, "sequence-vm-")),
    selectedRoot = path.join(directory, "root");
  for (const sourcePath of sequenceVmPaths) {
    const target = path.join(selectedRoot, sourcePath);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    const code = fs.readFileSync(path.join(root, sourcePath), "utf8");
    fs.writeFileSync(target, sourcePath === sequenceVmPaths[1] ? rewrite(code) : code);
  }
  fs.symlinkSync(path.join(root, "node_modules"), path.join(selectedRoot, "node_modules"));
  const context = () => sequenceVmUnusedContext(selectedRoot);
  const prepare = () => {
    const selected = context(),
      code = selected.origin.assembled;
    const module = parseSync(code, { syntax: "ecmascript" });
    const ranges = browserAstNodes(module)
      .filter(
        ({ node }) =>
          node.type === "Identifier" && node.value === "katanaNormalizeSequenceActivationSvg",
      )
      .map(({ node }) => {
        const span = astSpan.parse(node.span),
          bytes = Buffer.from(code);
        return {
          start: new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, span.start - 1))
            .length,
          end: new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, span.end - 1))
            .length,
        };
      });
    if (ranges.length !== 2 || !ranges[0] || !ranges[1])
      throw new Error("missing sequence VM declaration/read");
    const declaration = sharedUnusedSemanticPoint(
        selected,
        "assembled",
        ranges[0].start,
        ranges[0].end,
      ),
      read = sharedUnusedSemanticPoint(selected, "assembled", ranges[1].start, ranges[1].end);
    const source = selected.origin.raw.get(sequenceVmPaths[0]);
    if (!source) throw new Error("missing sequence VM raw source");
    const report = {
      summary: { errors: 0, warnings: 1, diagnosticsNotPrinted: 0 },
      diagnostics: [
        {
          severity: "warning",
          category: "lint/correctness/noUnusedVariables",
          message: "unused",
          location: {
            path: sequenceVmPaths[0],
            start: position(source.code, declaration.start),
            end: position(source.code, declaration.end),
          },
        },
      ],
    };
    const manifest: SharedUnusedSemanticManifest = {
      typescriptVersion: "7.0.2",
      api: "typescript/unstable/sync",
      sources: [...selected.origin.raw].map(([sourcePath, raw]) => ({
        sourcePath,
        sourceSha256: raw.sha256,
      })),
      contexts: [semanticContextPin(selected, "assembled")],
      candidates: [
        {
          diagnosticIndex: 0,
          symbol: "katanaNormalizeSequenceActivationSvg",
          raw: {
            sourcePath: declaration.sourcePath,
            sourceSha256: declaration.sourceSha256,
            start: declaration.start,
            end: declaration.end,
          },
          bindings: [{ realm: "sequence-vm-test", stage: "assembled", declaration, reads: [read] }],
        },
      ],
    };
    const run = () =>
      inspectSharedUnusedSemantics(
        selectedRoot,
        path.join(selectedRoot, "tmp/semantic"),
        [selected],
        report,
        1,
        manifest,
      );
    return { selected, manifest, report, run };
  };
  return { directory, selectedRoot, context, prepare };
}

test("current physical sequence VM literal proves one actual same-symbol read", () => {
  const item = sequenceVmFixture(),
    prepared = item.prepare(),
    config = createUnusedBiomeConfiguration(root, item.directory);
  const rawReport = path.join(item.directory, "raw-biome.json");
  const child = Bun.spawnSync(
    [
      "bunx",
      "--no-install",
      "biome",
      "lint",
      `--config-path=${config}`,
      "--only=correctness/noUnusedVariables",
      "--error-on-warnings",
      "--max-diagnostics=none",
      "--reporter=json",
      `--reporter-file=${rawReport}`,
      path.join(item.selectedRoot, sequenceVmPaths[0]),
    ],
    { cwd: root, timeout: 5000 },
  );
  const actual = z
    .object({
      diagnostics: z.array(
        z.object({
          message: z.string(),
          location: z.object({
            start: z.object({ line: z.number(), column: z.number() }),
            end: z.object({ line: z.number(), column: z.number() }),
          }),
        }),
      ),
    })
    .parse(JSON.parse(fs.readFileSync(rawReport, "utf8")));
  expect(child.exitCode).toBe(1);
  const diagnostic = actual.diagnostics.find((item) =>
    item.message.includes("katanaNormalizeSequenceActivationSvg"),
  );
  expect(diagnostic?.location.start).toEqual(prepared.report.diagnostics[0]?.location.start);
  expect(diagnostic?.location.end).toEqual(prepared.report.diagnostics[0]?.location.end);
  const original = createContext({}),
    projected = createContext({});
  const source = prepared.selected.origin.raw.get(sequenceVmPaths[0]);
  if (!source) throw new Error("missing actual sequence VM producer");
  runInContext(source.code, original);
  const actualValue: unknown = runInContext("katanaNormalizeSequenceActivationSvg", original),
    projectedValue: unknown = runInContext(prepared.selected.origin.assembled, projected);
  expect(typeof actualValue).toBe("function");
  expect(typeof projectedValue).toBe("function");
  if (typeof actualValue !== "function" || typeof projectedValue !== "function")
    throw new Error("sequence VM projection did not return function");
  expect(projectedValue('<svg viewBox="0 0 10 10"></svg>')).toEqual(
    actualValue('<svg viewBox="0 0 10 10"></svg>'),
  );
  const result = prepared.run();
  expect(result.dispositions[0]?.provenLive).toBe(true);
  expect(result.dispositions[0]?.bindings[0]?.readCount).toBe(1);
  expect(result.semanticProcessCount).toBe(1);
  expect(result.scopeComplete).toBe(false);
  expect(result.accepted).toBe(false);
});

const sequenceVmShapeNegatives: readonly [string, string, string][] = [
  [
    "different context",
    'runInContext("katanaNormalizeSequenceActivationSvg", context)',
    'runInContext("katanaNormalizeSequenceActivationSvg", createContext({}))',
  ],
  [
    "wrong literal",
    '"katanaNormalizeSequenceActivationSvg"',
    '"katanaNormalizeSequenceActivationSvgMissing"',
  ],
  [
    "escaped literal",
    '"katanaNormalizeSequenceActivationSvg"',
    '"katanaNormalizeSequenceActivationSv\\u0067"',
  ],
  [
    "result is not called",
    '(runInContext("katanaNormalizeSequenceActivationSvg", context) as SvgNormalizer)(svg)',
    '(runInContext("katanaNormalizeSequenceActivationSvg", context) as SvgNormalizer)',
  ],
  ["missing load", "runInContext(SEQUENCE_FIXES, context);", ""],
  [
    "extra VM script between load and lookup",
    "runInContext(SEQUENCE_FIXES, context);",
    'runInContext(SEQUENCE_FIXES, context); runInContext("delete globalThis.katanaNormalizeSequenceActivationSvg", context);',
  ],
  [
    "wrong load path",
    "../../crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_sequence_activation_fixes.js",
    "../../crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/other.js",
  ],
  ["wrong encoding", '"utf8"', '"utf16le"'],
  ["shadow context", "const context = createContext({});", "const context = {};"],
  [
    "context write",
    "const context = createContext({});",
    "const context = createContext({}); context = createContext({});",
  ],
];
for (const [name, before, after] of sequenceVmShapeNegatives)
  test(`sequence VM fails closed: ${name}`, () => {
    const item = sequenceVmFixture((code) => code.replace(before, after));
    expect(() => item.context()).toThrow();
  });

const sequenceVmSymbolNegatives: readonly [string, (code: string) => string, string][] = [
  [
    "VM module differs",
    (code) => code.replace('from "node:vm"', 'from "./fake-vm"'),
    "sequence VM actual import module differs",
  ],
  [
    "local VM shadow",
    (code) =>
      code.replace(
        'import { createContext, runInContext } from "node:vm";',
        'import { createContext } from "node:vm"; function runInContext(...args: unknown[]) { return args[0]; }',
      ),
    "unknown sequence VM import/SDK namespace shape",
  ],
  [
    "FS module differs",
    (code) => code.replace('from "node:fs"', 'from "./fake-fs"'),
    "sequence VM actual import module differs",
  ],
  [
    "imported VM write",
    (code) => `${code}\nrunInContext = () => "";`,
    "sequence VM imported binding write/unknown use",
  ],
  [
    "loaded binding write",
    (code) => `${code}\nSEQUENCE_FIXES = "";`,
    "sequence VM source binding write/unknown use",
  ],
];
for (const [name, rewrite, reason] of sequenceVmSymbolNegatives)
  test(`sequence VM real SDK rejects: ${name}`, () => {
    const item = sequenceVmFixture(rewrite);
    expect(() => item.prepare().run()).toThrow(reason);
  });

for (const name of [
  "caller drift",
  "producer drift",
  "caller symlink",
  "stale map",
  "unbound read",
  "wrong read span",
] as const)
  test(`sequence VM physical pin rejects: ${name}`, () => {
    const item = sequenceVmFixture(),
      prepared = item.prepare();
    const caller = path.join(item.selectedRoot, sequenceVmPaths[1]),
      producer = path.join(item.selectedRoot, sequenceVmPaths[0]);
    if (name === "caller drift") fs.appendFileSync(caller, "\n// changed");
    if (name === "producer drift") fs.appendFileSync(producer, "\n// changed");
    if (name === "caller symlink") {
      fs.renameSync(caller, `${caller}.old`);
      fs.symlinkSync(`${caller}.old`, caller);
    }
    if (name === "stale map") {
      if (!(prepared.selected.origin instanceof BrowserUnusedOrigin))
        throw new Error("missing VM origin");
      Object.assign(prepared.selected.origin, { map: "{}" });
      prepared.manifest.contexts = [semanticContextPin(prepared.selected, "assembled")];
    }
    const binding = prepared.manifest.candidates[0]?.bindings[0];
    if (!binding) throw new Error("missing VM binding");
    if (name === "unbound read") binding.reads = [];
    if (name === "wrong read span") {
      const read = binding.reads[0];
      if (read) read.generatedStart++;
    }
    expect(() => prepared.run()).toThrow();
  });

test("sequence VM realm cannot reuse unbound classic Origin reads", () => {
  const item = fixture();
  const context: RuntimeUnusedContext = { ...item.context, name: "sequence-vm-test" };
  item.manifest.contexts = [semanticContextPin(context, "assembled")];
  const binding = item.manifest.candidates[0]?.bindings[0];
  if (!binding) throw new Error("missing classic fixture binding");
  binding.realm = "sequence-vm-test";
  expect(() =>
    inspectSharedUnusedSemantics(
      root,
      path.join(item.directory, "semantic"),
      [context],
      item.report,
      1,
      item.manifest,
    ),
  ).toThrow("sequence VM realm requires finite owned projection");
});

for (const module of ["vm", "fs"] as const)
  test(`sequence VM rejects actual fake ${module} despite SDK dts reexport`, async () => {
    const item = sequenceVmFixture((code) =>
        code.replace(`from "node:${module}"`, `from "./fake-${module}"`),
      ),
      folder = path.join(item.selectedRoot, path.dirname(sequenceVmPaths[1]));
    const exported = module === "vm" ? "createContext, runInContext" : "readFileSync";
    fs.writeFileSync(
      path.join(folder, `fake-${module}.d.ts`),
      `export { ${exported} } from "node:${module}";`,
    );
    fs.writeFileSync(
      path.join(folder, `fake-${module}.js`),
      module === "vm"
        ? 'export const createContext=()=>({}); export const runInContext=()=>()=>"FAKE";'
        : 'export const readFileSync=()=>"not actual source";',
    );
    const actualModule: unknown = await import(path.join(folder, `fake-${module}.js`));
    const exports = z.record(z.string(), z.unknown()).parse(actualModule),
      method = exports[module === "vm" ? "runInContext" : "readFileSync"];
    if (typeof method !== "function") throw new Error("missing actual fake SDK method");
    const actualValue: unknown = method();
    if (module === "vm") {
      expect(typeof actualValue).toBe("function");
      if (typeof actualValue !== "function") throw new Error("fake VM did not return function");
      expect(actualValue()).toBe("FAKE");
    } else expect(actualValue).toBe("not actual source");
    const prepared = item.prepare();
    expect(() => prepared.run()).toThrow("sequence VM actual import module differs");
  });

const sequenceVmSdkMutationNegatives: readonly [
  string,
  (code: string) => string,
  string,
  "context" | "sdk",
][] = [
  [
    "unknown bun module",
    (code) => code.replace('from "bun:test"', 'from "./fake-bun"'),
    "unknown sequence VM import/SDK namespace shape",
    "context",
  ],
  [
    "unknown crypto module",
    (code) => code.replace('from "node:crypto"', 'from "./fake-crypto"'),
    "unknown sequence VM import/SDK namespace shape",
    "context",
  ],

  [
    "local URL shadow",
    (code) =>
      code.replace(
        "const SEQUENCE_FIXES",
        'class URL extends globalThis.URL { constructor(_input:string,_base:string) {super("file:///tmp/not-the-pinned-sequence.js");} }\nconst SEQUENCE_FIXES',
      ),
    "sequence VM URL constructor is not current builtin",
    "sdk",
  ],
  [
    "global URL write",
    (code) => `${code}\nglobalThis.URL = class extends globalThis.URL {};`,
    "sequence VM builtin URL write/unknown use",
    "sdk",
  ],
  [
    "URL property write",
    (code) => `${code}\nURL.prototype.toString = () => "fake";`,
    "sequence VM builtin URL write/unknown use",
    "sdk",
  ],
  [
    "VM method property write",
    (code) => `${code}\nrunInContext.call = () => "fake";`,
    "sequence VM imported binding write/unknown use",
    "sdk",
  ],
  [
    "FS method property write",
    (code) => `${code}\nreadFileSync.call = () => "fake";`,
    "sequence VM imported binding write/unknown use",
    "sdk",
  ],
  [
    "additional VM namespace write",
    (code) => `import * as vm from "node:vm";\n${code}\nvm.runInContext = () => "fake";`,
    "unknown sequence VM import/SDK namespace shape",
    "context",
  ],
  [
    "additional VM default write",
    (code) => `import vm from "node:vm";\n${code}\nvm.runInContext = () => "fake";`,
    "unknown sequence VM import/SDK namespace shape",
    "context",
  ],
  [
    "require SDK write",
    (code) => `${code}\nrequire("node:vm").runInContext = () => "fake";`,
    "unknown sequence VM dynamic SDK loader",
    "context",
  ],
  [
    "dynamic import SDK write",
    (code) => `${code}\nimport("node:vm").then(vm=>vm.runInContext = () => "fake");`,
    "unknown sequence VM dynamic SDK loader",
    "context",
  ],
  [
    "builtin SDK loader write",
    (code) => `${code}\nprocess.getBuiltinModule("vm").runInContext = () => "fake";`,
    "unknown sequence VM dynamic SDK loader",
    "context",
  ],
];
for (const [name, rewrite, reason, phase] of sequenceVmSdkMutationNegatives)
  test(`sequence VM bounded SDK mutation rejects: ${name}`, () => {
    const item = sequenceVmFixture(rewrite);
    if (phase === "context") expect(() => item.context()).toThrow(reason);
    else {
      const prepared = item.prepare();
      expect(() => prepared.run()).toThrow(reason);
    }
  });
