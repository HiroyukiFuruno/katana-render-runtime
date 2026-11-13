import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { parseSync } from "@swc/core";
import { z } from "zod";
import { MathJaxRuntimeBundleDefinition } from "../../crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition";
import type { FileRecord } from "../mermaid/native-build-receipt";
import { RuntimeRollupBundler } from "./runtime-rollup-bundler";
import { inspectUnusedBiomeDiagnostics } from "./runtime-unused-biome";
import {
  astObject,
  astSpan,
  browserAstNodes,
  physicalUnusedSource,
  type RawUnusedSource,
  type UnusedDiagnostic,
} from "./runtime-unused-origin";
import { Lines, mapIndex, sourceMapSchema } from "./runtime-unused-source-map";

export const MATHJAX_MODULE_ENTRY = "\0krr-runtime-bundle:mathjax.js";
export const MATHJAX_SOURCE_PATH =
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime/render_mathjax.js";
export const MATHJAX_GLOBAL = "katanaRunMathJaxRuntime";
const MATHJAX_RUST_FACTS_SCHEMA = "kdr-linter/mathjax-static-facts/v1" as const;
const MATHJAX_RUST_FACT_IDS = [
  "backend-render-entrypoint",
  "backend-runtime-path",
  "default-materialize",
  "default-materializer",
  "explicit-override",
  "global-invocation",
  "include-str-generated",
  "read-render-route",
  "runtime-path-resolver",
  "script-order",
  "v8-compile-order",
  "v8-script-origin-helper",
  "v8-script-private-module",
] as const;
const MATHJAX_RUST_FACT_PATHS: Record<(typeof MATHJAX_RUST_FACT_IDS)[number], string> = {
  "backend-render-entrypoint": "crates/katana-render-runtime/src/renderer/backends/mathjax.rs",
  "backend-runtime-path": "crates/katana-render-runtime/src/renderer/backends/mathjax.rs",
  "default-materialize": "crates/katana-render-runtime/src/markdown/mathjax_renderer/mod.rs",
  "default-materializer": "crates/katana-render-runtime/src/markdown/mathjax_renderer/mod.rs",
  "explicit-override": "crates/katana-render-runtime/src/markdown/mathjax_renderer/mod.rs",
  "global-invocation":
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime_scripts.rs",
  "include-str-generated":
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime_scripts.rs",
  "read-render-route": "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime.rs",
  "runtime-path-resolver": "crates/katana-render-runtime/src/renderer/runtime_path.rs",
  "script-order":
    "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime_scripts.rs",
  "v8-compile-order": "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
  "v8-script-origin-helper": "crates/katana-render-runtime/src/v8_script.rs",
  "v8-script-private-module": "crates/katana-render-runtime/src/lib.rs",
};
const MATHJAX_RUST_INPUT_PATHS = [
  "Cargo.lock",
  "Cargo.toml",
  "crates/katana-render-runtime/src/lib.rs",
  "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
  "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mathjax-runtime.min.js",
  "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition.ts",
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime.rs",
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime/render_mathjax.js",
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime_scripts.rs",
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/mod.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
  "crates/katana-render-runtime/src/markdown/runtime_assets.rs",
  "crates/katana-render-runtime/src/markdown/runtime_resource_loader.rs",
  "crates/katana-render-runtime/src/renderer/backends/mathjax.rs",
  "crates/katana-render-runtime/src/renderer/backends/mermaid.rs",
  "crates/katana-render-runtime/src/renderer/runtime_path.rs",
  "crates/katana-render-runtime/src/resource_bytes.rs",
  "crates/katana-render-runtime/src/v8_script.rs",
  "crates/kdr-linter/Cargo.toml",
  "scripts/runtime-bundles/runtime-bundle-definitions.ts",
] as const;
export const MATHJAX_STATIC_IMPORTS = [
  "@mathjax/src/mjs/adaptors/liteAdaptor.js",
  "@mathjax/src/mjs/handlers/html.js",
  "@mathjax/src/mjs/input/tex.js",
  "@mathjax/src/mjs/mathjax.js",
  "@mathjax/src/mjs/output/svg.js",
] as const;
const MATHJAX_INPUT_PATHS = [
  "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition.ts",
  "package.json",
  "bun.lock",
] as const;

