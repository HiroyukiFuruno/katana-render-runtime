import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { parseSync } from "@swc/core";
import { getLineInfo } from "acorn";
import { z } from "zod";
import { inspectUnusedBiomeReport } from "./runtime-unused-biome";
import { type RuntimeUnusedContext, readUnusedPhysicalSource } from "./runtime-unused-contexts";
import {
  type EntrypointOwner,
  entrypointOwnerPaths,
  generatedEntrypointRead,
  generatedEntrypointReadSchema,
  readEntrypointOwner,
} from "./runtime-unused-entrypoint-owner";
import {
  BrowserUnusedOrigin,
  browserAstNodes,
  inspectSequenceVmOwner,
  RuntimeUnusedOrigin,
  type UnusedDiagnostic,
} from "./runtime-unused-origin";
import { Lines } from "./runtime-unused-source-map";

const sha = (value: string) => createHash("sha256").update(value).digest("hex");
const hash = z.string().regex(/^[a-f0-9]{64}$/u);
const rawPoint = z
  .object({
    sourcePath: z.string(),
    sourceSha256: hash,
    occurrence: z.number().int().nonnegative(),
    start: z.number().int().nonnegative(),
    end: z.number().int().positive(),
  })
  .strict();
const point = rawPoint
  .extend({
    kind: z.literal("raw"),
    generatedStart: z.number().int().nonnegative(),
    generatedEnd: z.number().int().positive(),
  })
  .strict();
const readPoint = z.discriminatedUnion("kind", [point, generatedEntrypointReadSchema]);
const realm = z.enum([
  "mermaid",
  "drawio",
  "zenuml",
  "official-mermaid-browser",
  "rust-native-width-test-result",
  "sequence-vm-test",
]);
const contextPin = z
  .object({
    realm,
    stage: z.enum(["assembled", "normalized"]),
    codeSha256: hash,
    originSha256: hash,
  })
  .strict();
export const sharedUnusedSemanticManifestSchema = z
  .object({
    typescriptVersion: z.literal("7.0.2"),
    api: z.literal("typescript/unstable/sync"),
    sources: z.array(z.object({ sourcePath: z.string(), sourceSha256: hash }).strict()).nonempty(),
    contexts: z.array(contextPin).nonempty(),
    candidates: z.array(
      z
        .object({
          diagnosticIndex: z.number().int().nonnegative(),
          symbol: z.string().min(1),
          raw: rawPoint.omit({ occurrence: true }),
          bindings: z
            .array(
              z
                .object({
                  realm,
                  stage: contextPin.shape.stage,
                  declaration: point,
                  reads: z.array(readPoint),
                })
                .strict(),
            )
            .nonempty(),
        })
        .strict(),
    ),
  })
  .strict();
export type SharedUnusedSemanticManifest = z.infer<typeof sharedUnusedSemanticManifestSchema>;
type SemanticGroup = {
  code: string;
  nodes: ReturnType<typeof browserAstNodes>;
  owner: EntrypointOwner | null;
  vmOwner: ReturnType<typeof inspectSequenceVmOwner> | null;
  targets: {
    symbol: string;
    reads: z.infer<typeof readPoint>[];
    declaration: z.infer<typeof point>;
    result: { realm: string; stage: string; readCount: number };
  }[];
};

export function semanticContextPin(
  context: RuntimeUnusedContext,
  stage: "assembled" | "normalized",
) {
  if (!context.stages.includes(stage)) throw new Error("missing semantic context stage");
  return {
    realm: realm.parse(context.name),
    stage,
    codeSha256: sha(
      stage === "assembled" ? context.origin.assembled : context.origin.provenance.code,
    ),
    originSha256: sha(
      context.origin instanceof RuntimeUnusedOrigin
        ? JSON.stringify(context.origin.provenance)
        : context.origin.map,
    ),
  };
}

