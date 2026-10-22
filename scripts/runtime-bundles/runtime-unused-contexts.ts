import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { parseSync } from "@swc/core";
import MagicString, { Bundle } from "magic-string";
import { z } from "zod";
import { resolveOfficialRendererAssetInputs } from "../mermaid/official-renderer-asset-inputs";
import { MermaidRuntimeScripts } from "../mermaid/official-renderer-i18n";
import {
  OFFICIAL_BROWSER_CONSUMER_PINS,
  OFFICIAL_BROWSER_PROVIDER_COUNT,
} from "./official-browser-consumer-contract";
import { runtimeBundleDefinitions } from "./runtime-bundle-definitions";
import { RuntimeFragmentLoader } from "./runtime-fragment-loader";
import { readClassicUnusedBindings } from "./runtime-unused-classic-binding";
import { type UnusedClassicInputs, unusedClassicInputs } from "./runtime-unused-classic-capture";
import {
  inspectNativeWidthConsumerFacts,
  NATIVE_WIDTH_HASH_INPUT_PATHS,
} from "./runtime-unused-native-width-hash";
import {
  astObject,
  astSpan,
  BrowserUnusedOrigin,
  type BrowserUnusedProgram,
  browserAstNodes,
  callbackTypeRemovals,
  eraseCallbackTypes,
  importedPageHandle,
  inspectNodeFunctionPrograms,
  inspectSequenceVmPrograms,
  inspectSharedSvgCaptureProgram,
  nodeFunctionRoute,
  nodeFunctionRuntimeAst,
  physicalUnusedSource,
  type RawUnusedSource,
  RuntimeUnusedOrigin,
  sequenceVmPaths,
  sharedSvgCaptureRoute,
  type UnusedStage,
  unusedProgramAffixes,
  validatePageReceiver,
} from "./runtime-unused-origin";
import { RUST_CLASSIC_INPUT_PATHS } from "./runtime-unused-rust-classic";

export interface RuntimeUnusedContext {
  readonly name: string;
  readonly origin: RuntimeUnusedOrigin | BrowserUnusedOrigin;
  readonly stages: readonly UnusedStage[];
  readonly programs?: readonly BrowserUnusedProgram[];
  readonly codeBytesEqual: boolean;
  readonly externalInputs?: readonly OfficialStrictExternalInput[];
}
export const incompleteUnusedConsumers = [
  "official strict realm, external vendor programs and imported capture/image callbacks",
  "Rust generated runtime/vendor/result-script consumers",
  "VM template/provider and regex-selected function consumers",
] as const;

export function readUnusedPhysicalSource(root: string, name: string): RawUnusedSource {
  if (
    path.posix.normalize(name) !== name ||
    path.isAbsolute(name) ||
    path.win32.isAbsolute(name) ||
    name.includes("\\") ||
    name.split("/").includes("..")
  )
    throw new Error("unsafe raw source path");
  let file = root;
  for (const part of name.split("/")) {
    file = path.join(file, part);
    if (fs.lstatSync(file).isSymbolicLink()) throw new Error("raw source symlink");
  }
  if (!fs.lstatSync(file).isFile() || !fs.realpathSync(file).startsWith(`${root}${path.sep}`))
    throw new Error("raw source not a regular repository file");
  return physicalUnusedSource(fs.readFileSync(file));
}

export function runtimeUnusedContexts(
  root: string,
  inputs = actualOfficialStrictInputs(),
  classicInputs: UnusedClassicInputs = unusedClassicInputs(inputs.argv),
) {
  const definitions = runtimeBundleDefinitions();
  const classic = definitions.filter((definition) => definition.bundledEntryPath === null);
  if (classic.map((definition) => definition.name).join("\0") !== "mermaid\0drawio\0zenuml")
    throw new Error("unknown classic registry");
  const raw = new Map<string, RawUnusedSource>();
  for (const definition of classic)
    for (const fragment of definition.fragments)
      if (!raw.has(fragment.sourcePath))
        raw.set(fragment.sourcePath, readUnusedPhysicalSource(root, fragment.sourcePath));
  const loader = new RuntimeFragmentLoader(root);
  const contexts: RuntimeUnusedContext[] = classic.map((definition) => {
    const provenance = loader.loadDefinitionWithProvenance(definition);
    const normal = loader.loadDefinition(definition);
    const codeBytesEqual = Buffer.from(normal).equals(Buffer.from(provenance.code));
    if (!codeBytesEqual) throw new Error("canonical code bytes changed");
    return {
      name: definition.name,
      stages: ["assembled", "normalized"],
      origin: new RuntimeUnusedOrigin(provenance, raw),
      codeBytesEqual,
    };
  });
  contexts.push(
    officialMermaidUnusedContext(root, raw),
    nativeWidthUnusedContext(root, raw),
    nodeFunctionUnusedContext(root, raw),
    sharedSvgCaptureUnusedContext(root, raw),
    officialStrictUnusedContext(root, raw, inputs),
  );
  verifyUnusedSources(root, raw);
  const module = definitions.find((definition) => definition.name === "mathjax");
  if (!module || module.bundledEntryPath === null)
    throw new Error("missing separate MathJax module");
  const classicCaptureBindings = readClassicUnusedBindings(classicInputs);
  return { contexts, raw, moduleEntry: module.bundledEntryPath, classicCaptureBindings };
}

