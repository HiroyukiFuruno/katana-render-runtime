import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { type CallExpression, type Comment, parse } from "acorn";
import { full, fullAncestor } from "acorn-walk";
import { type RawSourceMap, SourceMapConsumer, SourceMapGenerator } from "source-map-js";
import { z } from "zod";
import { RuntimeBundleDefinition } from "#shared/runtime_bundle";
import { runtimeBundleChecksum } from "./runtime-bundle-checksum";
import { parseRuntimeBundleMode, parseRuntimeBundleOptions } from "./runtime-bundle-cli";
import { runtimeBundleDefinitions } from "./runtime-bundle-definitions";
import {
  type ClassicGenerationEvidence,
  type ClassicRollupGeneration,
  classicGenerationEvidenceSchema,
} from "./runtime-bundle-types";
import {
  classicEvalLiteral,
  parsedClassicGenerationMap,
  verifyClassicGeneration,
} from "./runtime-classic-generation";
import {
  classicGenerationInputPath,
  classicPhysicalMap,
  RuntimeRollupBundler,
} from "./runtime-rollup-bundler";

type SourceInventory = { sources: string[]; sourcesContent: string[] };
type BundleEvidenceRow = {
  definition: { name: string; outputFile: string };
  classicGeneration?: unknown;
};
const classicBundleNames = ["drawio", "mermaid", "zenuml"];
function requireCompleteClassicBundles<T extends BundleEvidenceRow>(bundles: readonly T[]): T[] {
  const classicBundles = bundles.filter((bundle) => bundle.classicGeneration !== undefined);
  const names = classicBundles.map((bundle) => bundle.definition.name).sort();
  if (JSON.stringify(names) !== JSON.stringify(classicBundleNames))
    throw new Error("actual completed evidence must include all three classic bundles");
  return classicBundles;
}

test("runtime bundle CLI keeps legacy mode parsing and accepts evidence only with check", () => {
  expect(parseRuntimeBundleMode(["--check"])).toBe("check");
  expect(parseRuntimeBundleMode(["--write"])).toBe("write");
  expect(() => parseRuntimeBundleMode(["--check", "--write"])).toThrow();
  expect(parseRuntimeBundleOptions(["--check"])).toEqual({
    mode: "check",
    classicGenerationEvidence: false,
  });
  expect(parseRuntimeBundleOptions(["--check", "--classic-generation-evidence"])).toEqual({
    mode: "check",
    classicGenerationEvidence: true,
  });
});
const invalidCliArguments: Array<[string[], string]> = [
  [[], "missing mode"],
  [["--classic-generation-evidence"], "evidence without mode"],
  [["--write", "--classic-generation-evidence"], "evidence with write"],
  [["--check", "--check"], "duplicate mode"],
  [["--check", "--write"], "conflicting modes"],
  [["--check", "--classic-generation-evidence", "--classic-generation-evidence"], "duplicate flag"],
  [["--check", "--unexpected"], "unknown argument"],
];
for (const [argv, label] of invalidCliArguments)
  test(`runtime bundle CLI rejects ${label}`, () => {
    expect(() => parseRuntimeBundleOptions(argv)).toThrow();
  });
const completeEvidenceRows = classicBundleNames.map((name) => ({
  definition: { name, outputFile: `${name}.js` },
  classicGeneration: {},
}));
for (const missingName of classicBundleNames)
  test(`actual completed evidence rejects missing ${missingName} classic bundle`, () => {
    const rows = completeEvidenceRows.filter((bundle) => bundle.definition.name !== missingName);
    expect(() => requireCompleteClassicBundles(rows)).toThrow(
      "actual completed evidence must include all three classic bundles",
    );
  });

