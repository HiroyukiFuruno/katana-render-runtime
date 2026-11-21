import { createHash } from "node:crypto";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import {
  isArrayLiteralExpression,
  isBinaryExpression,
  isCallExpression,
  isClassDeclaration,
  isConditionalExpression,
  isElementAccessExpression,
  isFunctionDeclaration,
  isIdentifier,
  isImportDeclaration,
  isImportSpecifier,
  isNewExpression,
  isPropertyAccessExpression,
  isStringLiteral,
  isTypeOfExpression,
  isVariableDeclaration,
  isVariableDeclarationList,
  type Node,
  NodeFlags,
  SyntaxKind,
} from "typescript/unstable/ast";
import { API, SymbolFlags } from "typescript/unstable/sync";
import { z } from "zod";

const range = z
  .object({ start: z.number().int().nonnegative(), end: z.number().int().positive() })
  .strict();
const requestSchema = z
  .object({
    cwd: z.string(),
    config: z.string(),
    file: z.string(),
    codeSha256: z.string().regex(/^[a-f0-9]{64}$/u),
    declarations: z.array(range.extend({ symbol: z.string().min(1) }).strict()),
    vmOwner: z
      .object({
        config: z.string(),
        pins: z
          .array(
            z
              .object({ sourcePath: z.string(), sourceSha256: z.string().regex(/^[a-f0-9]{64}$/u) })
              .strict(),
          )
          .length(2),
        sourceBindingStart: range.shape.start,
        readFileStart: range.shape.start,
        urlConstructorStart: range.shape.start,
        createContextStart: range.shape.start,
        loadCalleeStart: range.shape.start,
        lookupCalleeStart: range.shape.start,
        loadSourceStart: range.shape.start,
        loadContextStart: range.shape.start,
        lookupContextStart: range.shape.start,
        literalSpan: range,
        callerRange: range,
      })
      .strict()
      .optional(),
    owner: z
      .object({
        config: z.string(),
        pins: z
          .array(
            z
              .object({ sourcePath: z.string(), sourceSha256: z.string().regex(/^[a-f0-9]{64}$/u) })
              .strict(),
          )
          .length(5),
        argumentStart: range.shape.start,
        constructorStart: range.shape.start,
        exportStart: range.shape.start,
        registryStart: range.shape.start,
        literalSpan: range,
      })
      .strict()
      .optional(),
  })
  .strict();

function supportedDeclaration(node: Node) {
  return isFunctionDeclaration(node) || isVariableDeclaration(node) || isClassDeclaration(node);
}

function boundedExpressionRead(node: Node, parent: Node): boolean | undefined {
  if (isTypeOfExpression(parent)) return parent.expression === node;
  if (isArrayLiteralExpression(parent)) {
    const owner = parent.parent,
      list = owner?.parent;
    // 配列はdestructuringの書込み先にもなるため、実consumerのconst初期値だけを認める。
    if (
      !parent.elements.some((element) => element === node) ||
      !owner ||
      !isVariableDeclaration(owner) ||
      owner.initializer !== parent ||
      !list ||
      !isVariableDeclarationList(list) ||
      (list.flags & NodeFlags.BlockScoped) !== NodeFlags.Const
    )
      throw new Error("unbounded array value shape");
    return true;
  }
  // property書込みでもreceiver/indexのbinding自体は読み出す。property名はbinding readにしない。
  if (isPropertyAccessExpression(parent)) return parent.expression === node;
  if (isElementAccessExpression(parent))
    return parent.expression === node || parent.argumentExpression === node;
  if (isNewExpression(parent))
    return (
      parent.expression === node || parent.arguments?.some((argument) => argument === node) === true
    );
  if (isConditionalExpression(parent))
    return parent.condition === node || parent.whenTrue === node || parent.whenFalse === node;
  return undefined;
}