const sourceMapInputSchema = z.strictObject({
  version: z.literal(3),
  file: z.string(),
  sources: z.array(z.string()),
  sourcesContent: z.array(z.string()),
  names: z.array(z.string()),
  mappings: z.string(),
  x_google_ignoreList: z.array(z.number().int().nonnegative()).optional(),
});
const moduleSchema = z.strictObject({
  id: z.string(),
  code: z.string().nullable(),
  importedIds: z.array(z.string()),
  dynamicallyImportedIds: z.array(z.string()),
  importers: z.array(z.string()),
  dynamicImporters: z.array(z.string()),
  isEntry: z.boolean(),
  isExternal: z.boolean(),
  isIncluded: z.boolean().nullable(),
});
const outputSchema = z.strictObject({
  fileName: z.string(),
  code: z.string().min(1),
  facadeModuleId: z.string().nullable(),
  isDynamicEntry: z.boolean(),
  isEntry: z.boolean(),
  isImplicitEntry: z.boolean(),
  moduleIds: z.array(z.string()),
  name: z.string(),
  dynamicImports: z.array(z.string()),
  imports: z.array(z.string()),
  modules: z.record(z.string(), z.unknown()),
  map: z.unknown(),
});
const inspectionSchema = z.strictObject({
  input: z.string(),
  modules: z.array(moduleSchema),
  output: outputSchema,
  sourceMapAsset: z.strictObject({
    fileName: z.string(),
    source: z.union([z.string(), z.instanceof(Uint8Array)]),
  }),
});
export type MathJaxModuleInspectionInput = z.input<typeof inspectionSchema>;

export interface MathJaxModuleSource {
  readonly sourcePath: string;
  readonly sourceSha256: string;
  readonly rawLength: number;
}

export interface MathJaxGlobalBinding {
  readonly sourcePath: typeof MATHJAX_SOURCE_PATH;
  readonly sourceSha256: string;
  readonly generatedStart: number;
  readonly generatedEnd: number;
  readonly rawStart: number;
  readonly rawEnd: number;
}

export interface MathJaxModuleOrigin {
  readonly acceptedConsumerClosure: false;
  readonly auditOnly: true;
  readonly incompleteConsumer: "MathJax ESM consumer (not a classic program)";
  readonly input: typeof MATHJAX_MODULE_ENTRY;
  readonly moduleCount: number;
  readonly staticImportCount: number;
  readonly sourceMapSourceCount: number;
  readonly externalNodeCount: number;
  readonly dynamicNodeCount: number;
  readonly canonicalOutputSha256: string;
  readonly physicalSources: readonly MathJaxModuleSource[];
  readonly physicalInputPins: readonly MathJaxModuleSource[];
  readonly global: MathJaxGlobalBinding;
  readonly canonicalInspectionGlobalVerified: true;
  readonly rustCallerVerified: false;
  readonly overrideRouteVerified: false;
}

export interface MathJaxModuleInspector {
  readonly inspect: (candidate: unknown) => MathJaxModuleOrigin;
  readonly inspectCanonical: () => MathJaxModuleOrigin;
}

const rustFactRangeSchema = z.strictObject({
  columnEnd: z.number().int().positive(),
  columnStart: z.number().int().positive(),
  lineEnd: z.number().int().positive(),
  lineStart: z.number().int().positive(),
});
const mathJaxRustFactsSchema = z.strictObject({
  facts: z.array(
    z.strictObject({
      id: z.string().min(1),
      path: z.string().min(1),
      range: rustFactRangeSchema,
    }),
  ),
  generatedGlobalVerified: z.literal(false),
  inputSha256: z.record(z.string().min(1), z.string().regex(/^[a-f0-9]{64}$/)),
  overrideContentVerified: z.literal(false),
  parser: z.strictObject({
    crate: z.literal("syn"),
    features: z.tuple([z.literal("full"), z.literal("visit")]),
    resolvedVersion: z.literal("3.0.10"),
    spanLocations: z.literal(true),
  }),
  schema: z.literal(MATHJAX_RUST_FACTS_SCHEMA),
  status: z.literal("verified"),
});
export type MathJaxRustStaticFacts = z.infer<typeof mathJaxRustFactsSchema>;

export function validateMathJaxRustFacts(root: string, value: unknown): MathJaxRustStaticFacts {
  const facts = mathJaxRustFactsSchema.parse(value);
  const actualPaths = Object.keys(facts.inputSha256).sort();
  const expectedPaths = [...MATHJAX_RUST_INPUT_PATHS].sort();
  if (JSON.stringify(actualPaths) !== JSON.stringify(expectedPaths))
    throw new Error("MathJax Rust facts input inventory drift");
  for (const name of MATHJAX_RUST_INPUT_PATHS)
    if (readSource(root, name).sha256 !== facts.inputSha256[name])
      throw new Error(`MathJax Rust facts input hash drift: ${name}`);
  const ids = facts.facts.map((fact) => fact.id).sort();
  if (JSON.stringify(ids) !== JSON.stringify([...MATHJAX_RUST_FACT_IDS].sort()))
    throw new Error("MathJax Rust facts ID inventory drift");
  if (new Set(ids).size !== ids.length) throw new Error("duplicate MathJax Rust fact ID");
  for (const fact of facts.facts)
    if (MATHJAX_RUST_FACT_PATHS[fact.id as (typeof MATHJAX_RUST_FACT_IDS)[number]] !== fact.path)
      throw new Error(`MathJax Rust fact path drift: ${fact.id}`);
  for (const fact of facts.facts) validateMathJaxRustFactRange(root, fact);
  return facts;
}