export interface OfficialStrictInputOptions {
  readonly argv: readonly string[];
  readonly env: Readonly<Record<string, string | undefined>>;
  readonly cwd: string;
  readonly home: string;
}

export function actualOfficialStrictInputs(): OfficialStrictInputOptions {
  return { argv: process.argv.slice(2), env: process.env, cwd: process.cwd(), home: os.homedir() };
}

export interface OfficialStrictExternalInput {
  readonly option: "mermaidJs" | "mermaidZenumlJs";
  readonly absolutePath: string;
  readonly sha256: string;
  readonly byteLength: number;
}

function officialStrictExternalInput(
  option: OfficialStrictExternalInput["option"],
  absolutePath: string,
): OfficialStrictExternalInput {
  if (!path.isAbsolute(absolutePath) || !fs.lstatSync(absolutePath).isFile())
    throw new Error(`official strict external input is not a regular file: ${absolutePath}`);
  const bytes = fs.readFileSync(absolutePath);
  const source = physicalUnusedSource(bytes);
  return Object.freeze({ option, absolutePath, sha256: source.sha256, byteLength: bytes.length });
}

export function verifyOfficialStrictExternalInputs(inputs: readonly OfficialStrictExternalInput[]) {
  if (inputs.map((input) => input.option).join("/") !== "mermaidJs/mermaidZenumlJs")
    throw new Error("unknown official strict external input inventory/order");
  for (const input of inputs) {
    const current = officialStrictExternalInput(input.option, input.absolutePath);
    if (current.sha256 !== input.sha256 || current.byteLength !== input.byteLength)
      throw new Error(`official strict external input changed: ${input.absolutePath}`);
  }
}

export function verifyUnusedSources(root: string, raw: ReadonlyMap<string, RawUnusedSource>): void {
  for (const [name, previous] of raw)
    if (readUnusedPhysicalSource(root, name).sha256 !== previous.sha256)
      throw new Error(`physical raw source changed: ${name}`);
}

const rendererSource = "scripts/mermaid/official-renderer.ts";
const determinismSource = "scripts/mermaid/official-renderer-determinism.ts";
const callbackModes = {
  installRuntime: "owned-reference",
  installStorage: "both",
  capture: "both",
  renderSvg: "owned-reference",
  renderVendorOriginalSvg: "strictVendorOriginal",
  setPrimarySvg: "owned-reference",
  install: "both",
} as const;
export interface OfficialUnusedCallback {
  readonly method: string;
  readonly mode: (typeof callbackModes)[keyof typeof callbackModes];
  readonly start: number;
  readonly end: number;
  readonly removals: readonly { start: number; end: number }[];
  readonly data: string | null;
  readonly runtimeAstSha256: string;
  readonly callAstSha256: string;
}

export function inspectOfficialUnusedCallbacks(
  sourcePath: string,
  code: string,
): OfficialUnusedCallback[] {
  if (sourcePath !== rendererSource && sourcePath !== determinismSource)
    throw new Error("unknown browser callback producer");
  const module = parseSync(code, { syntax: "typescript" });
  const bytes = Buffer.from(code);
  const offset = (at: number) =>
    new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, at - module.span.start))
      .length;
  const nodes = browserAstNodes(module);
  const standaloneBridgeSpans = new Set(
    module.body
      .map((statement) =>
        statement.type === "ExportDeclaration" ? statement.declaration : statement,
      )
      .filter(
        (statement) =>
          statement.type === "FunctionDeclaration" &&
          statement.identifier.type === "Identifier" &&
          statement.identifier.value === "installSvgTransformBoundsBridge",
      )
      .flatMap((statement) =>
        browserAstNodes(statement).flatMap(({ node }) => {
          const span = astSpan.safeParse(node.span);
          return span.success ? [`${span.data.start}:${span.data.end}`] : [];
        }),
      ),
  );
  const found = nodes.filter(({ node }) => {
    const span = astSpan.safeParse(node.span);
    if (span.success && standaloneBridgeSpans.has(`${span.data.start}:${span.data.end}`))
      return false;
    if (node.type !== "MemberExpression") return false;
    const object = astObject.parse(node.object);
    if (object.type !== "Identifier" || object.value !== "page") return false;
    const property = astObject.parse(node.property);
    if (property.type === "Computed") throw new Error("computed page API is unknown");
    return property.type === "Identifier" && property.value === "evaluate";
  });
  const calls = nodes.filter(({ node }) => node.type === "CallExpression");
  const expected =
    sourcePath === rendererSource
      ? Object.keys(callbackModes).filter((name) => name !== "install")
      : ["install"];
  if (found.map((item) => item.method).join("\0") !== expected.join("\0"))
    throw new Error("missing/unknown browser callback site/order");
  const imported = importedPageHandle(nodes);
  return found.map(({ method, node, owner }) => {
    validatePageReceiver(imported, node, owner);
    const call = calls.find(
      (item) => JSON.stringify(item.node.callee) === JSON.stringify(node),
    )?.node;
    // 実calleeの全ASTを照合し、参照だけを別callbackへ付け替えない。
    if (!call) throw new Error("escaped page.evaluate API");
    const args = z.array(astObject).parse(call.arguments);
    const first = args[0];
    if (!first || args.length > 2 || args.some((argument) => argument.spread))
      throw new Error("unknown page.evaluate arguments");
    const callback = astObject.parse(first.expression);
    if (callback.type !== "ArrowFunctionExpression")
      throw new Error("unknown page.evaluate callback");
    const span = astSpan.parse(callback.span);
    const start = offset(span.start),
      end = offset(span.end);
    const removals = callbackTypeRemovals(callback, offset);
    const runtimeAst = eraseCallbackTypes(code, start, end, removals);
    const selected = Object.entries(callbackModes).find(([name]) => name === method);
    if (!selected) throw new Error("unknown callback mode");
    const data = args[1] ? astSpan.parse(astObject.parse(args[1].expression).span) : null;
    return {
      method,
      mode: selected[1],
      start,
      end,
      removals,
      data: data ? code.slice(offset(data.start), offset(data.end)) : null,
      runtimeAstSha256: createHash("sha256").update(runtimeAst).digest("hex"),
      callAstSha256: createHash("sha256").update(JSON.stringify(call)).digest("hex"),
    };
  });
}

