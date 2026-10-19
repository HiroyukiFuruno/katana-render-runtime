import { createHash } from "node:crypto";
import { parseSync, transformSync } from "@swc/core";
import MagicString from "magic-string";
import { z } from "zod";
import type {
  RuntimeDefinitionProvenance,
  RuntimeFragmentInput,
} from "./runtime-source-provenance";
import { Lines, type Link, mapIndex, sourceMapSchema } from "./runtime-unused-source-map";

export const unusedDiagnosticSchema = z.object({
  severity: z.enum(["warning", "error", "information"]),
  category: z.string(),
  message: z.string(),
  location: z.object({
    path: z.string(),
    start: z.object({ line: z.number().int().positive(), column: z.number().int().positive() }),
    end: z.object({ line: z.number().int().positive(), column: z.number().int().positive() }),
  }),
});
export type UnusedDiagnostic = z.infer<typeof unusedDiagnosticSchema>;
export type RawUnusedSource = Readonly<{ code: string; sha256: string }>;
export type UnusedStage = "normalized" | "assembled";
export interface UnusedRawOrigin {
  sourcePath: string;
  sourceSha256: string;
  occurrence: number;
  start: number;
  end: number;
}
interface NormalizedFrame {
  start: number;
  end: number;
  assembledStart: number;
}
interface Frame {
  input: RuntimeFragmentInput;
  start: number;
  end: number;
}

function transformedSource(input: RuntimeFragmentInput, raw: string): string {
  let end = 0;
  const parts: string[] = [];
  for (const removed of input.transformRemovals) {
    if (removed.start < end || removed.end < removed.start || removed.end > raw.length)
      throw new Error("invalid transform removal");
    parts.push(raw.slice(end, removed.start));
    end = removed.end;
  }
  parts.push(raw.slice(end));
  return parts.join("");
}

export class RuntimeUnusedOrigin {
  readonly assembled: string;
  private readonly assembly: Map<number, Link | null>;
  private readonly normalization: Map<number, Link | null>;
  private readonly frames: Frame[];
  private readonly normalizedFrames: NormalizedFrame[];
  private readonly assemblyOrigins: Map<number, Link & { occurrence: number }>;
  private readonly lines: Record<UnusedStage, Lines>;
  constructor(
    readonly provenance: RuntimeDefinitionProvenance,
    readonly raw: ReadonlyMap<string, RawUnusedSource>,
  ) {
    const normalization = sourceMapSchema.parse(JSON.parse(provenance.normalizationMap));
    if (
      normalization.sources.join("\0") !== "@runtime/assembled" ||
      normalization.sourcesContent.length !== 1
    )
      throw new Error("unknown normalization input");
    const assembled = normalization.sourcesContent[0];
    if (assembled === undefined) throw new Error("missing assembled input");
    this.assembled = assembled;
    this.lines = { normalized: new Lines(provenance.code), assembled: new Lines(assembled) };
    const sources = new Map([...raw].map(([name, source]) => [name, new Lines(source.code)]));
    sources.set(
      provenance.entrypoint.source,
      new Lines(
        assembled.slice(
          provenance.entrypoint.assembledRange.start,
          provenance.entrypoint.assembledRange.end,
        ),
      ),
    );
    this.assembly = mapIndex(
      sourceMapSchema.parse(JSON.parse(provenance.assemblyMap)),
      this.lines.assembled,
      sources,
    );
    this.normalization = mapIndex(
      normalization,
      this.lines.normalized,
      new Map([["@runtime/assembled", this.lines.assembled]]),
    );
    this.frames = this.inputFrames();
    this.normalizedFrames = this.normalizedInputFrames();
    this.assemblyOrigins = this.indexAssemblyOrigins();
  }
  private inputFrames(): Frame[] {
    const labels = this.provenance.generatedSpans.filter((span) => span.kind === "label");
    if (labels.length !== this.provenance.rawInputs.length) throw new Error("missing input frame");
    let previousEnd = 0;
    return this.provenance.rawInputs.map((input, index) => {
      const label = labels[index];
      const raw = this.raw.get(input.sourcePath);
      if (
        !label ||
        !raw ||
        input.occurrence !== index ||
        raw.sha256 !== input.sourceSha256 ||
        raw.code.length !== input.rawLength
      )
        throw new Error("missing/changed physical source");
      const prefix = `/* ${input.label} */\n`;
      if (this.assembled.slice(label.start, label.end) !== prefix)
        throw new Error("changed input label");
      const content = transformedSource(input, raw.code);
      const end = label.end + content.length;
      if (label.start < previousEnd || label.end < label.start)
        throw new Error("ambiguous input frames");
      previousEnd = end;
      if (this.assembled.slice(label.end, end) !== content)
        throw new Error("changed assembled input");
      return { input, start: label.end, end };
    });
  }
  private normalizedInputFrames(): NormalizedFrame[] {
    const frames: NormalizedFrame[] = [];
    const parts: string[] = [];
    let assembled = 0;
    let normalized = 0;
    for (const removed of [...this.provenance.duplicateRemovals].sort(
      (a, b) => a.start - b.start,
    )) {
      if (
        !Number.isInteger(removed.start) ||
        !Number.isInteger(removed.end) ||
        removed.start < assembled ||
        removed.end < removed.start ||
        removed.end > this.assembled.length
      )
        throw new Error("invalid normalization removal");
      const retained = this.assembled.slice(assembled, removed.start);
      frames.push({
        start: normalized,
        end: normalized + retained.length,
        assembledStart: assembled,
      });
      parts.push(retained, "\n");
      normalized += retained.length + 1;
      assembled = removed.end;
    }
    const retained = this.assembled.slice(assembled);
    frames.push({
      start: normalized,
      end: normalized + retained.length,
      assembledStart: assembled,
    });
    parts.push(retained);
    if (parts.join("") !== this.provenance.code)
      throw new Error("changed normalization code/ranges");
    return frames;
  }
  private indexAssemblyOrigins(): Map<number, Link & { occurrence: number }> {
    const result = new Map<number, Link & { occurrence: number }>();
    let frameIndex = 0;
    let generatedIndex = 0;
    const generated = [...this.provenance.generatedSpans].sort((a, b) => a.start - b.start);
    for (const [at, link] of this.assembly) {
      while (generated[generatedIndex] && at >= (generated[generatedIndex]?.end ?? 0))
        generatedIndex++;
      const span = generated[generatedIndex];
      if (span && at >= span.start && at < span.end) continue;
      while (this.frames[frameIndex] && at >= (this.frames[frameIndex]?.end ?? 0)) frameIndex++;
      const frame = this.frames[frameIndex];
      const point = this.retainedAssemblyPoint(at, link, frame);
      if (point) result.set(at, point);
    }
    return result;
  }
  private retainedAssemblyPoint(
    at: number,
    link: Link | null,
    frame: Frame | undefined,
  ): (Link & { occurrence: number }) | null {
    if (!frame || !link || at < frame.start || frame.input.sourcePath !== link.source) return null;
    if (
      frame.input.transformRemovals.some(
        (range) => link.offset >= range.start && link.offset < range.end,
      )
    )
      return null;
    let expectedRaw = at - frame.start;
    for (const removed of frame.input.transformRemovals)
      if (expectedRaw >= removed.start) expectedRaw += removed.end - removed.start;
    if (link.offset !== expectedRaw) return null;
    if (this.raw.get(link.source)?.code[link.offset] !== this.assembled[at]) return null;
    return { ...link, occurrence: frame.input.occurrence };
  }
  private point(offset: number, stage: UnusedStage): Link & { occurrence: number } {
    let assembled = offset;
    if (stage === "normalized") {
      const link = this.normalization.get(offset);
      if (link?.source !== "@runtime/assembled") throw new Error("unmapped normalization position");
      const frame = this.normalizedFrames.find((part) => offset >= part.start && offset < part.end);
      if (!frame || link.offset !== frame.assembledStart + offset - frame.start)
        throw new Error("changed/virtual normalization mapping");
      assembled = link.offset;
      if (
        this.provenance.duplicateRemovals.some(
          (range) => assembled >= range.start && assembled < range.end,
        )
      )
        throw new Error("normalizer removed/virtual replacement");
    }
    const point = this.assemblyOrigins.get(assembled);
    if (!point) throw new Error("generated/virtual/transformed-out/unmapped position");
    if (this.raw.get(point.source)?.code[point.offset] !== this.lines[stage].code[offset])
      throw new Error("map/raw character mismatch");
    return point;
  }
  resolve(diagnostic: UnusedDiagnostic, stage: UnusedStage): UnusedRawOrigin {
    return resolveUnusedSpan(
      this.lines[stage],
      diagnostic,
      (offset) => this.point(offset, stage),
      this.raw,
    );
  }