function validateMathJaxRustFactRange(
  root: string,
  fact: MathJaxRustStaticFacts["facts"][number],
): void {
  const source = readSource(root, fact.path);
  const lines = new Lines(source.code);
  const start = lines.offset(fact.range.lineStart, fact.range.columnStart - 1);
  const end = lines.offset(fact.range.lineEnd, fact.range.columnEnd - 1);
  if (end <= start) throw new Error(`MathJax Rust fact range is reversed or empty: ${fact.id}`);
  if (source.code.slice(start, end).trim().length === 0)
    throw new Error(`MathJax Rust fact range has no source text: ${fact.id}`);
}

export function parseMathJaxRustFactsOutput(root: string, stdout: string): MathJaxRustStaticFacts {
  if (!stdout.trim()) throw new Error("MathJax Rust facts command returned no JSON");
  try {
    return validateMathJaxRustFacts(root, JSON.parse(stdout));
  } catch (error) {
    throw new Error(`MathJax Rust facts output is invalid: ${String(error)}`);
  }
}

export function inspectMathJaxRustFacts(root: string): MathJaxRustStaticFacts {
  const repositoryRoot = fs.realpathSync(root);
  const child = Bun.spawnSync(["just", "mathjax-consumer-facts", repositoryRoot], {
    cwd: repositoryRoot,
    timeout: 120_000,
  });
  const stdout = Buffer.from(child.stdout).toString("utf8").trim();
  const stderr = Buffer.from(child.stderr).toString("utf8");
  if (child.exitCode !== 0)
    throw new Error(`MathJax Rust facts command failed (${child.exitCode}): ${stderr}`);
  return parseMathJaxRustFactsOutput(repositoryRoot, stdout);
}

export const MATHJAX_OWNED_DIAGNOSTIC_NAME = "mathjax-owned-raw";

export interface MathJaxDiagnosticsResult {
  readonly diagnostics: readonly UnusedDiagnostic[];
  readonly exit: number;
  readonly codeSha256: string;
}

export type MathJaxDiagnosticsRunner = (name: string, code: string) => MathJaxDiagnosticsResult;

export interface MathJaxOwnedRawBinding {
  readonly diagnostic: UnusedDiagnostic;
  readonly sourcePath: typeof MATHJAX_SOURCE_PATH;
  readonly sourceSha256: string;
  readonly start: number;
  readonly end: number;
  readonly rawText: string;
}

export interface MathJaxOwnedRawReport {
  readonly auditOnly: true;
  readonly acceptedConsumerClosure: false;
  readonly incompleteConsumer: "MathJax ESM consumer (not a classic program)";
  readonly sourcePath: typeof MATHJAX_SOURCE_PATH;
  readonly sourceSha256: string;
  readonly rawLength: number;
  readonly diagnosticName: typeof MATHJAX_OWNED_DIAGNOSTIC_NAME;
  readonly diagnostics: readonly MathJaxOwnedRawBinding[];
  readonly diagnosticExit: number;
  readonly canonical: {
    readonly moduleCount: number;
    readonly staticImportCount: number;
    readonly sourceMapSourceCount: number;
    readonly physicalSourceCount: number;
    readonly physicalInputPinCount: number;
    readonly canonicalOutputSha256: string;
    readonly canonicalInspectionGlobalVerified: true;
  };
}

function safeRelative(root: string, name: string): string {
  if (
    name.length === 0 ||
    name.includes("\\") ||
    path.posix.isAbsolute(name) ||
    path.win32.isAbsolute(name) ||
    path.posix.normalize(name) !== name ||
    name.split("/").some((part) => part === "." || part === "..")
  )
    throw new Error(`unsafe MathJax source path: ${name}`);
  const absolute = path.resolve(root, name);
  if (absolute !== root && !absolute.startsWith(`${root}${path.sep}`))
    throw new Error(`MathJax source escapes repository: ${name}`);
  let current = root;
  for (const part of name.split("/")) {
    current = path.join(current, part);
    if (fs.lstatSync(current).isSymbolicLink())
      throw new Error(`MathJax source path contains symlink: ${name}`);
  }
  if (!fs.lstatSync(absolute).isFile()) throw new Error(`MathJax source is not a file: ${name}`);
  if (fs.realpathSync(absolute) !== absolute)
    throw new Error(`MathJax source path is not canonical: ${name}`);
  return absolute;
}

