import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { readMathJaxAuditEvidence } from "./runtime-mathjax-audit-evidence";
import { inspectUnusedBiomeDiagnostics } from "./runtime-unused-biome";
import {
  bindMathJaxOwnedDiagnostics,
  inspectMathJaxRustFacts,
  MATHJAX_SOURCE_PATH,
  parseMathJaxRustFactsOutput,
  validateMathJaxRustFacts,
} from "./runtime-unused-mathjax";
import { physicalUnusedSource } from "./runtime-unused-origin";

const root = path.resolve(import.meta.dir, "../..");
// 準備済みhost/evidenceの検証をmodule読み込み時に一度だけ行い、5秒のBun hook制限による子process中断を防ぐ。
const evidence = readMathJaxAuditEvidence(root);
const rustFacts = inspectMathJaxRustFacts(root);

test("current physical actual MathJax producer evidence reports its empty unused result", () => {
  const report = evidence.report;

  expect(report).toMatchObject({
    auditOnly: true,
    acceptedConsumerClosure: false,
    incompleteConsumer: "MathJax ESM consumer (not a classic program)",
    sourcePath: MATHJAX_SOURCE_PATH,
    diagnosticExit: 0,
    canonical: {
      physicalSourceCount: 219,
      physicalInputPinCount: 3,
      canonicalInspectionGlobalVerified: true,
    },
  });
  expect(report.diagnostics).toHaveLength(0);
});

test("the guarded Rust facts route binds the live parser, facts, and input inventory", () => {
  expect(rustFacts).toMatchObject({
    schema: "kdr-linter/mathjax-static-facts/v1",
    status: "verified",
    parser: {
      crate: "syn",
      resolvedVersion: "3.0.9",
      features: ["full", "visit"],
      spanLocations: true,
    },
  });
  expect(rustFacts.facts).toHaveLength(13);
  expect(rustFacts.facts.map((fact) => fact.id)).toContain("default-materializer");
  expect(Object.keys(rustFacts.inputSha256)).toHaveLength(16);
  expect(
    rustFacts.inputSha256["crates/katana-render-runtime/src/markdown/runtime_assets.rs"],
  ).toMatch(/^[a-f0-9]{64}$/);
  expect(rustFacts.generatedGlobalVerified).toBe(false);
  expect(rustFacts.overrideContentVerified).toBe(false);
});

test.each([
  [
    "schema",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => Reflect.set(value, "schema", "forged"),
  ],
  [
    "fact ID",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      const fact = value.facts[0];
      if (!fact) throw new Error("missing fact fixture");
      fact.id = "forged";
    },
  ],
  [
    "default materializer fact",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      value.facts = value.facts.filter((fact) => fact.id !== "default-materializer");
    },
  ],
  [
    "input inventory",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      delete value.inputSha256["Cargo.lock"];
    },
  ],
  [
    "fact path",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      const fact = value.facts[0];
      if (!fact) throw new Error("missing fact fixture");
      fact.path = "forged.rs";
    },
  ],
  [
    "fact span",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      const fact = value.facts[0];
      if (!fact) throw new Error("missing fact fixture");
      fact.range.lineStart = 0;
    },
  ],
  [
    "reversed fact range",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      const fact = value.facts.find((item) => item.id === "default-materializer");
      if (!fact) throw new Error("missing materializer fact fixture");
      [fact.range.lineStart, fact.range.lineEnd] = [fact.range.lineEnd, fact.range.lineStart];
      [fact.range.columnStart, fact.range.columnEnd] = [
        fact.range.columnEnd,
        fact.range.columnStart,
      ];
    },
  ],
  [
    "out-of-file fact range",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      const fact = value.facts.find((item) => item.id === "default-materializer");
      if (!fact) throw new Error("missing materializer fact fixture");
      fact.range.lineStart = 999_999;
      fact.range.lineEnd = 999_999;
      fact.range.columnStart = 1;
      fact.range.columnEnd = 2;
    },
  ],
  [
    "source pin",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      value.inputSha256["Cargo.lock"] = "0".repeat(64);
    },
  ],
  [
    "shared read helper omitted",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      delete value.inputSha256["crates/katana-render-runtime/src/markdown/runtime_assets.rs"];
    },
  ],
  [
    "shared read helper hash",
    (value: ReturnType<typeof inspectMathJaxRustFacts>) => {
      value.inputSha256["crates/katana-render-runtime/src/markdown/runtime_assets.rs"] = "0".repeat(
        64,
      );
    },
  ],
])("Rust facts reject mutated %s", (_name, mutate) => {
  const value = structuredClone(rustFacts);
  mutate(value);
  expect(() => validateMathJaxRustFacts(root, value)).toThrow();
});