function isRead(
  node: Node,
  symbolId: number,
  project: NonNullable<ReturnType<ReturnType<API["updateSnapshot"]>["getProject"]>>,
) {
  const parent = node.parent;
  if (!parent) throw new Error("missing semantic reference parent");
  if (supportedDeclaration(parent) && parent.name === node) return false;
  for (let owner: Node | undefined = parent; owner; owner = owner.parent)
    if (
      supportedDeclaration(owner) &&
      owner.name &&
      project.checker.getSymbolAtLocation(owner.name)?.id === symbolId
    )
      return false;
  if (
    isBinaryExpression(parent) &&
    parent.left === node &&
    parent.operatorToken.kind >= SyntaxKind.FirstAssignment &&
    parent.operatorToken.kind <= SyntaxKind.LastAssignment
  )
    return false;
  const expressionRead = boundedExpressionRead(node, parent);
  if (expressionRead !== undefined) {
    if (!expressionRead) throw new Error("non-value semantic expression reference");
    return true;
  }
  // 未評価の式を使用証明へ昇格させず、現在のKRR consumer形だけを許可する。
  const known = [
    SyntaxKind.CallExpression,
    SyntaxKind.PropertyAssignment,
    SyntaxKind.ReturnStatement,
    SyntaxKind.VariableDeclaration,
    SyntaxKind.BinaryExpression,
  ];
  if (!known.includes(parent.kind)) throw new Error("unbounded semantic read shape");
  return true;
}

function assertEntrypointPins(
  root: string,
  pins: NonNullable<z.infer<typeof requestSchema>["owner"]>["pins"],
) {
  const base = "crates/katana-render-runtime/src/markdown/diagram_runtime/source";
  const names = [
    `${base}/mermaid/runtime_entrypoint.ts`,
    `${base}/mermaid/runtime_definition.ts`,
    `${base}/shared/runtime_bundle.ts`,
    "scripts/runtime-bundles/runtime-bundle-definitions.ts",
    "package.json",
  ];
  if (pins.map((pin) => pin.sourcePath).join("\0") !== names.join("\0"))
    throw new Error("unknown entrypoint owner inventory");
  for (const pin of pins) {
    const file = path.join(root, pin.sourcePath);
    if (
      fs.realpathSync(file) !== file ||
      createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== pin.sourceSha256
    )
      throw new Error("entrypoint owner source changed");
  }
  return names;
}

function inspectEntrypointOwner(
  request: z.infer<typeof requestSchema>,
  snapshot: ReturnType<API["updateSnapshot"]>,
) {
  const owner = request.owner;
  if (!owner) return;
  const names = assertEntrypointPins(request.cwd, owner.pins);
  const project = snapshot.getProject(owner.config);
  if (
    !project ||
    project.program.getSyntacticDiagnostics().length ||
    project.program.getBindDiagnostics().length ||
    names
      .slice(0, 4)
      .some(
        (name) => project.program.getSemanticDiagnostics(path.join(request.cwd, name)).length > 0,
      )
  )
    throw new Error("invalid entrypoint owner project");
  const producer = path.join(request.cwd, names[0] ?? ""),
    definition = path.join(request.cwd, names[1] ?? ""),
    shared = path.join(request.cwd, names[2] ?? "");
  const alias = (at: number, expected: string, target: string, from = definition) => {
    const symbol = project.checker.getSymbolAtPosition(from, at);
    if (!symbol || symbol.name !== expected || !(symbol.flags & SymbolFlags.Alias))
      throw new Error("entrypoint owner import symbol differs");
    const resolved = project.checker.getAliasedSymbol(symbol),
      declaration = resolved.valueDeclaration?.resolve();
    if (
      resolved.name !== expected ||
      resolved.declarations.length !== 1 ||
      !declaration ||
      declaration.getSourceFile().fileName !== target
    )
      throw new Error("entrypoint owner alias target differs");
    return resolved;
  };
  const exported = alias(owner.argumentStart, "MERMAID_RUNTIME_ENTRYPOINT", producer),
    constructorSymbol = alias(owner.constructorStart, "RuntimeBundleDefinition", shared);
  const factory = alias(
    owner.registryStart,
    "MermaidRuntimeBundleDefinition",
    definition,
    path.join(request.cwd, names[3] ?? ""),
  );
  const factoryDeclaration = factory.valueDeclaration?.resolve();
  if (
    !factoryDeclaration ||
    !isVariableDeclaration(factoryDeclaration) ||
    factoryDeclaration.initializer?.kind !== SyntaxKind.ObjectLiteralExpression
  )
    throw new Error("entrypoint owner registry factory differs");
  const constructorDeclaration = constructorSymbol.valueDeclaration?.resolve();
  if (!constructorDeclaration || !isClassDeclaration(constructorDeclaration))
    throw new Error("entrypoint owner constructor is not class");
  const declared = project.checker.getSymbolAtPosition(producer, owner.exportStart),
    declaration = declared?.valueDeclaration?.resolve();
  if (
    declared?.id !== exported.id ||
    !declaration ||
    !isVariableDeclaration(declaration) ||
    declaration.initializer?.kind !== SyntaxKind.NoSubstitutionTemplateLiteral ||
    declaration.initializer.getStart() !== owner.literalSpan.start ||
    declaration.initializer.end !== owner.literalSpan.end
  )
    throw new Error("entrypoint owner export/template shadow or span differs");
}

