import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { physicalUnusedSource, type RawUnusedSource } from "./runtime-unused-origin";
import {
  RUST_CLASSIC_INPUT_PATHS,
  physicalInputs as readRustClassicPhysicalInputs,
  validateOutput as validateRustClassicScriptFacts,
} from "./runtime-unused-rust-classic";

export const NATIVE_WIDTH_HASH_INPUT_PATHS = [
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/mod.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
] as const;

const nativeWidthFactsById = {
  "hash-fields": NATIVE_WIDTH_HASH_INPUT_PATHS[0],
  "svg-id-hash-route": NATIVE_WIDTH_HASH_INPUT_PATHS[1],
  "render-cache-hash-route": NATIVE_WIDTH_HASH_INPUT_PATHS[2],
} as const;

const nativeWidthHashFactsSchema = z.strictObject({
  facts: z.tuple([
    z.strictObject({
      id: z.literal("hash-fields"),
      path: z.literal(NATIVE_WIDTH_HASH_INPUT_PATHS[0]),
    }),
    z.strictObject({
      id: z.literal("svg-id-hash-route"),
      path: z.literal(NATIVE_WIDTH_HASH_INPUT_PATHS[1]),
    }),
    z.strictObject({
      id: z.literal("render-cache-hash-route"),
      path: z.literal(NATIVE_WIDTH_HASH_INPUT_PATHS[2]),
    }),
  ]),
  inputSha256: z.record(z.string().min(1), z.string().regex(/^[a-f0-9]{64}$/u)),
  nativeExecutionVerified: z.literal(false),
  performanceVerified: z.literal(false),
  schema: z.literal("kdr-linter/native-width-hash-facts/v1"),
  status: z.literal("verified"),
});

export type NativeWidthHashFacts = z.infer<typeof nativeWidthHashFactsSchema>;

export interface NativeWidthConsumerFacts {
  nativeWidth: NativeWidthHashFacts;
  classic: import("./runtime-unused-rust-classic").RustClassicScriptFacts;
}

function readNativeWidthPhysicalInputs(root: string): Map<string, RawUnusedSource> {
  return new Map(
    NATIVE_WIDTH_HASH_INPUT_PATHS.map((sourcePath) => {
      const absolute = path.join(root, sourcePath);
      return [sourcePath, physicalUnusedSource(fs.readFileSync(absolute))];
    }),
  );
}

function requireSourceSubset(
  paths: readonly string[],
  sources: ReadonlyMap<string, RawUnusedSource>,
): Map<string, RawUnusedSource> {
  const selected = new Map<string, RawUnusedSource>();
  for (const sourcePath of paths) {
    const source = sources.get(sourcePath);
    if (!source) throw new Error(`missing native width physical input: ${sourcePath}`);
    selected.set(sourcePath, source);
  }
  return selected;
}

function assertStablePhysicalInputs(
  paths: readonly string[],
  before: ReadonlyMap<string, RawUnusedSource>,
  after: ReadonlyMap<string, RawUnusedSource>,
  message: string,
): void {
  for (const sourcePath of paths) {
    if (before.get(sourcePath)?.sha256 !== after.get(sourcePath)?.sha256)
      throw new Error(`${message}: ${sourcePath}`);
  }
}

export function validateNativeWidthHashFacts(
  value: unknown,
  sources: ReadonlyMap<string, RawUnusedSource>,
): NativeWidthHashFacts {
  const facts = nativeWidthHashFactsSchema.parse(value);
  const actualPaths = Object.keys(facts.inputSha256).sort();
  const expectedPaths = [...NATIVE_WIDTH_HASH_INPUT_PATHS].sort();
  if (JSON.stringify(actualPaths) !== JSON.stringify(expectedPaths))
    throw new Error("NativeWidth Rust facts input inventory drift");
  if (JSON.stringify([...sources.keys()].sort()) !== JSON.stringify(expectedPaths))
    throw new Error("NativeWidth physical source inventory drift");
  for (const sourcePath of NATIVE_WIDTH_HASH_INPUT_PATHS) {
    const source = sources.get(sourcePath);
    if (!source) throw new Error(`missing NativeWidth physical input: ${sourcePath}`);
    const actualSha256 = createHash("sha256")
      .update(Buffer.from(source.code, "utf8"))
      .digest("hex");
    if (source.sha256 !== actualSha256 || facts.inputSha256[sourcePath] !== actualSha256)
      throw new Error(`NativeWidth Rust facts input hash drift: ${sourcePath}`);
  }
  for (const fact of facts.facts)
    if (nativeWidthFactsById[fact.id] !== fact.path)
      throw new Error(`NativeWidth Rust fact path drift: ${fact.id}`);
  return facts;
}

