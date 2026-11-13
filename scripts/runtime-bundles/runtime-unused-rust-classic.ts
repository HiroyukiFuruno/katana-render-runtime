import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { physicalUnusedSource, type RawUnusedSource } from "./runtime-unused-origin";
import { Lines } from "./runtime-unused-source-map";

export const RUST_CLASSIC_INPUT_PATHS = [
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_scripts.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs",
  "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
  "crates/katana-render-runtime/src/v8_script.rs",
  "crates/katana-render-runtime/src/lib.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
  "crates/katana-render-runtime/src/markdown/runtime_resource_loader.rs",
  "crates/katana-render-runtime/src/renderer/backends/mermaid.rs",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/render_instance.rs",
  "crates/katana-render-runtime/src/resource_bytes.rs",
  "Cargo.toml",
  "Cargo.lock",
  "crates/kdr-linter/Cargo.toml",
] as const;

const factPaths = {
  "mermaid-build-forwarding": RUST_CLASSIC_INPUT_PATHS[0],
  "mermaid-classic-script-order": RUST_CLASSIC_INPUT_PATHS[0],
  "mermaid-classic-shared-realm-loop": RUST_CLASSIC_INPUT_PATHS[2],
  "mermaid-classic-source-identities": RUST_CLASSIC_INPUT_PATHS[0],
  "mermaid-render-format": RUST_CLASSIC_INPUT_PATHS[0],
  "mermaid-request-build-render-route": RUST_CLASSIC_INPUT_PATHS[1],
  "v8-compile-order": RUST_CLASSIC_INPUT_PATHS[2],
  "v8-script-origin-helper": RUST_CLASSIC_INPUT_PATHS[3],
  "v8-script-private-module": RUST_CLASSIC_INPUT_PATHS[4],
} as const;

const rangeSchema = z.strictObject({
  lineStart: z.number().int().positive(),
  columnStart: z.number().int().positive(),
  lineEnd: z.number().int().positive(),
  columnEnd: z.number().int().positive(),
});

function scriptEntry(name: string, sourceBinding: string, owned = false) {
  return z.strictObject({
    name: z.literal(name),
    ownership: z.literal(owned ? "owned" : "borrowed"),
    sourceBinding: z.literal(sourceBinding),
    producer: z.literal(owned ? "render_script" : "identity"),
  });
}

const runtimeEntry = scriptEntry("mermaid-runtime.min.js", "MERMAID_RUNTIME");
const vendorEntry = scriptEntry("mermaid.min.js", "bundle");
const pluginEntry = scriptEntry("mermaid-zenuml.min.js", "zenuml_bundle");
const zenumlEntry = scriptEntry("zenuml-runtime.min.js", "ZENUML_RUNTIME");
const renderEntry = scriptEntry("render-mermaid.js", "request_json", true);

function relativeIdentity(binding: string, relativePath: string) {
  return z.strictObject({
    binding: z.literal(binding),
    identity: z.strictObject({
      kind: z.literal("include-relative"),
      path: z.literal(relativePath),
    }),
  });
}

const manifestSchema = z.strictObject({
  normal: z.tuple([runtimeEntry, vendorEntry, pluginEntry, renderEntry]),
  zenuml: z.tuple([runtimeEntry, vendorEntry, pluginEntry, zenumlEntry, renderEntry]),
  sourceIdentities: z.tuple([
    relativeIdentity("MERMAID_RUNTIME", "../diagram_runtime/generated/mermaid-runtime.min.js"),
    relativeIdentity("ZENUML_RUNTIME", "../diagram_runtime/generated/zenuml-runtime.min.js"),
    z.strictObject({
      binding: z.literal("MERMAID_ZENUML_RUNTIME"),
      identity: z.strictObject({
        kind: z.literal("build-output"),
        environment: z.literal("OUT_DIR"),
        suffix: z.literal("/mermaid-zenuml.min.js"),
      }),
    }),
  ]),
  formatProducer: z.strictObject({
    semanticTemplate: z.literal("katanaRunMermaidRuntime({request_json});"),
    entrypoint: z.literal("katanaRunMermaidRuntime"),
    interpolation: z.literal("request_json"),
    interpolationRole: z.literal("JSON data; actual bytes unavailable"),
  }),
  scriptKind: z.literal("individual classic scripts in one V8 realm"),
});