export function officialMermaidUnusedContext(
  root: string,
  raw: Map<string, RawUnusedSource>,
): RuntimeUnusedContext {
  if (fs.realpathSync(process.cwd()) !== root) throw new Error("browser provider root mismatch");
  // 正規consumerの既存レビュー済み経路を再利用する。経路変更時はprovider/modeを再レビューする。
  for (const [name, sha256] of Object.entries(OFFICIAL_BROWSER_CONSUMER_PINS)) {
    const source = readUnusedPhysicalSource(root, name);
    if (source.sha256 !== sha256) throw new Error("changed official browser consumer route");
    raw.set(name, source);
  }
  const provider = [...MermaidRuntimeScripts.paths(), MermaidRuntimeScripts.zenumlBridgePath()];
  if (
    provider.length !== OFFICIAL_BROWSER_PROVIDER_COUNT ||
    new Set(provider).size !== OFFICIAL_BROWSER_PROVIDER_COUNT
  )
    throw new Error("unknown browser provider inventory");
  const programs: BrowserUnusedProgram[] = provider.flatMap((file) => {
    const sourcePath = path.relative(root, file).split(path.sep).join("/");
    const source = readUnusedPhysicalSource(root, sourcePath);
    const previous = raw.get(sourcePath);
    if (previous && previous.sha256 !== source.sha256)
      throw new Error("changed browser provider input");
    raw.set(sourcePath, source);
    if (source.code.length === 0) return [];
    return [
      {
        sourcePath,
        sourceSha256: source.sha256,
        role: "provider",
        mode: "owned-reference",
        start: 0,
        end: source.code.length,
        removals: [],
        data: null,
      },
    ];
  });
  for (const sourcePath of [determinismSource, rendererSource]) {
    const source = readUnusedPhysicalSource(root, sourcePath);
    raw.set(sourcePath, source);
    for (const callback of inspectOfficialUnusedCallbacks(sourcePath, source.code))
      programs.push({ ...callback, sourcePath, sourceSha256: source.sha256, role: "callback" });
  }
  const selected = programs.filter((program) => program.mode !== "strictVendorOriginal");
  const { code, map } = browserLintProjection(selected, raw);
  return {
    name: "official-mermaid-browser",
    stages: ["assembled"],
    programs,
    codeBytesEqual: false,
    origin: new BrowserUnusedOrigin(code, map, selected, raw),
  };
}

function browserLintProjection(
  selected: readonly BrowserUnusedProgram[],
  raw: ReadonlyMap<string, RawUnusedSource>,
  label: BrowserUnusedOrigin["projectionLabel"] = "browser",
) {
  const bundle = new Bundle({ separator: label === "node-function" ? "\n" : "\n;\n" });
  for (const [occurrence, program] of selected.entries()) {
    const source = raw.get(program.sourcePath);
    if (!source) throw new Error("missing browser program");
    const rendered = new MagicString(source.code);
    rendered.remove(0, program.start).remove(program.end, source.code.length);
    for (const range of program.removals) rendered.remove(range.start, range.end);
    const { prefix, suffix } = unusedProgramAffixes(program, label, occurrence);
    rendered.prepend(prefix).append(suffix);
    bundle.addSource({ content: rendered, filename: program.sourcePath });
  }
  const code = bundle.toString();
  const map = bundle.generateMap({ hires: true, includeContent: true }).toString();
  return { code, map };
}