export function parseNativeWidthHashFactsOutput(
  sources: ReadonlyMap<string, RawUnusedSource>,
  stdout: string,
): NativeWidthHashFacts {
  if (!stdout.trim()) throw new Error("NativeWidth Rust facts command returned no JSON");
  try {
    return validateNativeWidthHashFacts(JSON.parse(stdout), sources);
  } catch (error) {
    throw new Error(`NativeWidth Rust facts output is invalid: ${String(error)}`);
  }
}

export function inspectNativeWidthHashFacts(
  sourceRoot: string,
  sources: ReadonlyMap<string, RawUnusedSource>,
): NativeWidthHashFacts {
  const repositoryRoot = path.resolve(import.meta.dir, "../..");
  const actualSourceRoot = fs.realpathSync(sourceRoot);
  const child = Bun.spawnSync(
    ["just", "native-width-hash-facts", repositoryRoot, actualSourceRoot],
    { cwd: repositoryRoot, timeout: 120_000 },
  );
  const stdout = Buffer.from(child.stdout).toString("utf8").trim();
  const stderr = Buffer.from(child.stderr).toString("utf8");
  if (child.exitCode !== 0) {
    const rejection = stderr.match(
      /(?:^|\r?\n)((?:hash-fields|hash-route|svg-id-hash-route|render-cache-hash-route):[^\r\n]*)/u,
    )?.[1];
    if (rejection) throw new Error(`changed native width consumer: ${rejection}`);
    throw new Error(`NativeWidth Rust facts command failed (${child.exitCode}): ${stderr}`);
  }
  return parseNativeWidthHashFactsOutput(sources, stdout);
}

export function inspectNativeWidthConsumerFacts(
  sourceRoot: string,
  sources: ReadonlyMap<string, RawUnusedSource>,
): NativeWidthConsumerFacts {
  const repositoryRoot = path.resolve(import.meta.dir, "../..");
  const actualSourceRoot = fs.realpathSync(sourceRoot);
  const classicBefore = readRustClassicPhysicalInputs(actualSourceRoot);
  const hashBefore = readNativeWidthPhysicalInputs(actualSourceRoot);
  const allInputPaths = [...RUST_CLASSIC_INPUT_PATHS, ...NATIVE_WIDTH_HASH_INPUT_PATHS];
  const sourceBefore = requireSourceSubset(allInputPaths, sources);
  assertStablePhysicalInputs(
    allInputPaths,
    sourceBefore,
    new Map([...classicBefore, ...hashBefore]),
    "changed native width consumer",
  );
  const child = Bun.spawnSync(
    ["just", "native-width-classic-facts", repositoryRoot, actualSourceRoot],
    { cwd: repositoryRoot, timeout: 120_000 },
  );
  const stdout = Buffer.from(child.stdout).toString("utf8").trim();
  const stderr = Buffer.from(child.stderr).toString("utf8");
  if (child.exitCode !== 0)
    throw new Error(
      `native-width-classic facts command failed (${child.exitCode}): ${stderr}\nstdout: ${stdout}`,
    );
  if (!stdout) throw new Error("native-width-classic facts command returned no JSON");
  let wrapper: unknown;
  try {
    wrapper = JSON.parse(stdout);
  } catch (error) {
    throw new Error(`native-width-classic facts output is invalid: ${String(error)}`);
  }
  if (
    typeof wrapper !== "object" ||
    wrapper === null ||
    Array.isArray(wrapper) ||
    JSON.stringify(Object.keys(wrapper).sort()) !== JSON.stringify(["classic", "nativeWidth"])
  )
    throw new Error("native-width-classic facts output has an unsupported shape");
  const value = wrapper as { nativeWidth?: unknown; classic?: unknown };
  const hashSources = requireSourceSubset(NATIVE_WIDTH_HASH_INPUT_PATHS, sources);
  const classicSources = requireSourceSubset(RUST_CLASSIC_INPUT_PATHS, sources);
  const nativeWidth = validateNativeWidthHashFacts(value.nativeWidth, hashSources);
  const classic = validateRustClassicScriptFacts(value.classic, classicSources);
  const classicAfter = readRustClassicPhysicalInputs(actualSourceRoot);
  const hashAfter = readNativeWidthPhysicalInputs(actualSourceRoot);
  assertStablePhysicalInputs(
    allInputPaths,
    sourceBefore,
    new Map([...classicAfter, ...hashAfter]),
    "native width consumer changed during inspection",
  );
  return { nativeWidth, classic };
}