function diagnosticSpan(
  code: string,
  start: number,
  end: number,
  category: string,
): UnusedDiagnostic {
  const first = getLineInfo(code, start),
    last = getLineInfo(code, end);
  return {
    severity: "warning",
    category,
    message: "semantic origin binding",
    location: {
      path: "@runtime/semantic",
      start: {
        line: first.line,
        column: Array.from(code.slice(start - first.column, start)).length + 1,
      },
      end: { line: last.line, column: Array.from(code.slice(end - last.column, end)).length + 1 },
    },
  };
}

function rawDiagnosticPoint(root: string, diagnostic: UnusedDiagnostic) {
  const supplied = diagnostic.location.path;
  if (path.isAbsolute(supplied) && path.normalize(supplied) !== supplied)
    throw new Error("noncanonical diagnostic path");
  const sourcePath = path.isAbsolute(supplied)
    ? path.relative(root, supplied).split(path.sep).join("/")
    : supplied;
  const source = readUnusedPhysicalSource(root, sourcePath);
  const lines = new Lines(source.code);
  const offset = (position: { line: number; column: number }) => {
    return lines.biome(position.line, position.column);
  };
  return {
    sourcePath,
    sourceSha256: source.sha256,
    start: offset(diagnostic.location.start),
    end: offset(diagnostic.location.end),
  };
}

export function sharedUnusedSemanticPoint(
  context: RuntimeUnusedContext,
  stage: "assembled" | "normalized",
  generatedStart: number,
  generatedEnd: number,
) {
  if (!context.stages.includes(stage)) throw new Error("missing semantic point stage");
  const code = stage === "assembled" ? context.origin.assembled : context.origin.provenance.code;
  const mapped = context.origin.resolve(
    diagnosticSpan(code, generatedStart, generatedEnd, "lint/correctness/noUnusedVariables"),
    stage,
  );
  return { kind: "raw" as const, ...mapped, generatedStart, generatedEnd };
}

function verifyPoint(
  root: string,
  context: RuntimeUnusedContext,
  stage: "assembled" | "normalized",
  binding: z.infer<typeof readPoint>,
  symbol: string,
  category: string,
  nodes: ReturnType<typeof browserAstNodes>,
) {
  const code = stage === "assembled" ? context.origin.assembled : context.origin.provenance.code;
  if (
    binding.generatedStart >= binding.generatedEnd ||
    code.slice(binding.generatedStart, binding.generatedEnd) !== symbol
  )
    throw new Error("changed generated UTF16 span");
  const utf16 = (at: number) =>
    new TextDecoder("utf-8", { fatal: true }).decode(Buffer.from(code).subarray(0, at - 1)).length;
  if (
    !nodes.some(
      ({ node }) =>
        node.type === "Identifier" &&
        node.value === symbol &&
        z.object({ start: z.number(), end: z.number() }).parse(node.span).start > 0 &&
        utf16(z.object({ start: z.number() }).parse(node.span).start) === binding.generatedStart &&
        utf16(z.object({ end: z.number() }).parse(node.span).end) === binding.generatedEnd,
    )
  )
    throw new Error("missing exact SWC identifier");
  if (binding.kind === "generated-entrypoint") {
    if (
      JSON.stringify(
        generatedEntrypointRead(root, context, stage, binding.generatedStart, binding.generatedEnd),
      ) !== JSON.stringify(binding)
    )
      throw new Error("changed generated entrypoint owner/span");
    return;
  }
  const mapped = context.origin.resolve(
    diagnosticSpan(code, binding.generatedStart, binding.generatedEnd, category),
    stage,
  );
  const { kind: _kind, generatedStart: _start, generatedEnd: _end, ...expected } = binding;
  if (JSON.stringify(mapped) !== JSON.stringify(expected))
    throw new Error("changed raw origin map/span");
}

const resultSchema = z
  .object({
    nodeVersion: z.string(),
    typescriptVersion: z.literal("7.0.2"),
    api: z.literal("typescript/unstable/sync"),
    ownerVerified: z.boolean(),
    vmOwnerVerified: z.boolean(),
    rows: z.array(
      z
        .object({
          symbol: z.string(),
          reads: z.array(z.object({ start: z.number().int(), end: z.number().int() }).strict()),
        })
        .strict(),
    ),
  })
  .strict();

