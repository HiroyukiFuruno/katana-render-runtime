import { createHash } from "node:crypto";
import { parseSync } from "@swc/core";
import { z } from "zod";
import { type RuntimeUnusedContext, readUnusedPhysicalSource } from "./runtime-unused-contexts";
import { astObject, astSpan, browserAstNodes, RuntimeUnusedOrigin } from "./runtime-unused-origin";
import { Lines, mapIndex, sourceMapSchema } from "./runtime-unused-source-map";

const base = "crates/katana-render-runtime/src/markdown/diagram_runtime/source";
export const entrypointOwnerPaths = [
  `${base}/mermaid/runtime_entrypoint.ts`,
  `${base}/mermaid/runtime_definition.ts`,
  `${base}/shared/runtime_bundle.ts`,
  "scripts/runtime-bundles/runtime-bundle-definitions.ts",
  "package.json",
] as const;
const hash = z.string().regex(/^[a-f0-9]{64}$/u);
const span = z
  .object({ start: z.number().int().nonnegative(), end: z.number().int().positive() })
  .strict();
export const entrypointOwnerSchema = z
  .object({
    kind: z.literal("owned-mermaid-template"),
    definition: z.literal("mermaid"),
    outputFile: z.literal("mermaid-runtime.min.js"),
    generatedSource: z.literal("@runtime/entrypoint/mermaid"),
    rawSourcePath: z.null(),
    exportSymbol: z.literal("MERMAID_RUNTIME_ENTRYPOINT"),
    pins: z.array(z.object({ sourcePath: z.string(), sourceSha256: hash }).strict()).length(5),
    exportSpan: span,
    registrySpan: span,
    literalSpan: span,
    constructorSpan: span,
    argumentSpan: span,
    cookedSha256: hash,
    generatedValueSha256: hash,
  })
  .strict();
export type EntrypointOwner = z.infer<typeof entrypointOwnerSchema>;
export const generatedEntrypointReadSchema = z
  .object({
    kind: z.literal("generated-entrypoint"),
    rawSourcePath: z.null(),
    generatedStart: span.shape.start,
    generatedEnd: span.shape.end,
    entryValueStart: span.shape.start,
    entryValueEnd: span.shape.end,
    generatedValueSha256: hash,
    owner: entrypointOwnerSchema,
  })
  .strict();

const sha = (value: string) => createHash("sha256").update(value).digest("hex");
function utf16Span(code: string, value: unknown) {
  const selected = astSpan.parse(value),
    bytes = Buffer.from(code);
  const offset = (at: number) =>
    new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, at - 1)).length;
  return { start: offset(selected.start), end: offset(selected.end) };
}

function ownedExport(code: string, name: string) {
  const module = parseSync(code, { syntax: "typescript" });
  const exports = module.body.filter(
    (node) => node.type === "ExportDeclaration" && node.declaration.type === "VariableDeclaration",
  );
  const declarations = exports.flatMap((node) =>
    node.type === "ExportDeclaration" && node.declaration.type === "VariableDeclaration"
      ? node.declaration.declarations
      : [],
  );
  const matches = declarations.filter(
    (node) => node.id.type === "Identifier" && node.id.value === name,
  );
  if (
    matches.length !== 1 ||
    declarations.length !== 1 ||
    exports.length !== 1 ||
    exports[0]?.type !== "ExportDeclaration" ||
    exports[0].declaration.type !== "VariableDeclaration" ||
    exports[0].declaration.kind !== "const"
  )
    throw new Error("unknown owned export multiplicity");
  return matches[0];
}

function ownedTemplate(code: string) {
  if (parseSync(code, { syntax: "typescript" }).body.length !== 1)
    throw new Error("unknown owned entrypoint module shape");
  const declaration = ownedExport(code, "MERMAID_RUNTIME_ENTRYPOINT");
  if (
    declaration?.init?.type !== "TemplateLiteral" ||
    declaration.init.expressions.length !== 0 ||
    declaration.init.quasis.length !== 1
  )
    throw new Error("unknown owned entrypoint template/export");
  const value = declaration.init.quasis[0]?.cooked;
  if (typeof value !== "string") throw new Error("missing owned entrypoint cooked value");
  return {
    value,
    exportSpan: utf16Span(code, astObject.parse(declaration.id).span),
    literalSpan: utf16Span(code, declaration.init.span),
  };
}