test.each([
  "if(/^\\u00a0+$/.test(text))return",
  "const value='\\u{1F600}'; /*😀é*/",
  "\ufeffvar value=1;\r\nvar other=2;",
  "var text='\\\\unicode';",
])("same SWC physical literal and Acorn decode retain exact bytes: %s", (expected) => {
  const call = `/*😀é*/ globalThis.eval(${JSON.stringify(expected)});`;
  expect(Buffer.from(classicEvalLiteral(call))).toEqual(Buffer.from(expected));
  expect(classicEvalLiteral(`import x from 'unused'; ${call}`, true)).toBe(expected);
});
test.each([
  "void 0;",
  "globalThis.eval('a');globalThis.eval('a');",
  "globalThis['eval']('a');",
  "const call=globalThis.eval;call('a');",
  "let globalThis;globalThis.eval('a');",
  "globalThis=null;globalThis.eval('a');",
  "globalThis.eval=null;globalThis.eval('a');",
  "globalThis.eval('a'+'b');",
  "globalThis.eval(...['a']);",
  "globalThis.eval('a','b');",
  "function f(globalThis){return globalThis;}globalThis.eval('a');",
])("canonical eval structure rejects changed source: %s", (code) => {
  expect(() => classicEvalLiteral(code)).toThrow();
});
function sparseSource(code: string) {
  const map = new SourceMapGenerator();
  map.setSourceContent("physical.js", code);
  map.addMapping({
    generated: { line: 1, column: 0 },
    original: { line: 1, column: 0 },
    source: "physical.js",
  });
  map.addMapping({
    generated: { line: 2, column: 0 },
    original: { line: 2, column: 0 },
    source: "physical.js",
  });
  return { code, sha256: runtimeBundleChecksum(code), map: map.toString() };
}
test("physical map shifts actual sparse anchors and leaves header/trim/tail unmapped", async () => {
  const source = sparseSource("var emoji='😀é';\r\nvar end=1;\r\n ");
  const header = "// generated\n// name\n";
  const physical = await classicPhysicalMap(source, header, "unit.js");
  const map = JSON.parse(physical) as RawSourceMap;
  const consumer = new SourceMapConsumer(map);
  const generated: Array<{ line: number; column: number }> = [];
  consumer.eachMapping((item) =>
    generated.push({ line: item.generatedLine, column: item.generatedColumn }),
  );
  expect(generated).toEqual([
    { line: 4, column: 0 },
    { line: 5, column: 0 },
  ]);
  expect(consumer.originalPositionFor({ line: 1, column: 0 }).source).toBeNull();
  expect(consumer.originalPositionFor({ line: 6, column: 0 }).source).toBeNull();
});
test.each(["generated range", "original range", "sourceRoot", "missing source content"])(
  "map validation rejects %s",
  (kind) => {
    const source = sparseSource("a\nb");
    const map = JSON.parse(source.map) as {
      sources: string[];
      sourcesContent: string[];
      sourceRoot?: string;
      mappings: string;
    };
    if (kind === "sourceRoot") map.sourceRoot = "/changed";
    if (kind === "missing source content") map.sourcesContent = [];
    if (kind === "generated range" || kind === "original range") {
      const generator = new SourceMapGenerator();
      generator.setSourceContent("physical.js", source.code);
      generator.addMapping({
        generated: { line: kind === "generated range" ? 5 : 1, column: 0 },
        original: { line: kind === "original range" ? 8 : 1, column: 0 },
        source: "physical.js",
      });
      map.mappings = JSON.parse(generator.toString()).mappings as string;
    }
    expect(() => parsedClassicGenerationMap({ ...source, map: JSON.stringify(map) })).toThrow();
  },
);
test.each(["regular", "absolute", "traversal", "outside", "symlink", "parent symlink"])(
  "generation physical input guard handles %s before bytes read",
  async (kind) => {
    const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "classic-path-")));
    try {
      fs.writeFileSync(path.join(root, "input.js"), "unit");
      let name = "input.js";
      if (kind === "absolute") name = path.join(root, name);
      if (kind === "traversal") name = "../input.js";
      if (kind === "outside") name = "/outside/never-read.js";
      if (kind === "symlink" || kind === "parent symlink") {
        fs.symlinkSync(kind === "symlink" ? path.join(root, name) : root, path.join(root, "link"));
        name = kind === "symlink" ? "link" : "link/input.js";
      }
      const result = classicGenerationInputPath(root, name);
      if (kind === "regular") await expect(result).resolves.toBe(path.join(root, name));
      else await expect(result).rejects.toThrow(/canonical|root/);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  },
);
const allocationContracts = [
  {
    realm: "drawio",
    sourcePath:
      "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/drawio_svg_postprocess.js",
    owner: "katanaPostprocessDrawioSvg",
    callee: "katanaCropDrawioSvgToContent",
  },
  {
    realm: "mermaid",
    sourcePath:
      "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_normalize.js",
    owner: "katanaNormalizeDiagramSpecificSvg",
    callee: "katanaNormalizeReviewFeedbackSvg",
  },
] as const;
type AllocationContract = (typeof allocationContracts)[number];