  coverage(stage: UnusedStage) {
    const flags = this.frames.map((frame) => new Uint8Array(frame.input.rawLength));
    const table = stage === "normalized" ? this.normalization : this.assembly;
    for (const offset of table.keys()) {
      try {
        const point = this.point(offset, stage);
        const selected = flags[point.occurrence];
        if (selected) selected[point.offset] = 1;
      } catch {
        /* 未帰属点はcoveredへ昇格せず残す。 */
      }
    }
    return this.frames.map((frame, index) => {
      const units = flags[index];
      if (!units) throw new Error("missing coverage frame");
      let mapped = 0;
      for (const unit of units) mapped += unit;
      return {
        occurrence: frame.input.occurrence,
        sourcePath: frame.input.sourcePath,
        sourceSha256: frame.input.sourceSha256,
        rawUtf16: units.length,
        mappedUtf16: mapped,
        uncoveredUtf16: units.length - mapped,
        transformOut: frame.input.transformRemovals,
      };
    });
  }
}

export function physicalUnusedSource(bytes: Uint8Array): RawUnusedSource {
  return {
    code: new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes),
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

function resolveUnusedSpan(
  lines: Lines,
  diagnostic: UnusedDiagnostic,
  point: (offset: number) => Link & { occurrence: number },
  raw: ReadonlyMap<string, RawUnusedSource>,
): UnusedRawOrigin {
  const start = lines.biome(diagnostic.location.start.line, diagnostic.location.start.column);
  const end = lines.biome(diagnostic.location.end.line, diagnostic.location.end.column);
  if (end <= start) throw new Error("invalid diagnostic span");
  const first = point(start);
  for (let offset = start; offset < end; offset++) {
    const current = point(offset);
    if (
      current.source !== first.source ||
      current.occurrence !== first.occurrence ||
      current.offset !== first.offset + offset - start
    )
      throw new Error("discontinuous/ambiguous diagnostic span");
  }
  const source = raw.get(first.source);
  if (!source) throw new Error("missing raw source");
  return {
    sourcePath: first.source,
    sourceSha256: source.sha256,
    occurrence: first.occurrence,
    start: first.offset,
    end: first.offset + end - start,
  };
}

export interface BrowserUnusedProgram {
  readonly sourcePath: string;
  readonly sourceSha256: string;
  readonly role:
    | "provider"
    | "callback"
    | "native-test-trace"
    | "rust-result-literal"
    | "node-function-parameters"
    | "node-function-body"
    | "node-function-return"
    | "vm-source"
    | "vm-result-literal";
  readonly mode:
    | "owned-reference"
    | "both"
    | "strictVendorOriginal"
    | "rust-native-test"
    | "node-function-test"
    | "shared-svg-capture"
    | "sequence-vm-test";
  readonly start: number;
  readonly end: number;
  readonly removals: readonly { start: number; end: number }[];
  readonly data: string | null;
  readonly runtimeAstSha256?: string;
  readonly callAstSha256?: string;
  readonly consumerRoute?: readonly { sourcePath: string; sourceSha256: string }[];
  readonly captureCallers?: readonly {
    sourcePath: string;
    sourceSha256: string;
    start: number;
    end: number;
    callAstSha256: string;
    mode: "officialFallback" | "strictViewBox";
    strictSource: string;
  }[];
  readonly callerRange?: { sourcePath: string; sourceSha256: string; start: number; end: number };
}

export class BrowserUnusedOrigin {
  readonly provenance: { code: string };
  private readonly lines: Lines;
  private readonly points = new Map<number, Link & { occurrence: number }>();
  constructor(
    readonly assembled: string,
    readonly map: string,
    readonly programs: readonly BrowserUnusedProgram[],
    readonly raw: ReadonlyMap<string, RawUnusedSource>,
    readonly projectionLabel:
      | "browser"
      | "rust-native-width"
      | "node-function"
      | "shared-svg-capture"
      | "sequence-vm-test" = "browser",
  ) {
    if (programs.some((program) => !validProjectionProgram(program, projectionLabel)))
      throw new Error("wrong projection program realm");
    if (
      projectionLabel === "node-function" &&
      programs.map((program) => program.role).join("/") !==
        "node-function-parameters/node-function-body/node-function-body/node-function-return"
    )
      throw new Error("unknown node Function program order");
    if (projectionLabel === "node-function") verifyNodeFunctionProvenance(programs, raw, assembled);
    if (projectionLabel === "shared-svg-capture") verifySharedSvgCaptureProvenance(programs, raw);
    verifySequenceVmProjection(projectionLabel, programs, raw);
    this.provenance = { code: assembled };
    this.lines = new Lines(assembled);
    const sources = new Map([...raw].map(([name, source]) => [name, new Lines(source.code)]));
    const table = mapIndex(sourceMapSchema.parse(JSON.parse(map)), this.lines, sources);
    let generated = 0;
    for (const [occurrence, program] of programs.entries()) {
      generated = this.indexProgram(program, occurrence, generated, table);
      if (occurrence < programs.length - 1) {
        const separator = projectionLabel === "node-function" ? "\n" : "\n;\n";
        if (assembled.slice(generated, generated + separator.length) !== separator)
          throw new Error("changed browser program separator");
        generated += separator.length;
      }
    }
    if (generated !== assembled.length) throw new Error("unknown browser projection tail");
  }
  private indexProgram(
    program: BrowserUnusedProgram,
    occurrence: number,
    generated: number,
    table: ReadonlyMap<number, Link | null>,
  ): number {
    const source = this.raw.get(program.sourcePath);
    if (
      !source ||
      source.sha256 !== program.sourceSha256 ||
      program.start < 0 ||
      program.end > source.code.length ||
      program.start >= program.end
    )
      throw new Error("missing/changed browser program");
    const { body, positions } = browserRetainedProgram(program, source.code);
    const { prefix, suffix } = unusedProgramAffixes(program, this.projectionLabel, occurrence);
    if (
      this.assembled.slice(generated, generated + prefix.length + body.length + suffix.length) !==
      prefix + body + suffix
    )
      throw new Error("changed browser projection/program order");
    for (const [index, offset] of positions.entries()) {
      const at = generated + prefix.length + index;
      const link = table.get(at);
      if (
        link?.source === program.sourcePath &&
        link.offset === offset &&
        source.code[offset] === this.assembled[at]
      )
        this.points.set(at, { ...link, occurrence });
    }
    return generated + prefix.length + body.length + suffix.length;
  }
  private point(offset: number) {
    const point = this.points.get(offset);
    if (!point) throw new Error("generated/virtual/type-erased/unmapped browser position");
    return point;
  }
  resolve(diagnostic: UnusedDiagnostic, stage: UnusedStage): UnusedRawOrigin {
    if (stage !== "assembled") throw new Error("browser programs are not normalized");
    return resolveUnusedSpan(this.lines, diagnostic, (offset) => this.point(offset), this.raw);
  }
  coverage(stage: UnusedStage) {
    if (stage !== "assembled") throw new Error("browser programs are not normalized");
    const mapped = this.programs.map(() => 0);
    for (const point of this.points.values())
      mapped[point.occurrence] = (mapped[point.occurrence] ?? 0) + 1;
    return this.programs.map((program, occurrence) => ({
      occurrence,
      sourcePath: program.sourcePath,
      sourceSha256: program.sourceSha256,
      rawUtf16: program.end - program.start,
      mappedUtf16: mapped[occurrence] ?? 0,
      uncoveredUtf16: program.end - program.start - (mapped[occurrence] ?? 0),
      transformOut: program.removals,
      programRole: program.role,
      mode: program.mode,
      rawRange: { start: program.start, end: program.end },
      ...(["node-function", "shared-svg-capture", "sequence-vm-test"].includes(this.projectionLabel)
        ? {
            fullSourceUtf16: this.raw.get(program.sourcePath)?.code.length,
            sourceClosureComplete: false,
          }
        : {}),
    }));
  }
}

function browserRetainedProgram(program: BrowserUnusedProgram, code: string) {
  let rawAt = program.start;
  const retained: string[] = [];
  const positions: number[] = [];
  for (const removed of [...program.removals, { start: program.end, end: program.end }]) {
    if (removed.start < rawAt || removed.end < removed.start || removed.end > program.end)
      throw new Error("unknown browser type erasure");
    retained.push(code.slice(rawAt, removed.start));
    for (let offset = rawAt; offset < removed.start; offset++) positions.push(offset);
    rawAt = removed.end;
  }
  return { body: retained.join(""), positions };
}

export function unusedProgramAffixes(
  program: BrowserUnusedProgram,
  label: BrowserUnusedOrigin["projectionLabel"],
  occurrence: number,
) {
  const comment = `/* ${label} ${program.role}: ${program.sourcePath} */\n`;
  // VMの文字列評価は値を返す。literal本体だけをrawへ写し、返値wrapperは仮想に保つ。
  if (label === "sequence-vm-test" && program.role === "vm-result-literal")
    return { prefix: `${comment}(function(){return `, suffix: ";})();" };
  if (label !== "node-function")
    return {
      prefix: comment + (program.role === "callback" ? "(" : ""),
      suffix: program.role === "callback" ? ");" : "",
    };
  return {
    prefix: (occurrence === 0 ? "(function(" : occurrence === 1 ? "){\n" : "") + comment,
    suffix: occurrence === 3 ? "\n});" : "",
  };
}

export const astObject = z.record(z.string(), z.unknown());
export const astSpan = z.object({
  start: z.number().int().positive(),
  end: z.number().int().positive(),
});
export function browserAstNodes(
  value: unknown,
  method = "",
  owner: Record<string, unknown> | null = null,
): { node: Record<string, unknown>; method: string; owner: Record<string, unknown> | null }[] {
  if (Array.isArray(value)) return value.flatMap((child) => browserAstNodes(child, method, owner));
  const parsed = astObject.safeParse(value);
  if (!parsed.success) return [];
  const node = parsed.data;
  if (node.type === "ClassMethod" || node.type === "MethodProperty") {
    owner = node;
    const key = astObject.parse(node.key);
    method = key.type === "Identifier" && typeof key.value === "string" ? key.value : "";
  }
  return [
    { node, method, owner },
    ...Object.entries(node)
      .filter(([key]) => key !== "span")
      .flatMap(([, child]) => browserAstNodes(child, method, owner)),
  ];
}
export function compilerExpression(code: string): string {
  const compiled = transformSync(code, {
    jsc: { parser: { syntax: "typescript" }, target: "es2022" },
    module: { type: "es6" },
  }).code;
  return JSON.stringify(parseSync(compiled, { syntax: "ecmascript" }), (key, value: unknown) =>
    ["span", "ctxt", "raw"].includes(key) ? undefined : value,
  );
}

export const pageIdentifier = z.object({
  type: z.literal("Identifier"),
  value: z.string(),
  ctxt: z.number().int().nonnegative(),
  span: astSpan,
});
const nodeFunctionBase = "crates/katana-render-runtime/src/renderer/backends/html_runtime/";
export const nodeFunctionRoute = [
  {
    sourcePath: `${nodeFunctionBase}dom_bootstrap_test.js`,
  },
  {
    sourcePath: `${nodeFunctionBase}dom_bootstrap.js`,
  },
] as const;
const nodeFunctionSelectedSpanSha256 = [
  "0ccea4f540973429a27ef926369005d82b90c288e3d920fb4d0e899e61416ee4",
  "e2d2c18a1a0a1197aa55fab3856b59ed1dfe0b5115efdcaa4c546bab8ec0dd33",
  "79067331066423188375ae973f6367a855a0e9d041a6d975f5c0f287b53833f8",
] as const;
const nodeFunctionParameters = [
  "globalThis",
  "window",
  "document",
  "__krrLifecyclePropertyOverrides",
  "__krrStoreEventHandler",
];
const nodeFunctionReturn = "return globalThis.__krrInstallStaticBodyLoadHandler;";
const nodeFunctionRegex = [
  /^const __krrInstallBodyLoadHandler = \(body, source\) => \{[\s\S]*?^\};/m,
  /^globalThis\.__krrInstallStaticBodyLoadHandler = \(parserSource, laterToken = false\) => \{[\s\S]*?^\};/m,
];

function nodeFunctionSpan(code: string, start: number, span: unknown) {
  const range = astSpan.parse(span);
  const bytes = Buffer.from(code);
  const offset = (at: number) =>
    new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, at - start)).length;
  return { start: offset(range.start), end: offset(range.end) };
}