function verifySequenceVmPins(
  root: string,
  pins: NonNullable<z.infer<typeof requestSchema>["vmOwner"]>["pins"],
) {
  const names = [
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_sequence_activation_fixes.js",
    "scripts/mermaid/svg_sequence_activation_fixes_test.ts",
  ];
  if (pins.map((pin) => pin.sourcePath).join("\0") !== names.join("\0"))
    throw new Error("unknown sequence VM owner inventory");
  for (const pin of pins) {
    const file = path.join(root, pin.sourcePath);
    if (
      fs.realpathSync(file) !== file ||
      createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== pin.sourceSha256
    )
      throw new Error("sequence VM physical owner changed");
  }
  return names;
}

type SemanticProject = NonNullable<ReturnType<ReturnType<API["updateSnapshot"]>["getProject"]>>;
function verifySequenceVmSourceUses(
  nodes: readonly Node[],
  declarationName: Node,
  checker: SemanticProject["checker"],
  runSymbolId: number,
) {
  for (const node of nodes) {
    if (node === declarationName) continue;
    if (
      !node.parent ||
      !isCallExpression(node.parent) ||
      node.parent.arguments?.[0] !== node ||
      !isIdentifier(node.parent.expression) ||
      checker.getSymbolAtLocation(node.parent.expression)?.id !== runSymbolId
    )
      throw new Error("sequence VM source binding write/unknown use");
  }
}

function verifySequenceVmImport(
  symbol: Parameters<SemanticProject["checker"]["getReferencesToSymbolInFile"]>[1],
  module: string,
) {
  const declaration = symbol.declarations[0]?.resolve(),
    imported = declaration?.parent?.parent?.parent;
  if (
    symbol.declarations.length !== 1 ||
    !declaration ||
    !isImportSpecifier(declaration) ||
    declaration.isTypeOnly ||
    !imported ||
    !isImportDeclaration(imported) ||
    !isStringLiteral(imported.moduleSpecifier) ||
    ![module, `node:${module}`].includes(imported.moduleSpecifier.text)
  )
    throw new Error("sequence VM actual import module differs");
}

function verifySequenceVmUrl(root: string, caller: string, at: number, project: SemanticProject) {
  const symbol = project.checker.getSymbolAtPosition(caller, at);
  const paths = [
    path.join(
      root,
      `node_modules/@typescript/typescript-${process.platform}-${process.arch}/lib/lib.dom.d.ts`,
    ),
    path.join(root, "node_modules/bun-types/globals.d.ts"),
    path.join(root, "node_modules/@types/node/web-globals/url.d.ts"),
  ].map((file) => fs.realpathSync(file));
  if (
    symbol?.name !== "URL" ||
    symbol.flags & SymbolFlags.Alias ||
    !symbol.valueDeclaration ||
    !symbol.declarations.length ||
    symbol.declarations.some((handle) => {
      const node = handle.resolve();
      return !node || !paths.includes(fs.realpathSync(node.getSourceFile().fileName));
    })
  )
    throw new Error("sequence VM URL constructor is not current builtin");
  const refs = project.checker.getReferencesToSymbolInFile(caller, symbol);
  if (
    refs.length !== 1 ||
    refs.some((handle) => {
      const node = handle.resolve();
      return (
        !node ||
        node.getStart() !== at ||
        !node.parent ||
        !isNewExpression(node.parent) ||
        node.parent.expression !== node ||
        project.checker.getSymbolAtLocation(node)?.id !== symbol.id
      );
    })
  )
    throw new Error("sequence VM builtin URL write/unknown use");
}