function calleeAnnotation(source: string, contract: AllocationContract): Comment {
  const comments: Comment[] = [];
  const script = parse(source, {
    ecmaVersion: "latest",
    sourceType: "script",
    onComment: comments,
  });
  const owner = script.body.find(
    (node) => node.type === "FunctionDeclaration" && node.id?.name === contract.owner,
  );
  if (owner?.type !== "FunctionDeclaration")
    throw new Error(`Actual ${contract.realm} owner missing`);
  const calls: CallExpression[] = [];
  full(owner, (node) => {
    if (
      node.type === "CallExpression" &&
      node.callee.type === "Identifier" &&
      node.callee.name === contract.callee
    )
      calls.push(node);
  });
  if (calls.length !== 1 || !calls[0])
    throw new Error(`Actual ${contract.realm} helper call is not unique`);
  const call = calls[0];
  const annotation = comments.find(
    (comment) =>
      comment.type === "Block" &&
      comment.value === "@__NOINLINE__" &&
      comment.end <= call.start &&
      source.slice(comment.end, call.start).trim() === "",
  );
  if (!annotation)
    throw new Error(`Actual ${contract.realm} helper call lacks the supported Terser annotation`);
  return annotation;
}

function calleePlacement(source: string, contract: AllocationContract) {
  const script = parse(source, { ecmaVersion: "latest", sourceType: "script" });
  const global = script.body.filter(
    (node) => node.type === "FunctionDeclaration" && node.id?.name === contract.callee,
  ).length;
  let perRender = 0;
  fullAncestor(script, (node, _state, ancestors) => {
    if (node.type !== "FunctionExpression" || node.id?.name !== contract.callee) return;
    if (
      ancestors
        .slice(0, -1)
        .some(
          (ancestor) =>
            (ancestor.type === "FunctionDeclaration" || ancestor.type === "FunctionExpression") &&
            ancestor.id?.name === contract.owner,
        )
    )
      perRender += 1;
  });
  return { global, perRender };
}