function nodeFunctionDeclarations(code: string) {
  const module = parseSync(code, { syntax: "ecmascript" });
  const declarations = module.body.flatMap((statement) =>
    statement.type === "VariableDeclaration" && statement.kind === "const"
      ? statement.declarations
      : [],
  );
  const find = (name: string) => {
    const found = declarations.filter(
      (declaration) => declaration.id.type === "Identifier" && declaration.id.value === name,
    );
    if (found.length !== 1 || !found[0]?.init)
      throw new Error("missing/ambiguous node Function binding");
    return found[0].init;
  };
  return { module, find };
}

function nodeFunctionStringParameters(
  arguments_: readonly { expression: Record<string, unknown> }[],
  code: string,
  moduleStart: number,
) {
  const strings = arguments_
    .slice(0, -1)
    .map(({ expression }) =>
      z
        .object({ type: z.literal("StringLiteral"), value: z.string(), span: astSpan })
        .parse(expression),
    );
  if (
    strings.some((argument) => {
      const range = nodeFunctionSpan(code, moduleStart, argument.span);
      return code.slice(range.start, range.end) !== JSON.stringify(argument.value);
    }) ||
    JSON.stringify(strings.map((argument) => argument.value)) !==
      JSON.stringify(nodeFunctionParameters)
  )
    throw new Error("changed node Function parameters");
  return strings;
}

function nodeFunctionBodyTemplate(arguments_: readonly { expression: Record<string, unknown> }[]) {
  const template = z
    .object({
      type: z.literal("TemplateLiteral"),
      expressions: z.array(pageIdentifier),
      quasis: z.array(z.object({ cooked: z.string(), raw: z.string(), span: astSpan })),
    })
    .parse(arguments_.at(-1)?.expression);
  if (
    template.expressions.map((expression) => expression.value).join("/") !==
      "bodyLoadHandler/staticBodyLoadHandler" ||
    template.quasis.map((quasi) => quasi.cooked).join("|") !== `|\n|\n${nodeFunctionReturn}`
  )
    throw new Error("changed node Function body/return");
  return template;
}