function definitionExpressions(module: ReturnType<typeof parseSync>, fragments: unknown) {
  const constant = module.body[3],
    helper = module.body[5];
  if (
    constant?.type !== "VariableDeclaration" ||
    constant.kind !== "const" ||
    constant.declarations.length !== 1 ||
    constant.declarations[0]?.id.type !== "Identifier" ||
    constant.declarations[0].id.value !== "MERMAID_JS_RUNTIME" ||
    constant.declarations[0].init?.type !== "StringLiteral" ||
    helper?.type !== "FunctionDeclaration" ||
    helper.async ||
    helper.generator ||
    helper.params.length !== 0 ||
    helper.body?.stmts.length !== 1 ||
    helper.body.stmts[0]?.type !== "ReturnStatement"
  )
    throw new Error("unknown owned fragment factory shape");
  const array = astObject.parse(fragments),
    elements = z.array(astObject).parse(array.elements);
  if (
    array.type !== "ArrayExpression" ||
    elements.length !== 2 ||
    elements.some((e) => e.spread === null)
  )
    throw new Error("unknown owned fragment composition");
  const calls = elements.map((e) => astObject.parse(e.expression));
  const first = calls[0],
    second = calls[1];
  if (
    first?.type !== "CallExpression" ||
    second?.type !== "CallExpression" ||
    z.array(astObject).parse(first.arguments).length !== 0 ||
    z.array(astObject).parse(second.arguments).length !== 0 ||
    astObject.parse(first.callee).type !== "MemberExpression" ||
    astObject.parse(astObject.parse(first.callee).object).type !== "Identifier" ||
    astObject.parse(astObject.parse(first.callee).object).value !== "SharedDomFragments" ||
    astObject.parse(astObject.parse(first.callee).property).type !== "Identifier" ||
    astObject.parse(astObject.parse(first.callee).property).value !== "all" ||
    astObject.parse(second.callee).type !== "Identifier" ||
    astObject.parse(second.callee).value !== "mermaidRuntimeFragments"
  )
    throw new Error("unknown owned fragment composition calls");
  const returned = astObject.parse(helper.body.stmts[0].argument);
  if (returned.type !== "ArrayExpression") throw new Error("unknown owned fragment inventory");
  for (const element of z.array(astObject).parse(returned.elements)) {
    const call = astObject.parse(element.expression),
      member = astObject.parse(call.callee);
    const args = z.array(astObject).parse(call.arguments);
    const label = astObject.parse(args[0]?.expression),
      source = astObject.parse(args[1]?.expression);
    if (
      element.spread !== null ||
      call.type !== "CallExpression" ||
      member.type !== "MemberExpression" ||
      astObject.parse(member.object).type !== "Identifier" ||
      astObject.parse(member.object).value !== "RuntimeFragmentFactory" ||
      astObject.parse(member.property).type !== "Identifier" ||
      astObject.parse(member.property).value !== "verbatim" ||
      args.length !== 2 ||
      args.some((a) => a.spread !== null) ||
      label.type !== "StringLiteral" ||
      source.type !== "TemplateLiteral" ||
      z.array(astObject).parse(source.expressions).length !== 1 ||
      astObject.parse(z.array(astObject).parse(source.expressions)[0]).type !== "Identifier" ||
      astObject.parse(z.array(astObject).parse(source.expressions)[0]).value !==
        "MERMAID_JS_RUNTIME" ||
      z.array(astObject).parse(source.quasis).length !== 2
    )
      throw new Error("unknown owned fragment call expression");
    calls.push(call);
  }
  const spans = calls.map((call) => astSpan.parse(call.span).start);
  if (
    browserAstNodes(module).some(
      ({ node }) =>
        node.type === "CallExpression" && !spans.includes(astSpan.parse(node.span).start),
    )
  )
    throw new Error("unknown owned definition call expression");
}