function readSource(root: string, name: string): RawUnusedSource {
  return physicalUnusedSource(fs.readFileSync(safeRelative(root, name)));
}

function readInputPins(root: string): MathJaxModuleSource[] {
  return MATHJAX_INPUT_PATHS.map((sourcePath) => {
    const source = readSource(root, sourcePath);
    return { sourcePath, sourceSha256: source.sha256, rawLength: source.code.length };
  });
}

export function bindMathJaxOwnedDiagnostics(
  root: string,
  directory: string,
  name: string,
  raw: RawUnusedSource,
  diagnostics: readonly UnusedDiagnostic[],
): readonly MathJaxOwnedRawBinding[] {
  const expectedFile = path.resolve(directory, `${name}.js`);
  const lines = new Lines(raw.code);
  return diagnostics.map((diagnostic) => {
    if (path.resolve(root, diagnostic.location.path) !== expectedFile)
      throw new Error("MathJax diagnostic source does not match owned temporary input");
    const start = lines.biome(diagnostic.location.start.line, diagnostic.location.start.column);
    const end = lines.biome(diagnostic.location.end.line, diagnostic.location.end.column);
    if (end <= start) throw new Error("MathJax diagnostic range is empty or reversed");
    return {
      diagnostic,
      sourcePath: MATHJAX_SOURCE_PATH,
      sourceSha256: raw.sha256,
      start,
      end,
      rawText: raw.code.slice(start, end),
    };
  });
}

function freezeOrigin(origin: MathJaxModuleOrigin): MathJaxModuleOrigin {
  const physicalSources = Object.freeze(
    origin.physicalSources.map((source) => Object.freeze({ ...source })),
  );
  const physicalInputPins = Object.freeze(
    origin.physicalInputPins.map((source) => Object.freeze({ ...source })),
  );
  const global = Object.freeze({ ...origin.global });
  return Object.freeze({ ...origin, physicalSources, physicalInputPins, global });
}

function relativeModuleId(root: string, id: string): string {
  if (!path.isAbsolute(id)) throw new Error(`unresolved MathJax module id: ${id}`);
  const relative = path.relative(root, id).split(path.sep).join("/");
  if (relative === "" || relative.startsWith("../") || relative === "..")
    throw new Error(`MathJax module escapes repository: ${id}`);
  safeRelative(root, relative);
  return relative;
}

function findGlobalAssignment(code: string): {
  start: number;
  end: number;
  memberStart: number;
  memberEnd: number;
} {
  const ast = parseSync(code, { syntax: "ecmascript" });
  const writes = browserAstNodes(ast).filter(({ node }) => {
    if (node.type !== "AssignmentExpression" || node.operator !== "=") return false;
    const left = astObject.safeParse(node.left);
    if (!left.success || left.data.type !== "MemberExpression") return false;
    const object = astObject.safeParse(left.data.object);
    const property = astObject.safeParse(left.data.property);
    return (
      object.success &&
      property.success &&
      object.data.type === "Identifier" &&
      object.data.value === "globalThis" &&
      property.data.type === "Identifier" &&
      property.data.value === MATHJAX_GLOBAL
    );
  });
  if (writes.length !== 1) throw new Error("missing/ambiguous MathJax global assignment");
  const write = writes[0]?.node;
  if (!write) throw new Error("missing MathJax global assignment");
  const left = astObject.parse(write.left);
  const right = astObject.parse(write.right);
  if (left.type !== "MemberExpression" || right.type !== "FunctionExpression")
    throw new Error("changed MathJax global assignment");
  const span = astSpan.parse(write.span);
  const member = astSpan.parse(left.span);
  const bytes = Buffer.from(code);
  const offset = (at: number) =>
    new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, at - 1)).length;
  const memberStart = offset(member.start);
  const memberEnd = offset(member.end);
  if (code.slice(memberStart, memberEnd) !== `globalThis.${MATHJAX_GLOBAL}`)
    throw new Error("changed MathJax global target");
  return {
    start: offset(span.start),
    end: offset(span.end),
    memberStart,
    memberEnd,
  };
}