function nodeFunctionConstructor(code: string) {
  const { module, find } = nodeFunctionDeclarations(code);
  const caller = find("installStaticLoadHandler");
  if (
    caller.type !== "ArrowFunctionExpression" ||
    astObject.parse(caller.body).type !== "FunctionBody"
  )
    throw new Error("unknown node Function caller");
  const constructors = browserAstNodes(caller).filter(
    ({ node }) =>
      node.type === "NewExpression" && astObject.parse(node.callee).value === "Function",
  );
  if (constructors.length !== 1) throw new Error("missing/ambiguous node Function constructor");
  const construction = constructors[0]?.node;
  if (!construction) throw new Error("missing node Function constructor");
  const arguments_ = z
    .array(z.object({ spread: z.null(), expression: astObject }))
    .parse(construction.arguments);
  const strings = nodeFunctionStringParameters(arguments_, code, module.span.start);
  const template = nodeFunctionBodyTemplate(arguments_);
  const span = (value: unknown) => nodeFunctionSpan(code, module.span.start, value);
  return { find, caller, strings, template, span };
}

function nodeFunctionBodyPrograms(
  resource: RawUnusedSource,
  inspected: ReturnType<typeof nodeFunctionConstructor>,
  consumerRoute: readonly { sourcePath: string; sourceSha256: string }[],
) {
  return nodeFunctionRegex.map((regex, index): BrowserUnusedProgram => {
    const selection = inspected.find(index === 0 ? "bodyLoadHandler" : "staticBodyLoadHandler");
    const literals = browserAstNodes(selection).filter(({ node }) => node.type === "RegExpLiteral");
    if (
      literals.length !== 1 ||
      literals[0]?.node.pattern !== regex.source ||
      literals[0]?.node.flags !== regex.flags
    )
      throw new Error("changed node Function regex selector");
    const match = regex.exec(resource.code);
    if (!match || resource.code.slice(match.index + match[0].length).match(regex))
      throw new Error("missing/ambiguous node Function body resource");
    if (
      createHash("sha256").update(match[0]).digest("hex") !==
      nodeFunctionSelectedSpanSha256[index + 1]
    )
      throw new Error("changed node Function selected resource body");
    parseSync(match[0], { syntax: "ecmascript" });
    return {
      sourcePath: nodeFunctionRoute[1].sourcePath,
      sourceSha256: resource.sha256,
      mode: "node-function-test",
      role: "node-function-body",
      start: match.index,
      end: match.index + match[0].length,
      removals: [],
      data: null,
      consumerRoute,
    };
  });
}

function verifyNodeFunctionBinding(producer: RawUnusedSource, resource: RawUnusedSource) {
  if (
    physicalUnusedSource(Buffer.from(producer.code)).sha256 !== producer.sha256 ||
    physicalUnusedSource(Buffer.from(resource.code)).sha256 !== resource.sha256
  )
    throw new Error("changed node Function consumer/resource");
}

function nodeFunctionLiveRoute(producer: RawUnusedSource, resource: RawUnusedSource) {
  return [
    { sourcePath: nodeFunctionRoute[0].sourcePath, sourceSha256: producer.sha256 },
    { sourcePath: nodeFunctionRoute[1].sourcePath, sourceSha256: resource.sha256 },
  ];
}

export function inspectNodeFunctionPrograms(
  producer: RawUnusedSource,
  resource: RawUnusedSource,
): BrowserUnusedProgram[] {
  verifyNodeFunctionBinding(producer, resource);
  const inspected = nodeFunctionConstructor(producer.code);
  const consumerRoute = nodeFunctionLiveRoute(producer, resource);
  const callerRange = inspected.span(inspected.caller.span);
  if (
    createHash("sha256")
      .update(producer.code.slice(callerRange.start, callerRange.end))
      .digest("hex") !== nodeFunctionSelectedSpanSha256[0]
  )
    throw new Error("changed node Function selected caller");
  const ranges = inspected.strings.map((argument) => inspected.span(argument.span));
  const first = ranges[0],
    last = ranges.at(-1),
    tail = inspected.template.quasis.at(-1);
  if (!first || !last || !tail || tail.raw !== `\\n${nodeFunctionReturn}`)
    throw new Error("unknown node Function source ranges");
  const callerSourceRange = {
    sourcePath: nodeFunctionRoute[0].sourcePath,
    sourceSha256: producer.sha256,
    ...callerRange,
  };
  const callAstSha256 = createHash("sha256")
    .update(compilerExpression(producer.code.slice(callerRange.start, callerRange.end)))
    .digest("hex");
  const common = {
    callerRange: callerSourceRange,
    callAstSha256,
    sourcePath: nodeFunctionRoute[0].sourcePath,
    sourceSha256: producer.sha256,
    mode: "node-function-test" as const,
    data: null,
    consumerRoute,
  };
  return nodeFunctionSelectedPrograms(resource, inspected, ranges, first, last, tail, common);
}

function nodeFunctionSelectedPrograms(
  resource: RawUnusedSource,
  inspected: ReturnType<typeof nodeFunctionConstructor>,
  ranges: readonly { start: number; end: number }[],
  first: { start: number; end: number },
  last: { start: number; end: number },
  tail: { span: unknown },
  common: Omit<BrowserUnusedProgram, "role" | "start" | "end" | "removals"> &
    Required<Pick<BrowserUnusedProgram, "callerRange" | "callAstSha256" | "consumerRoute">>,
): BrowserUnusedProgram[] {
  const tailSpan = inspected.span(tail.span);
  return [
    {
      ...common,
      role: "node-function-parameters",
      start: first.start,
      end: last.end,
      removals: ranges.flatMap((range) => [
        { start: range.start, end: range.start + 1 },
        { start: range.end - 1, end: range.end },
      ]),
    },
    ...nodeFunctionBodyPrograms(resource, inspected, common.consumerRoute).map((program) => ({
      ...program,
      callerRange: common.callerRange,
      callAstSha256: common.callAstSha256,
    })),
    {
      ...common,
      role: "node-function-return",
      start: tailSpan.start + 2,
      end: tailSpan.end,
      removals: [],
    },
  ];
}

export function nodeFunctionRuntimeAst(
  programs: readonly BrowserUnusedProgram[],
  raw: ReadonlyMap<string, RawUnusedSource>,
  code: string,
) {
  const body = programs
    .slice(1)
    .map((program) => {
      const source = raw.get(program.sourcePath);
      if (!source) throw new Error("missing node Function program resource");
      return source.code.slice(program.start, program.end);
    })
    .join("\n");
  const runtimeAst = compilerExpression(
    `(function(${nodeFunctionParameters.join(",")}){\n${body}\n});`,
  );
  if (compilerExpression(code) !== runtimeAst) throw new Error("node Function runtime AST changed");
  const runtimeAstSha256 = createHash("sha256").update(runtimeAst).digest("hex");
  return runtimeAstSha256;
}

const nodeFunctionRangeSchema = z
  .object({ start: z.number().int().nonnegative(), end: z.number().int().nonnegative() })
  .strict();
const nodeFunctionSourceSchema = z
  .object({ sourcePath: z.string(), sourceSha256: z.string().regex(/^[a-f0-9]{64}$/) })
  .strict();
