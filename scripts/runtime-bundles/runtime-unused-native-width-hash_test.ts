import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import {
  NATIVE_WIDTH_HASH_INPUT_PATHS,
  parseNativeWidthHashFactsOutput,
  validateNativeWidthHashFacts,
} from "./runtime-unused-native-width-hash";
import { physicalUnusedSource } from "./runtime-unused-origin";

const repositoryRoot = path.resolve(import.meta.dir, "../..");

function actualSources() {
  return new Map(
    NATIVE_WIDTH_HASH_INPUT_PATHS.map((sourcePath) => [
      sourcePath,
      physicalUnusedSource(fs.readFileSync(path.join(repositoryRoot, sourcePath))),
    ]),
  );
}

function currentFacts() {
  const sources = actualSources();
  return {
    facts: [
      { id: "hash-fields", path: NATIVE_WIDTH_HASH_INPUT_PATHS[0] },
      { id: "svg-id-hash-route", path: NATIVE_WIDTH_HASH_INPUT_PATHS[1] },
      { id: "render-cache-hash-route", path: NATIVE_WIDTH_HASH_INPUT_PATHS[2] },
    ],
    inputSha256: Object.fromEntries(
      NATIVE_WIDTH_HASH_INPUT_PATHS.map((sourcePath) => [
        sourcePath,
        sources.get(sourcePath)?.sha256,
      ]),
    ),
    nativeExecutionVerified: false,
    performanceVerified: false,
    schema: "kdr-linter/native-width-hash-facts/v1",
    status: "verified",
  };
}

test("NativeWidth Rust facts bind the exact current three-file hash route", () => {
  const facts = validateNativeWidthHashFacts(currentFacts(), actualSources());
  expect(facts.facts).toHaveLength(3);
  expect(Object.keys(facts.inputSha256).sort()).toEqual([...NATIVE_WIDTH_HASH_INPUT_PATHS].sort());
  expect(facts.nativeExecutionVerified).toBe(false);
  expect(facts.performanceVerified).toBe(false);
});

test("NativeWidth Rust facts parse from JSON and reject empty or malformed output", () => {
  expect(
    parseNativeWidthHashFactsOutput(actualSources(), JSON.stringify(currentFacts())).facts,
  ).toHaveLength(3);
  expect(() => parseNativeWidthHashFactsOutput(actualSources(), "")).toThrow("no JSON");
  expect(() => parseNativeWidthHashFactsOutput(actualSources(), "not JSON")).toThrow("invalid");
});

test("NativeWidth Rust facts reject an unpinned or extra physical source", () => {
  const sources = new Map<string, ReturnType<typeof physicalUnusedSource>>(actualSources());
  sources.set("extra.rs", physicalUnusedSource(Buffer.from("unlisted")));
  expect(() => validateNativeWidthHashFacts(currentFacts(), sources)).toThrow(
    "physical source inventory drift",
  );
});

test("NativeWidth Rust facts reject source bytes that do not match their raw SHA", () => {
  const sources = actualSources();
  const sourcePath = NATIVE_WIDTH_HASH_INPUT_PATHS[0];
  const source = sources.get(sourcePath);
  if (!source) throw new Error("missing fixture source");
  sources.set(sourcePath, { ...source, code: `${source.code}\n` });
  expect(() => validateNativeWidthHashFacts(currentFacts(), sources)).toThrow("input hash drift");
});

test.each([
  ["unknown schema", (facts: ReturnType<typeof currentFacts>) => ({ ...facts, schema: "unknown" })],
  [
    "missing fact",
    (facts: ReturnType<typeof currentFacts>) => ({ ...facts, facts: facts.facts.slice(1) }),
  ],
  [
    "duplicate fact",
    (facts: ReturnType<typeof currentFacts>) => ({
      ...facts,
      facts: [facts.facts[0], facts.facts[0], facts.facts[2]],
    }),
  ],
  [
    "wrong fact path",
    (facts: ReturnType<typeof currentFacts>) => ({
      ...facts,
      facts: facts.facts.map((fact, index) =>
        index === 0 ? { ...fact, path: "forged.rs" } : fact,
      ),
    }),
  ],
  [
    "extra input",
    (facts: ReturnType<typeof currentFacts>) => ({
      ...facts,
      inputSha256: { ...facts.inputSha256, "extra.rs": "0".repeat(64) },
    }),
  ],
  [
    "missing input",
    (facts: ReturnType<typeof currentFacts>) => {
      const { [NATIVE_WIDTH_HASH_INPUT_PATHS[2]]: _removed, ...inputSha256 } = facts.inputSha256;
      return { ...facts, inputSha256 };
    },
  ],
  [
    "changed current source hash",
    (facts: ReturnType<typeof currentFacts>) => ({
      ...facts,
      inputSha256: {
        ...facts.inputSha256,
        [NATIVE_WIDTH_HASH_INPUT_PATHS[1]]: "0".repeat(64),
      },
    }),
  ],
  [
    "native execution claim",
    (facts: ReturnType<typeof currentFacts>) => ({ ...facts, nativeExecutionVerified: true }),
  ],
  [
    "performance claim",
    (facts: ReturnType<typeof currentFacts>) => ({ ...facts, performanceVerified: true }),
  ],
])("NativeWidth Rust facts reject %s", (_label, mutate) => {
  expect(() => validateNativeWidthHashFacts(mutate(currentFacts()), actualSources())).toThrow();
});