function staticImports(code: string): string[] {
  const ast = parseSync(code, { syntax: "ecmascript" });
  const nodes = browserAstNodes(ast).map(({ node }) => node);
  if (
    nodes.some((node) => {
      if (node.type !== "CallExpression") return false;
      const callee = astObject.safeParse(node.callee);
      return (
        callee.success &&
        (callee.data.type === "Import" ||
          (callee.data.type === "Identifier" && callee.data.value === "require"))
      );
    })
  )
    throw new Error("MathJax raw source has dynamic/CJS import");
  return nodes
    .filter((node) => node.type === "ImportDeclaration")
    .map((node) => {
      const source = astObject.parse(node.source);
      if (source.type !== "StringLiteral" || typeof source.value !== "string")
        throw new Error("unknown MathJax static import");
      return source.value;
    });
}

function assertNoModuleTail(code: string): void {
  const ast = parseSync(code, { syntax: "ecmascript" });
  const nodes = browserAstNodes(ast).map(({ node }) => node);
  if (
    nodes.some(
      (node) =>
        node.type === "ImportDeclaration" ||
        (typeof node.type === "string" && node.type.startsWith("Export")),
    )
  )
    throw new Error("MathJax output retains module syntax");
  if (
    nodes.some((node) => {
      if (node.type !== "CallExpression") return false;
      const callee = astObject.safeParse(node.callee);
      return callee.success && callee.data.type === "Import";
    })
  )
    throw new Error("MathJax output retains dynamic import");
}

function parseAssetJson(source: string | Uint8Array): unknown {
  const text =
    typeof source === "string" ? source : new TextDecoder("utf-8", { fatal: true }).decode(source);
  try {
    return JSON.parse(text);
  } catch {
    throw new Error("MathJax source map asset is not JSON");
  }
}

function validateGraphCollections(value: z.infer<typeof inspectionSchema>): Set<string> {
  if (value.input !== MATHJAX_MODULE_ENTRY) throw new Error("unknown MathJax inspection entry");
  const ids = new Set(value.modules.map((module) => module.id));
  if (ids.size !== value.modules.length) throw new Error("duplicate MathJax module id");
  if (new Set(value.output.moduleIds).size !== value.output.moduleIds.length)
    throw new Error("duplicate MathJax output module id");
  if (
    value.output.moduleIds.length !== ids.size ||
    [...ids].some((id) => !value.output.moduleIds.includes(id))
  )
    throw new Error("MathJax graph/output module drift");
  const outputModuleIds = Object.keys(value.output.modules);
  if (outputModuleIds.length !== ids.size || outputModuleIds.some((id) => !ids.has(id)))
    throw new Error("MathJax output has unknown module");
  if ([...ids].some((id) => !outputModuleIds.includes(id)))
    throw new Error("MathJax output module metadata is incomplete");
  return ids;
}

function validateGraphEntry(
  root: string,
  value: z.infer<typeof inspectionSchema>,
  ids: ReadonlySet<string>,
): string {
  const entries = value.modules.filter((module) => module.isEntry);
  if (entries.length !== 1 || entries[0]?.id !== MATHJAX_MODULE_ENTRY)
    throw new Error("missing/ambiguous MathJax graph entry");
  if (entries[0].importedIds.length !== 1 || entries[0].importedIds[0] === undefined)
    throw new Error("changed MathJax graph entry import");
  const renderId = path.resolve(root, MATHJAX_SOURCE_PATH);
  if (entries[0].importedIds[0] !== renderId) throw new Error("MathJax graph entry source drift");
  if (!ids.has(renderId)) throw new Error("MathJax graph render module is missing");
  return renderId;
}

function validateGraphModules(
  value: z.infer<typeof inspectionSchema>,
  ids: ReadonlySet<string>,
): void {
  for (const module of value.modules) {
    if (
      module.isExternal ||
      module.dynamicallyImportedIds.length > 0 ||
      module.dynamicImporters.length > 0
    )
      throw new Error("external/dynamic MathJax module edge");
    if (
      module.id === MATHJAX_MODULE_ENTRY ? module.isIncluded !== false : module.isIncluded !== true
    )
      throw new Error("changed MathJax module inclusion");
    if (module.importedIds.some((id) => !ids.has(id)))
      throw new Error("unresolved MathJax static edge");
  }
}

function validateGraphOutput(value: z.infer<typeof inspectionSchema>): void {
  if (
    value.output.facadeModuleId !== MATHJAX_MODULE_ENTRY ||
    !value.output.isEntry ||
    value.output.isDynamicEntry ||
    value.output.isImplicitEntry ||
    value.output.imports.length !== 0 ||
    value.output.dynamicImports.length !== 0
  )
    throw new Error("changed MathJax IIFE output contract");
  if (value.output.fileName !== "_krr-runtime-bundle_mathjax.js")
    throw new Error("changed MathJax output file");
}