const nodeFunctionProgramSchema = nodeFunctionSourceSchema
  .extend({
    mode: z.literal("node-function-test"),
    role: z.enum(["node-function-parameters", "node-function-body", "node-function-return"]),
    start: z.number().int().nonnegative(),
    end: z.number().int().nonnegative(),
    removals: z.array(nodeFunctionRangeSchema),
    data: z.null(),
    runtimeAstSha256: z.string().regex(/^[a-f0-9]{64}$/),
    callAstSha256: z.string().regex(/^[a-f0-9]{64}$/),
    callerRange: nodeFunctionSourceSchema.extend(nodeFunctionRangeSchema.shape).strict(),
    consumerRoute: z.array(nodeFunctionSourceSchema),
  })
  .strict();

function verifyNodeFunctionProvenance(
  programs: readonly BrowserUnusedProgram[],
  raw: ReadonlyMap<string, RawUnusedSource>,
  code: string,
) {
  const producer = raw.get(nodeFunctionRoute[0].sourcePath);
  const resource = raw.get(nodeFunctionRoute[1].sourcePath);
  if (!producer || !resource) throw new Error("missing node Function provenance source");
  const expected = inspectNodeFunctionPrograms(producer, resource);
  const runtimeAstSha256 = nodeFunctionRuntimeAst(expected, raw, code);
  const bound = expected.map((program) => ({ ...program, runtimeAstSha256 }));
  if (
    JSON.stringify(programs.map((program) => nodeFunctionProgramSchema.parse(program))) !==
    JSON.stringify(bound.map((program) => nodeFunctionProgramSchema.parse(program)))
  )
    throw new Error("changed node Function provenance metadata");
}

export function callbackTypeRemovals(
  callback: Record<string, unknown>,
  offset: (at: number) => number,
) {
  return browserAstNodes(callback)
    .flatMap(({ node: type }) => {
      if (type.type === "TsTypeAnnotation" || type.type === "TsTypeParameterInstantiation") {
        const range = astSpan.parse(type.span);
        return [{ start: offset(range.start), end: offset(range.end) }];
      }
      if (type.type === "TsAsExpression") {
        const expression = astObject.parse(type.expression);
        return [
          {
            start: offset(astSpan.parse(expression.span).end),
            end: offset(astSpan.parse(type.span).end),
          },
        ];
      }
      return [];
    })
    .sort((a, b) => a.start - b.start);
}
export function eraseCallbackTypes(
  code: string,
  start: number,
  end: number,
  removals: readonly { start: number; end: number }[],
) {
  const emitted = new MagicString(code);
  emitted.remove(0, start).remove(end, code.length);
  let previous = start;
  for (const range of removals) {
    if (range.start < previous || range.end > end)
      throw new Error("overlapping/unknown callback type erasure");
    emitted.remove(range.start, range.end);
    previous = range.end;
  }
  if (
    compilerExpression(`(${code.slice(start, end)});`) !==
    compilerExpression(`(${emitted.toString()});`)
  )
    throw new Error("callback runtime AST changed");
  parseSync(`(${emitted.toString()});`, { syntax: "ecmascript" });
  return compilerExpression(`(${emitted.toString()});`);
}

export function importedPageHandle(nodes: readonly { node: Record<string, unknown> }[]) {
  const imports = nodes.filter(({ node }) => node.type === "ImportDeclaration");
  const bindings = imports.flatMap(({ node }) => {
    const source = astObject.parse(node.source);
    if (source.value !== "./official-renderer-types" || node.typeOnly !== true) return [];
    return z
      .array(astObject)
      .parse(node.specifiers)
      .filter((specifier) => {
        if (specifier.type !== "ImportSpecifier") return false;
        const imported = specifier.imported === null ? specifier.local : specifier.imported;
        return astObject.parse(imported).value === "PageHandle";
      })
      .map((specifier) => pageIdentifier.parse(specifier.local));
  });
  if (bindings.length !== 1 || !bindings[0]) throw new Error("unknown PageHandle import binding");
  return bindings[0];
}
function methodPageParameter(owner: Record<string, unknown> | null) {
  if (!owner) throw new Error("page callback outside actual method");
  const fn = owner.type === "ClassMethod" ? astObject.parse(owner.function) : owner;
  const first = z.array(astObject).parse(fn.params)[0];
  if (!first) throw new Error("missing PageHandle method parameter");
  return astObject.parse(first.pat);
}
function forbiddenPageBinding(
  owner: Record<string, unknown>,
  binding: z.infer<typeof pageIdentifier>,
) {
  return browserAstNodes(owner).some(({ node }) => {
    const targets = pageBindingTargets(node);
    return targets.some((target) =>
      browserAstNodes(target).some(({ node: identifier }) => {
        if (identifier.type !== "Identifier" || identifier.value !== binding.value) return false;
        const span = astSpan.parse(identifier.span);
        return span.start !== binding.span.start || span.end !== binding.span.end;
      }),
    );
  });
}
export function validatePageReceiver(
  imported: z.infer<typeof pageIdentifier>,
  member: Record<string, unknown>,
  owner: Record<string, unknown> | null,
  pickEvaluate = false,
) {
  const parameter = methodPageParameter(owner);
  const binding = pageIdentifier.parse(parameter),
    receiver = pageIdentifier.parse(member.object);
  const annotation = astObject.parse(astObject.parse(parameter.typeAnnotation).typeAnnotation);
  const type = pickEvaluate
    ? pagePickEvaluateType(annotation)
    : pageIdentifier.parse(annotation.typeName);
  if (
    annotation.type !== "TsTypeReference" ||
    type.value !== imported.value ||
    type.ctxt !== imported.ctxt ||
    parameter.optional === true
  )
    throw new Error("unknown PageHandle parameter type binding");
  if (binding.value !== receiver.value || binding.ctxt !== receiver.ctxt)
    throw new Error("page callback receiver differs from actual parameter");
  if (!owner || forbiddenPageBinding(owner, binding))
    throw new Error("page callback parameter shadow/reassignment");
}

function pageBindingTargets(node: Record<string, unknown>): unknown[] {
  switch (node.type) {
    case "VariableDeclarator":
      return [node.id];
    case "Parameter":
      return [node.pat];
    case "CatchClause":
      return [node.param];
    case "ArrowFunctionExpression":
      return z.array(z.unknown()).parse(node.params);
    case "AssignmentExpression":
      return [node.left];
    case "UpdateExpression":
      return [node.argument];
    default:
      return [];
  }
}

export const sharedSvgCaptureRoute = [
  {
    sourcePath: "scripts/mermaid/official-renderer.ts",
    sourceSha256: "9417b6982aad961e55b2bfb9ebc4365f85bf22d8f946ccbe555bf21ba9c1c716",
  },
  {
    sourcePath: "scripts/mermaid/rasterize-svg-dir.ts",
    sourceSha256: "b9ab42f3fcc4753ce410dd66840bee25a86e56c484342e67294e2c321a274eec",
  },
  {
    sourcePath: "scripts/mermaid/raster-capture-observation.ts",
    sourceSha256: "29eaac2b5bc227fbaaa6504c076863a6989f88597c1a5ce994b43c12da09b6fb",
  },
  {
    sourcePath: "scripts/mermaid/official-renderer-types.ts",
    sourceSha256: "a9b927eaac4cae8d6f07dab25dc6db0e95721db7d149470947120d3c8731811f",
  },
] as const;