function constructorBinding(code: string, shared: string) {
  const module = parseSync(code, { syntax: "typescript" });
  if (
    module.body.length !== 6 ||
    module.body.slice(0, 3).some((node) => node.type !== "ImportDeclaration") ||
    module.body[3]?.type !== "VariableDeclaration" ||
    module.body[4]?.type !== "ExportDeclaration" ||
    module.body[5]?.type !== "FunctionDeclaration" ||
    module.body[5].identifier?.value !== "mermaidRuntimeFragments"
  )
    throw new Error("unknown owned definition module shape");
  const factory = ownedExport(code, "MermaidRuntimeBundleDefinition");
  if (factory?.init?.type !== "ObjectExpression")
    throw new Error("unknown owned definition factory export");
  const method = factory.init.properties[0];
  if (
    factory.init.properties.length !== 1 ||
    method?.type !== "MethodProperty" ||
    method.key.type !== "Identifier" ||
    method.key.value !== "create" ||
    method.params.length !== 0 ||
    method.async ||
    method.generator ||
    method.body?.stmts.length !== 1 ||
    method.body.stmts[0]?.type !== "ReturnStatement"
  )
    throw new Error("unknown owned definition factory return");
  const call = astObject.parse(method.body.stmts[0].argument);
  if (
    call.type !== "NewExpression" ||
    astObject.parse(call.callee).type !== "Identifier" ||
    astObject.parse(call.callee).value !== "RuntimeBundleDefinition"
  )
    throw new Error("unknown owned definition constructor");
  const argumentsList = z.array(astObject).parse(call.arguments);
  if (argumentsList.some((argument) => argument.spread !== null))
    throw new Error("unknown owned definition constructor spread");
  const args = argumentsList.map((argument) => astObject.parse(argument.expression));
  if (
    args.length !== 4 ||
    args[0]?.type !== "StringLiteral" ||
    args[0].value !== "mermaid" ||
    args[1]?.type !== "StringLiteral" ||
    args[1].value !== "mermaid-runtime.min.js" ||
    args[3]?.type !== "Identifier" ||
    args[3].value !== "MERMAID_RUNTIME_ENTRYPOINT"
  )
    throw new Error("owned definition constructor argument differs");
  definitionExpressions(module, args[2]);
  rejectOwnedWrites(module);
  verifySharedConstructor(shared);
  return {
    constructorSpan: utf16Span(code, astObject.parse(call.callee).span),
    argumentSpan: utf16Span(code, args[3].span),
  };
}

function verifySharedConstructor(shared: string) {
  const sharedModule = parseSync(shared, { syntax: "typescript" });
  if (
    sharedModule.body.some(
      (node) =>
        node.type !== "ExportDeclaration" ||
        ![
          "TsTypeAliasDeclaration",
          "TsInterfaceDeclaration",
          "ClassDeclaration",
          "VariableDeclaration",
        ].includes(node.declaration.type),
    )
  )
    throw new Error("unknown owned constructor module shape");
  verifySharedFragments(shared);
  rejectOwnedWrites(sharedModule);
  if (
    browserAstNodes(sharedModule).some(({ node }) =>
      ["CallExpression", "NewExpression", "AwaitExpression", "YieldExpression"].includes(
        String(node.type),
      ),
    )
  )
    throw new Error("unknown owned constructor expression");
  const classes = sharedModule.body.flatMap((node) =>
    node.type === "ExportDeclaration" && node.declaration.type === "ClassDeclaration"
      ? [node.declaration]
      : [],
  );
  if (
    classes.length !== 1 ||
    classes[0]?.identifier?.value !== "RuntimeBundleDefinition" ||
    classes[0].superClass !== null ||
    classes[0].body.length !== 1 ||
    classes[0].body[0]?.type !== "Constructor"
  )
    throw new Error("unknown owned constructor class shape");
  const constructors = browserAstNodes(sharedModule).filter(
    ({ node }) => node.type === "Constructor",
  );
  const ctor = constructors[0]?.node;
  if (
    constructors.length !== 1 ||
    !ctor ||
    z.array(z.unknown()).parse(astObject.parse(ctor.body).stmts).length !== 0
  )
    throw new Error("owned constructor mutates entrypoint");
  verifyConstructorParameters(ctor.params);
}

function verifyConstructorParameters(value: unknown) {
  const parameters = z.array(astObject).parse(value);
  const names = ["name", "outputFile", "fragments", "entrypoint", "bundledEntryPath"];
  if (parameters.length !== names.length)
    throw new Error("owned constructor property inventory differs");
  for (const [index, parameter] of parameters.entries()) {
    if (parameter.type !== "TsParameterProperty" || parameter.readonly !== true)
      throw new Error("owned constructor readonly property mapping differs");
    if (index === 4) verifyClassicDefault(parameter);
    else {
      const binding = astObject.parse(parameter.param);
      if (binding.type !== "Identifier" || binding.value !== names[index])
        throw new Error("owned constructor property name/order/default differs");
    }
  }
}

function verifyClassicDefault(parameter: unknown) {
  const property = astObject.parse(parameter),
    pattern = astObject.parse(property.param);
  if (
    property.type !== "TsParameterProperty" ||
    property.readonly !== true ||
    pattern.type !== "AssignmentPattern" ||
    astObject.parse(pattern.left).type !== "Identifier" ||
    astObject.parse(pattern.left).value !== "bundledEntryPath" ||
    astObject.parse(pattern.right).type !== "NullLiteral"
  )
    throw new Error("owned constructor classic default differs");
}