function validateGraph(root: string, value: z.infer<typeof inspectionSchema>): string {
  const ids = validateGraphCollections(value);
  const renderId = validateGraphEntry(root, value, ids);
  validateGraphModules(value, ids);
  validateGraphOutput(value);
  return renderId;
}

function parseAssetMap(
  value: z.infer<typeof inspectionSchema>,
): z.infer<typeof sourceMapInputSchema> {
  const map = sourceMapInputSchema.parse(value.output.map);
  if (map.file !== value.output.fileName) throw new Error("MathJax source map file drift");
  if (map.sources.length === 0 || map.sources.length !== map.sourcesContent.length)
    throw new Error("changed MathJax source map source count");
  if (value.sourceMapAsset.fileName !== `${value.output.fileName}.map`)
    throw new Error("MathJax source map asset name mismatch");
  if (!value.output.code.includes(`//# sourceMappingURL=${value.sourceMapAsset.fileName}`))
    throw new Error("MathJax output source map link is missing");
  if (
    !isDeepStrictEqual(parseAssetJson(value.sourceMapAsset.source), JSON.parse(JSON.stringify(map)))
  )
    throw new Error("MathJax source map asset drift");
  return map;
}

function normalizedInspection(value: unknown): unknown {
  const parsed = inspectionSchema.parse(value);
  const assetMap = parseAssetJson(parsed.sourceMapAsset.source);
  return {
    ...parsed,
    modules: [...parsed.modules].sort((left, right) => left.id.localeCompare(right.id)),
    output: { ...parsed.output, map: JSON.parse(JSON.stringify(parsed.output.map)) },
    sourceMapAsset: { fileName: parsed.sourceMapAsset.fileName, source: assetMap },
  };
}

function assertTrustedInspection(trusted: unknown, candidate: unknown): void {
  if (!isDeepStrictEqual(trusted, normalizedInspection(candidate)))
    throw new Error("MathJax inspection differs from the trusted Rollup inspection");
}

function readMapSources(
  root: string,
  map: z.infer<typeof sourceMapInputSchema>,
): {
  sources: MathJaxModuleSource[];
  sourceLines: Map<string, Lines>;
} {
  const sourceRecords: MathJaxModuleSource[] = [];
  const sourceLines = new Map<string, Lines>();
  for (const [index, sourcePath] of map.sources.entries()) {
    const source = readSource(root, sourcePath);
    const content = map.sourcesContent[index];
    if (content === undefined || source.code !== content)
      throw new Error(`MathJax source content drift: ${sourcePath}`);
    sourceLines.set(sourcePath, new Lines(source.code));
    sourceRecords.push({ sourcePath, sourceSha256: source.sha256, rawLength: source.code.length });
  }
  if (new Set(sourceRecords.map(({ sourcePath }) => sourcePath)).size !== sourceRecords.length)
    throw new Error("duplicate MathJax source-map source");
  return { sources: sourceRecords, sourceLines };
}

function bindGlobal(
  outputCode: string,
  raw: RawUnusedSource,
  map: z.infer<typeof sourceMapInputSchema>,
  sourceLines: ReadonlyMap<string, Lines>,
  renderPath: string,
): MathJaxGlobalBinding {
  const table = mapIndex(sourceMapSchema.parse(map), new Lines(outputCode), sourceLines);
  const rawAssignment = findGlobalAssignment(raw.code);
  const generatedAssignment = findGlobalAssignment(outputCode);
  const mapped: { source: string; offset: number }[] = [];
  for (let offset = generatedAssignment.start; offset < generatedAssignment.end; offset += 1) {
    const link = table.get(offset);
    if (link !== undefined && link !== null) mapped.push(link);
  }
  if (mapped.length < 2 || mapped.some((link) => link.source !== renderPath))
    throw new Error("MathJax global assignment is not source mapped");
  const min = Math.min(...mapped.map((link) => link.offset));
  const max = Math.max(...mapped.map((link) => link.offset));
  if (min < rawAssignment.start || max >= rawAssignment.end)
    throw new Error("MathJax global source map range drift");
  return {
    sourcePath: MATHJAX_SOURCE_PATH,
    sourceSha256: raw.sha256,
    generatedStart: generatedAssignment.start,
    generatedEnd: generatedAssignment.end,
    rawStart: rawAssignment.start,
    rawEnd: rawAssignment.end,
  };
}