function inspectSequenceVmOwner(
  request: z.infer<typeof requestSchema>,
  snapshot: ReturnType<API["updateSnapshot"]>,
) {
  const owner = request.vmOwner;
  if (!owner) return;
  const names = verifySequenceVmPins(request.cwd, owner.pins);
  const project = snapshot.getProject(owner.config),
    caller = path.join(request.cwd, names[1] ?? "");
  if (
    !project ||
    project.program.getSyntacticDiagnostics().length ||
    project.program.getBindDiagnostics().length
  )
    throw new Error("invalid sequence VM owner project");
  const references = (symbol: Parameters<typeof project.checker.getReferencesToSymbolInFile>[1]) =>
    project.checker.getReferencesToSymbolInFile(caller, symbol).map((handle) => {
      const node = handle.resolve();
      if (
        !node ||
        node.getSourceFile().fileName !== caller ||
        project.checker.getSymbolAtLocation(node)?.id !== symbol.id
      )
        throw new Error("different sequence VM symbol reference");
      return node;
    });
  const alias = (at: number, expected: string, module: string) => {
    const symbol = project.checker.getSymbolAtPosition(caller, at);
    if (!symbol || symbol.name !== expected || !(symbol.flags & SymbolFlags.Alias))
      throw new Error("sequence VM import alias differs");
    verifySequenceVmImport(symbol, module);
    const resolved = project.checker.getAliasedSymbol(symbol);
    if (
      resolved.name !== expected ||
      !resolved.declarations.length ||
      resolved.declarations.some((handle) => {
        const declaration = handle.resolve();
        return (
          !declaration ||
          fs.realpathSync(declaration.getSourceFile().fileName) !==
            fs.realpathSync(path.join(request.cwd, `node_modules/@types/node/${module}.d.ts`))
        );
      })
    )
      throw new Error("sequence VM Node import target differs");
    const nodes = references(symbol);
    if (
      nodes.some(
        (node) =>
          node.parent?.kind !== SyntaxKind.ImportSpecifier &&
          !(node.parent && isCallExpression(node.parent) && node.parent.expression === node),
      )
    )
      throw new Error("sequence VM imported binding write/unknown use");
    const node = nodes.find((node) => node.getStart() === at);
    if (!node?.parent || !isCallExpression(node.parent) || node.parent.expression !== node)
      throw new Error("sequence VM alias is not direct call");
    return { symbol, call: node.parent };
  };
  verifySequenceVmUrl(request.cwd, caller, owner.urlConstructorStart, project);
  const read = alias(owner.readFileStart, "readFileSync", "fs"),
    create = alias(owner.createContextStart, "createContext", "vm"),
    load = alias(owner.loadCalleeStart, "runInContext", "vm"),
    lookup = alias(owner.lookupCalleeStart, "runInContext", "vm");
  if (load.symbol.id !== lookup.symbol.id) throw new Error("different sequence VM run alias");
  const loaded = project.checker.getSymbolAtPosition(caller, owner.sourceBindingStart),
    context = project.checker.getSymbolAtPosition(caller, owner.loadContextStart);
  const declaration = loaded?.valueDeclaration?.resolve(),
    contextDeclaration = context?.valueDeclaration?.resolve();
  const constDeclaration = (node: Node | undefined) =>
    node &&
    isVariableDeclaration(node) &&
    node.parent &&
    isVariableDeclarationList(node.parent) &&
    (node.parent.flags & NodeFlags.BlockScoped) === NodeFlags.Const;
  if (
    !loaded ||
    !context ||
    !declaration ||
    !isVariableDeclaration(declaration) ||
    !contextDeclaration ||
    !isVariableDeclaration(contextDeclaration) ||
    !constDeclaration(declaration) ||
    !constDeclaration(contextDeclaration) ||
    declaration.initializer !== read.call ||
    contextDeclaration.initializer !== create.call ||
    project.checker.getSymbolAtPosition(caller, owner.loadSourceStart)?.id !== loaded.id ||
    project.checker.getSymbolAtPosition(caller, owner.lookupContextStart)?.id !== context.id
  )
    throw new Error("sequence VM source/context binding differs");
  if (
    references(context).length !== 3 ||
    references(context).some(
      (node) =>
        ![
          contextDeclaration.name.getStart(),
          owner.loadContextStart,
          owner.lookupContextStart,
        ].includes(node.getStart()),
    )
  )
    throw new Error("sequence VM context escapes/rebinds");
  verifySequenceVmSourceUses(references(loaded), declaration.name, project.checker, load.symbol.id);
  const literal = lookup.call.arguments?.[0];
  if (
    literal?.kind !== SyntaxKind.StringLiteral ||
    literal.getStart() !== owner.literalSpan.start ||
    literal.end !== owner.literalSpan.end ||
    load.call.arguments?.[1]?.getStart() !== owner.loadContextStart ||
    lookup.call.arguments?.[1]?.getStart() !== owner.lookupContextStart
  )
    throw new Error("sequence VM literal/call span differs");
}