const strictCaptureSource = "scripts/mermaid/raster-capture-observation.ts";
const strictImageSource = "scripts/mermaid/strict-svg-external-images.ts";
export const officialStrictConsumerRoute = [
  ...sharedSvgCaptureRoute,
  {
    sourcePath: strictImageSource,
    sourceSha256: "fce80ffe2c30649308fcabe1f47ac65250bfb03085506e7d3bc9f58fcefe9515",
  },
  {
    sourcePath: "scripts/mermaid/official-renderer-asset-inputs.ts",
    sourceSha256: "8e6976c739355714bad12a6dccefb065a67c6c80f905cf3d6f7fdb1f334bb146",
  },
  {
    sourcePath: "scripts/mermaid/diagram-update.ts",
    sourceSha256: "697d79751ef214cc68ff13efee2da4764b932608fb740d3697bdac78fdd4820c",
  },
] as const;

const strictSourceBindingSchema = z
  .object({
    sourcePath: z.string(),
    sourceSha256: z.string().regex(/^[a-f0-9]{64}$/u),
  })
  .strict();
const strictRangeSchema = z
  .object({ start: z.number().int().nonnegative(), end: z.number().int().positive() })
  .strict();
export const officialStrictProgramSchema = strictSourceBindingSchema
  .extend({
    method: z.literal("renderVendorOriginalSvg").optional(),
    role: z.literal("callback"),
    mode: z.literal("strictVendorOriginal"),
    start: z.number().int().nonnegative(),
    end: z.number().int().positive(),
    removals: z.array(strictRangeSchema),
    data: z.string().nullable(),
    runtimeAstSha256: z.string().regex(/^[a-f0-9]{64}$/u),
    callAstSha256: z.string().regex(/^[a-f0-9]{64}$/u),
    consumerRoute: z
      .array(strictSourceBindingSchema)
      .refine(
        (route) =>
          route.map((binding) => binding.sourcePath).join("/") ===
          officialStrictConsumerRoute.map((binding) => binding.sourcePath).join("/"),
        "unknown official strict consumer route",
      ),
  })
  .strict()
  .refine((program) => program.start < program.end, "unknown official strict callback span")
  .refine(
    (program) =>
      officialStrictConsumerRoute.some((binding) => binding.sourcePath === program.sourcePath) &&
      program.consumerRoute.some(
        (binding) =>
          binding.sourcePath === program.sourcePath &&
          binding.sourceSha256 === program.sourceSha256,
      ),
    "unknown official strict program source pin",
  );

// 全sourceの再構成ではなく、既存の正規caller/branchを有限のAST registryで照合する。
const officialStrictCallerMethods = {
  [rendererSource]: {
    renderPage: "28da7c4e7d4f0d06eb1a6aec989e50b122150b74ff1ea2e8d5123521733ecb48",
    installModeRuntime: "6bc1b915ac208e68f94619c42daa935adb438d87187b51f8cc6f9abb330c5de8",
    isStrictMode: "e39e8e91a3f433430ed395504ed991cc8345e13722a8191002200cb105b8e89c",
    capture: "df4b66224ec2eb411650a75a5a1a3f7e0683e2f869a52d5f473262388c66a07d",
    renderSvg: "752c43d5ca595bb6659d0396b54691f7bd96b5beae145ae36ba490ccf669a176",
    renderVendorOriginalSvg: "92b9b0ee782111a5c2f84d27b3a950f8eeddaa288a66e3e6b023fab9e7916995",
    resizeCapture: "14e5a286e27378bd59229322c046e991c43ad1bb6050b18543f46e177ea8c694",
    waitForSvgImages: "612669970b33f1cb5fd8df340700576e328ab09c0bbcad7cff19da21f2505877",
  },
  "scripts/mermaid/rasterize-svg-dir.ts": {
    capture: "69d159bfe466f6f325a3ed91aac281fc02f0246a91db938362c8df1bfbd415c4",
    waitUntilReady: "8471a9070aa4a4b931e36999cca0c5af88fe63c773fa78d236f2320269cd98c4",
  },
} as const;
const officialStrictHelperFunctions = {
  assignStrictSvg: "65d171841d33751fad405b751dc7e2591ed833a5dd9a76efd9d11f06333237a6",
  observeCaptureSnapshot: "bda9006f824aca086c5d461c3cc42bf9b6af4fe95e59c250c5e1949173f0fe8b",
  waitForStrictSvgExternalImages:
    "6ba0cc16e1533d6120ce1c975a2da3c992946febbf196a6cff909d3a5d60027a",
} as const;