function verifySharedFragments(shared: string) {
  const factory = ownedExport(shared, "RuntimeFragmentFactory");
  if (factory?.init?.type !== "ObjectExpression" || factory.init.properties.length !== 2)
    throw new Error("unknown owned shared fragment export");
  for (const [index, method] of factory.init.properties.entries()) {
    if (
      method.type !== "MethodProperty" ||
      method.key.type !== "Identifier" ||
      method.key.value !== ["verbatim", "transformed"][index] ||
      method.async ||
      method.generator ||
      method.params.length !== [2, 3][index] ||
      method.params.some(
        (parameter, at) =>
          parameter.pat.type !== "Identifier" ||
          parameter.pat.value !== ["label", "sourcePath", "transform"][at] ||
          (parameter.decorators?.length ?? 0) !== 0,
      ) ||
      method.body?.stmts.length !== 1 ||
      method.body.stmts[0]?.type !== "ReturnStatement" ||
      method.body.stmts[0].argument?.type !== "ObjectExpression"
    )
      throw new Error("unknown owned shared fragment method");
    const properties = method.body.stmts[0].argument.properties;
    if (
      properties.length !== 3 ||
      properties.some((property, at) => {
        const key = ["label", "sourcePath", "transform"][at];
        if (property.type === "Identifier")
          return (at === 2 && index === 0) || property.value !== key;
        return (
          index !== 0 ||
          at !== 2 ||
          property.type !== "KeyValueProperty" ||
          property.key.type !== "Identifier" ||
          property.key.value !== "transform" ||
          property.value.type !== "StringLiteral" ||
          property.value.value !== "verbatim"
        );
      })
    )
      throw new Error("unknown owned shared fragment return");
  }
}

function rejectOwnedWrites(module: ReturnType<typeof parseSync>) {
  if (
    browserAstNodes(module).some(
      ({ node }) =>
        node.type === "AssignmentExpression" ||
        node.type === "UpdateExpression" ||
        (node.type === "UnaryExpression" && node.operator === "delete"),
    )
  )
    throw new Error("owned definition has an unsupported mutation");
}

function registryBinding(code: string) {
  const module = parseSync(code, { syntax: "typescript" });
  if (
    module.body.length !== 6 ||
    module.body.slice(0, 5).some((node) => node.type !== "ImportDeclaration") ||
    module.body[5]?.type !== "ExportDeclaration"
  )
    throw new Error("unknown owned registry module shape");
  const entry = module.body.find(
    (node) =>
      node.type === "ExportDeclaration" &&
      node.declaration.type === "FunctionDeclaration" &&
      node.declaration.identifier?.value === "runtimeBundleDefinitions",
  );
  if (
    entry?.type !== "ExportDeclaration" ||
    entry.declaration.type !== "FunctionDeclaration" ||
    entry.declaration.params.length !== 0 ||
    entry.declaration.async ||
    entry.declaration.generator ||
    entry.declaration.body?.stmts.length !== 1 ||
    entry.declaration.body.stmts[0]?.type !== "ReturnStatement"
  )
    throw new Error("owned registry factory return differs");
  const array = astObject.parse(entry.declaration.body.stmts[0].argument);
  if (array.type !== "ArrayExpression") throw new Error("owned registry factory inventory differs");
  const elements = z.array(astObject).parse(array.elements);
  if (elements.length !== 4 || elements.some((element) => element.spread !== null))
    throw new Error("owned registry factory inventory differs");
  const expected = [
    "MermaidRuntimeBundleDefinition",
    "DrawioRuntimeBundleDefinition",
    "ZenumlRuntimeBundleDefinition",
    "MathJaxRuntimeBundleDefinition",
  ];
  for (const [index, element] of elements.entries()) {
    const call = astObject.parse(element.expression),
      member = astObject.parse(call.callee);
    if (
      call.type !== "CallExpression" ||
      z.array(astObject).parse(call.arguments).length !== 0 ||
      member.type !== "MemberExpression" ||
      astObject.parse(member.object).type !== "Identifier" ||
      astObject.parse(member.object).value !== expected[index] ||
      astObject.parse(member.property).type !== "Identifier" ||
      astObject.parse(member.property).value !== "create"
    )
      throw new Error("unknown owned registry call expression");
  }
  const element = z.array(astObject).parse(array.elements)[0],
    call = astObject.parse(element?.expression),
    member = astObject.parse(call.callee),
    object = astObject.parse(member.object);
  if (
    call.type !== "CallExpression" ||
    z.array(astObject).parse(call.arguments).length !== 0 ||
    member.type !== "MemberExpression" ||
    object.type !== "Identifier" ||
    object.value !== "MermaidRuntimeBundleDefinition" ||
    astObject.parse(member.property).value !== "create"
  )
    throw new Error("owned registry Mermaid factory differs");
  return utf16Span(code, object.span);
}

