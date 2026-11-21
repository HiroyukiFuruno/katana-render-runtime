import { afterAll, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { getLineInfo } from "acorn";
import { type RawSourceMap, SourceMapConsumer } from "source-map-js";
import { z } from "zod";
import { DrawioRuntimeBundleDefinition } from "#drawio/runtime_definition";
import { MermaidRuntimeBundleDefinition } from "#mermaid/runtime_definition";
import { RuntimeBundleDefinition, type RuntimeFragmentTransform } from "#shared/runtime_bundle";
import { runtimeBundleDefinitions } from "./runtime-bundle-definitions";
import { RuntimeFragmentLoader } from "./runtime-fragment-loader";
import { RuntimeScriptNormalizer } from "./runtime-script-normalizer";

const root = path.resolve(import.meta.dir, "../..");
const loader = new RuntimeFragmentLoader(root);

test.each([
  ["Mermaid", MermaidRuntimeBundleDefinition.create()],
  ["Drawio", DrawioRuntimeBundleDefinition.create()],
] as const)("%s preserves the production compiler input", (_label, definition) => {
  const previousAssembly = [
    ...definition.fragments.map((fragment) => loader.load(fragment)),
    definition.entrypoint.trim(),
    "",
  ].join("\n");
  const previousInput = new RuntimeScriptNormalizer().deduplicateFunctionDeclarations(
    previousAssembly,
  );
  expect(loader.loadDefinition(definition)).toBe(previousInput);
});

test("shared assembly rejects a module entry rather than inspecting an empty program", () => {
  const definition = new RuntimeBundleDefinition("mathjax", "mathjax.js", [], "", "module.ts");
  expect(() => loader.loadDefinition(definition)).toThrow("Runtime definition uses a module entry");
});

test("shared assembly retains the last declaration and the real entrypoint", () => {
  const definition = new RuntimeBundleDefinition(
    "mermaid",
    "mermaid.js",
    [],
    " function shared() { return 1; }\nfunction shared() { return 2; }\nshared(); ",
  );
  const program = loader.loadDefinition(definition);
  expect(program).not.toContain("return 1");
  expect(program).toContain("function shared() { return 2; }");
  expect(program.endsWith("shared();\n")).toBe(true);
});

const fixtureDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "runtime-provenance-"));
let fixtureIndex = 0;
afterAll(() => fs.rmSync(fixtureDirectory, { recursive: true, force: true }));

function fixture(
  source: string,
  transform: RuntimeFragmentTransform = "verbatim",
  entrypoint = "",
) {
  const sourcePath = `fragment-${fixtureIndex++}.js`;
  fs.writeFileSync(path.join(fixtureDirectory, sourcePath), source);
  const definition = new RuntimeBundleDefinition(
    "drawio",
    "test.js",
    [{ label: "fixture.js", sourcePath, transform }],
    entrypoint,
  );
  const sourceLoader = new RuntimeFragmentLoader(fixtureDirectory);
  return {
    definition,
    sourcePath,
    sourceLoader,
    result: sourceLoader.loadDefinitionWithProvenance(definition),
  };
}

function libraryMap(map: string): SourceMapConsumer {
  // V3実値を検査し、外部宣言だけのversion:string/null不整合に境界を閉じる。
  const schema = z.object({
    version: z.literal(3),
    sources: z.array(z.string()),
    names: z.array(z.string()),
    mappings: z.string(),
    sourcesContent: z.array(z.string().nullable()).optional(),
    file: z.string().optional(),
    sourceRoot: z.string().optional(),
  });
  const value: unknown = JSON.parse(map);
  return new SourceMapConsumer(schema.parse(value) as unknown as RawSourceMap);
}

const definitions = runtimeBundleDefinitions();
test.each(definitions.filter((definition) => definition.bundledEntryPath === null))(
  "%s provenance preserves canonical classic input bytes",
  (definition) => {
    const result = loader.loadDefinitionWithProvenance(definition);
    expect(result.code).toBe(loader.loadDefinition(definition));
    expect(result.rawInputs.map((input) => input.sourcePath)).toEqual(
      definition.fragments.map((fragment) => fragment.sourcePath),
    );
    expect(result.entrypoint.rawSourcePath).toBeNull();
    expect(result.entrypoint.sourceSha256).toBe(
      createHash("sha256").update(definition.entrypoint.trim()).digest("hex"),
    );
  },
);

test("provenance retains canonical module rejection before empty assembly", () => {
  const definition = definitions.find((item) => item.name === "mathjax");
  if (!definition) throw new Error("canonical MathJax definition missing");
  expect(() => loader.loadDefinitionWithProvenance(definition)).toThrow(
    "Runtime definition uses a module entry",
  );
});

const transformCases = [
  ["drawio-runtime-adapter-without-installer", "katanaInstallDrawioRuntimeAdapter();"],
  ["drawio-render-without-auto-call", "katanaRenderDrawioSvg();"],
] as const;
test.each(transformCases)("%s records only the first exact removal", (transform, target) => {
  const raw = `${target}\nconst retained = 1;\n${target}`;
  const { result, definition, sourceLoader } = fixture(raw, transform);
  expect(result.code).toBe(sourceLoader.loadDefinition(definition));
  expect(result.code.split(target)).toHaveLength(2);
  expect(result.rawInputs[0]?.transformRemovals).toEqual([
    {
      start: 0,
      end: target.length,
      startPosition: { line: 1, column: 0 },
      endPosition: { line: 1, column: target.length },
    },
  ]);
  expect(result.rawInputs[0]?.sourceSha256).toBe(createHash("sha256").update(raw).digest("hex"));
});