function strictSemanticSha256(node: Record<string, unknown>) {
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

function strictHelperInventory(sourcePath: string, code: string) {
  const module = parseSync(code, { syntax: "typescript" });
  if (module.span.start !== 1)
    throw new Error(`official strict parser source position changed: ${sourcePath}`);
  const expected =
    sourcePath === strictCaptureSource
      ? ["resizeSvgCapture", "assignStrictSvg", "observeCaptureSnapshot"]
      : ["waitForStrictSvgExternalImages", "waitForStrictSvgExternalImages"];
  const found: { method: string; node: Record<string, unknown>; owner: Record<string, unknown> }[] =
    [];
  for (const entry of module.body) {
    if (entry.type !== "ExportDeclaration" || entry.declaration.type !== "FunctionDeclaration")
      continue;
    const owner = astObject.parse(entry.declaration);
    const method = entry.declaration.identifier?.value ?? "";
    for (const { node } of browserAstNodes(owner, method, owner).filter(strictPageMember)) {
      if (astObject.parse(node.property).type === "Computed")
        throw new Error("computed official strict page API");
      found.push({ method, node, owner });
    }
  }
  const allMembers = browserAstNodes(module).filter(strictPageMember);
  if (
    found.map(({ method }) => method).join("/") !== expected.join("/") ||
    allMembers.length !== expected.length
  )
    throw new Error(`official strict callback inventory/order changed: ${sourcePath}`);
  return { module, found };
}

function strictPageMember({ node }: { node: Record<string, unknown> }) {
  return (
    node.type === "MemberExpression" &&
    astObject.parse(node.object).value === "page" &&
    (astObject.parse(node.property).value === "evaluate" ||
      astObject.parse(node.property).type === "Computed")
  );
}

function strictHelperCallbacks(
  sourcePath: string,
  source: RawUnusedSource,
  consumerRoute: readonly { sourcePath: string; sourceSha256: string }[],
): BrowserUnusedProgram[] {
  const { module, found } = strictHelperInventory(sourcePath, source.code);
  const bytes = Buffer.from(source.code);
  const offset = (at: number) =>
    new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, at - module.span.start))
      .length;
  const imported = importedPageHandle(browserAstNodes(module));
  const programs: BrowserUnusedProgram[] = [];
  for (const { method, node, owner } of found) {
    validatePageReceiver(
      imported,
      node,
      owner,
      method === "resizeSvgCapture" || sourcePath === strictImageSource,
    );
    const call = browserAstNodes(owner).find(
      ({ node: candidate }) =>
        candidate.type === "CallExpression" &&
        JSON.stringify(candidate.callee) === JSON.stringify(node),
    )?.node;
    if (!call) throw new Error("escaped official strict page.evaluate API");
    const args = z
      .array(z.object({ spread: z.null(), expression: astObject }))
      .parse(call.arguments);
    const callback = args[0]?.expression;
    if (callback?.type !== "ArrowFunctionExpression" || args.length > 2)
      throw new Error("unknown official strict callback arguments");
    if (method === "resizeSvgCapture") continue;
    const expectedHash = Object.entries(officialStrictHelperFunctions).find(
      ([name]) => name === method,
    )?.[1];
    if (!expectedHash || strictSemanticSha256(owner) !== expectedHash)
      throw new Error(`official strict helper call/branch changed: ${method}`);
    programs.push(
      strictCallbackProgram(
        sourcePath,
        source,
        callback,
        call,
        args[1]?.expression,
        offset,
        consumerRoute,
      ),
    );
  }
  return programs;
}

function strictCallbackProgram(
  sourcePath: string,
  source: RawUnusedSource,
  callback: Record<string, unknown>,
  call: Record<string, unknown>,
  dataExpression: Record<string, unknown> | undefined,
  offset: (at: number) => number,
  consumerRoute: readonly { sourcePath: string; sourceSha256: string }[],
): BrowserUnusedProgram {
  const span = astSpan.parse(callback.span),
    start = offset(span.start),
    end = offset(span.end);
  // Promise<object型>内の型注釈は親型の消去に含まれる。部分交差は既存の検査で拒否する。
  const removals = callbackTypeRemovals(callback, offset).filter(
    (range, index, ranges) =>
      !ranges
        .slice(0, index)
        .some((parent) => parent.start <= range.start && parent.end >= range.end),
  );
  const data = dataExpression ? astSpan.parse(dataExpression.span) : null;
  return {
    sourcePath,
    sourceSha256: source.sha256,
    role: "callback",
    mode: "strictVendorOriginal",
    start,
    end,
    removals,
    data: data ? source.code.slice(offset(data.start), offset(data.end)) : null,
    runtimeAstSha256: createHash("sha256")
      .update(eraseCallbackTypes(source.code, start, end, removals))
      .digest("hex"),
    callAstSha256: strictSemanticSha256(call),
    consumerRoute,
  };
}

function verifyOfficialStrictCallers(raw: ReadonlyMap<string, RawUnusedSource>) {
  for (const [sourcePath, methods] of Object.entries(officialStrictCallerMethods)) {
    const source = raw.get(sourcePath);
    if (!source) throw new Error("missing official strict caller");
    const nodes = browserAstNodes(parseSync(source.code, { syntax: "typescript" }));
    for (const [method, sha256] of Object.entries(methods)) {
      const owners = nodes.filter(
        ({ node, method: name }) =>
          ["ClassMethod", "MethodProperty"].includes(String(node.type)) && name === method,
      );
      if (owners.length !== 1 || !owners[0] || strictSemanticSha256(owners[0].node) !== sha256)
        throw new Error(`official strict caller call/branch changed: ${sourcePath}:${method}`);
    }
  }
}