function verifySemanticResult(
  row: z.infer<typeof resultSchema>["rows"][number] | undefined,
  symbol: string,
  reads: readonly z.infer<typeof readPoint>[],
) {
  if (
    row?.symbol !== symbol ||
    JSON.stringify(row.reads) !==
      JSON.stringify(reads.map((read) => ({ start: read.generatedStart, end: read.generatedEnd })))
  )
    throw new Error("unregistered/changed semantic read/tool");
}

function admitSemanticDirectory(root: string, directory: string) {
  if (
    fs.realpathSync(root) !== root ||
    !path.isAbsolute(directory) ||
    path.resolve(directory) !== directory ||
    !directory.startsWith(`${root}${path.sep}tmp${path.sep}`)
  )
    throw new Error("semantic adapter directory/root mismatch");
  let existing = directory;
  while (!fs.existsSync(existing)) existing = path.dirname(existing);
  if (fs.realpathSync(existing) !== existing)
    throw new Error("semantic adapter destination symlink");
  fs.mkdirSync(directory, { recursive: true });
  if (fs.realpathSync(directory) !== directory)
    throw new Error("semantic adapter destination changed");
}

function verifyPhysicalContextSources(root: string, contexts: readonly RuntimeUnusedContext[]) {
  for (const context of contexts)
    for (const [name, source] of context.origin.raw) {
      const physical = readUnusedPhysicalSource(root, name);
      if (source.sha256 !== physical.sha256 || source.code !== physical.code)
        throw new Error("changed physical semantic source bytes");
    }
}

function currentSemanticContexts(root: string, contexts: readonly RuntimeUnusedContext[]) {
  verifyPhysicalContextSources(root, contexts);
  // Originは構築時にmapをcacheするため、現在のprovenanceから同epochを再検証する。
  return contexts.map((context): RuntimeUnusedContext => {
    const previous = context.origin;
    if (
      (context.name === "sequence-vm-test") !==
      (previous instanceof BrowserUnusedOrigin && previous.projectionLabel === "sequence-vm-test")
    )
      throw new Error("sequence VM realm requires finite owned projection");
    const raw = new Map(
      [...previous.raw].map(([name]) => [name, readUnusedPhysicalSource(root, name)]),
    );
    const origin =
      previous instanceof RuntimeUnusedOrigin
        ? new RuntimeUnusedOrigin(structuredClone(previous.provenance), raw)
        : new BrowserUnusedOrigin(
            previous.assembled,
            previous.map,
            structuredClone(previous.programs),
            raw,
            previous.projectionLabel,
          );
    return { ...context, origin };
  });
}