export function inspectSemanticNodeRequest(value: unknown) {
  const request = requestSchema.parse(value);
  const assertCode = () => {
    if (
      !fs.lstatSync(request.file).isFile() ||
      fs.lstatSync(request.file).isSymbolicLink() ||
      createHash("sha256").update(fs.readFileSync(request.file)).digest("hex") !==
        request.codeSha256
    )
      throw new Error("semantic program bytes changed");
  };
  assertCode();
  const api = new API({ cwd: request.cwd });
  try {
    const snapshot = api.updateSnapshot({
      openProjects: [
        request.config,
        ...(request.owner ? [request.owner.config] : []),
        ...(request.vmOwner ? [request.vmOwner.config] : []),
      ],
    });
    try {
      inspectEntrypointOwner(request, snapshot);
      inspectSequenceVmOwner(request, snapshot);
      const project = snapshot.getProject(request.config);
      if (
        !project ||
        project.program.getSourceFileNames().join("\0") !== request.file ||
        project.program.getSyntacticDiagnostics().length ||
        project.program.getBindDiagnostics().length
      )
        throw new Error("changed isolated semantic realm");
      const rows = request.declarations.map((item) => {
        const symbol = project.checker.getSymbolAtPosition(request.file, item.start);
        if (!symbol || symbol.name !== item.symbol || symbol.declarations.length !== 1)
          throw new Error("missing/ambiguous declaration symbol");
        const declaration = symbol.valueDeclaration?.resolve();
        if (
          !declaration ||
          !supportedDeclaration(declaration) ||
          !declaration.name ||
          declaration.name.getStart() !== item.start ||
          declaration.name.end !== item.end
        )
          throw new Error("declaration UTF16 span mismatch");
        const owner = isVariableDeclaration(declaration)
          ? declaration.parent?.parent?.parent
          : declaration.parent;
        if (owner?.kind !== SyntaxKind.SourceFile) throw new Error("non-global declaration symbol");
        const reads = project.checker
          .getReferencesToSymbolInFile(request.file, symbol)
          .flatMap((handle) => {
            const node = handle.resolve();
            if (
              !node ||
              node.getSourceFile().fileName !== request.file ||
              project.checker.getSymbolAtLocation(node)?.id !== symbol.id
            )
              throw new Error("different semantic reference symbol/realm");
            return isRead(node, symbol.id, project)
              ? [{ start: node.getStart(), end: node.end }]
              : [];
          });
        return { symbol: item.symbol, reads };
      });
      const pkg: unknown = createRequire(import.meta.url)("typescript/package.json");
      assertCode();
      inspectEntrypointOwner(request, snapshot);
      inspectSequenceVmOwner(request, snapshot);
      return {
        nodeVersion: process.version,
        typescriptVersion: z.object({ version: z.literal("7.0.2") }).parse(pkg).version,
        api: "typescript/unstable/sync",
        ownerVerified: Boolean(request.owner),
        vmOwnerVerified: Boolean(request.vmOwner),
        rows,
      };
    } finally {
      snapshot.dispose();
    }
  } finally {
    api.close();
  }
}

if (process.argv[1] === new URL(import.meta.url).pathname) {
  const input = process.argv[2];
  if (!input || process.argv.length !== 3)
    throw new Error("semantic helper requires one request file");
  console.log(
    JSON.stringify(inspectSemanticNodeRequest(JSON.parse(fs.readFileSync(input, "utf8")))),
  );
}