export function inspectOfficialStrictPrograms(
  raw: ReadonlyMap<string, RawUnusedSource>,
): BrowserUnusedProgram[] {
  inspectSharedSvgCaptureProgram(raw);
  const consumerRoute = officialStrictConsumerRoute.map((binding, index) => {
    const source = raw.get(binding.sourcePath);
    if (
      !source ||
      physicalUnusedSource(Buffer.from(source.code)).sha256 !== source.sha256 ||
      (index >= sharedSvgCaptureRoute.length && source.sha256 !== binding.sourceSha256)
    )
      throw new Error(`missing/stale official strict physical source: ${binding.sourcePath}`);
    return { sourcePath: binding.sourcePath, sourceSha256: source.sha256 };
  });
  const renderer = raw.get(rendererSource),
    capture = raw.get(strictCaptureSource),
    images = raw.get(strictImageSource);
  if (!renderer || !capture || !images)
    throw new Error("missing official strict callback producers");
  const vendor = inspectOfficialUnusedCallbacks(rendererSource, renderer.code).filter(
    (callback) => callback.method === "renderVendorOriginalSvg",
  );
  if (vendor.length !== 1 || !vendor[0]) throw new Error("unknown official strict vendor callback");
  const programs: BrowserUnusedProgram[] = [
    {
      ...vendor[0],
      sourcePath: rendererSource,
      sourceSha256: renderer.sha256,
      role: "callback",
      consumerRoute,
    },
    ...strictHelperCallbacks(strictCaptureSource, capture, consumerRoute),
    ...strictHelperCallbacks(strictImageSource, images, consumerRoute),
  ];
  verifyOfficialStrictCallers(raw);
  for (const binding of officialStrictConsumerRoute)
    if (!raw.has(binding.sourcePath))
      throw new Error(`official strict owned source pin changed: ${binding.sourcePath}`);
  if (programs.length !== 5) throw new Error("unknown official strict five callback schedule");
  return programs.map((program) => {
    const validated = officialStrictProgramSchema.parse(program);
    if (JSON.stringify(validated.consumerRoute) !== JSON.stringify(consumerRoute))
      throw new Error("official strict source route differs from live physical inputs");
    return validated;
  });
}

export function officialStrictUnusedContext(
  root: string,
  raw: Map<string, RawUnusedSource>,
  inputs: OfficialStrictInputOptions,
): RuntimeUnusedContext {
  for (const binding of officialStrictConsumerRoute) {
    const source = readUnusedPhysicalSource(root, binding.sourcePath),
      previous = raw.get(binding.sourcePath);
    if (previous && previous.sha256 !== source.sha256)
      throw new Error("changed official strict owned input");
    raw.set(binding.sourcePath, source);
  }
  const programs = inspectOfficialStrictPrograms(raw);
  const selected = resolveOfficialRendererAssetInputs(
    inputs.argv,
    inputs.env,
    inputs.cwd,
    inputs.home,
  );
  const externalInputs = Object.freeze([
    officialStrictExternalInput("mermaidJs", selected.mermaidJs),
    officialStrictExternalInput("mermaidZenumlJs", selected.mermaidZenumlJs),
  ]);
  verifyOfficialStrictExternalInputs(externalInputs);
  const { code, map } = browserLintProjection(programs, raw);
  return {
    name: "official-mermaid-strict-browser",
    stages: ["assembled"],
    programs,
    externalInputs,
    codeBytesEqual: false,
    origin: new BrowserUnusedOrigin(code, map, programs, raw),
  };
}

const nativeWidthBase = "crates/katana-render-runtime/src/markdown/mermaid_renderer/";
const nativeWidthHelper = `${nativeWidthBase}js_runtime_native_width_helpers.rs`;
const nativeWidthTrace = `${nativeWidthBase}js_runtime_native_width_trace.js`;
const nativeWidthBBoxTrace = `${nativeWidthBase}js_runtime_native_width_bbox_trace.js`;
const nativeWidthV8Script = "crates/katana-render-runtime/src/v8_script.rs";
const nativeWidthLibrary = "crates/katana-render-runtime/src/lib.rs";
const nativeWidthDiagramRuntime = "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs";
const nativeWidthDiagramProjectionSha256 =
  "7c8f590e31555dba41d4536163fce4a68cf73cd5686ecd79ae2a917451811752";
const nativeWidthV8ScriptSha256 =
  "5ebae43ec806441146dca8edca9a8b9b04b3d259ea0dfb966827f7bee90464dc";