function executeSemanticGroup(
  root: string,
  directory: string,
  index: number,
  group: SemanticGroup,
  nodeVersion: string,
) {
  const stem = path.join(directory, `semantic-${index}`),
    file = `${stem}.js`,
    config = `${stem}.json`,
    request = `${stem}-request.json`;
  fs.writeFileSync(file, group.code, { flag: "wx" });
  fs.writeFileSync(
    config,
    JSON.stringify({
      compilerOptions: {
        allowJs: true,
        checkJs: false,
        noLib: true,
        types: [],
        moduleDetection: "legacy",
      },
      files: [file],
    }),
    { flag: "wx" },
  );
  const ownerConfig = `${stem}-owner.json`;
  if (group.owner)
    fs.writeFileSync(
      ownerConfig,
      JSON.stringify({
        compilerOptions: {
          target: "ESNext",
          module: "ESNext",
          moduleResolution: "bundler",
          noLib: true,
          types: [],
        },
        files: entrypointOwnerPaths.slice(0, 4).map((name) => path.join(root, name)),
      }),
      { flag: "wx" },
    );
  const vmConfig = `${stem}-vm-owner.json`;
  if (group.vmOwner)
    fs.writeFileSync(
      vmConfig,
      JSON.stringify({
        compilerOptions: {
          target: "ESNext",
          module: "ESNext",
          moduleResolution: "bundler",
          types: ["node", "bun"],
        },
        files: [path.join(root, group.vmOwner.pins[1]?.sourcePath ?? "")],
      }),
      { flag: "wx" },
    );
  fs.writeFileSync(
    request,
    JSON.stringify({
      cwd: root,
      config,
      file,
      codeSha256: sha(group.code),
      ...(group.owner
        ? {
            owner: {
              config: ownerConfig,
              pins: group.owner.pins,
              argumentStart: group.owner.argumentSpan.start,
              constructorStart: group.owner.constructorSpan.start,
              exportStart: group.owner.exportSpan.start,
              registryStart: group.owner.registrySpan.start,
              literalSpan: group.owner.literalSpan,
            },
          }
        : {}),
      ...(group.vmOwner ? { vmOwner: { config: vmConfig, ...group.vmOwner } } : {}),
      declarations: group.targets.map((target) => ({
        symbol: target.symbol,
        start: target.declaration.generatedStart,
        end: target.declaration.generatedEnd,
      })),
    }),
    { flag: "wx" },
  );
  const output = execFileSync(
    "node",
    [
      "--experimental-strip-types",
      path.join(import.meta.dir, "runtime-unused-semantic-node.ts"),
      request,
    ],
    { encoding: "utf8", timeout: 30000 },
  );
  const actual = resultSchema.parse(JSON.parse(output));
  if (!fs.readFileSync(file).equals(Buffer.from(group.code)))
    throw new Error("semantic projection bytes changed");
  if (
    actual.nodeVersion !== nodeVersion ||
    actual.rows.length !== group.targets.length ||
    actual.ownerVerified !== Boolean(group.owner) ||
    actual.vmOwnerVerified !== Boolean(group.vmOwner)
  )
    throw new Error("changed semantic group/tool");
  group.targets.forEach((target, rowIndex) => {
    const row = actual.rows[rowIndex];
    verifySemanticResult(row, target.symbol, target.reads);
    if (!row) throw new Error("missing semantic row");
    target.result.readCount = row.reads.length;
  });
  fs.writeFileSync(`${stem}-result.json`, output, { flag: "wx" });
}

function registerGroupTarget(
  root: string,
  context: RuntimeUnusedContext,
  binding: SharedUnusedSemanticManifest["candidates"][number]["bindings"][number],
  symbol: string,
  category: string,
  group: SemanticGroup,
) {
  for (const selected of [binding.declaration, ...binding.reads])
    verifyPoint(root, context, binding.stage, selected, symbol, category, group.nodes);
  for (const selected of binding.reads) {
    if (selected.kind !== "generated-entrypoint") continue;
    if (group.owner && JSON.stringify(group.owner) !== JSON.stringify(selected.owner))
      throw new Error("mixed generated owner epochs");
    group.owner = selected.owner;
  }
  const result = { realm: binding.realm, stage: binding.stage, readCount: 0 };
  group.targets.push({ symbol, reads: binding.reads, declaration: binding.declaration, result });
  return result;
}

function verifyGroupOwnerSources(root: string, groups: Iterable<SemanticGroup>) {
  for (const group of groups)
    if (group.owner && JSON.stringify(readEntrypointOwner(root)) !== JSON.stringify(group.owner))
      throw new Error("owned entrypoint source changed during inspection");
}