const sharedSvgSelectedNodeSha256 = {
  baseHtml: "9d13c143a282d73441506a59807c9d486957ff64a4d42816581c3c914a619750",
  rasterCaptureHtml: "b1f95211bdab98685fe00dc49eb2082282b180ef9d0aac19f0117b06c9e1bc1e",
} as const;
const sharedSvgTypeNodeSha256 = {
  BrowserLauncher: "917e723ef0f3bd4e2c000abec9a0480fe31fff40f3c8181c57c55351c1a94142",
  PlaywrightModule: "1174ef035b916fd60bbb72203df912cdaf3b927bde8c25ee921794a4ee112897",
  BrowserIdentityCDPSession: "3775ad04597e9aeeb8ef951210ffa61f63c5b98be355b4d0de2b363eaaf6f401",
  BrowserHandle: "a14f95523dc02dfd153afb297a83d79a889dcc34c51a0fe896447ccbbf8ea67b",
  PageHandle: "ce800e7d26d601cc4482ab04fbb68ed7c064ff082e98ad8d5beb73f2cebb77df",
  LocatorHandle: "4e643e91e1c0b876d67f12813c02dc1bd4f66cf810a2a3142309eca9edd6c756",
} as const;

function sharedSvgSemanticSha256(node: Record<string, unknown>) {
  return createHash("sha256")
    .update(
      JSON.stringify(node, (key, value: unknown) =>
        ["span", "ctxt", "raw"].includes(key)
          ? undefined
          : key === "spread" && value !== null
            ? true
            : value,
      ),
    )
    .digest("hex");
}

function verifySharedSvgSelectedNodes(raster: string, types: string) {
  const rasterModule = parseSync(raster, { syntax: "typescript" });
  const rasterDeclarations = rasterModule.body
    .filter((statement) => statement.type === "ExportDeclaration")
    .map((statement) => astObject.parse(statement.declaration));
  const rasterizer = rasterDeclarations.filter(
    (declaration) =>
      declaration.type === "ClassDeclaration" &&
      astObject.parse(declaration.identifier).value === "SvgBrowserRasterizer",
  );
  const baseHtml = rasterizer.flatMap((declaration) =>
    z
      .array(astObject)
      .parse(declaration.body)
      .filter(
        (member) =>
          member.type === "ClassMethod" && astObject.parse(member.key).value === "baseHtml",
      ),
  );
  const captureHtml = rasterDeclarations.filter(
    (declaration) =>
      declaration.type === "FunctionDeclaration" &&
      astObject.parse(declaration.identifier).value === "rasterCaptureHtml",
  );
  if (
    rasterizer.length !== 1 ||
    baseHtml.length !== 1 ||
    captureHtml.length !== 1 ||
    !baseHtml[0] ||
    !captureHtml[0] ||
    sharedSvgSemanticSha256(baseHtml[0]) !== sharedSvgSelectedNodeSha256.baseHtml ||
    sharedSvgSemanticSha256(captureHtml[0]) !== sharedSvgSelectedNodeSha256.rasterCaptureHtml
  )
    throw new Error("changed shared SVG raster HTML source nodes");

  const typeNodes = browserAstNodes(parseSync(types, { syntax: "typescript" }));
  for (const [name, sha256] of Object.entries(sharedSvgTypeNodeSha256)) {
    const selected = typeNodes.filter(
      ({ node }) =>
        node.type === "TsInterfaceDeclaration" && astObject.parse(node.id).value === name,
    );
    if (
      selected.length !== 1 ||
      !selected[0] ||
      sharedSvgSemanticSha256(selected[0].node) !== sha256
    )
      throw new Error(`changed shared SVG renderer ABI: ${name}`);
  }
}

function sharedSvgLiveRoute(sources: readonly SharedSvgPhysicalSource[]) {
  return sources.map(({ sourcePath, sha256 }) => ({ sourcePath, sourceSha256: sha256 }));
}

function pagePickEvaluateType(annotation: Record<string, unknown>) {
  const name = pageIdentifier.parse(annotation.typeName);
  const args = z.array(astObject).parse(astObject.parse(annotation.typeParams).params);
  if (name.value !== "Pick" || name.ctxt !== 1 || args.length !== 2)
    throw new Error("unknown PageHandle Pick binding");
  const first = args[0],
    second = args[1];
  if (
    !first ||
    !second ||
    first.type !== "TsTypeReference" ||
    second.type !== "TsLiteralType" ||
    astObject.parse(second.literal).value !== "evaluate"
  )
    throw new Error("unknown PageHandle Pick member");
  return pageIdentifier.parse(first.typeName);
}

type SharedSvgPhysicalSource = {
  sourcePath: string;
  sourceSha256: string;
  code: string;
  sha256: string;
};

function sharedSvgPhysicalSources(
  raw: ReadonlyMap<string, RawUnusedSource>,
): SharedSvgPhysicalSource[] {
  const sources = sharedSvgCaptureRoute.map((binding, index) => {
    const source = raw.get(binding.sourcePath);
    if (
      !source ||
      (index !== 1 && index !== 3 && source.sha256 !== binding.sourceSha256) ||
      physicalUnusedSource(Buffer.from(source.code)).sha256 !== source.sha256
    )
      throw new Error(`changed/missing shared SVG consumer: ${binding.sourcePath}`);
    return {
      sourcePath: binding.sourcePath,
      sourceSha256: source.sha256,
      code: source.code,
      sha256: source.sha256,
    };
  });
  const raster = sources[1],
    types = sources[3];
  if (!raster || !types) throw new Error("missing shared SVG selected consumers");
  verifySharedSvgSelectedNodes(raster.code, types.code);
  return sources;
}

function sharedSvgImport(nodes: ReturnType<typeof browserAstNodes>) {
  const imports = nodes.filter(
    ({ node }) =>
      node.type === "ImportDeclaration" &&
      astObject.parse(node.source).value === "./raster-capture-observation" &&
      node.typeOnly !== true,
  );
  const bindings = imports.flatMap(({ node }) =>
    z
      .array(astObject)
      .parse(node.specifiers)
      .filter(
        (specifier) =>
          specifier.type === "ImportSpecifier" &&
          specifier.isTypeOnly !== true &&
          astObject.parse(specifier.imported ?? specifier.local).value === "resizeSvgCapture",
      )
      .map((specifier) => pageIdentifier.parse(specifier.local)),
  );
  if (bindings.length !== 1 || !bindings[0]) throw new Error("unknown shared SVG import binding");
  return bindings[0];
}

function sharedSvgCaller(
  source: SharedSvgPhysicalSource,
  mode: "officialFallback" | "strictViewBox",
) {
  const module = parseSync(source.code, { syntax: "typescript" });
  const nodes = browserAstNodes(module),
    imported = sharedSvgImport(nodes);
  if (forbiddenPageBinding(astObject.parse(module), imported))
    throw new Error("shared SVG import shadow/reassignment");
  const calls = nodes.filter(
    ({ node }) =>
      node.type === "CallExpression" &&
      astObject.parse(node.callee).type === "Identifier" &&
      astObject.parse(node.callee).value === imported.value &&
      astObject.parse(node.callee).ctxt === imported.ctxt,
  );
  const selected = calls[0];
  if (
    calls.length !== 1 ||
    !selected ||
    selected.method !== (mode === "officialFallback" ? "resizeCapture" : "capture")
  )
    throw new Error("unknown shared SVG caller/facade");
  const args = z
    .array(z.object({ spread: z.null(), expression: astObject }))
    .parse(selected.node.arguments);
  if (
    args.length !== 3 ||
    !args[0] ||
    !args[1] ||
    !args[2] ||
    args[2].expression.type !== "StringLiteral" ||
    args[2].expression.value !== mode
  )
    throw new Error("changed shared SVG caller mode");
  validatePageReceiver(importedPageHandle(nodes), { object: args[0].expression }, selected.owner);
  const range = nodeFunctionSpan(source.code, module.span.start, selected.node.span);
  const strictRange = nodeFunctionSpan(source.code, module.span.start, args[1].expression.span);
  const strictSource = source.code.slice(strictRange.start, strictRange.end);
  if (
    compilerExpression(`(${strictSource});`) !==
    compilerExpression(
      mode === "officialFallback"
        ? "(strict);"
        : '(this.options.referenceMode === "strictVendorOriginal");',
    )
  )
    throw new Error("changed shared SVG strict argument");
  return {
    sourcePath: source.sourcePath,
    sourceSha256: source.sha256,
    ...range,
    mode,
    strictSource,
    callAstSha256: createHash("sha256")
      .update(compilerExpression(`(${source.code.slice(range.start, range.end)});`))
      .digest("hex"),
  };
}