const nativeWidthV8Import = "use crate::v8_script::ScriptCompiler;";
const nativeWidthV8CompileCall = "v8::Script::compile(scope, source, Some(&origin))";
const nativeWidthV8CompilerCall = "ScriptCompiler::compile_with_origin(scope, source, origin_name)";
const nativeWidthLegacyRoute = {
  [nativeWidthHelper]: "fe94f65de29ced98302d2174d7fb212a6526074601fc0019f8894c5b9e49b85d",
  [`${nativeWidthBase}js_runtime_native_width_tests.rs`]:
    "544cc51402f5b518ef02b722cb46ea11d258c7449b9271abf2e31402ec2e6e81",
  [nativeWidthTrace]: "7f16ff359f8964f894e34a1d1cff49964eb06c3607c40fd6c4231a244e01bc8b",
  [nativeWidthBBoxTrace]: "a1c72bb7a16e809b52fbd98a79a079749e4ceb1154ada17781b1007ede8a64cf",
} as const;
const nativeWidthRoute = [
  ...new Set([
    ...Object.keys(nativeWidthLegacyRoute),
    ...NATIVE_WIDTH_HASH_INPUT_PATHS,
    nativeWidthDiagramRuntime,
    nativeWidthLibrary,
    nativeWidthV8Script,
    ...RUST_CLASSIC_INPUT_PATHS,
  ]),
];

function exactSingle(source: string, value: string, label: string) {
  const first = source.indexOf(value);
  if (first < 0 || source.indexOf(value, first + value.length) >= 0)
    throw new Error(`changed native width consumer: ${label}`);
  return first;
}