const annotationCases = [
  ["supported", "/*@__NOINLINE__*/", true],
  ["missing", "", false],
  ["malformed", "/*__NOINLINE__*/", false],
] as const;
test.each(
  allocationContracts.flatMap((contract) =>
    annotationCases.map(
      ([name, marker, retained]) => [contract.realm, name, contract, marker, retained] as const,
    ),
  ),
)(
  "actual %s producer keeps global helper allocation only with %s annotation",
  async (_realm, _name, contract, marker, retained) => {
    const root = path.resolve(import.meta.dir, "../..");
    const definition = runtimeBundleDefinitions().find((item) => item.name === contract.realm);
    if (!definition || definition.bundledEntryPath !== null)
      throw new Error(`Actual classic ${contract.realm} definition missing`);
    const sourcePath = contract.sourcePath;
    const original = fs.readFileSync(path.join(root, sourcePath));
    const source = original.toString("utf8");
    const annotation = calleeAnnotation(source, contract);
    const generatedPath = path.join(
      root,
      "crates/katana-render-runtime/src/markdown/diagram_runtime/generated",
      definition.outputFile,
    );
    const generated = fs.readFileSync(generatedPath);
    const baseline = generated.toString("utf8");
    const baselineInner = classicEvalLiteral(baseline);
    expect(calleePlacement(baselineInner, contract)).toEqual({ global: 1, perRender: 0 });
    const directory = fs.mkdtempSync(
      path.join(root, "tmp", `${contract.realm}-noinline-generation-test-`),
    );
    try {
      const candidatePath = path.join(directory, path.basename(sourcePath));
      const candidate = `${source.slice(0, annotation.start)}${marker}${source.slice(annotation.end)}`;
      fs.writeFileSync(candidatePath, candidate);
      const fragments = definition.fragments.map((fragment) =>
        fragment.sourcePath === sourcePath
          ? { ...fragment, sourcePath: path.relative(root, candidatePath) }
          : fragment,
      );
      expect(
        fragments.filter((fragment) => fragment.sourcePath === path.relative(root, candidatePath)),
      ).toHaveLength(1);
      const observed: ClassicRollupGeneration[] = [];
      const body = await new RuntimeRollupBundler(root).bundle(
        new RuntimeBundleDefinition(
          definition.name,
          definition.outputFile,
          fragments,
          definition.entrypoint,
        ),
        (generation) => observed.push(generation),
      );
      if (observed.length !== 1 || !observed[0])
        throw new Error(`Actual ${contract.realm} generation evidence missing`);
      const generation = observed[0];
      const header = `// @generated by scripts/runtime-bundles/bundle-runtime.ts\n// bundle: ${definition.name}\n`;
      expect(Buffer.from(`${header}\n${body}\n`).equals(generated)).toBe(retained);
      expect(generation.inner.code === baselineInner).toBe(retained);
      expect(generation.normalized.code.includes(marker || "/*@__NOINLINE__*/")).toBe(
        marker !== "",
      );
      expect(calleePlacement(generation.inner.code, contract)).toEqual(
        retained ? { global: 1, perRender: 0 } : { global: 0, perRender: 1 },
      );
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
    expect(fs.readFileSync(path.join(root, sourcePath))).toEqual(original);
    expect(fs.readFileSync(generatedPath)).toEqual(generated);
  },
);

const evidencePath = process.env.KRR_CLASSIC_GENERATION_EVIDENCE_PATH;
if (evidencePath === undefined) {
  test.skip("actual completed four-bundle evidence input was not provided", () => {});
} else {
  const bundles = z
    .array(
      z.object({
        definition: z.object({ name: z.string(), outputFile: z.string() }),
        content: z.string(),
        checksum: z.string(),
        classicGeneration: z.unknown().optional(),
      }),
    )
    .parse(JSON.parse(fs.readFileSync(evidencePath, "utf8")));
  test("actual completed producer evidence retains all four names and separate module scope", () => {
    expect(bundles.map((bundle) => bundle.definition.name)).toEqual([
      "mermaid",
      "drawio",
      "zenuml",
      "mathjax",
    ]);
    expect(bundles[3]?.classicGeneration).toBeUndefined();
    for (const bundle of bundles)
      expect(runtimeBundleChecksum(bundle.content)).toBe(bundle.checksum);
  });
  for (const bundle of requireCompleteClassicBundles(bundles)) {
    test(`actual completed ${bundle.definition.name} evidence verifies same-pass maps and decoded bytes`, async () => {
      const evidence = classicGenerationEvidenceSchema.parse(bundle.classicGeneration);
      const pins = z
        .array(
          z
            .object({ path: z.string(), sha256: z.string(), size: z.number().int().nonnegative() })
            .strict(),
        )
        .parse(JSON.parse(evidence.inputPins ?? "null"));
      expect(pins.length).toBeGreaterThan(0);
      for (const pin of pins) {
        const bytes = fs.readFileSync(pin.path);
        expect(bytes.length).toBe(pin.size);
        expect(createHash("sha256").update(bytes).digest("hex")).toBe(pin.sha256);
      }
      const result = await verifyClassicGeneration(
        evidence,
        bundle.content,
        bundle.definition.outputFile,
      );
      expect(result.contentSha256).toBe(bundle.checksum);
      expect(Object.isFrozen(result.generation.inner)).toBe(true);
      expect(result).not.toHaveProperty("acceptedConsumerClosure");
    });
  }
  const first = bundles[0];
  if (!first) throw new Error("actual generation first bundle missing");
  const completed = classicGenerationEvidenceSchema.parse(first.classicGeneration);
  function changeProvenance(
    value: ClassicGenerationEvidence,
    mutate: (p: Record<string, unknown>) => void,
  ) {
    const p = JSON.parse(value.generation.normalizedProvenance) as Record<string, unknown>;
    mutate(p);
    Object.assign(value.generation, { normalizedProvenance: JSON.stringify(p) });
  }
  const setters: Record<string, (value: ClassicGenerationEvidence) => void> = {
    "changed input pins": (v) => Object.assign(v, { inputPins: "[]" }),
    "inner/outer swapped": (v) => Object.assign(v.generation, { inner: v.generation.rollup }),
    "changed source/hash": (v) => Object.assign(v.generation.inner, { code: "changed" }),
    "unknown evidence": (v) => Object.assign(v, { extra: true }),
    header: (v) => Object.assign(v, { header: v.header.replace("generated", "changed") }),
    "physical map": (v) => Object.assign(v, { physicalMap: v.generation.final.map }),
    "content SHA": (v) => Object.assign(v, { contentSha256: "0".repeat(64) }),
  };
  const cases: Array<[string, (value: ClassicGenerationEvidence) => void]> = [
    ...Object.entries(setters),
    ...[
      "runtime-unused-origin.ts",
      "native-build-receipt.ts",
      "bundle-runtime.ts",
      "runtime-bundle-cli.ts",
    ].map<[string, (v: ClassicGenerationEvidence) => void]>((name) => [
      `stale guard source pin ${name}`,
      (value) => {
        const pins = JSON.parse(value.inputPins ?? "null") as Array<{
          path: string;
          sha256: string;
        }>;
        const pin = pins.find((p) => p.path.endsWith(`/${name}`));
        if (!pin) throw new Error("canonical guard source pin missing");
        pin.sha256 = "0".repeat(64);
        Object.assign(value, { inputPins: JSON.stringify(pins) });
      },
    ]),
    [
      "missing normalized maps/rawInputs",
      (value) =>
        Object.assign(value.generation, {
          normalizedProvenance: JSON.stringify({ code: value.generation.normalized.code }),
        }),
    ],
    [
      "changed normalized assembly map",
      (value) =>
        changeProvenance(value, (p) => {
          p.assemblyMap = "{}";
        }),
    ],
    ...["../outside.js", "/outside.js", "different/inventory.js"].map<
      [string, (v: ClassicGenerationEvidence) => void]
    >((name) => [
      name,
      (value) =>
        changeProvenance(value, (p) => {
          const raw = p.rawInputs as Array<{ sourcePath: string }>;
          const first = raw[0];
          if (!first) throw new Error("test first source missing");
          first.sourcePath = name;
        }),
    ]),
    ["omitted completed input pins", (value) => Reflect.deleteProperty(value, "inputPins")],
    [
      "final removed source",
      (value) => {
        const map = JSON.parse(value.generation.final.map) as SourceInventory;
        map.sources.pop();
        map.sourcesContent.pop();
        Object.assign(value.generation.final, { map: JSON.stringify(map) });
      },
    ],
    [
      "inner source content",
      (value) => {
        const map = JSON.parse(value.generation.inner.map) as { sourcesContent: string[] };
        map.sourcesContent = ["other normalized source"];
        Object.assign(value.generation.inner, { map: JSON.stringify(map) });
      },
    ],
    [
      "final added source",
      (value) => {
        const map = JSON.parse(value.generation.final.map) as SourceInventory;
        map.sources.push("extra.js");
        map.sourcesContent.push("var extra=1;");
        Object.assign(value.generation.final, { map: JSON.stringify(map) });
      },
    ],
    [
      "changed literal with coherent hash",
      (value) => {
        const code = "globalThis.eval('other inner');";
        Object.assign(value.generation.final, {
          code,
          sha256: runtimeBundleChecksum(code),
          map: new SourceMapGenerator().toString(),
        });
      },
    ],
  ];
  test.each(cases)("completed evidence rejects %s", async (_name, mutate) => {
    const value = JSON.parse(JSON.stringify(completed)) as ClassicGenerationEvidence;
    mutate(value);
    await expect(
      verifyClassicGeneration(value, first.content, first.definition.outputFile),
    ).rejects.toThrow();
  });
  test.each(["header prefix", "tail", "body"])(
    "completed physical content rejects %s",
    async (kind) => {
      const content =
        kind === "header prefix"
          ? `x${first.content}`
          : kind === "tail"
            ? `${first.content}x`
            : first.content.replace("globalThis.eval", "globalThis.other");
      expect(content).not.toBe(first.content);
      await expect(
        verifyClassicGeneration(completed, content, first.definition.outputFile),
      ).rejects.toThrow();
    },
  );
}