function sharedSvgFunction(code: string) {
  const module = parseSync(code, { syntax: "typescript" });
  const exports = module.body.filter(
    (node) =>
      node.type === "ExportDeclaration" &&
      node.declaration.type === "FunctionDeclaration" &&
      node.declaration.identifier?.value === "resizeSvgCapture",
  );
  const selected = exports[0];
  if (
    exports.length !== 1 ||
    !selected ||
    selected.type !== "ExportDeclaration" ||
    selected.declaration.type !== "FunctionDeclaration"
  )
    throw new Error("unknown shared SVG function export");
  const owner = astObject.parse(selected.declaration);
  const nodes = browserAstNodes(owner, "resizeSvgCapture", owner);
  const members = nodes.filter(
    ({ node }) =>
      node.type === "MemberExpression" &&
      astObject.parse(node.object).type === "Identifier" &&
      astObject.parse(node.object).value === "page" &&
      astObject.parse(node.property).value === "evaluate",
  );
  const member = members[0]?.node;
  if (members.length !== 1 || !member) throw new Error("unknown shared SVG evaluate site");
  validatePageReceiver(importedPageHandle(browserAstNodes(module)), member, owner, true);
  const call = nodes.find(
    ({ node }) =>
      node.type === "CallExpression" && JSON.stringify(node.callee) === JSON.stringify(member),
  )?.node;
  if (!call) throw new Error("escaped shared SVG evaluate call");
  return { module, owner, call };
}

export function inspectSharedSvgCaptureCallback(code: string) {
  const { module, owner, call } = sharedSvgFunction(code);
  const args = z.array(z.object({ spread: z.null(), expression: astObject })).parse(call.arguments);
  const callback = args[0]?.expression,
    data = args[1]?.expression;
  if (
    args.length !== 2 ||
    callback?.type !== "ArrowFunctionExpression" ||
    data?.type !== "ObjectExpression"
  )
    throw new Error("unknown shared SVG callback/data");
  const properties = z.array(pageIdentifier).parse(data.properties);
  const parameters = z
    .array(astObject)
    .parse(owner.params)
    .slice(1)
    .map((parameter) => pageIdentifier.parse(parameter.pat));
  if (
    properties.map((property) => property.value).join("/") !== "strict/mode" ||
    parameters.map((parameter) => parameter.value).join("/") !== "strict/mode" ||
    properties.some((property, index) => property.ctxt !== parameters[index]?.ctxt)
  )
    throw new Error("changed shared SVG serialized parameter binding");
  const span = (value: unknown) => nodeFunctionSpan(code, module.span.start, value);
  const range = span(callback.span),
    dataRange = span(data.span);
  const offset = (at: number) =>
    nodeFunctionSpan(code, module.span.start, { start: at, end: at }).start;
  const removals = callbackTypeRemovals(callback, offset);
  const runtimeAst = eraseCallbackTypes(code, range.start, range.end, removals);
  const compiled = transformSync(`(${code.slice(range.start, range.end)});`, {
    jsc: { parser: { syntax: "typescript" }, target: "es2022" },
  }).code;
  if (
    browserAstNodes(parseSync(compiled, { syntax: "ecmascript" })).some(
      ({ node }) =>
        node.type === "Identifier" &&
        node.ctxt === 1 &&
        !["document", "String", "Number", "Math", "Error", "undefined"].includes(
          String(node.value),
        ),
    )
  )
    throw new Error("shared SVG callback has free host closure");
  return {
    ...range,
    removals,
    data: code.slice(dataRange.start, dataRange.end),
    runtimeAstSha256: createHash("sha256").update(runtimeAst).digest("hex"),
    callAstSha256: createHash("sha256").update(JSON.stringify(call)).digest("hex"),
  };
}

export function inspectSharedSvgCaptureProgram(
  raw: ReadonlyMap<string, RawUnusedSource>,
): BrowserUnusedProgram {
  const sources = sharedSvgPhysicalSources(raw);
  const official = sources[0],
    raster = sources[1],
    helper = sources[2];
  if (!official || !raster || !helper) throw new Error("missing shared SVG source route");
  return {
    sourcePath: helper.sourcePath,
    sourceSha256: helper.sourceSha256,
    role: "callback",
    mode: "shared-svg-capture",
    ...inspectSharedSvgCaptureCallback(helper.code),
    consumerRoute: sharedSvgLiveRoute(sources),
    captureCallers: inspectSharedSvgCaptureCallers(official, raster),
  };
}

export const sharedSvgProgramSchema = nodeFunctionSourceSchema
  .extend({
    role: z.literal("callback"),
    mode: z.literal("shared-svg-capture"),
    start: z.number().int().nonnegative(),
    end: z.number().int().nonnegative(),
    removals: z.array(nodeFunctionRangeSchema),
    data: z.string(),
    runtimeAstSha256: z.string().regex(/^[a-f0-9]{64}$/),
    callAstSha256: z.string().regex(/^[a-f0-9]{64}$/),
    consumerRoute: z.array(nodeFunctionSourceSchema),
    captureCallers: z.array(
      nodeFunctionSourceSchema
        .extend(nodeFunctionRangeSchema.shape)
        .extend({
          callAstSha256: z.string().regex(/^[a-f0-9]{64}$/),
          mode: z.enum(["officialFallback", "strictViewBox"]),
          strictSource: z.string(),
        })
        .strict(),
    ),
  })
  .strict();

function verifySharedSvgCaptureProvenance(
  programs: readonly BrowserUnusedProgram[],
  raw: ReadonlyMap<string, RawUnusedSource>,
) {
  const expected = inspectSharedSvgCaptureProgram(raw);
  if (
    programs.length !== 1 ||
    !programs[0] ||
    JSON.stringify(sharedSvgProgramSchema.parse(programs[0])) !==
      JSON.stringify(sharedSvgProgramSchema.parse(expected))
  )
    throw new Error("changed shared SVG provenance metadata");
}

function validProjectionProgram(
  program: BrowserUnusedProgram,
  label: BrowserUnusedOrigin["projectionLabel"],
): boolean {
  switch (label) {
    case "sequence-vm-test":
      return (
        program.mode === "sequence-vm-test" &&
        ["vm-source", "vm-result-literal"].includes(program.role)
      );
    case "shared-svg-capture":
      return program.mode === "shared-svg-capture" && program.role === "callback";
    case "node-function":
      return (
        program.mode === "node-function-test" &&
        ["node-function-parameters", "node-function-body", "node-function-return"].includes(
          program.role,
        )
      );
    case "rust-native-width":
      return (
        program.mode === "rust-native-test" &&
        ["native-test-trace", "rust-result-literal"].includes(program.role)
      );
    default:
      return (
        ![
          "rust-native-test",
          "node-function-test",
          "shared-svg-capture",
          "sequence-vm-test",
        ].includes(program.mode) && ["provider", "callback"].includes(program.role)
      );
  }
}

export function inspectSharedSvgCaptureCallers(official: RawUnusedSource, raster: RawUnusedSource) {
  const [officialBinding, rasterBinding] = sharedSvgCaptureRoute;
  if (!officialBinding || !rasterBinding) throw new Error("missing shared SVG caller route");
  return [
    sharedSvgCaller(
      {
        sourcePath: officialBinding.sourcePath,
        sourceSha256: officialBinding.sourceSha256,
        code: official.code,
        sha256: official.sha256,
      },
      "officialFallback",
    ),
    sharedSvgCaller(
      {
        sourcePath: rasterBinding.sourcePath,
        sourceSha256: rasterBinding.sourceSha256,
        code: raster.code,
        sha256: raster.sha256,
      },
      "strictViewBox",
    ),
  ];
}