function inspectNativeWidthV8Route(
  diagramSource: string,
  librarySource: string,
  v8ScriptSource: string,
) {
  const renderStart = exactSingle(
    diagramSource,
    "    pub(crate) fn render(",
    nativeWidthDiagramRuntime,
  );
  const ensureStart = exactSingle(
    diagramSource,
    "    pub(crate) fn ensure_initialized(",
    nativeWidthDiagramRuntime,
  );
  const evaluateStart = exactSingle(diagramSource, "fn evaluate(", nativeWidthDiagramRuntime);
  const resolveStart = exactSingle(diagramSource, "fn resolve_value(", nativeWidthDiagramRuntime);
  if (ensureStart <= renderStart || resolveStart <= evaluateStart)
    throw new Error(`changed native width consumer: ${nativeWidthDiagramRuntime}`);
  const projection =
    diagramSource.slice(renderStart, ensureStart) +
    diagramSource.slice(evaluateStart, resolveStart);
  exactSingle(diagramSource, nativeWidthV8CompilerCall, nativeWidthDiagramRuntime);
  if (
    createHash("sha256").update(projection).digest("hex") !== nativeWidthDiagramProjectionSha256 ||
    diagramSource.includes("v8::ScriptOrigin::new(") ||
    diagramSource.includes("v8::Script::compile(")
  )
    throw new Error(`changed native width consumer: ${nativeWidthDiagramRuntime}`);

  if (librarySource.match(/^mod v8_script;$/gmu)?.length !== 1)
    throw new Error(`changed native width consumer: ${nativeWidthLibrary}`);
  exactSingle(diagramSource, nativeWidthV8Import, nativeWidthDiagramRuntime);
  exactSingle(v8ScriptSource, nativeWidthV8CompileCall, nativeWidthV8Script);
  exactSingle(v8ScriptSource, "v8::ScriptOrigin::new(", nativeWidthV8Script);
}
// 実test登録→trace挿入→result追加→単一V8Contextの順をレビュー済み経路に限定する。
// 経路変更時はsource-mapの成功だけで実consumerを認定せず再レビューする。
export function inspectNativeWidthResultLiteral(sourcePath: string, code: string) {
  if (sourcePath !== nativeWidthHelper) throw new Error("unknown native width result producer");
  // Rustの一般構文を推論せず、実無escape literalだけを物理範囲へ対応させる。
  const prefix = "fn result_script() -> &'static str {\n    \"";
  const start = code.indexOf(prefix) + prefix.length;
  if (start < prefix.length || code.indexOf(prefix, start) !== -1)
    throw new Error("missing/unknown native width result literal");
  const end = code.indexOf('"\n}', start);
  const literal = code.slice(start, end);
  if (
    end < start ||
    /["\\\r\n]/u.test(literal) ||
    literal !==
      "if (globalThis.__krrActualBBoxTraceOverflow) throw new Error('native width getBBox trace overflow'); if (globalThis.__krrActualBBoxTraceFailure) throw new Error('native width getBBox snapshot failed: ' + globalThis.__krrActualBBoxTraceFailure); JSON.stringify({ calls: globalThis.__krrActualWidths, boxes: globalThis.__krrActualBBoxes, probes: nativeWidthProbeResult() })"
  )
    throw new Error("unknown native width result literal shape");
  parseSync(literal, { syntax: "ecmascript" });
  return { start, end };
}

export function nativeWidthUnusedContext(
  root: string,
  raw: Map<string, RawUnusedSource>,
): RuntimeUnusedContext {
  const sources = new Map<string, RawUnusedSource>();
  for (const sourcePath of nativeWidthRoute) {
    const source = readUnusedPhysicalSource(root, sourcePath);
    const expected = nativeWidthLegacyRoute[sourcePath as keyof typeof nativeWidthLegacyRoute];
    if (expected && source.sha256 !== expected)
      throw new Error(`changed native width consumer: ${sourcePath}`);
    if (sourcePath === nativeWidthV8Script && source.sha256 !== nativeWidthV8ScriptSha256)
      throw new Error(`changed native width consumer: ${sourcePath}`);
    const previous = raw.get(sourcePath);
    if (previous && previous.sha256 !== source.sha256)
      throw new Error("changed native width input");
    raw.set(sourcePath, source);
    sources.set(sourcePath, source);
  }
  const diagram = sources.get(nativeWidthDiagramRuntime);
  const library = sources.get(nativeWidthLibrary);
  const v8Script = sources.get(nativeWidthV8Script);
  if (!diagram || !library || !v8Script) throw new Error("missing native width physical input");
  try {
    inspectNativeWidthConsumerFacts(root, sources);
  } catch (error) {
    throw new Error(`changed native width consumer: ${String(error)}`);
  }
  inspectNativeWidthV8Route(diagram.code, library.code, v8Script.code);
  const route = nativeWidthRoute.map((sourcePath) => {
    const source = sources.get(sourcePath);
    if (!source) throw new Error("missing native width physical input");
    return { sourcePath, sourceSha256: source.sha256 };
  });
  const helper = raw.get(nativeWidthHelper),
    trace = raw.get(nativeWidthTrace),
    bboxTrace = raw.get(nativeWidthBBoxTrace);
  if (!helper || !trace || !bboxTrace) throw new Error("missing native width physical input");
  const programs: BrowserUnusedProgram[] = [
    {
      sourcePath: nativeWidthTrace,
      sourceSha256: trace.sha256,
      role: "native-test-trace",
      start: 0,
      end: trace.code.length,
      mode: "rust-native-test",
      removals: [],
      data: null,
      consumerRoute: route,
    },
    {
      sourcePath: nativeWidthBBoxTrace,
      sourceSha256: bboxTrace.sha256,
      role: "native-test-trace",
      start: 0,
      end: bboxTrace.code.length,
      mode: "rust-native-test",
      removals: [],
      data: null,
      consumerRoute: route,
    },
    {
      sourcePath: nativeWidthHelper,
      sourceSha256: helper.sha256,
      role: "rust-result-literal",
      ...inspectNativeWidthResultLiteral(nativeWidthHelper, helper.code),
      mode: "rust-native-test",
      removals: [],
      data: null,
      consumerRoute: route,
    },
  ];
  const { code, map } = browserLintProjection(programs, raw, "rust-native-width");
  return {
    name: "rust-native-width-test-result",
    stages: ["assembled"],
    programs,
    codeBytesEqual: false,
    origin: new BrowserUnusedOrigin(code, map, programs, raw, "rust-native-width"),
  };
}

export function nodeFunctionUnusedContext(
  root: string,
  raw: Map<string, RawUnusedSource>,
): RuntimeUnusedContext {
  const sources = nodeFunctionRoute.map((binding) => {
    const source = readUnusedPhysicalSource(root, binding.sourcePath);
    raw.set(binding.sourcePath, source);
    return source;
  });
  const producer = sources[0],
    resource = sources[1];
  if (!producer || !resource) throw new Error("missing node Function physical sources");
  const programs = inspectNodeFunctionPrograms(producer, resource);
  const { code, map } = browserLintProjection(programs, raw, "node-function");
  const runtimeAstSha256 = nodeFunctionRuntimeAst(programs, raw, code);
  const bound = programs.map((program) => ({ ...program, runtimeAstSha256 }));
  return {
    name: "node-function-static-load-test",
    stages: ["assembled"],
    programs: bound,
    origin: new BrowserUnusedOrigin(code, map, bound, raw, "node-function"),
    codeBytesEqual: false,
  };
}

export { inspectNodeFunctionPrograms } from "./runtime-unused-origin";

export function sharedSvgCaptureUnusedContext(
  root: string,
  raw: Map<string, RawUnusedSource>,
): RuntimeUnusedContext {
  for (const binding of sharedSvgCaptureRoute)
    raw.set(binding.sourcePath, readUnusedPhysicalSource(root, binding.sourcePath));
  const programs = [inspectSharedSvgCaptureProgram(raw)];
  const { code, map } = browserLintProjection(programs, raw, "shared-svg-capture");
  return {
    name: "shared-svg-capture-sizing",
    stages: ["assembled"],
    programs,
    codeBytesEqual: false,
    origin: new BrowserUnusedOrigin(code, map, programs, raw, "shared-svg-capture"),
  };
}

export function sequenceVmUnusedContext(
  root: string,
  raw = new Map<string, RawUnusedSource>(),
): RuntimeUnusedContext {
  for (const sourcePath of sequenceVmPaths)
    raw.set(sourcePath, readUnusedPhysicalSource(root, sourcePath));
  const programs = inspectSequenceVmPrograms(raw);
  const { code, map } = browserLintProjection(programs, raw, "sequence-vm-test");
  return {
    name: "sequence-vm-test",
    stages: ["assembled"],
    programs,
    codeBytesEqual: false,
    origin: new BrowserUnusedOrigin(code, map, programs, raw, "sequence-vm-test"),
  };
}