test.each(transformCases)("%s preserves missing-target failure", (transform) => {
  expect(() => fixture("const retained = 1;", transform)).toThrow(
    "Runtime bundle transform target was not found",
  );
});

test("non-ASCII retained raw span maps through both standard library layers", () => {
  const raw =
    "function shared(){return 1;}\n/* 日本😀 */ const retained = 2;\nfunction shared(){return 3;}";
  const { result, sourcePath, definition, sourceLoader } = fixture(raw, "verbatim", " shared(); ");
  expect(result.code).toBe(sourceLoader.loadDefinition(definition));
  expect(result.code).not.toContain("return 1");
  const generated = getLineInfo(result.code, result.code.indexOf("retained"));
  const intermediate = libraryMap(result.normalizationMap).originalPositionFor(generated);
  const original = libraryMap(result.assemblyMap).originalPositionFor(intermediate);
  const expected = getLineInfo(raw, raw.indexOf("retained"));
  expect({ source: original.source, line: original.line, column: original.column }).toEqual({
    source: sourcePath,
    ...expected,
  });
  expect(original.name).toBeNull();
  expect(result.duplicateRemovals).toHaveLength(1);
  expect(result.rawInputs[0]?.rawLength).toBe(raw.length);
});

test("generated labels and virtual entrypoint are not assigned raw origin", () => {
  const { result } = fixture("const retained = 1;", "verbatim", " retained; ");
  const assembly = libraryMap(result.assemblyMap);
  const label = result.generatedSpans.find((span) => span.kind === "label");
  if (!label) throw new Error("generated label provenance missing");
  expect(assembly.originalPositionFor(label.startPosition).source).toBeNull();
  expect(assembly.originalPositionFor(result.entrypoint.assembledRange.startPosition).source).toBe(
    result.entrypoint.source,
  );
  expect(result.entrypoint.kind).toBe("entrypoint");
  expect(result.entrypoint.rawSourcePath).toBeNull();
});

test("removed function newline retains separate exclusion evidence despite map anchor", () => {
  const { result } = fixture("function shared(){return 1;}\nfunction shared(){return 2;}");
  const removed = result.duplicateRemovals[0];
  const label = result.generatedSpans.find((span) => span.kind === "label");
  if (!removed || !label) throw new Error("duplicate/label evidence missing");
  const generated = getLineInfo(result.code, label.end);
  const anchor = libraryMap(result.normalizationMap).originalPositionFor(generated);
  expect({ line: anchor.line, column: anchor.column }).toEqual(removed.startPosition);
  expect(removed.end).toBeGreaterThan(removed.start);
  expect(result.code).not.toContain("return 1");
});

test("empty classic input keeps the original final newline and no raw attribution", () => {
  const definition = new RuntimeBundleDefinition("mermaid", "test.js", [], "   ");
  const result = loader.loadDefinitionWithProvenance(definition);
  expect(result.code).toBe(loader.loadDefinition(definition));
  expect(result.code).toBe("\n");
  expect(result.rawInputs).toEqual([]);
  expect(result.duplicateRemovals).toEqual([]);
  expect(result.entrypoint.assembledRange.start).toBe(result.entrypoint.assembledRange.end);
});

test.each([
  ["BOM", "\ufeffconst retained = 1;"],
  ["Unicode", "/* 日本😀 */ const retained = 1;"],
  ["CRLF", "const retained = 1;\r\nconst next = 2;\r\n"],
] as const)("%s provenance retains physical file SHA and canonical bytes", (_name, source) => {
  const { result, sourcePath, definition, sourceLoader } = fixture(source);
  const bytes = fs.readFileSync(path.join(fixtureDirectory, sourcePath));
  expect(Buffer.from(result.code)).toEqual(Buffer.from(sourceLoader.loadDefinition(definition)));
  expect(result.rawInputs[0]?.sourceSha256).toBe(createHash("sha256").update(bytes).digest("hex"));
  expect(result.rawInputs[0]?.rawLength).toBe(source.length);
  const original = libraryMap(result.assemblyMap).sourceContentFor(sourcePath);
  expect(original).toBe(source);
});

test("distinct malformed UTF8 physical files cannot share a decoded provenance identity", () => {
  const sources = [0xff, 0xfe].map((byte) => {
    const input = fixture("/* valid */");
    const file = path.join(fixtureDirectory, input.sourcePath);
    fs.writeFileSync(file, Buffer.from([0x2f, 0x2a, byte, 0x2a, 0x2f]));
    return { ...input, bytes: fs.readFileSync(file), decoded: fs.readFileSync(file, "utf8") };
  });
  const [first, second] = sources;
  if (!first || !second) throw new Error("malformed UTF8 pair missing");
  expect(first.decoded).toBe(second.decoded);
  expect(createHash("sha256").update(first.bytes).digest("hex")).not.toBe(
    createHash("sha256").update(second.bytes).digest("hex"),
  );
  expect(first.sourceLoader.loadDefinition(first.definition)).toBe(
    second.sourceLoader.loadDefinition(second.definition),
  );
  for (const input of sources)
    expect(() => input.sourceLoader.loadDefinitionWithProvenance(input.definition)).toThrow();
});