export function readEntrypointOwner(root: string): EntrypointOwner {
  const sources = entrypointOwnerPaths.map((name) => readUnusedPhysicalSource(root, name));
  const producer = sources[0],
    definitionSource = sources[1],
    shared = sources[2],
    registry = sources[3],
    packageSource = sources[4];
  if (!producer || !definitionSource || !shared || !registry || !packageSource)
    throw new Error("missing owned entrypoint source");
  const template = ownedTemplate(producer.code),
    constructorFacts = constructorBinding(definitionSource.code, shared.code);
  z.object({
    imports: z.object({
      "#mermaid/*": z.literal(`./${base}/mermaid/*.ts`),
      "#shared/*": z.literal(`./${base}/shared/*.ts`),
    }),
  }).parse(JSON.parse(packageSource.code));
  const registrySpan = registryBinding(registry.code);
  return entrypointOwnerSchema.parse({
    kind: "owned-mermaid-template",
    definition: "mermaid",
    outputFile: "mermaid-runtime.min.js",
    generatedSource: "@runtime/entrypoint/mermaid",
    rawSourcePath: null,
    exportSymbol: "MERMAID_RUNTIME_ENTRYPOINT",
    pins: entrypointOwnerPaths.map((sourcePath, index) => ({
      sourcePath,
      sourceSha256: sources[index]?.sha256,
    })),
    exportSpan: template.exportSpan,
    registrySpan,
    literalSpan: template.literalSpan,
    ...constructorFacts,
    cookedSha256: sha(template.value),
    generatedValueSha256: sha(template.value.trim()),
  });
}

export function generatedEntrypointRead(
  root: string,
  context: RuntimeUnusedContext,
  stage: "assembled" | "normalized",
  generatedStart: number,
  generatedEnd: number,
) {
  if (
    !(context.origin instanceof RuntimeUnusedOrigin) ||
    context.name !== "mermaid" ||
    !context.stages.includes(stage)
  )
    throw new Error("unknown generated entrypoint realm/stage");
  const owner = readEntrypointOwner(root),
    provenance = context.origin.provenance,
    normalization = sourceMapSchema.parse(JSON.parse(provenance.normalizationMap));
  const assembled = normalization.sourcesContent[0];
  if (
    normalization.sources.join("\0") !== "@runtime/assembled" ||
    normalization.sourcesContent.length !== 1 ||
    assembled === undefined
  )
    throw new Error("unknown generated entrypoint normalization");
  const current = new RuntimeUnusedOrigin(structuredClone(provenance), context.origin.raw);
  const entry = current.provenance.entrypoint,
    value = readUnusedPhysicalSource(root, entrypointOwnerPaths[0]).code;
  const trimmed = ownedTemplate(value).value.trim();
  if (
    entry.rawSourcePath !== null ||
    entry.definition !== owner.definition ||
    entry.source !== owner.generatedSource ||
    entry.sourceSha256 !== owner.generatedValueSha256 ||
    assembled.slice(entry.assembledRange.start, entry.assembledRange.end) !== trimmed
  )
    throw new Error("generated entrypoint owner bytes differ");
  const table =
    stage === "normalized"
      ? mapIndex(
          normalization,
          new Lines(provenance.code),
          new Map([["@runtime/assembled", new Lines(assembled)]]),
        )
      : null;
  const start = table?.get(generatedStart)?.offset ?? generatedStart;
  if (
    generatedStart >= generatedEnd ||
    start < entry.assembledRange.start ||
    start + generatedEnd - generatedStart > entry.assembledRange.end
  )
    throw new Error("generated read outside entrypoint range");
  for (let at = generatedStart; at < generatedEnd; at++) {
    const link = table?.get(at);
    if (
      table &&
      (link?.source !== "@runtime/assembled" || link.offset !== start + at - generatedStart)
    )
      throw new Error("unmapped generated entrypoint read");
  }
  return generatedEntrypointReadSchema.parse({
    kind: "generated-entrypoint",
    rawSourcePath: null,
    generatedStart,
    generatedEnd,
    entryValueStart: start - entry.assembledRange.start,
    entryValueEnd: start - entry.assembledRange.start + generatedEnd - generatedStart,
    generatedValueSha256: owner.generatedValueSha256,
    owner,
  });
}