function validateRawAndMap(
  root: string,
  value: z.infer<typeof inspectionSchema>,
  renderId: string,
): {
  sources: MathJaxModuleSource[];
  map: z.infer<typeof sourceMapInputSchema>;
  raw: RawUnusedSource;
  global: MathJaxGlobalBinding;
} {
  const map = parseAssetMap(value);
  const { sources, sourceLines } = readMapSources(root, map);
  const renderPath = path.relative(root, renderId).split(path.sep).join("/");
  if (!map.sources.includes(renderPath))
    throw new Error("MathJax render source is absent from map");
  const raw = readSource(root, MATHJAX_SOURCE_PATH);
  const renderIndex = map.sources.indexOf(renderPath);
  if (map.sourcesContent[renderIndex] !== raw.code)
    throw new Error("MathJax render source hash drift");
  const global = bindGlobal(value.output.code, raw, map, sourceLines, renderPath);
  return { sources, map, raw, global };
}

function validateTrustedMathJaxInspection(root: string, inspection: unknown): MathJaxModuleOrigin {
  const repositoryRoot = fs.realpathSync(root);
  const value = inspectionSchema.parse(inspection);
  const renderId = validateGraph(repositoryRoot, value);
  const renderSource = readSource(repositoryRoot, MATHJAX_SOURCE_PATH);
  const imports = staticImports(renderSource.code);
  if (JSON.stringify(imports) !== JSON.stringify(MATHJAX_STATIC_IMPORTS))
    throw new Error("changed MathJax static import contract");
  const renderModule = value.modules.find((module) => module.id === renderId);
  if (!renderModule || renderModule.code !== renderSource.code)
    throw new Error("MathJax render module source drift");
  const resolvedImports = renderModule.importedIds.map((id) =>
    relativeModuleId(repositoryRoot, id),
  );
  const expectedResolvedImports = MATHJAX_STATIC_IMPORTS.map((name) => `node_modules/${name}`);
  if (JSON.stringify(resolvedImports) !== JSON.stringify(expectedResolvedImports))
    throw new Error("changed MathJax resolved import contract");
  const { sources, global } = validateRawAndMap(repositoryRoot, value, renderId);
  const physicalInputPins = readInputPins(repositoryRoot);
  assertNoModuleTail(value.output.code);
  return {
    acceptedConsumerClosure: false,
    auditOnly: true,
    incompleteConsumer: "MathJax ESM consumer (not a classic program)",
    input: MATHJAX_MODULE_ENTRY,
    moduleCount: value.modules.length,
    staticImportCount: MATHJAX_STATIC_IMPORTS.length,
    sourceMapSourceCount: sources.length,
    externalNodeCount: value.modules.filter((module) => module.isExternal).length,
    dynamicNodeCount: value.modules.filter(
      (module) => module.dynamicallyImportedIds.length > 0 || module.dynamicImporters.length > 0,
    ).length,
    canonicalOutputSha256: createHash("sha256").update(value.output.code).digest("hex"),
    physicalSources: sources,
    physicalInputPins,
    global,
    canonicalInspectionGlobalVerified: true,
    rustCallerVerified: false,
    overrideRouteVerified: false,
  };
}

function assertCurrentPhysicalState(
  root: string,
  trustedInspection: unknown,
  trustedOrigin: MathJaxModuleOrigin,
): void {
  const current = validateTrustedMathJaxInspection(root, trustedInspection);
  if (
    !isDeepStrictEqual(current.physicalSources, trustedOrigin.physicalSources) ||
    !isDeepStrictEqual(current.physicalInputPins, trustedOrigin.physicalInputPins) ||
    !isDeepStrictEqual(current.global, trustedOrigin.global)
  )
    throw new Error("MathJax physical source pins changed after inspection");
}

export async function createMathJaxModuleInspector(
  root: string,
  observeInspection?: (inspection: MathJaxModuleInspectionInput) => void,
): Promise<MathJaxModuleInspector> {
  const repositoryRoot = fs.realpathSync(root);
  const trustedInspection = await new RuntimeRollupBundler(repositoryRoot).inspect(
    MathJaxRuntimeBundleDefinition.create(),
  );
  const trusted = normalizedInspection(trustedInspection);
  const trustedOrigin = freezeOrigin(
    validateTrustedMathJaxInspection(repositoryRoot, trustedInspection),
  );
  if (observeInspection) {
    observeInspection(
      inspectionSchema.parse({
        ...JSON.parse(JSON.stringify(trustedInspection)),
        sourceMapAsset: {
          fileName: trustedInspection.sourceMapAsset.fileName,
          source:
            typeof trustedInspection.sourceMapAsset.source === "string"
              ? trustedInspection.sourceMapAsset.source
              : Buffer.from(trustedInspection.sourceMapAsset.source).toString("utf8"),
        },
      }),
    );
    assertCurrentPhysicalState(repositoryRoot, trustedInspection, trustedOrigin);
  }
  return Object.freeze({
    inspect(candidate: unknown): MathJaxModuleOrigin {
      assertTrustedInspection(trusted, candidate);
      assertCurrentPhysicalState(repositoryRoot, trustedInspection, trustedOrigin);
      return trustedOrigin;
    },
    inspectCanonical(): MathJaxModuleOrigin {
      assertCurrentPhysicalState(repositoryRoot, trustedInspection, trustedOrigin);
      return trustedOrigin;
    },
  });
}