const reportSchema = z.strictObject({
  schema: z.literal("kdr-linter/mermaid-classic-script-facts/v1"),
  status: z.literal("verified"),
  parser: z.strictObject({
    crate: z.literal("syn"),
    resolvedVersion: z.literal("3.0.10"),
    features: z.tuple([z.literal("full"), z.literal("visit")]),
    spanLocations: z.literal(true),
  }),
  facts: z
    .array(z.strictObject({ id: z.string().min(1), path: z.string().min(1), range: rangeSchema }))
    .length(9),
  inputSha256: z.record(z.enum(RUST_CLASSIC_INPUT_PATHS), z.string().regex(/^[a-f0-9]{64}$/u)),
  manifest: manifestSchema,
  sourceProofVerified: z.literal(true),
  requestBytesVerified: z.literal(false),
  materializedScriptsVerified: z.literal(false),
  vendorBytesVerified: z.literal(false),
  pluginBytesVerified: z.literal(false),
  nativeExecutionVerified: z.literal(false),
  performanceVerified: z.literal(false),
  acceptedConsumerClosure: z.literal(false),
});

export type RustClassicScriptFacts = z.infer<typeof reportSchema>;

export function physicalInputs(root: string): ReadonlyMap<string, RawUnusedSource> {
  return new Map(
    RUST_CLASSIC_INPUT_PATHS.map((relative) => {
      const absolute = path.join(root, relative);
      if (!fs.lstatSync(absolute).isFile() || fs.realpathSync(absolute) !== absolute)
        throw new Error(`Rust classic input is not a canonical regular file: ${relative}`);
      return [relative, physicalUnusedSource(fs.readFileSync(absolute))];
    }),
  );
}

function validateRange(fact: RustClassicScriptFacts["facts"][number], source: RawUnusedSource) {
  const lines = new Lines(source.code);
  const start = lines.offset(fact.range.lineStart, fact.range.columnStart - 1);
  const end = lines.offset(fact.range.lineEnd, fact.range.columnEnd - 1);
  if (end <= start || source.code.slice(start, end).trim().length === 0)
    throw new Error(`Rust classic fact range is empty or reversed: ${fact.id}`);
}

export function validateOutput(value: unknown, sources: ReadonlyMap<string, RawUnusedSource>) {
  const report = reportSchema.parse(value);
  const expectedIds = Object.keys(factPaths).sort();
  const actualIds = report.facts.map((fact) => fact.id).sort();
  if (JSON.stringify(expectedIds) !== JSON.stringify(actualIds))
    throw new Error("Rust classic fact ID inventory drift");
  for (const fact of report.facts) {
    const expectedPath = factPaths[fact.id as keyof typeof factPaths];
    const source = sources.get(fact.path);
    if (fact.path !== expectedPath || !source)
      throw new Error(`Rust classic fact path drift: ${fact.id}`);
    validateRange(fact, source);
  }
  for (const relative of RUST_CLASSIC_INPUT_PATHS) {
    if (sources.get(relative)?.sha256 !== report.inputSha256[relative])
      throw new Error(`Rust classic physical input drift: ${relative}`);
  }
  return report;
}

export function inspectRustClassicScriptFacts(sourceRoot: string): RustClassicScriptFacts {
  const repositoryRoot = fs.realpathSync(path.resolve(import.meta.dir, "../.."));
  const actualSourceRoot = fs.realpathSync(sourceRoot);
  const before = physicalInputs(actualSourceRoot);
  const child = Bun.spawnSync(
    ["just", "mermaid-classic-script-facts", repositoryRoot, actualSourceRoot],
    { cwd: repositoryRoot, timeout: 120_000 },
  );
  const stdout = Buffer.from(child.stdout).toString("utf8").trim();
  const stderr = Buffer.from(child.stderr).toString("utf8");
  if (child.exitCode !== 0)
    throw new Error(
      `Rust classic facts command failed (${child.exitCode}): ${stderr}\nRust classic facts stdout: ${stdout}`,
    );
  if (!stdout) throw new Error("Rust classic facts command returned no JSON");
  const value: unknown = JSON.parse(stdout);
  const report = validateOutput(value, before);
  const after = physicalInputs(actualSourceRoot);
  for (const relative of RUST_CLASSIC_INPUT_PATHS) {
    if (after.get(relative)?.sha256 !== before.get(relative)?.sha256)
      throw new Error(`Rust classic input changed during inspection: ${relative}`);
  }
  return Object.freeze(report);
}