export function inspectSharedUnusedSemantics(
  root: string,
  directory: string,
  inputContexts: readonly RuntimeUnusedContext[],
  report: unknown,
  exitCode: number,
  value: unknown,
) {
  const manifest = sharedUnusedSemanticManifestSchema.parse(value);
  const diagnostics = inspectUnusedBiomeReport(report, exitCode);
  admitSemanticDirectory(root, directory);
  const contexts = currentSemanticContexts(root, inputContexts);
  const inventory = new Map(
    contexts.flatMap((context) =>
      [...context.origin.raw].map(([sourcePath, source]) => [sourcePath, source.sha256] as const),
    ),
  );
  if (
    manifest.sources.length !== inventory.size ||
    new Set(manifest.sources.map((source) => source.sourcePath)).size !== inventory.size ||
    manifest.sources.some(
      (source) =>
        inventory.get(source.sourcePath) !== source.sourceSha256 ||
        readUnusedPhysicalSource(root, source.sourcePath).sha256 !== source.sourceSha256,
    )
  )
    throw new Error("changed semantic source inventory/pin");
  const pins = contexts.flatMap((context) =>
    context.stages.map((stage) => semanticContextPin(context, stage)),
  );
  if (JSON.stringify(pins) !== JSON.stringify(manifest.contexts))
    throw new Error("changed semantic realm/map inventory");
  if (
    manifest.candidates.length !== diagnostics.length ||
    new Set(manifest.candidates.map((candidate) => candidate.diagnosticIndex)).size !==
      diagnostics.length
  )
    throw new Error("unregistered raw diagnostic");
  const nodeVersion = execFileSync("node", ["--version"], {
    encoding: "utf8",
    timeout: 30000,
  }).trim();
  const groups = new Map<string, SemanticGroup>();
  const dispositions = manifest.candidates.map((candidate) => {
    const diagnostic = diagnostics[candidate.diagnosticIndex];
    if (
      diagnostic?.category !== "lint/correctness/noUnusedVariables" ||
      JSON.stringify(rawDiagnosticPoint(root, diagnostic)) !== JSON.stringify(candidate.raw)
    )
      throw new Error("changed raw diagnostic category/path/SHA/span");
    const bindingKeys = candidate.bindings.map((binding) => `${binding.realm}/${binding.stage}`);
    if (new Set(bindingKeys).size !== bindingKeys.length)
      throw new Error("duplicate semantic realm binding");
    const bound = candidate.bindings.map((binding) => {
      const context = contexts.find(
        (item) => item.name === binding.realm && item.stages.includes(binding.stage),
      );
      if (!context) throw new Error("missing/different semantic realm");
      const {
        kind: _kind,
        occurrence: _rawOccurrence,
        generatedStart: _start,
        generatedEnd: _end,
        ...raw
      } = binding.declaration;
      if (JSON.stringify(raw) !== JSON.stringify(candidate.raw))
        throw new Error("declaration/diagnostic mismatch");
      const code =
        binding.stage === "assembled" ? context.origin.assembled : context.origin.provenance.code;
      const key = `${binding.realm}/${binding.stage}`;
      const group = groups.get(key) ?? {
        code,
        nodes: browserAstNodes(parseSync(code, { syntax: "ecmascript" })),
        owner: null,
        vmOwner:
          context.origin instanceof BrowserUnusedOrigin &&
          context.origin.projectionLabel === "sequence-vm-test"
            ? inspectSequenceVmOwner(context.origin.raw)
            : null,
        targets: [],
      };
      groups.set(key, group);
      return registerGroupTarget(
        root,
        context,
        binding,
        candidate.symbol,
        diagnostic.category,
        group,
      );
    });
    return {
      diagnosticIndex: candidate.diagnosticIndex,
      symbol: candidate.symbol,
      provenLive: false,
      deadProved: false as const,
      bindings: bound,
    };
  });
  for (const [index, group] of [...groups.values()].entries())
    executeSemanticGroup(root, directory, index, group, nodeVersion);
  for (const disposition of dispositions)
    disposition.provenLive = disposition.bindings.some((binding) => binding.readCount > 0);
  for (const source of manifest.sources)
    if (readUnusedPhysicalSource(root, source.sourcePath).sha256 !== source.sourceSha256)
      throw new Error("semantic source changed during inspection");
  verifyGroupOwnerSources(root, groups.values());
  return {
    dispositions,
    semanticProcessCount: groups.size,
    scopeComplete: false as const,
    accepted: false as const,
    actualNative: false as const,
    actualBrowser: false as const,
    nodeVersion,
    typescriptVersion: manifest.typescriptVersion,
  };
}