test("Rust facts reject empty or trailing unknown command output", () => {
  expect(() => parseMathJaxRustFactsOutput(root, "")).toThrow("no JSON");
  expect(() => parseMathJaxRustFactsOutput(root, `${JSON.stringify(rustFacts)}\nforged`)).toThrow(
    "invalid",
  );
});

test("Rust facts reject stale or failed guarded-tool metadata", () => {
  const failed = structuredClone(rustFacts);
  Reflect.set(failed, "status", "failed");
  expect(() => validateMathJaxRustFacts(root, failed)).toThrow();
  const staleParser = structuredClone(rustFacts);
  Reflect.set(staleParser.parser, "resolvedVersion", "3.0.8");
  expect(() => validateMathJaxRustFacts(root, staleParser)).toThrow();
});

test("current physical actual Unicode diagnostic evidence binds and rejects invalid raw spans", () => {
  const { directory, unicode: result } = evidence;
  const { code } = result;
  expect(code).toBe(
    `${fs.readFileSync(path.join(root, MATHJAX_SOURCE_PATH), "utf8")}\nconst displayed = "😀";\nconst unused_Ω = "😀";\nconsole.log(displayed);\n`,
  );
  expect(result.diagnostics).toHaveLength(1);
  const diagnostic = result.diagnostics[0];
  if (!diagnostic) throw new Error("missing real Unicode unused diagnostic");
  const bindings = bindMathJaxOwnedDiagnostics(
    root,
    directory,
    "mathjax-unicode-unused",
    physicalUnusedSource(Buffer.from(code)),
    result.diagnostics,
  );
  const expectedStart = code.indexOf("unused_Ω");
  const expectedEnd = expectedStart + "unused_Ω".length;
  expect(expectedStart).toBeGreaterThan(0);
  expect(bindings[0]).toMatchObject({
    sourcePath: MATHJAX_SOURCE_PATH,
    rawText: "unused_Ω",
    start: expectedStart,
    end: expectedEnd,
  });
  expect(() =>
    bindMathJaxOwnedDiagnostics(
      root,
      directory,
      "mathjax-unicode-unused",
      physicalUnusedSource(Buffer.from(code)),
      [
        {
          ...diagnostic,
          location: { ...diagnostic.location, path: "outside.js" },
        },
      ],
    ),
  ).toThrow("source does not match");
  expect(() =>
    bindMathJaxOwnedDiagnostics(
      root,
      directory,
      "mathjax-unicode-unused",
      physicalUnusedSource(Buffer.from(code)),
      [
        {
          ...diagnostic,
          location: {
            ...diagnostic.location,
            start: { ...diagnostic.location.start, column: 0 },
          },
        },
      ],
    ),
  ).toThrow("invalid Biome column");
});

test("MathJax callback diagnostics reject invalid exit and diagnostic contracts", () => {
  const diagnostic = {
    severity: "warning" as const,
    category: "lint/correctness/noUnusedVariables",
    message: "unused",
    location: {
      path: "mathjax-owned-raw.js",
      start: { line: 1, column: 1 },
      end: { line: 1, column: 2 },
    },
  };
  expect(() => inspectUnusedBiomeDiagnostics([], 17)).toThrow("incomplete/failed Biome report");
  expect(() =>
    inspectUnusedBiomeDiagnostics([{ ...diagnostic, category: "lint/unknown" }], 1),
  ).toThrow("unknown Biome diagnostic");
  expect(() => inspectUnusedBiomeDiagnostics([{ ...diagnostic, severity: "error" }], 1)).toThrow(
    "unknown Biome diagnostic",
  );
});