export const sequenceVmPaths = [
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_sequence_activation_fixes.js",
  "scripts/mermaid/svg_sequence_activation_fixes_test.ts",
] as const;
const sequenceVmSymbol = "katanaNormalizeSequenceActivationSvg";
const sequenceVmLoaded = `readFileSync(new URL("../../${sequenceVmPaths[0]}", import.meta.url), "utf8")`;
const sequenceVmNormalize = `function normalize(svg: string): string {
  const context = createContext({});
  runInContext(SEQUENCE_FIXES, context);
  return (runInContext("${sequenceVmSymbol}", context) as SvgNormalizer)(svg);
}`;

function verifySequenceVmImports(module: ReturnType<typeof parseSync>) {
  const imports = module.body.filter((statement) => statement.type === "ImportDeclaration");
  const names = [
    ["expect", "test"],
    ["createHash"],
    ["readFileSync"],
    ["createContext", "runInContext"],
  ];
  if (
    imports.length !== names.length ||
    imports.some(
      (statement, index) =>
        (index < 2 && statement.source.value !== ["bun:test", "node:crypto"][index]) ||
        statement.typeOnly ||
        statement.specifiers
          .map((specifier) =>
            specifier.type === "ImportSpecifier" && !specifier.isTypeOnly
              ? specifier.local.value
              : "",
          )
          .join("/") !== names[index]?.join("/"),
    )
  )
    throw new Error("unknown sequence VM import/SDK namespace shape");
  if (
    browserAstNodes(module).some(
      ({ node }) =>
        node.type === "CallExpression" &&
        (astObject.parse(node.callee).type === "Import" ||
          astObject.parse(node.callee).value === "require" ||
          (astObject.parse(node.callee).type === "MemberExpression" &&
            astObject.parse(astObject.parse(node.callee).property).value === "getBuiltinModule")),
    )
  )
    throw new Error("unknown sequence VM dynamic SDK loader");
}

export function inspectSequenceVmOwner(raw: ReadonlyMap<string, RawUnusedSource>) {
  const source = raw.get(sequenceVmPaths[0]),
    caller = raw.get(sequenceVmPaths[1]);
  if (!source || !caller) throw new Error("missing sequence VM physical source");
  const module = parseSync(caller.code, { syntax: "typescript" });
  verifySequenceVmImports(module);
  const nodes = browserAstNodes(module);
  const span = (node: Record<string, unknown>) => nodeFunctionSpan(caller.code, 1, node.span);
  const selected = nodes.filter(
    ({ node }) =>
      node.type === "FunctionDeclaration" && astObject.parse(node.identifier).value === "normalize",
  );
  if (selected.length !== 1 || !selected[0])
    throw new Error("missing/ambiguous sequence VM consumer");
  const callerRange = span(selected[0].node);
  if (
    compilerExpression(caller.code.slice(callerRange.start, callerRange.end)) !==
    compilerExpression(sequenceVmNormalize)
  )
    throw new Error("changed sequence VM load/context/result call");
  const loaded = module.body
    .flatMap((statement) =>
      statement.type === "VariableDeclaration" && statement.kind === "const"
        ? statement.declarations
        : [],
    )
    .filter(
      (declaration) =>
        declaration.id.type === "Identifier" && declaration.id.value === "SEQUENCE_FIXES",
    );
  const binding = loaded[0];
  if (
    loaded.length !== 1 ||
    !binding?.init ||
    compilerExpression(
      caller.code.slice(
        span(astObject.parse(binding.init)).start,
        span(astObject.parse(binding.init)).end,
      ),
    ) !== compilerExpression(sequenceVmLoaded)
  )
    throw new Error("changed sequence VM loaded JS path/encoding");
  const calls = browserAstNodes(selected[0].node).filter(
    ({ node }) => node.type === "CallExpression",
  );
  const callee = (name: string) =>
    calls.filter(({ node }) => astObject.parse(node.callee).value === name);
  const contexts = callee("createContext"),
    runs = callee("runInContext");
  if (contexts.length !== 1 || runs.length !== 2 || !contexts[0] || !runs[0] || !runs[1])
    throw new Error("changed sequence VM call inventory");
  const args = (node: Record<string, unknown>) =>
    z
      .array(astObject)
      .parse(node.arguments)
      .map((arg) => astObject.parse(arg.expression));
  const loadArgs = args(runs[0].node),
    lookupArgs = args(runs[1].node);
  const literal = lookupArgs[0];
  if (literal?.type !== "StringLiteral" || literal.value !== sequenceVmSymbol)
    throw new Error("changed sequence VM lookup literal");
  const literalSpan = span(literal);
  if (caller.code.slice(literalSpan.start, literalSpan.end) !== JSON.stringify(sequenceVmSymbol))
    throw new Error("escaped sequence VM lookup literal");
  const readCall = astObject.parse(binding.init);
  const at = (node: unknown) => span(astObject.parse(node)).start;
  return {
    pins: sequenceVmPaths.map((sourcePath) => ({
      sourcePath,
      sourceSha256: raw.get(sourcePath)?.sha256 ?? "",
    })),
    sourceBindingStart: at(binding.id),
    readFileStart: at(readCall.callee),
    urlConstructorStart: at(astObject.parse(args(readCall)[0]).callee),
    createContextStart: at(contexts[0].node.callee),
    loadCalleeStart: at(runs[0].node.callee),
    lookupCalleeStart: at(runs[1].node.callee),
    loadSourceStart: at(loadArgs[0]),
    loadContextStart: at(loadArgs[1]),
    lookupContextStart: at(lookupArgs[1]),
    literalSpan,
    callerRange,
  };
}

export function inspectSequenceVmPrograms(
  raw: ReadonlyMap<string, RawUnusedSource>,
): BrowserUnusedProgram[] {
  const owner = inspectSequenceVmOwner(raw),
    source = raw.get(sequenceVmPaths[0]);
  if (!source) throw new Error("missing sequence VM producer");
  return [
    {
      sourcePath: sequenceVmPaths[0],
      sourceSha256: source.sha256,
      role: "vm-source",
      mode: "sequence-vm-test",
      start: 0,
      end: source.code.length,
      removals: [],
      data: null,
    },
    {
      sourcePath: sequenceVmPaths[1],
      sourceSha256: owner.pins[1]?.sourceSha256 ?? "",
      role: "vm-result-literal",
      mode: "sequence-vm-test",
      start: owner.literalSpan.start + 1,
      end: owner.literalSpan.end - 1,
      removals: [],
      data: null,
      consumerRoute: owner.pins,
      callerRange: {
        sourcePath: sequenceVmPaths[1],
        sourceSha256: owner.pins[1]?.sourceSha256 ?? "",
        ...owner.callerRange,
      },
    },
  ];
}

function verifySequenceVmProvenance(
  programs: readonly BrowserUnusedProgram[],
  raw: ReadonlyMap<string, RawUnusedSource>,
) {
  if (JSON.stringify(programs) !== JSON.stringify(inspectSequenceVmPrograms(raw)))
    throw new Error("changed sequence VM provenance");
}

function verifySequenceVmProjection(
  label: BrowserUnusedOrigin["projectionLabel"],
  programs: readonly BrowserUnusedProgram[],
  raw: ReadonlyMap<string, RawUnusedSource>,
) {
  if (label === "sequence-vm-test") verifySequenceVmProvenance(programs, raw);
}