export async function auditMathJaxOwnedSource(
  root: string,
  directory: string,
  runDiagnostics: MathJaxDiagnosticsRunner,
  observeInspection?: (inspection: MathJaxModuleInspectionInput) => void,
): Promise<MathJaxOwnedRawReport> {
  const inspector = await createMathJaxModuleInspector(root, observeInspection);
  const before = inspector.inspectCanonical();
  const source = before.physicalSources.find((entry) => entry.sourcePath === MATHJAX_SOURCE_PATH);
  if (!source) throw new Error("MathJax owned render source is missing from physical inventory");
  const raw = readSource(root, MATHJAX_SOURCE_PATH);
  if (raw.sha256 !== source.sourceSha256 || raw.code.length !== source.rawLength)
    throw new Error("MathJax owned render source pin changed before diagnostics");
  const result = runDiagnostics(MATHJAX_OWNED_DIAGNOSTIC_NAME, raw.code);
  if (result.codeSha256 !== raw.sha256) throw new Error("MathJax diagnostic input hash changed");
  const validatedDiagnostics = inspectUnusedBiomeDiagnostics(result.diagnostics, result.exit);
  const diagnostics = bindMathJaxOwnedDiagnostics(
    root,
    directory,
    MATHJAX_OWNED_DIAGNOSTIC_NAME,
    raw,
    validatedDiagnostics,
  );
  const after = inspector.inspectCanonical();
  const afterSource = after.physicalSources.find(
    (entry) => entry.sourcePath === MATHJAX_SOURCE_PATH,
  );
  if (!afterSource || !isDeepStrictEqual(afterSource, source))
    throw new Error("MathJax owned render source pin changed after diagnostics");
  return ownedRawReport(after, raw, diagnostics, result.exit);
}

export function readMathJaxOwnedAuditEvidence(
  root: string,
  directory: string,
  inspection: unknown,
  result: MathJaxDiagnosticsResult,
  inputPins?: readonly FileRecord[],
): MathJaxOwnedRawReport {
  const origin = validateTrustedMathJaxInspection(root, inspection);
  if (
    inputPins &&
    [...origin.physicalSources, ...origin.physicalInputPins].some(
      (source) =>
        !inputPins.some(
          (pin) =>
            pin.path === path.resolve(root, source.sourcePath) &&
            pin.sha256 === source.sourceSha256,
        ),
    )
  )
    throw new Error("MathJax inspection source/input is absent from acquisition pins");
  const raw = readSource(root, MATHJAX_SOURCE_PATH);
  if (result.codeSha256 !== raw.sha256) throw new Error("MathJax diagnostic input hash changed");
  const diagnostics = bindMathJaxOwnedDiagnostics(
    root,
    directory,
    MATHJAX_OWNED_DIAGNOSTIC_NAME,
    raw,
    inspectUnusedBiomeDiagnostics(result.diagnostics, result.exit),
  );
  return ownedRawReport(origin, raw, diagnostics, result.exit);
}

function ownedRawReport(
  origin: MathJaxModuleOrigin,
  raw: RawUnusedSource,
  diagnostics: readonly MathJaxOwnedRawBinding[],
  diagnosticExit: number,
): MathJaxOwnedRawReport {
  return {
    auditOnly: true,
    acceptedConsumerClosure: false,
    incompleteConsumer: "MathJax ESM consumer (not a classic program)",
    sourcePath: MATHJAX_SOURCE_PATH,
    sourceSha256: raw.sha256,
    rawLength: raw.code.length,
    diagnosticName: MATHJAX_OWNED_DIAGNOSTIC_NAME,
    diagnostics,
    diagnosticExit,
    canonical: {
      moduleCount: origin.moduleCount,
      staticImportCount: origin.staticImportCount,
      sourceMapSourceCount: origin.sourceMapSourceCount,
      physicalSourceCount: origin.physicalSources.length,
      physicalInputPinCount: origin.physicalInputPins.length,
      canonicalOutputSha256: origin.canonicalOutputSha256,
      canonicalInspectionGlobalVerified: true,
    },
  };
}

export async function inspectMathJaxModule(root: string): Promise<MathJaxModuleOrigin> {
  return (await createMathJaxModuleInspector(root)).inspectCanonical();
}
