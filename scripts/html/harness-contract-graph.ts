import { createHash } from "node:crypto";
import { isBuiltin } from "node:module";
import path from "node:path";
import { parseSync, version as swcVersion } from "@swc/core";
import { z } from "zod";
import { MOON_QUALITY_INPUTS } from "./moon-quality-inputs";
import {
  nativeCoverageCacheTask,
  validateNativeCoverageCacheTask,
} from "./native-coverage-cache-contract";

const strings = z.array(z.string());
export const policyContract = z.object({
  schema: z.literal(1),
  codeRoots: strings,
  localRecipes: strings,
  localBridge: z.object({ recipe: z.string(), source: z.string() }),
  ciWorkflow: z.string(),
  recipeDispatch: z.record(z.string(), z.object({ source: z.string(), recipes: strings })),
  python: z
    .object({ lintRecipe: z.string(), typeRecipe: z.string(), ruff: z.string(), mypy: z.string() })
    .optional(),
  rust: z.object({ lintRecipe: z.string(), workspace: z.string() }).optional(),
  moon: z
    .object({
      workspace: z.string(),
      config: z.string(),
      project: z.string(),
      entry: z.string(),
      barrier: z.string(),
      ciJob: z.string(),
      requiredStatic: strings,
      requiredHeavy: strings,
      ciEntry: z.string().optional(),
      downstreamJob: z.string().optional(),
      ciRequiredStatic: strings.optional(),
      ciBeforeHeavy: strings.optional(),
    })
    .optional(),
});

export type ContractPolicy = z.infer<typeof policyContract>;

export interface ExecutionGraph {
  local: Set<string>;
  ci: Set<string>;
  errors: string[];
  localRecipes?: Set<string>;
  ciRecipes?: Set<string>;
  unitExecutableBindings?: Array<UnitExecutableBinding & { lane: "local" | "ci" }>;
  externalToolBindings?: Array<ExternalToolBinding & { lane: "local" | "ci" }>;
  nativeConsumerBindings?: Array<NativeConsumerBinding & { lane: "local" | "ci" }>;
}

export interface NativeConsumerBinding {
  proofKind: "source-ownership-classification";
  artifactVerified: false;
  executionVerified: false;
  caller: string;
  callerSourceSha256: string;
  function: string;
  argvClass: "html-feature-lib-test" | "html-toc-lib-test" | "html-fidelity-example";
  swcVersion: string;
  manifest: string;
  entrypoint: string;
  expectedProducerSources: Array<{ path: string; sourceSha256: string }>;
}

export interface ExternalToolRequest {
  kind: "typescript-bootstrap";
  caller: string;
  target: string;
  executable: "process.execPath";
  argvClass: "typescript-show-config-safe-project";
  callerSourceSha256: string;
  callerAstSha256: string;
  swcVersion: string;
}

export interface ExternalToolBinding extends ExternalToolRequest {
  packageName: "typescript";
  packageVersion: "7.0.2";
  verification: "observed-local-pins";
  artifactVerified: false;
  inputPins: Array<{ name: string; physicalPath: string; sha256: string }>;
  runtime: { kind: "bun"; version: string; path: string; sha256: string };
  lockIdentities: Array<{
    name: string;
    version: string;
    defaultRegistry: boolean;
    integrity: string;
  }>;
}

export type ExternalToolResolver = (
  request: ExternalToolRequest,
) => ExternalToolBinding | undefined;

export interface UnitExecutableBinding {
  proofKind: "reviewed-canonical-unit-source";
  artifactVerified: false;
  caller: string;
  callerSourceSha256: string;
  callerAstSha256: string;
  wrapper: string;
  wrapperSourceSha256: string;
  wrapperAstSha256: string;
  swcVersion: string;
  executions: Array<
    | { kind: "unit-self-runtime-version"; executable: "process.execPath"; argv: ["--version"] }
    | {
        kind: "unit-known-absent-temp-path";
        executable: "streamFixture().missingBinary";
        argv: [[], ["--version"]];
      }
    | {
        kind: "unit-self-runtime-temp-environment";
        executable: "process.execPath";
        argv: ["-e", "childProgram"];
        temporaryDirectory: "tempRoots.entries().tempRoot";
      }
    | {
        kind: "unit-runtime-asset-compressor-cli";
        executable: "process.execPath";
        script: "scripts/runtime-assets/runtime-package-asset-compressor.ts";
        argv: [
          ["--write", "--zenuml-only"],
          ["--zenuml-only", "--check"],
          ["--check", "--zenuml-only"],
        ];
        cwd: "temporaryRoot()";
        stdio: { stdout: "pipe"; stderr: "pipe" };
      }
  >;
}

interface SourceCodeContext {
  name: string;
  importer?: string;
  importerSource?: string;
  read: (name: string) => string | undefined;
  onUnitBinding?: (binding: UnitExecutableBinding) => void;
  onNativeConsumerBinding?: (binding: NativeConsumerBinding) => void;
  resolveExternalTool?: ExternalToolResolver;
  onExternalToolBinding?: (binding: ExternalToolBinding) => void;
}

export type ManagedCodeResolver = (reference: string, importer: string | undefined) => string;

export const isCode = (name: string): boolean => /\.(?:[cm]?[jt]sx?|py|sh|rs)$/.test(name);
const object = (value: unknown): Record<string, unknown> | undefined =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
const literal = (value: unknown): string | undefined => {
  const node = object(value);
  return node?.type === "StringLiteral" && typeof node.value === "string" ? node.value : undefined;
};

function staticString(value: unknown, bindings: Map<string, string>): string | undefined {
  const node = object(value);
  if (node?.type === "StringLiteral")
    return typeof node.value === "string" ? node.value : undefined;
  if (node?.type === "TemplateLiteral") {
    const expressions = Array.isArray(node.expressions) ? node.expressions : [];
    const quasis = Array.isArray(node.quasis) ? node.quasis : [];
    const cooked = object(quasis[0])?.cooked;
    return expressions.length === 0 && quasis.length === 1 && typeof cooked === "string"
      ? cooked
      : undefined;
  }
  return node?.type === "Identifier" ? bindings.get(identifierKey(node) ?? "") : undefined;
}

function assignmentTarget(node: Record<string, unknown>): unknown {
  if (node.type === "AssignmentExpression") return node.left;
  if (node.type === "UpdateExpression") return node.argument;
  return node.type === "ForInStatement" || node.type === "ForOfStatement" ? node.left : undefined;
}

function assignedBindings(module: Record<string, unknown>): Set<string> {
  return new Set(
    astNodes(module).flatMap((node) =>
      astNodes(assignmentTarget(node)).flatMap((target) => {
        const key = identifierKey(target);
        return key ? [key] : [];
      }),
    ),
  );
}

interface StaticStringCandidate {
  key: string;
  kind: string;
  value?: string;
  importPath?: string;
  block?: object;
  end?: number;
}

interface StaticStringBinding {
  value?: string;
  importPath?: string;
  block: object;
  end: number;
}

interface ResolvedStaticBindings {
  strings: Map<string, string>;
  importPaths: Map<string, string>;
}

function executionBlocks(module: Record<string, unknown>): WeakMap<object, object> {
  const blocks = new WeakMap<object, object>();
  const visit = (value: unknown, inherited: object): void => {
    if (Array.isArray(value)) {
      for (const child of value) visit(child, inherited);
      return;
    }
    const node = object(value);
    if (!node) return;
    // SWCのmethod.functionも境界に含め、default parameterから外側constを解決しない。
    const boundary =
      [
        "Module",
        "Script",
        "BlockStatement",
        "FunctionDeclaration",
        "FunctionExpression",
        "ArrowFunctionExpression",
        "ClassDeclaration",
        "ClassExpression",
        "GetterProperty",
        "SetterProperty",
        "TsModuleBlock",
        "SwitchCase",
        "ForStatement",
        "ForInStatement",
        "ForOfStatement",
        "CatchClause",
      ].includes(String(node.type)) ||
      (Array.isArray(node.params) && "body" in node);
    const block = boundary ? node : inherited;
    blocks.set(node, block);
    for (const child of Object.values(node)) visit(child, block);
  };
  visit(module, module);
  return blocks;
}

function callStaticBindings(
  call: Record<string, unknown>,
  bindings: Map<string, StaticStringBinding>,
  blocks: WeakMap<object, object>,
): ResolvedStaticBindings {
  const result: ResolvedStaticBindings = { strings: new Map(), importPaths: new Map() };
  const start = object(call.span)?.start;
  for (const [key, binding] of bindings) {
    if (typeof start !== "number" || binding.block !== blocks.get(call) || binding.end >= start)
      continue;
    if (binding.value !== undefined) result.strings.set(key, binding.value);
    if (binding.importPath !== undefined) result.importPaths.set(key, binding.importPath);
  }
  return result;
}

function staticStringCandidate(
  node: Record<string, unknown>,
  value: unknown,
  blocks: WeakMap<object, object>,
  paths: Set<string>,
): StaticStringCandidate[] {
  const declaration = object(value);
  const key = identifierKey(declaration?.id);
  if (!key) return [];
  const candidate: StaticStringCandidate = { key, kind: String(node.kind) };
  const block = declaration && blocks.get(declaration);
  const end = object(declaration?.span)?.end;
  if (block) candidate.block = block;
  if (typeof end === "number" && Number.isFinite(end)) candidate.end = end;
  const staticValue = staticString(declaration?.init, new Map());
  if (staticValue !== undefined) candidate.value = staticValue;
  const importPath = importDirectoryJoin(declaration?.init, paths);
  if (importPath !== undefined) candidate.importPath = importPath;
  return [candidate];
}

function staticStringCandidates(
  module: Record<string, unknown>,
  blocks: WeakMap<object, object>,
  paths: Set<string>,
): StaticStringCandidate[] {
  return astNodes(module).flatMap((node) => {
    if (node.type !== "VariableDeclaration") return [];
    const declarations = Array.isArray(node.declarations) ? node.declarations : [];
    return declarations.flatMap((value) => staticStringCandidate(node, value, blocks, paths));
  });
}

function staticConstStringBindings(
  module: Record<string, unknown>,
  blocks: WeakMap<object, object>,
): Map<string, StaticStringBinding> {
  const assigned = assignedBindings(module);
  const paths = new Set([...nodePathBindings(module)].filter((key) => !assigned.has(key)));
  const candidates = new Map<string, { count: number; candidate: StaticStringCandidate }>();
  for (const candidate of staticStringCandidates(module, blocks, paths)) {
    candidates.set(candidate.key, {
      count: (candidates.get(candidate.key)?.count ?? 0) + 1,
      candidate,
    });
  }
  return new Map(
    [...candidates].flatMap(([key, { count, candidate }]) =>
      count === 1 &&
      candidate.kind === "const" &&
      (candidate.value !== undefined || candidate.importPath !== undefined) &&
      candidate.block !== undefined &&
      candidate.end !== undefined &&
      !assigned.has(key)
        ? [
            [
              key,
              {
                ...(candidate.value !== undefined ? { value: candidate.value } : {}),
                ...(candidate.importPath !== undefined ? { importPath: candidate.importPath } : {}),
                block: candidate.block,
                end: candidate.end,
              },
            ],
          ]
        : [],
    ),
  );
}

const SHELL_WORD_PATTERN = /"[^"\n]*"|'[^'\n]*'|[^\s;|&<>]+/g;

export function shellWords(command: string): string[] {
  return (command.match(SHELL_WORD_PATTERN) ?? []).map((word) =>
    /^["']/.test(word) ? word.slice(1, -1) : word,
  );
}

function commandAt(words: string[], index: number, scripts: Record<string, string>): string[] {
  const next = words[index + 1] ?? "";
  const argument = words[index + 2] ?? "";
  if (next === "-e") return sourceCode(argument).map((name) => `!child-import:${name}`);
  if (next === "run")
    return scripts[argument] ? commandCode(shellWords(scripts[argument]), {}) : [argument];
  if (next === "test")
    return words.slice(index + 2).filter((name) => isCode(name) || name.startsWith("scripts/html"));
  return next && !next.startsWith("-") ? [next] : [];
}

function commandCode(words: string[], scripts: Record<string, string>): string[] {
  // WHY: recipe抽出時のjust接頭辞は、検証済みPlaywright引数が続く場合だけ認識する。
  if (words[0] === "just") {
    const prefixedInputs = playwrightTestRunnerFiles(words.slice(1));
    if (prefixedInputs) return prefixedInputs;
  }
  const playwrightInputs = playwrightTestRunnerFiles(words);
  if (playwrightInputs) return playwrightInputs;
  return words
    .flatMap((word, index) =>
      ["bun", "node", "python", "python3", "bash", "sh"].includes(word)
        ? commandAt(words, index, scripts)
        : [],
    )
    .filter(
      (name) =>
        name.startsWith("!child-import:") ||
        isCode(name) ||
        name.startsWith("scripts/html") ||
        name.startsWith("tmp/") ||
        name.startsWith("./tmp/") ||
        path.isAbsolute(name),
    );
}

function identifierKey(value: unknown): string | undefined {
  const node = object(value);
  return node?.type === "Identifier" ? `${node.value}:${node.ctxt}` : undefined;
}

function nodePathBindings(module: Record<string, unknown>): Set<string> {
  const body = Array.isArray(module.body) ? module.body : [];
  return new Set(
    body.flatMap((value) => {
      const node = object(value);
      if (
        node?.type !== "ImportDeclaration" ||
        node.typeOnly ||
        literal(node.source) !== "node:path"
      )
        return [];
      const specifiers = Array.isArray(node.specifiers) ? node.specifiers : [];
      return specifiers.flatMap((value) => {
        const specifier = object(value);
        if (
          !["ImportDefaultSpecifier", "ImportNamespaceSpecifier"].includes(String(specifier?.type))
        )
          return [];
        return identifierKey(specifier?.local) ?? [];
      });
    }),
  );
}

type ProcessBinding = { method: string; api: "node" | "bun" };

const processMethods = ["spawn", "spawnSync", "execFile", "execFileSync", "exec", "execSync"];
const nodeProcessModules = ["node:child_process", "child_process"];

function processBindings(module: Record<string, unknown>): Map<string, ProcessBinding> {
  const body = Array.isArray(module.body) ? module.body : [];
  return new Map(
    body.flatMap((value): [string, ProcessBinding][] => {
      const node = object(value);
      if (
        node?.type !== "ImportDeclaration" ||
        node.typeOnly ||
        !["node:child_process", "child_process", "bun"].includes(literal(node.source) ?? "")
      )
        return [];
      const specifiers = Array.isArray(node.specifiers) ? node.specifiers : [];
      return specifiers.flatMap((value): [string, ProcessBinding][] => {
        const specifier = object(value);
        const key = identifierKey(specifier?.local);
        if (!key || specifier?.isTypeOnly) return [];
        const method =
          specifier?.type === "ImportSpecifier"
            ? String(object(specifier.imported)?.value ?? object(specifier.local)?.value)
            : "*";
        return [[key, { method, api: literal(node.source) === "bun" ? "bun" : "node" }]];
      });
    }),
  );
}

function processMethod(
  callee: Record<string, unknown> | undefined,
  bindings: Map<string, ProcessBinding>,
): ProcessBinding | undefined {
  if (callee?.type === "Identifier") return bindings.get(identifierKey(callee) ?? "");
  if (callee?.type !== "MemberExpression") return undefined;
  const key = identifierKey(callee.object);
  const binding = bindings.get(key ?? "");
  if (key !== "Bun:1" && binding?.method !== "*") return undefined;
  return object(callee.property)?.type === "Identifier"
    ? {
        method: String(object(callee.property)?.value),
        api: key === "Bun:1" ? "bun" : (binding?.api ?? "node"),
      }
    : undefined;
}

function importDirectoryJoin(value: unknown, bindings: Set<string>): string | undefined {
  const node = object(value);
  const callee = object(node?.callee);
  const args = Array.isArray(node?.arguments) ? node.arguments : [];
  const directory = object(object(args[0])?.expression);
  const relative = literal(object(args[1])?.expression);
  if (
    node?.type !== "CallExpression" ||
    args.length !== 2 ||
    args.some((arg) => object(arg)?.spread) ||
    callee?.type !== "MemberExpression" ||
    !bindings.has(identifierKey(callee.object) ?? "") ||
    object(callee.property)?.type !== "Identifier" ||
    object(callee.property)?.value !== "join" ||
    directory?.type !== "MemberExpression" ||
    object(directory.object)?.type !== "MetaProperty" ||
    object(directory.object)?.kind !== "import.meta" ||
    object(directory.property)?.value !== "dir" ||
    !relative ||
    relative.includes("\\") ||
    path.posix.isAbsolute(relative) ||
    path.win32.isAbsolute(relative)
  )
    return undefined;
  return relative;
}

function evalChildCode(elements: unknown[], words: string[]): string[] {
  if (elements.length !== 3 || elements.some((element) => object(element)?.spread) || !words[2])
    return ["!unresolved-executable"];
  return commandCode(words, {});
}

function externalCommand(words: string[], elements?: unknown[]): boolean {
  const wrapped = words[0] === "rtk";
  if (wrapped && (!elements || words[1] !== "proxy")) return false;
  if (
    wrapped &&
    elements &&
    (literal(object(elements[0])?.expression) !== "rtk" ||
      literal(object(elements[1])?.expression) !== "proxy")
  )
    return false;
  const command = wrapped ? words.slice(2) : words;
  if (["git", "magick", "uvx"].includes(command[0] ?? "")) return true;
  return (
    command[0] === "bunx" &&
    command[1] === "--no-install" &&
    ["biome", "tsc"].includes(command[2] ?? "")
  );
}

function runtimeBundleMetadataProbe(words: string[], elements: unknown[]): boolean {
  const approved = [
    ["just", "--version"],
    ["bash", "--version"],
    ["node", "--version"],
    ["node", "node_modules/@moonrepo/cli/moon.js", "--version"],
  ];
  return (
    !elements.some((element) => object(element)?.spread) &&
    approved.some(
      (argv) =>
        argv.length === words.length &&
        argv.every(
          (word, index) =>
            words[index] === word && literal(object(elements[index])?.expression) === word,
        ),
    )
  );
}

function ownRuntime(value: unknown): boolean {
  const node = object(value);
  return (
    node?.type === "MemberExpression" &&
    identifierKey(node.object) === "process:1" &&
    object(node.property)?.type === "Identifier" &&
    object(node.property)?.value === "execPath"
  );
}

function relativeChildCode(
  elements: unknown[],
  words: string[],
  bindings: Set<string>,
  staticBindings: ResolvedStaticBindings,
): string | undefined {
  if (!["bun", "node", "python", "python3", "bash", "sh"].includes(words[0] ?? ""))
    return undefined;
  const scriptIndex = words[0] === "node" && words[1] === "--experimental-strip-types" ? 2 : 1;
  const expression = object(elements[scriptIndex]);
  if (expression?.spread) return undefined;
  return (
    importDirectoryJoin(expression?.expression, bindings) ??
    staticBindings.importPaths.get(identifierKey(expression?.expression) ?? "")
  );
}

function externalDataCommand(words: string[], elements: unknown[]): boolean {
  const exact = (index: number, value: string): boolean =>
    literal(object(elements[index])?.expression) === value;
  if (elements.some((element) => object(element)?.spread)) return false;
  if (words.length === 4 && exact(0, "unzip") && exact(1, "-p")) return true;
  if (words.length === 4 && exact(0, "zip") && exact(1, "-q")) return true;
  return (
    words.length === 5 &&
    ["gh", "auth", "token", "--hostname", "github.com"].every((word, index) => exact(index, word))
  );
}

function gitFilesDataSpread(elements: unknown[]): boolean {
  return (
    elements.length === 4 &&
    literal(object(elements[0])?.expression) === "git" &&
    literal(object(elements[1])?.expression) === "ls-files" &&
    Boolean(object(elements[2])?.spread) &&
    literal(object(elements[3])?.expression) === "-z" &&
    [0, 1, 3].every((index) => !object(elements[index])?.spread)
  );
}

function literalChildSource(elements: unknown[], words: string[]): string | undefined {
  if (!["bun", "node", "python", "python3", "bash", "sh"].includes(words[0] ?? ""))
    return undefined;
  if (words[1] === "node_modules/playwright/cli.js") return undefined;
  const index = words[0] === "bun" && words[1] === "run" ? 2 : 1;
  if (elements.slice(0, index + 1).some((element) => object(element)?.spread)) return undefined;
  const child = words[index];
  return child && !child.startsWith("-") && isCode(child) ? child : undefined;
}

function externalShellControls(command: string): boolean {
  // 既存tokenizerが引用dataとして読む部分だけ除き、外部tool後の未追跡実行を拒否する。
  const unquoted = command.replace(SHELL_WORD_PATTERN, (word) => {
    if (word.startsWith("'")) return "";
    if (word.startsWith('"') && !/[$`]/.test(word)) return "";
    return word;
  });
  return /[;|&<>\r\n$`]/.test(unquoted);
}

function shellChildCode(command: string | undefined): string[] {
  if (command === undefined) return ["!unresolved-executable"];
  const words = shellWords(command);
  if (externalCommand(words))
    return externalShellControls(command) ? ["!unresolved-executable"] : [];
  if (!["bun", "node", "python", "python3", "bash", "sh"].includes(words[0] ?? ""))
    return ["!unresolved-executable"];
  return commandCode(words, {}).map((name) => `!execute:${name}`);
}

function playwrightTestRunnerFiles(words: string[]): string[] | undefined {
  if (
    words.length !== 5 ||
    words[0] !== "node" ||
    words[1] !== "node_modules/playwright/cli.js" ||
    words[2] !== "test" ||
    words[3] !== "--config" ||
    words[4] !== "scripts/html/playwright.config.ts"
  )
    return undefined;
  return [
    "scripts/html/playwright.config.ts",
    "scripts/html/feature-catalog_test.ts",
    "scripts/html/chrome_badge_metrics_test.ts",
  ];
}

function playwrightTestRunnerInputs(words: string[]): string[] | undefined {
  return playwrightTestRunnerFiles(words)?.map((file) => `!execute-local:${file}`);
}

function justCommandChildCode(
  elements: unknown[],
  words: string[],
  bindings: Set<string>,
  staticBindings: ResolvedStaticBindings,
): string[] | undefined {
  if (words[0] !== "just" || words[1] !== "--command") return undefined;
  if (
    elements.some((element) => object(element)?.spread) ||
    literal(object(elements[0])?.expression) !== "just" ||
    literal(object(elements[1])?.expression) !== "--command"
  )
    return ["!unresolved-executable"];
  // Justの環境適用後も、実行されるinterpreterとsourceを通常の規則で追う。
  return childCode(
    { type: "ArrayExpression", elements: elements.slice(2) },
    bindings,
    staticBindings,
  );
}

function childCode(
  first: unknown,
  bindings: Set<string>,
  staticBindings: ResolvedStaticBindings,
): string[] {
  const array = object(first);
  if (array?.type !== "ArrayExpression") {
    return shellChildCode(literal(first));
  }
  const elements = Array.isArray(array.elements) ? array.elements : [];
  const words = elements.map(
    (element) => staticString(object(element)?.expression, staticBindings.strings) ?? "",
  );
  const justCommand = justCommandChildCode(elements, words, bindings, staticBindings);
  if (justCommand) return justCommand;
  return interpreterArrayChildCode(elements, words, bindings, staticBindings);
}

function interpreterArrayChildCode(
  elements: unknown[],
  words: string[],
  bindings: Set<string>,
  staticBindings: ResolvedStaticBindings,
): string[] {
  if (ownRuntime(object(elements[0])?.expression)) words[0] = "bun";
  if (!words[0] || object(elements[0])?.spread) return ["!unresolved-executable"];
  if (runtimeBundleMetadataProbe(words, elements)) return [];
  if (words[0] === "node" && /^node_modules\/.+\/moon\.js$/u.test(words[1] ?? ""))
    return ["!unresolved-executable"];
  const relative = relativeChildCode(elements, words, bindings, staticBindings);
  if (relative) return [`!execute-local:${relative}`];
  const child = literalChildSource(elements, words);
  if (child) return [`!execute:${child}`];
  if (gitFilesDataSpread(elements) || externalDataCommand(words, elements)) return [];
  if (elements.some((element) => object(element)?.spread)) return ["!unresolved-executable"];
  if (externalCommand(words, elements)) return [];
  return interpreterChildCode(elements, words);
}

function interpreterChildCode(elements: unknown[], words: string[]): string[] {
  if (!["bun", "node", "python", "python3", "bash", "sh"].includes(words[0] ?? ""))
    return ["!unresolved-executable"];
  if (words.length === 2 && words[0] === "node" && words[1] === "--version") return [];
  if (words[1] === "-e") return evalChildCode(elements, words);
  if (!words[1]) return ["!unresolved-executable"];
  const playwrightInputs = playwrightTestRunnerInputs(words);
  if (playwrightInputs) return playwrightInputs;
  if (words[1] === "node_modules/playwright/cli.js") return ["!unresolved-executable"];
  if (["run", "test"].includes(words[1]) && !words[2]) return ["!unresolved-executable"];
  if (words[1].startsWith("-")) return ["!unresolved-executable"];
  if (!["run", "test"].includes(words[1])) return [`!execute:${words[1]}`];
  const childNames = commandCode(words, {});
  return childNames.map((name) => `!execute:${name}`);
}

function execFileCode(
  args: unknown[],
  bindings: Set<string>,
  staticBindings: ResolvedStaticBindings,
): string[] {
  const first = object(args[0])?.expression;
  const executable = ownRuntime(first) ? { type: "StringLiteral", value: "bun" } : first;
  if (literal(executable) === undefined) return ["!unresolved-executable"];
  const argumentsArray = object(object(args[1])?.expression);
  if (argumentsArray?.type !== "ArrayExpression") {
    return ["bun", "node", "python", "python3", "bash", "sh"].includes(literal(executable) ?? "")
      ? ["!unresolved-executable"]
      : ["git", "magick", "uvx"].includes(literal(executable) ?? "")
        ? []
        : ["!unresolved-executable"];
  }
  const elements = Array.isArray(argumentsArray.elements) ? argumentsArray.elements : [];
  return childCode(
    { type: "ArrayExpression", elements: [{ expression: executable }, ...elements] },
    bindings,
    staticBindings,
  );
}

interface DomainProcessRoute {
  nativeBinary?: string;
  unitBinary?: string;
  unitProgramCall?: Record<string, unknown>;
  chromeFontIdentityProbeCall?: Record<string, unknown>;
  observerRequest?: string;
  collector?: string;
  nativeWidthCall?: Record<string, unknown>;
  nativeWidthConsumerCall?: Record<string, unknown>;
  classicFactsCall?: Record<string, unknown>;
  mathjaxFactsCall?: Record<string, unknown>;
}

function astNodes(value: unknown): Record<string, unknown>[] {
  const nodes: Record<string, unknown>[] = [];
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const child of value) visit(child);
      return;
    }
    const node = object(value);
    if (!node) return;
    nodes.push(node);
    for (const child of Object.values(node)) visit(child);
  };
  visit(value);
  return nodes;
}

function functionNamed(
  module: Record<string, unknown>,
  name: string,
): Record<string, unknown> | undefined {
  const body = Array.isArray(module.body) ? module.body : [];
  return body
    .map((value) => {
      const node = object(value);
      return node?.type === "ExportDeclaration" ? object(node.declaration) : node;
    })
    .find(
      (node) => node?.type === "FunctionDeclaration" && object(node.identifier)?.value === name,
    );
}

function parameterKey(fn: Record<string, unknown> | undefined, index: number): string | undefined {
  const params = Array.isArray(fn?.params) ? fn.params : [];
  return identifierKey(object(params[index])?.pat);
}

function importKey(
  module: Record<string, unknown>,
  source: string,
  name: string,
): string | undefined {
  const body = Array.isArray(module.body) ? module.body : [];
  for (const value of body) {
    const node = object(value);
    if (node?.type !== "ImportDeclaration" || node.typeOnly || literal(node.source) !== source)
      continue;
    const specifiers = Array.isArray(node.specifiers) ? node.specifiers : [];
    const selected = specifiers
      .map(object)
      .find(
        (specifier) =>
          specifier?.type === "ImportSpecifier" &&
          !specifier.isTypeOnly &&
          (object(specifier.imported)?.value ?? object(specifier.local)?.value) === name,
      );
    if (selected) return identifierKey(selected.local);
  }
  return undefined;
}

function callArguments(value: unknown, key: string | undefined): unknown[] | undefined {
  let node = object(value);
  if (node?.type === "AwaitExpression") node = object(node.argument);
  if (!key || node?.type !== "CallExpression" || identifierKey(node.callee) !== key)
    return undefined;
  const args = Array.isArray(node.arguments) ? node.arguments : [];
  return args.some((arg) => object(arg)?.spread)
    ? undefined
    : args.map((arg) => object(arg)?.expression);
}

function memberOf(value: unknown, key: string | undefined, property: string): boolean {
  const node = object(value);
  return (
    key !== undefined &&
    node?.type === "MemberExpression" &&
    identifierKey(node.object) === key &&
    object(node.property)?.type === "Identifier" &&
    object(node.property)?.value === property
  );
}

function functionStatements(fn: Record<string, unknown> | undefined): Record<string, unknown>[] {
  const body = object(fn?.body);
  return Array.isArray(body?.stmts)
    ? body.stmts.map(object).filter((node): node is Record<string, unknown> => node !== undefined)
    : [];
}

function initializedCall(
  fn: Record<string, unknown> | undefined,
  name: string,
  key: string | undefined,
): { args: unknown[]; key: string; index: number } | undefined {
  const statements = functionStatements(fn);
  for (const [index, statement] of statements.entries()) {
    if (statement.type !== "VariableDeclaration") continue;
    const declarations = Array.isArray(statement.declarations) ? statement.declarations : [];
    for (const value of declarations) {
      const declaration = object(value);
      if (object(declaration?.id)?.value !== name) continue;
      const args = callArguments(declaration?.init, key);
      const id = identifierKey(declaration?.id);
      if (
        args &&
        id &&
        !statements
          .slice(0, index)
          .some((prior) => astNodes(prior).some((node) => node.type === "ReturnStatement"))
      )
        return { args, key: id, index };
    }
  }
  return undefined;
}

function unchangedParameter(
  fn: Record<string, unknown> | undefined,
  key: string | undefined,
): boolean {
  return (
    key !== undefined &&
    !astNodes(fn).some(
      (node) =>
        ["AssignmentExpression", "UpdateExpression"].includes(String(node.type)) &&
        astNodes(node.left ?? node.argument).some((target) => identifierKey(target) === key),
    )
  );
}

function buildArguments(args: unknown[] | undefined, input: string | undefined): boolean {
  return (
    args?.length === 3 &&
    ["buildReceipt", "binary", "runtime"].every((property, index) =>
      memberOf(args[index], input, property),
    )
  );
}

function nativeProducerVerified(source: string | undefined): boolean {
  if (!source) return false;
  const module = parseSync(source, { syntax: "typescript" }) as unknown as Record<string, unknown>;
  const verifier = importKey(module, "./native-build-receipt", "verifyNativeBuildReceipt");
  const prepare = functionNamed(module, "prepareRender");
  const run = functionNamed(module, "run");
  const before = initializedCall(prepare, "build", verifier);
  const prepared = initializedCall(run, "prepared", identifierKey(prepare?.identifier));
  const batchKey = identifierKey(functionNamed(module, "renderFixtures")?.identifier);
  // 下流が同じparameterを使っても、未検証入力でbatchを追加起動してはならない。
  if (!batchKey || astNodes(module).filter((node) => identifierKey(node) === batchKey).length !== 2)
    return false;
  const rows = initializedCall(run, "rows", batchKey);
  const after = initializedCall(run, "afterBuild", verifier);
  return (
    buildArguments(before?.args, parameterKey(prepare, 0)) &&
    prepared?.args.length === 1 &&
    identifierKey(prepared.args[0]) === parameterKey(run, 0) &&
    rows !== undefined &&
    prepared.index < rows.index &&
    after !== undefined &&
    rows.index < after.index &&
    buildArguments(after.args, parameterKey(run, 0)) &&
    memberOf(rows.args[2], prepared.key, "build") &&
    identifierKey(rows.args[1]) === parameterKey(run, 0) &&
    unchangedParameter(prepare, parameterKey(prepare, 0)) &&
    unchangedParameter(run, parameterKey(run, 0)) &&
    nativeFixtureRoute(module)
  );
}

function nativeFixtureRoute(module: Record<string, unknown>): boolean {
  const fixtures = functionNamed(module, "renderFixtures");
  const fixture = functionNamed(module, "renderFixture");
  const wrapper = importKey(module, "./native-cli-process", "spawnNativeCli");
  if (!wrapper || astNodes(module).filter((node) => identifierKey(node) === wrapper).length !== 2)
    return false;
  const launch = initializedCall(fixture, "child", wrapper);
  const input = parameterKey(fixture, 1);
  const fixtureKey = identifierKey(fixture?.identifier);
  const nodes = astNodes(module);
  const calls = nodes.filter(
    (node) => node.type === "CallExpression" && identifierKey(node.callee) === fixtureKey,
  );
  // 正規呼出しを一つ残して、別入力の呼出しやaliasを追加する抜け穴を拒否する。
  const paired =
    fixtureKey !== undefined &&
    calls.length > 0 &&
    nodes.filter((node) => identifierKey(node) === fixtureKey).length === calls.length + 1 &&
    calls.every((node) => {
      const args = callArguments(node, fixtureKey);
      return (
        args?.length === 3 &&
        identifierKey(args[1]) === parameterKey(fixtures, 1) &&
        identifierKey(args[2]) === parameterKey(fixtures, 2)
      );
    });
  return (
    paired &&
    launch !== undefined &&
    memberOf(launch.args[0], input, "binary") &&
    unchangedParameter(fixtures, parameterKey(fixtures, 1)) &&
    unchangedParameter(fixture, input)
  );
}

function collectorSourceVerified(module: Record<string, unknown>): string | undefined {
  const collector = functionNamed(module, "collectorPath");
  if (!collector || (Array.isArray(collector.params) && collector.params.length !== 0))
    return undefined;
  const statements = functionStatements(collector);
  const returned = object(statements.length === 1 ? statements[0]?.argument : undefined);
  const callee = object(returned?.callee);
  const args = Array.isArray(returned?.arguments) ? returned.arguments : [];
  const directory = object(object(args[0])?.expression);
  const directoryCallee = object(directory?.callee);
  const directoryArgs = Array.isArray(directory?.arguments) ? directory.arguments : [];
  const urlArgs = callArguments(
    object(directoryArgs[0])?.expression,
    importKey(module, "node:url", "fileURLToPath"),
  );
  const meta = object(urlArgs?.[0]);
  const paths = nodePathBindings(module);
  if ([...paths].some((key) => !unchangedParameter(module, key))) return undefined;
  if (
    returned?.type !== "CallExpression" ||
    !memberOf(callee, identifierKey(callee?.object), "join") ||
    !paths.has(identifierKey(callee?.object) ?? "") ||
    args.length !== 2 ||
    literal(object(args[1])?.expression) !== "browser-process-observe.py"
  )
    return undefined;
  if (
    directory?.type !== "CallExpression" ||
    !memberOf(directoryCallee, identifierKey(callee?.object), "dirname") ||
    directoryArgs.length !== 1 ||
    urlArgs?.length !== 1 ||
    meta?.type !== "MemberExpression" ||
    object(meta.object)?.type !== "MetaProperty" ||
    object(meta.object)?.kind !== "import.meta" ||
    object(meta.property)?.value !== "url"
  )
    return undefined;
  return identifierKey(collector.identifier);
}

function observerRouteVerified(
  module: Record<string, unknown>,
  collector: string,
): string | undefined {
  const observer = functionNamed(module, "observeActualBrowserIdentity");
  const route = functionNamed(module, "requireApprovedBrowserObservationRoute");
  const identity = importKey(module, "./raster-comparison-binding", "physicalFileIdentity");
  const request = parameterKey(observer, 1);
  const first = functionStatements(observer)[0];
  const approved = callArguments(first?.expression, identifierKey(route?.identifier));
  const calls = functionStatements(route)
    .filter((statement) => statement.type === "ExpressionStatement")
    .map((statement) => callArguments(statement.expression, identity));
  if (astNodes(route).some((node) => node.type === "ReturnStatement")) return undefined;
  const hasTool = calls.some(
    (args) => args?.length === 1 && identifierKey(args[0]) === parameterKey(route, 0),
  );
  const hasSource = calls.some(
    (args) => args?.length === 1 && callArguments(args[0], collector)?.length === 0,
  );
  return approved?.length === 1 &&
    memberOf(approved[0], request, "observerPython") &&
    hasTool &&
    hasSource &&
    unchangedParameter(observer, request)
    ? request
    : undefined;
}

// レビュー済みunit全体だけを識別し、test名や動的binaryそのものを許可条件にしない。
const NATIVE_UNIT_PROGRAM = {
  wrapper: "scripts/mermaid/native-cli-process.ts",
  wrapperSourceSha256: "b78db7b249eeda045f8fb8784e0441fbafb060db41d5516bfdf44455b806f8f1",
  wrapperAstSha256: "c67d46e919b9c227a78b1f5de6f406035ad2d9597859de21da02cf7a50200430",
  caller: "scripts/mermaid/native-cli-process_test.ts",
  callerSourceSha256: "aab01ce01af2d99be294032112a0e61fb3926c84928b0a0865600cd6596d1c0f",
  callerAstSha256: "35a7bd9aa284fdc3a94f0fe7aab8f793c3b5ccd75a5e230703c4b22d15e5c93f",
  swcVersion: "1.16.18",
};

const RUNTIME_ASSET_COMPRESSOR_UNIT_PROGRAM = {
  caller: "scripts/runtime-assets/runtime-package-asset-compressor_test.ts",
  callerSourceSha256: "11ed165e8385ff593ad73fc65f69d08cd0486ffa88a0521a819ff479a66c0670",
  callerAstSha256: "fbf473330e96fbb1a26a2ba68203c125e318c9af6ee643aa9593838f3c5544c8",
  wrapper: "scripts/runtime-assets/runtime-package-asset-compressor.ts",
  wrapperSourceSha256: "1eae43d7e77deaf44bda83585159c5ccde6dcd24d0d524d395d031c081621f48",
  wrapperAstSha256: "b3b54b4f989cc4afa735785789688131dcc9f04916a2dbd24755e6208d211f85",
  swcVersion: "1.16.18",
};

const CHROME_FONT_IDENTITY_PROBE_IMPORTERS: readonly string[] = [
  "scripts/html/fidelity-acceptance-cli.ts",
  "scripts/html/visual-state-fidelity_test.ts",
  "scripts/html/dynamic-fidelity_test.ts",
  "scripts/html/feature-catalog_test.ts",
];

const CHROME_FONT_IDENTITY_PROBE = {
  module: "scripts/html/visual-state-fidelity.ts",
  importers: CHROME_FONT_IDENTITY_PROBE_IMPORTERS,
  functionName: "runChromeFontIdentityProbe",
  functionAstSha256: "be2fa01a198d93f05f9e36d564be1a030570ce9013b382f18646b5c17531b704",
  swcVersion: "1.16.18",
} as const;

function sourceDigest(source: string): string {
  return createHash("sha256").update(source).digest("hex");
}

function canonicalNativeUnitAstDigest(module: unknown): string {
  return sourceDigest(
    JSON.stringify(module, (key: string, value: unknown) =>
      ["span", "ctxt", "bodyCtxt", "raw"].includes(key) ? undefined : value,
    ),
  );
}

// 実queryとsafeFileのレビュー済みASTに限定する。変更時は同じ境界の再レビューが必要。
const TYPESCRIPT_QUERY_AST_SHA256 =
  "2f86771e099cb671a0a7f184ca9a9188e63f541f2479636fc4812e521e0fa213";

function typeScriptQueryCall(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Record<string, unknown> | undefined {
  if (context?.name !== "scripts/quality/owned-type-projects.ts") return undefined;
  const names = ["safeRelative", "safeFile", "queryTypeProject"];
  const functions = names.map((name) => functionNamed(module, name));
  if (functions.some((fn) => !fn)) return undefined;
  const moduleNodes = astNodes(module);
  if (
    functions.some(
      (fn) =>
        moduleNodes.filter(
          (node) =>
            node.type === "FunctionDeclaration" &&
            identifierKey(node.identifier) === identifierKey(fn?.identifier),
        ).length !== 1,
    )
  )
    return undefined;
  if (canonicalNativeUnitAstDigest(functions) !== TYPESCRIPT_QUERY_AST_SHA256) return undefined;
  const launch = importKey(module, "node:child_process", "execFileSync");
  const paths = nodePathBindings(module);
  const keys = [launch, ...paths, ...functions.map((fn) => identifierKey(fn?.identifier))];
  if (keys.some((key) => !unchangedParameter(module, key))) return undefined;
  const calls = astNodes(functions[2]).filter(
    (node) => node.type === "CallExpression" && identifierKey(node.callee) === launch,
  );
  const call = calls.length === 1 ? calls[0] : undefined;
  const args = callArguments(call, launch);
  const argv = object(args?.[1]);
  const elements = Array.isArray(argv?.elements) ? argv.elements : [];
  return args?.length === 3 &&
    memberOf(args[0], "process:1", "execPath") &&
    argv?.type === "ArrayExpression" &&
    elements.length === 4 &&
    !elements.some((element) => object(element)?.spread) &&
    importDirectoryJoin(object(elements[0])?.expression, paths) ===
      "../../node_modules/typescript/bin/tsc" &&
    literal(object(elements[1])?.expression) === "--showConfig" &&
    literal(object(elements[2])?.expression) === "--project"
    ? call
    : undefined;
}

// 同じ実arrayをspawnとrawログへ渡す既存関数に限定し、汎用const-array推論はしない。
const BIOME_CONTEXT_AST_SHA256 = "b99a3c031a8da67cf61e11d2248401446f7fda9309e62fb3720ace692d9f7002";

function biomeContextCommand(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
  functionName: "contextDiagnostics" | "collectStandardUnusedRawBiome" = "contextDiagnostics",
): { call: Record<string, unknown>; array: Record<string, unknown> } | undefined {
  if (context?.name !== "scripts/runtime-bundles/runtime-unused-check.ts") return undefined;
  const fn = functionNamed(module, functionName);
  if (
    !fn ||
    (functionName === "contextDiagnostics" &&
      canonicalNativeUnitAstDigest(fn) !== BIOME_CONTEXT_AST_SHA256)
  )
    return undefined;
  if (
    astNodes(module).filter(
      (node) =>
        node.type === "FunctionDeclaration" &&
        identifierKey(node.identifier) === identifierKey(fn.identifier),
    ).length !== 1
  )
    return undefined;
  const nodes = astNodes(fn);
  const calls = nodes.filter(
    (node) => processMethod(object(node.callee), new Map())?.method === "spawnSync",
  );
  const call = calls.length === 1 ? calls[0] : undefined;
  const args = Array.isArray(call?.arguments) ? call.arguments : [];
  const key = identifierKey(object(args[0])?.expression);
  const statements = functionStatements(fn);
  const declarations = statements
    .filter((node) => node.type === "VariableDeclaration" && node.kind === "const")
    .flatMap((node) => (Array.isArray(node.declarations) ? node.declarations : []))
    .map(object)
    .filter((node) => identifierKey(node?.id) === key);
  const array = object(declarations[0]?.init);
  const selected =
    call &&
    key &&
    declarations.length === 1 &&
    array?.type === "ArrayExpression" &&
    unchangedParameter(fn, key) &&
    args.length === 2 &&
    !args.some((arg) => object(arg)?.spread)
      ? { call, array }
      : undefined;
  return selected &&
    (functionName === "contextDiagnostics" ||
      standardRawBiomeArrayVerified(fn, selected, key, context))
    ? selected
    : undefined;
}

function standardRawBiomeArrayVerified(
  fn: Record<string, unknown>,
  selected: { call: Record<string, unknown>; array: Record<string, unknown> },
  key: string | undefined,
  context: SourceCodeContext,
): boolean {
  const elements = Array.isArray(selected.array.elements) ? selected.array.elements : [];
  const output = object(object(elements[4])?.expression);
  const parts = Array.isArray(output?.quasis) ? output.quasis : [];
  const expressions = Array.isArray(output?.expressions) ? output.expressions : [];
  const command = ["bun", "run", "biome", "--reporter=json"];
  const snapshot = context.read("package.json");
  if (
    elements.length !== 6 ||
    elements.some((element) => object(element)?.spread) ||
    command.some((value, index) => literal(object(elements[index])?.expression) !== value) ||
    output?.type !== "TemplateLiteral" ||
    parts.length !== 2 ||
    object(parts[0])?.cooked !== "--reporter-file=" ||
    object(parts[1])?.cooked !== "" ||
    expressions.length !== 1 ||
    object(expressions[0])?.type !== "Identifier" ||
    literal(object(elements[5])?.expression) !== "--max-diagnostics=none" ||
    !snapshot ||
    object(object(JSON.parse(snapshot))?.scripts)?.biome !== "biome check --error-on-warnings"
  )
    return false;
  // 同arrayの既知4参照だけを許可し、追加alias・push・別consumerへの流出を拒否する。
  const nodes = astNodes(fn);
  const references = nodes.filter((node) => identifierKey(node) === key);
  const freeze = nodes.find(
    (node) =>
      node.type === "CallExpression" &&
      memberOf(node.callee, "Object:1", "freeze") &&
      identifierKey(
        object(Array.isArray(node.arguments) ? node.arguments[0] : undefined)?.expression,
      ) === key,
  );
  const freezeArgs = Array.isArray(freeze?.arguments)
    ? freeze.arguments.map((arg) => object(arg)?.expression)
    : [];
  const log = nodes.find(
    (node) => node.type === "CallExpression" && memberOf(node.callee, "JSON:1", "stringify"),
  );
  const logValue = object(
    object(Array.isArray(log?.arguments) ? log.arguments[0] : undefined)?.expression,
  );
  const properties = Array.isArray(logValue?.properties) ? logValue.properties : [];
  return (
    references.length === 4 &&
    freezeArgs?.length === 1 &&
    identifierKey(freezeArgs[0]) === key &&
    properties.filter((node) => identifierKey(node) === key).length === 1 &&
    nodes.every(
      (node) =>
        node.type !== "CallExpression" ||
        !astNodes(node.callee).some((target) => identifierKey(target) === key),
    )
  );
}

// 公式launcherのruntime照会だけを登録し、任意のNode評価を許可しない。
function standardBiomeToolCommands(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Set<Record<string, unknown>> {
  const result = new Set<Record<string, unknown>>();
  if (context?.name !== "scripts/runtime-bundles/runtime-unused-check.ts") return result;
  const pin = functionNamed(module, "standardBiomeToolPin"),
    tools = functionNamed(module, "standardBiomeTools");
  if (
    !pin ||
    !tools ||
    canonicalNativeUnitAstDigest(pin) !==
      "d9d214d772554d23b0c51d9dae5ad038bd6536665d246acdf07525496cf72460" ||
    canonicalNativeUnitAstDigest(tools) !==
      "12f01534b71031c731490becfd02df1cec63cdb94558e2050c63137013ceb6ae"
  )
    return result;
  const nativePackage = functionNamed(module, "standardBiomeNativePackage");
  if (
    !nativePackage ||
    canonicalNativeUnitAstDigest(nativePackage) !==
      "566f38f1efe9f61b318ae126a2d7cb5eb346c0c50e9c81e152f7add10717e4a0"
  )
    return result;
  const exec = importKey(module, "node:child_process", "execFileSync");
  for (const call of [...astNodes(tools), ...astNodes(nativePackage)])
    if (callArguments(call, exec)) result.add(call);
  return result;
}

function gitInventoryConsumersVerified(
  module: Record<string, unknown>,
  context: SourceCodeContext,
): boolean {
  const local = functionNamed(module, "assertCurrentGitSourceSet");
  const nativeCalls = astNodes(module).filter((node) =>
    callArguments(node, identifierKey(local?.identifier)),
  );
  if (
    canonicalNativeUnitAstDigest(nativeCalls) !==
    "f6d7d84c736c6c7ccbfc2e4aa62fbb24b6853cb115684babb510a35925f3857a"
  )
    return false;
  const source = context.read("scripts/mermaid/native-cli-managed-inputs.ts");
  if (!source) return false;
  const managed = parseSync(source, { syntax: "typescript", tsx: true }) as unknown as Record<
    string,
    unknown
  >;
  const key = importKey(managed, "./native-build-receipt", "assertCurrentGitSourceSet");
  const managedCalls = astNodes(managed).filter((node) => callArguments(node, key));
  return (
    canonicalNativeUnitAstDigest(managedCalls) ===
    "8802f95529c87305942d1b71c4d18604a69cd00f435b5325557dea898902f3ab"
  );
}

// native receipt の現source集合確認に使う二つのGit読取tupleだけを登録する。
function gitSourceInventoryCalls(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Set<Record<string, unknown>> {
  const result = new Set<Record<string, unknown>>();
  if (context?.name !== "scripts/mermaid/native-build-receipt.ts") return result;
  const inventory = functionNamed(module, "assertCurrentGitSourceSet");
  if (
    !inventory ||
    canonicalNativeUnitAstDigest(inventory) !==
      "b7c9e2ba472c37dca75164d8fe527444e40381bce575451ae6680629565918c5" ||
    !gitInventoryConsumersVerified(module, context)
  )
    return result;
  const exec = importKey(module, "node:child_process", "execFileSync");
  for (const node of astNodes(inventory)) if (callArguments(node, exec)) result.add(node);
  return result;
}

function reviewedFixtureVariables(
  nodes: Record<string, unknown>[],
  units: Readonly<Record<string, string>>,
): Record<string, unknown>[] | undefined {
  const selected: Record<string, unknown>[] = [];
  for (const [name, digest] of Object.entries(units)) {
    const declarations = nodes.filter(
      (node) => node.type === "VariableDeclarator" && object(node.id)?.value === name,
    );
    const declaration = declarations[0];
    if (
      declarations.length !== 1 ||
      !declaration ||
      canonicalNativeUnitAstDigest(declaration) !== digest
    )
      return undefined;
    selected.push(declaration);
  }
  return selected;
}

// scratchへ実ファイルを複製する既存producerと、その同じpathをimportするconsumerを一組で拘束する。
function semanticFixtureImports(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Map<Record<string, unknown>, string[]> {
  const result = new Map<Record<string, unknown>, string[]>();
  if (context?.name !== "scripts/runtime-bundles/runtime-unused-semantic-adapter_test.ts")
    return result;
  const units = {
    physicalOwnerFixture: "e3d2a4f5238eb3e2c932bdc6286b5c67cbc8056b1211f523ae575e2a458d730e",
    actualPhysicalFactory: "c0a4404adce42c06263c954f434b64834ab03e3d2ba088b6adae40975323011a",
  };
  const nodes = astNodes(module),
    fs = defaultImportBinding(module, "node:fs"),
    paths = defaultImportBinding(module, "node:path");
  if (
    !fs ||
    !paths ||
    !unchangedParameter(module, fs.key) ||
    !unchangedParameter(module, paths.key)
  )
    return result;
  const selected = reviewedFixtureVariables(nodes, units);
  if (!selected) return result;
  if (
    selected
      .flatMap(astNodes)
      .some(
        (node) =>
          node.type === "Identifier" &&
          ((node.value === "fs" && identifierKey(node) !== fs.key) ||
            (node.value === "path" && identifierKey(node) !== paths.key)),
      )
  )
    return result;
  const catalog = context.read("scripts/runtime-bundles/runtime-unused-entrypoint-owner.ts");
  const catalogModule = catalog ? parseSync(catalog, { syntax: "typescript" }) : undefined;
  const declarations = astNodes(catalogModule).filter(
    (node) =>
      node.type === "VariableDeclarator" &&
      ["base", "entrypointOwnerPaths"].includes(String(object(node.id)?.value)),
  );
  const entrypoint = importKey(module, "./runtime-unused-entrypoint-owner", "entrypointOwnerPaths");
  if (
    entrypoint &&
    declarations.length === 2 &&
    canonicalNativeUnitAstDigest(declarations) ===
      "ea538f6fbbd72c277c6eae304096fbb524f6516fc8fa6744b4d164b0fcfeeb3c" &&
    unchangedParameter(module, entrypoint)
  ) {
    const calls = astNodes(selected[1]).filter(
      (node) => node.type === "CallExpression" && object(node.callee)?.type === "Import",
    );
    if (calls.length === 1 && calls[0])
      result.set(calls[0], [
        "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid/runtime_definition.ts",
      ]);
  }
  // この二つはSDK偽装を拒否する実JS反例であり、生成内容にも追加のimport/実行を許可しない。
  const loops = nodes.filter(
    (node) =>
      node.type === "ForOfStatement" &&
      canonicalNativeUnitAstDigest(node) ===
        "852e87695c6a51a99b3699644912258fe958636c800edabd56b0f1037bdcb282",
  );
  if (loops.length === 1) {
    const calls = astNodes(loops[0]).filter(
      (node) => node.type === "CallExpression" && object(node.callee)?.type === "Import",
    );
    if (calls.length === 1 && calls[0]) result.set(calls[0], []);
  }
  return result;
}

const DUPLICATION_CLI_FIXTURE = {
  caller: "scripts/quality/duplication-check_test.ts",
  sources: [
    "scripts/quality/duplication-check.ts",
    "scripts/quality/duplication-schema.ts",
    "scripts/quality/owned-source-inventory.ts",
  ],
  moduleAstSha256: "e65943a768e7ab81a1ab0cf207c803a69ce62b5b6841a2a8e44f10e8d9811d57",
};

// 一時repoで現sourceをhash照合してからCLIを起動するtest fixtureだけを実sourceへ結ぶ。
function duplicationFixtureExecutions(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): { calls: Map<Record<string, unknown>, string[]>; errors: string[] } {
  const result = new Map<Record<string, unknown>, string[]>();
  if (context?.name !== DUPLICATION_CLI_FIXTURE.caller) return { calls: result, errors: [] };
  if (DUPLICATION_CLI_FIXTURE.sources.some((source) => context.read(source) === undefined))
    return { calls: result, errors: ["!unresolved-executable"] };
  const fixture = functionNamed(module, "withDuplicationCliFixture");
  const runner = functionNamed(module, "runDuplicationCli");
  if (
    !fixture ||
    !runner ||
    canonicalNativeUnitAstDigest(module) !== DUPLICATION_CLI_FIXTURE.moduleAstSha256
  )
    return { calls: result, errors: ["!unresolved-executable"] };
  const calls = astNodes(runner).filter((node) => {
    const callee = object(node.callee);
    return (
      node.type === "CallExpression" &&
      callee?.type === "MemberExpression" &&
      object(callee.object)?.value === "Bun" &&
      object(callee.property)?.value === "spawnSync"
    );
  });
  if (calls.length === 1 && calls[0]) result.set(calls[0], ["!execute-local:duplication-check.ts"]);
  return {
    calls: result,
    errors: result.size === 1 ? [] : ["!unresolved-executable"],
  };
}

export function externalToolBindingMatches(
  request: ExternalToolRequest,
  binding: ExternalToolBinding,
): boolean {
  return (
    Object.entries(request).every(([key, value]) => Reflect.get(binding, key) === value) &&
    binding.packageName === "typescript" &&
    binding.packageVersion === "7.0.2" &&
    binding.verification === "observed-local-pins" &&
    binding.artifactVerified === false &&
    binding.inputPins.length > 0 &&
    binding.inputPins.every(
      (pin) => pin.physicalPath.length > 0 && /^[a-f0-9]{64}$/.test(pin.sha256),
    ) &&
    binding.runtime.kind === "bun" &&
    binding.runtime.path.length > 0 &&
    /^[a-f0-9]{64}$/.test(binding.runtime.sha256) &&
    binding.lockIdentities.length > 0 &&
    binding.lockIdentities.every(
      (identity) => identity.defaultRegistry && identity.integrity.startsWith("sha512-"),
    )
  );
}

function nativeUnitProgramVerified(
  module: Record<string, unknown>,
  source: string,
  context: SourceCodeContext,
): boolean {
  if (!reviewedUnitSourceMatches(source, module, NATIVE_UNIT_PROGRAM, "wrapper")) return false;
  const caller = context.importerSource;
  if (
    caller === undefined ||
    caller !== context.read(NATIVE_UNIT_PROGRAM.caller) ||
    !reviewedUnitSourceMatches(caller, undefined, NATIVE_UNIT_PROGRAM, "caller")
  )
    return false;
  return true;
}

function reviewedUnitSourceMatches(
  source: string,
  module: unknown,
  program: {
    wrapperSourceSha256: string;
    wrapperAstSha256: string;
    callerSourceSha256: string;
    callerAstSha256: string;
    swcVersion: string;
  },
  role: "wrapper" | "caller",
): boolean {
  const expectedSource =
    role === "wrapper" ? program.wrapperSourceSha256 : program.callerSourceSha256;
  const expectedAst = role === "wrapper" ? program.wrapperAstSha256 : program.callerAstSha256;
  if (sourceDigest(source) !== expectedSource || swcVersion !== program.swcVersion) return false;
  try {
    const ast = module ?? parseSync(source, { syntax: "typescript", tsx: true });
    return canonicalNativeUnitAstDigest(ast) === expectedAst;
  } catch {
    return false;
  }
}

function nativeUnitRoute(
  module: Record<string, unknown>,
  source: string,
  context: SourceCodeContext,
): string | undefined {
  if (
    context.name !== NATIVE_UNIT_PROGRAM.wrapper ||
    context.importer !== NATIVE_UNIT_PROGRAM.caller ||
    !nativeUnitProgramVerified(module, source, context)
  )
    return undefined;
  const binary = parameterKey(functionNamed(module, "spawnNativeCli"), 0);
  if (!binary) return undefined;
  context.onUnitBinding?.({
    ...NATIVE_UNIT_PROGRAM,
    proofKind: "reviewed-canonical-unit-source",
    artifactVerified: false,
    executions: [
      { kind: "unit-self-runtime-version", executable: "process.execPath", argv: ["--version"] },
      {
        kind: "unit-known-absent-temp-path",
        executable: "streamFixture().missingBinary",
        argv: [[], ["--version"]],
      },
      {
        kind: "unit-self-runtime-temp-environment",
        executable: "process.execPath",
        argv: ["-e", "childProgram"],
        temporaryDirectory: "tempRoots.entries().tempRoot",
      },
    ],
  });
  return binary;
}

function runtimeAssetCompressorUnitCall(
  module: Record<string, unknown>,
  source: string,
  context: SourceCodeContext | undefined,
): Record<string, unknown> | undefined {
  const program = RUNTIME_ASSET_COMPRESSOR_UNIT_PROGRAM;
  if (
    context?.name !== program.caller ||
    context.importer !== undefined ||
    !reviewedUnitSourceMatches(source, module, program, "caller")
  )
    return undefined;
  const wrapper = context.read(program.wrapper);
  const calls = astNodes(module).filter(
    (node) => node.type === "CallExpression" && memberOf(node.callee, "Bun:1", "spawn"),
  );
  const runner = functionNamed(module, "runCompressorCli");
  if (
    wrapper === undefined ||
    !reviewedUnitSourceMatches(wrapper, undefined, program, "wrapper") ||
    calls.length !== 1 ||
    !runner ||
    !runtimeAssetCompressorLaunchShape(module, calls[0], runner) ||
    !runtimeAssetCompressorArgvVectors(module)
  )
    return undefined;
  context.onUnitBinding?.({
    proofKind: "reviewed-canonical-unit-source",
    artifactVerified: false,
    caller: program.caller,
    callerSourceSha256: program.callerSourceSha256,
    callerAstSha256: program.callerAstSha256,
    wrapper: program.wrapper,
    wrapperSourceSha256: program.wrapperSourceSha256,
    wrapperAstSha256: program.wrapperAstSha256,
    swcVersion: program.swcVersion,
    executions: [
      {
        kind: "unit-runtime-asset-compressor-cli",
        executable: "process.execPath",
        script: "scripts/runtime-assets/runtime-package-asset-compressor.ts",
        argv: [
          ["--write", "--zenuml-only"],
          ["--zenuml-only", "--check"],
          ["--check", "--zenuml-only"],
        ],
        cwd: "temporaryRoot()",
        stdio: { stdout: "pipe", stderr: "pipe" },
      },
    ],
  });
  return calls[0];
}

function chromeFontIdentityProbeCall(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Record<string, unknown> | undefined {
  const importer = context?.importer;
  if (
    context?.name !== CHROME_FONT_IDENTITY_PROBE.module ||
    !CHROME_FONT_IDENTITY_PROBE.importers.some((candidate) => candidate === importer) ||
    swcVersion !== CHROME_FONT_IDENTITY_PROBE.swcVersion
  )
    return undefined;
  const probe = functionNamed(module, CHROME_FONT_IDENTITY_PROBE.functionName);
  if (
    !probe ||
    canonicalNativeUnitAstDigest(probe) !== CHROME_FONT_IDENTITY_PROBE.functionAstSha256
  )
    return undefined;
  const processes = processBindings(module);
  const calls = astNodes(probe).filter((node) => {
    if (node.type !== "CallExpression") return false;
    const method = processMethod(object(node.callee), processes);
    return method?.api === "node" && method.method === "spawnSync";
  });
  return calls.length === 1 ? calls[0] : undefined;
}

function runtimeAssetCompressorLaunchShape(
  module: Record<string, unknown>,
  spawn: Record<string, unknown> | undefined,
  runner: Record<string, unknown>,
): boolean {
  const args = Array.isArray(spawn?.arguments) ? spawn.arguments : [];
  const command = object(object(args[0])?.expression);
  const elements = Array.isArray(command?.elements) ? command.elements : [];
  const scriptCall = object(object(elements[2])?.expression);
  const fileURLToPath = importKey(module, "node:url", "fileURLToPath");
  const url = object(callArguments(scriptCall, fileURLToPath)?.[0]);
  const urlArgs = Array.isArray(url?.arguments) ? url.arguments : [];
  const meta = object(object(urlArgs[1])?.expression);
  const options = object(object(args[1])?.expression);
  const properties = Array.isArray(options?.properties)
    ? options.properties.map(object).filter((item): item is Record<string, unknown> => !!item)
    : [];
  const option = (name: string) =>
    properties.find((item) => item.type === "KeyValueProperty" && object(item.key)?.value === name);
  const root = parameterKey(runner, 0);
  const argsParameter = parameterKey(runner, 1);
  return (
    args.length === 2 &&
    command?.type === "ArrayExpression" &&
    elements.length === 4 &&
    memberOf(object(elements[0])?.expression, "process:1", "execPath") &&
    literal(object(elements[1])?.expression) === "run" &&
    scriptCall?.type === "CallExpression" &&
    identifierKey(scriptCall.callee) === fileURLToPath &&
    url?.type === "NewExpression" &&
    object(url.callee)?.value === "URL" &&
    urlArgs.length === 2 &&
    literal(object(urlArgs[0])?.expression) === "./runtime-package-asset-compressor.ts" &&
    meta?.type === "MemberExpression" &&
    object(meta.object)?.kind === "import.meta" &&
    object(meta.property)?.value === "url" &&
    !!object(elements[3])?.spread &&
    identifierKey(object(elements[3])?.expression) === argsParameter &&
    properties.length === 3 &&
    identifierKey(option("cwd")?.value) === root &&
    literal(option("stdout")?.value) === "pipe" &&
    literal(option("stderr")?.value) === "pipe"
  );
}

function runtimeAssetCompressorArgvVectors(
  module: Record<string, unknown>,
): string[][] | undefined {
  const runner = functionNamed(module, "runCompressorCli");
  const runnerKey = identifierKey(runner?.identifier);
  const rootKey = parameterKey(runner, 0);
  const calls = astNodes(module).filter(
    (node) => node.type === "CallExpression" && identifierKey(node.callee) === runnerKey,
  );
  if (!runnerKey || !rootKey || calls.length !== 3) return undefined;
  const vectors = calls.map((call) => {
    const args = callArguments(call, runnerKey);
    const array = object(args?.[1]);
    const elements = Array.isArray(array?.elements) ? array.elements : [];
    if (args?.length !== 2 || object(args[0])?.value !== "root" || elements.length !== 2)
      return undefined;
    const values = elements.map((item) => literal(object(item)?.expression));
    return values.every((value): value is string => typeof value === "string") ? values : undefined;
  });
  const expected = [
    ["--write", "--zenuml-only"],
    ["--zenuml-only", "--check"],
    ["--check", "--zenuml-only"],
  ];
  return vectors.every(
    (vector, index) =>
      vector !== undefined && JSON.stringify(vector) === JSON.stringify(expected[index]),
  )
    ? (vectors as string[][])
    : undefined;
}

export const CLASSIC_FACTS_TEST = "scripts/runtime-bundles/runtime-unused-rust-classic_test.ts";

export function normalizeClassicFactsTestCommands(lines: string[]): string[] | undefined {
  const testPath = `./${CLASSIC_FACTS_TEST}`;
  const matches = lines.flatMap((line, index) =>
    shellWords(line).includes(testPath) ? [index] : [],
  );
  if (matches.length === 0) return [...lines];
  if (matches.length !== 1) return undefined;
  const index = matches[0];
  if (index === undefined) return undefined;
  const line = lines[index];
  if (!line) return undefined;
  const words = shellWords(line);
  if (
    words[0] !== "bun" ||
    words[1] !== "test" ||
    words[2] !== "--path-ignore-patterns" ||
    words[3] !== "tmp/**" ||
    words[4] !== "./scripts/runtime-bundles/runtime-fragment-loader_test.ts" ||
    words.at(-1) !== testPath ||
    words.at(-2) !== "./scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts" ||
    words.filter((word) => word === testPath).length !== 1 ||
    line.split(testPath).length !== 2
  )
    return undefined;
  const normalized = [...lines];
  normalized[index] = line.replace(` ${testPath}`, "");
  return normalized[index] === line ? undefined : normalized;
}

export const FACTS_CARGO_GUARD_SOURCE = "scripts/build/cargo_capacity_guard.py";
const FACTS_JUST_DEFINITIONS = [
  'set shell := ["bash", "-uc"]',
  "REPO_ROOT := justfile_directory()",
  'RTK := env_var_or_default("RTK", `command -v rtk 2> /dev/null || true`)',
  'RTK_CMD := if RTK == "" { "" } else { RTK + " " }',
  'CARGO := env_var_or_default("CARGO", RTK_CMD + "cargo")',
  'CARGO_BUILD := "python3 " + quote(REPO_ROOT + "/scripts/build/cargo_capacity_guard.py") + " --retain-build-profile debug -- " + CARGO',
] as const;
const FACTS_BUILD_COMMAND =
  '@CARGO_TARGET_DIR={{quote(root + "/target")}} {{CARGO_BUILD}} build --locked -p kdr-linter --bin mathjax_consumer_facts';
const FACTS_RECIPE_MODES = {
  "native-width-hash-facts": "native-width",
  "native-width-classic-facts": "native-width-classic",
  "mermaid-classic-script-facts": "classic",
  "mathjax-consumer-facts": "mathjax",
} as const;

export function guardedFactsRecipeVerified(
  source: string,
  recipe: keyof typeof FACTS_RECIPE_MODES,
): boolean {
  const lines = source.split(/\r?\n/u);
  const settings = lines.filter((line) => /^\s*set\s+shell(?:\s|:|=)/u.test(line));
  if (settings.length !== 1 || settings[0] !== FACTS_JUST_DEFINITIONS[0]) return false;
  const names = ["REPO_ROOT", "RTK", "RTK_CMD", "CARGO", "CARGO_BUILD"] as const;
  for (const [index, name] of names.entries()) {
    const definitions = lines.filter((line) =>
      new RegExp(`^\\s*(?:export\\s+)?${name}\\b`, "u").test(line),
    );
    if (definitions.length !== 1 || definitions[0] !== FACTS_JUST_DEFINITIONS[index + 1])
      return false;
  }
  const headers = lines.filter(
    (line) => line.startsWith(`${recipe} `) || line.startsWith(`${recipe}:`),
  );
  const commands = (justRecipes(source).get(recipe) ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const mode = FACTS_RECIPE_MODES[recipe];
  const host = justRecipes(source);
  const hostCommands = (host.get("rust-facts-host-build") ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const metadataCommands = (host.get("rust-facts-host-metadata") ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const hostHeaders = lines.filter((line) =>
    /^rust-facts-host-(?:build|metadata)(?: |:)/u.test(line),
  );
  return (
    headers.length === 1 &&
    headers[0] ===
      `${recipe} root=REPO_ROOT${recipe === "mathjax-consumer-facts" ? "" : " source_root=REPO_ROOT"}:` &&
    hostHeaders.length === 2 &&
    hostHeaders.includes("rust-facts-host-build root=REPO_ROOT:") &&
    hostHeaders.includes("rust-facts-host-metadata root=REPO_ROOT:") &&
    hostCommands.length === 1 &&
    hostCommands[0] === FACTS_BUILD_COMMAND &&
    metadataCommands.length === 1 &&
    metadataCommands[0] ===
      '@CARGO_TARGET_DIR={{quote(root + "/target")}} {{CARGO}} metadata --locked --format-version 1' &&
    commands.length === 1 &&
    commands[0] ===
      `@CARGO_TARGET_DIR={{quote(root + "/target")}} bun {{quote(root + "/scripts/runtime-bundles/runtime-rust-facts-host.ts")}} run {{quote(root)}} ${mode} ${recipe === "mathjax-consumer-facts" ? "{{quote(root)}}" : "{{quote(source_root)}}"} {{quote(root + "/tmp/quality/rust-facts-host/current/receipt.json")}}`
  );
}

const NATIVE_WIDTH_HASH_CALLER = "scripts/runtime-bundles/runtime-unused-native-width-hash.ts";

const NATIVE_WIDTH_CONSUMER_AST = {
  readNativeWidthPhysicalInputs: "40e1e85daf01047b426729d6ac8806afb14f617f5789929c756eee9f7de9306e",
  requireSourceSubset: "abe613fc9fba49248ca4e10032fa6ad3087ce85e9bf661c44795cb7ef3b93ccf",
  assertStablePhysicalInputs: "c2610c33fdfc7380a65aeb44cfadf19af1c193fd86cc5b6bef76919e690498a5",
  validateNativeWidthHashFacts: "3f58b9222722e6e65daa3cac6307e57fd442e23e84008fca8f03797b0569c42d",
  parseNativeWidthHashFactsOutput:
    "93fb8ae9308e22324620741fb915b5e8e81be83e075c85cae457147959a2d16a",
  inspectNativeWidthConsumerFacts:
    "4d9bd0bf0501ccd998ca3126f91a6f12cce2595d642b769154bb8d313a48d6df",
} as const;

function nativeWidthConsumerFactsVerified(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): boolean {
  if (context?.name !== NATIVE_WIDTH_HASH_CALLER) return false;
  if (!factsDeclarationsVerified(module, NATIVE_WIDTH_CONSUMER_AST)) return false;
  const fn = functionNamed(module, "inspectNativeWidthConsumerFacts");
  if (!fn) return false;
  const path = defaultImportBinding(module, "node:path");
  const fs = defaultImportBinding(module, "node:fs");
  const repository = namedCallInitializer(fn, "repositoryRoot", path?.key, "resolve");
  const actualSource = namedCallInitializer(fn, "actualSourceRoot", fs?.key, "realpathSync");
  const child = namedCallInitializer(fn, "child", "Bun:1", "spawnSync");
  const calls = astNodes(fn).filter(bunSpawnSyncCall);
  const classicReader = importKey(module, "./runtime-unused-rust-classic", "physicalInputs");
  const nativeReader = identifierKey(
    functionNamed(module, "readNativeWidthPhysicalInputs")?.identifier,
  );
  const physicalCalls = (key: string | undefined) =>
    key
      ? astNodes(fn).filter(
          (node) => node.type === "CallExpression" && identifierKey(node.callee) === key,
        )
      : [];
  const classicReads = physicalCalls(classicReader);
  const nativeReads = physicalCalls(nativeReader);
  const justfile = context.read("Justfile");
  return (
    !!fn &&
    !!path &&
    !!fs &&
    !!repository &&
    !!actualSource &&
    !!child &&
    repository.index === 0 &&
    actualSource.index === 1 &&
    calls.length === 1 &&
    calls[0] === child.call &&
    nativeWidthSpawnArgumentsVerified(
      child.call,
      repository,
      actualSource,
      "native-width-classic-facts",
    ) &&
    classicReads.length === 2 &&
    nativeReads.length === 2 &&
    !!justfile &&
    guardedFactsRecipeVerified(justfile, "native-width-classic-facts") &&
    !!importKey(module, "./runtime-unused-rust-classic", "validateOutput") &&
    !!functionNamed(module, "validateNativeWidthHashFacts")
  );
}

function nativeWidthConsumerFactsCall(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Record<string, unknown> | undefined {
  if (!nativeWidthConsumerFactsVerified(module, context)) return undefined;
  const fn = functionNamed(module, "inspectNativeWidthConsumerFacts");
  if (!fn) return undefined;
  const fs = defaultImportBinding(module, "node:fs");
  const child = namedCallInitializer(fn, "child", "Bun:1", "spawnSync");
  const calls = fn ? astNodes(fn).filter(bunSpawnSyncCall) : [];
  return fs && child && calls.length === 1 && child.call === calls[0] ? child.call : undefined;
}

function defaultImportBinding(
  module: Record<string, unknown>,
  source: string,
): { key: string; local: Record<string, unknown> } | undefined {
  const imports = (Array.isArray(module.body) ? module.body : [])
    .map(object)
    .filter(
      (node) =>
        node?.type === "ImportDeclaration" && !node.typeOnly && literal(node.source) === source,
    );
  if (imports.length !== 1) return undefined;
  const specifiers = Array.isArray(imports[0]?.specifiers) ? imports[0]?.specifiers : [];
  const defaults = specifiers.map(object).filter((node) => node?.type === "ImportDefaultSpecifier");
  const local = object(defaults[0]?.local);
  const key = identifierKey(local);
  return defaults.length === 1 && local && key ? { key, local } : undefined;
}

function memberCallArguments(
  value: unknown,
  objectKey: string | undefined,
  property: string,
): unknown[] | undefined {
  const node = object(value);
  const args = Array.isArray(node?.arguments) ? node.arguments : [];
  if (
    node?.type !== "CallExpression" ||
    object(node.callee)?.computed === true ||
    !memberOf(node.callee, objectKey, property) ||
    args.some((arg) => object(arg)?.spread)
  )
    return undefined;
  return args.map((arg) => object(arg)?.expression);
}

function importMetaDirectory(value: unknown): boolean {
  const node = object(value);
  const meta = object(node?.object);
  return (
    node?.type === "MemberExpression" &&
    meta?.type === "MetaProperty" &&
    meta.kind === "import.meta" &&
    object(node.property)?.type === "Identifier" &&
    object(node.property)?.value === "dir"
  );
}

function namedCallInitializer(
  fn: Record<string, unknown>,
  name: string,
  objectKey: string | undefined,
  property: string,
):
  | {
      args: unknown[];
      binding: string;
      call: Record<string, unknown>;
      identifier: Record<string, unknown>;
      index: number;
      memberObject: Record<string, unknown>;
    }
  | undefined {
  const statements = functionStatements(fn);
  const matches = statements.flatMap((statement, index) => {
    if (statement.type !== "VariableDeclaration" || statement.kind !== "const") return [];
    const declarations = Array.isArray(statement.declarations) ? statement.declarations : [];
    return declarations.flatMap((value) => {
      const declaration = object(value);
      const args = memberCallArguments(declaration?.init, objectKey, property);
      const binding = identifierKey(declaration?.id);
      const call = object(declaration?.init);
      const identifier = object(declaration?.id);
      const memberObject = object(object(call?.callee)?.object);
      return object(declaration?.id)?.value === name &&
        args &&
        binding &&
        call &&
        identifier &&
        memberObject
        ? [{ args, binding, call, identifier, index, memberObject }]
        : [];
    });
  });
  return matches.length === 1 ? matches[0] : undefined;
}

function bunSpawnSyncCall(node: Record<string, unknown>): boolean {
  const callee = object(node.callee);
  return (
    node.type === "CallExpression" &&
    callee?.type === "MemberExpression" &&
    object(callee.object)?.type === "Identifier" &&
    object(callee.object)?.value === "Bun" &&
    object(callee.property)?.type === "Identifier" &&
    object(callee.property)?.value === "spawnSync"
  );
}

function identifierUsesAre(
  root: unknown,
  key: string | undefined,
  allowed: readonly Record<string, unknown>[],
): boolean {
  if (!key) return false;
  const uses = astNodes(root).filter((node) => identifierKey(node) === key);
  return uses.length === allowed.length && allowed.every((node) => uses.includes(node));
}

function unshadowedGlobalMemberCalls(
  module: Record<string, unknown>,
  name: string,
  property: string,
): Record<string, unknown>[] | undefined {
  const key = `${name}:1`;
  const calls = astNodes(module).filter((node) => memberCallArguments(node, key, property));
  const members = calls.map((call) => object(object(call.callee)?.object));
  const valid = members.filter((node): node is Record<string, unknown> => node !== undefined);
  return valid.length === calls.length && identifierUsesAre(module, key, valid) ? calls : undefined;
}

function nativeWidthModuleBindingsVerified(
  module: Record<string, unknown>,
  pathBinding: ReturnType<typeof defaultImportBinding>,
  fsBinding: ReturnType<typeof defaultImportBinding>,
  repository: ReturnType<typeof namedCallInitializer>,
  actualSource: ReturnType<typeof namedCallInitializer>,
  child: ReturnType<typeof namedCallInitializer>,
): boolean {
  const objectKeyCalls = unshadowedGlobalMemberCalls(module, "Object", "keys");
  const paired = functionNamed(module, "inspectNativeWidthConsumerFacts");
  const reader = functionNamed(module, "readNativeWidthPhysicalInputs");
  const globalEscape = astNodes(module).some(
    (node) =>
      node.type === "Identifier" &&
      ["Reflect", "globalThis", "global", "window", "self"].includes(String(node.value)),
  );
  if (
    !pathBinding ||
    !fsBinding ||
    !repository ||
    !actualSource ||
    !child ||
    !paired ||
    !reader ||
    objectKeyCalls?.length !== 2
  )
    return false;
  // 新しいhelper内の参照は有限ASTで拘束し、他のmodule領域へのescapeは認めない。
  const helperUses = (key: string) =>
    [paired, reader].flatMap((scope) =>
      astNodes(scope).filter((node) => identifierKey(node) === key),
    );
  return (
    !globalEscape &&
    identifierUsesAre(module, pathBinding.key, [
      pathBinding.local,
      repository.memberObject,
      ...helperUses(pathBinding.key),
    ]) &&
    identifierUsesAre(module, fsBinding.key, [
      fsBinding.local,
      actualSource.memberObject,
      ...helperUses(fsBinding.key),
    ]) &&
    identifierUsesAre(module, "Bun:1", [child.memberObject, ...helperUses("Bun:1")])
  );
}

function nativeWidthLocalBindingsVerified(
  fn: Record<string, unknown>,
  sourceRoot: string | undefined,
  sourceCanonical: unknown[] | undefined,
  repository: ReturnType<typeof namedCallInitializer>,
  actualSource: ReturnType<typeof namedCallInitializer>,
  argvValues: unknown[],
  cwd: Record<string, unknown> | undefined,
): boolean {
  if (!sourceRoot || !sourceCanonical || !repository || !actualSource) return false;
  const sourceParameter = object(object(Array.isArray(fn.params) ? fn.params[0] : undefined)?.pat);
  const sourceArgument = object(sourceCanonical[0]);
  const repositoryArgument = object(argvValues[2]);
  const canonicalArgument = object(argvValues[3]);
  const cwdArgument = object(cwd?.value);
  if (
    !sourceParameter ||
    !sourceArgument ||
    !repositoryArgument ||
    !canonicalArgument ||
    !cwdArgument
  )
    return false;
  return (
    sourceCanonical.length === 1 &&
    identifierKey(sourceArgument) === sourceRoot &&
    unchangedParameter(fn, sourceRoot) &&
    identifierUsesAre(fn, sourceRoot, [sourceParameter, sourceArgument]) &&
    unchangedParameter(fn, repository.binding) &&
    identifierUsesAre(fn, repository.binding, [
      repository.identifier,
      repositoryArgument,
      cwdArgument,
    ]) &&
    unchangedParameter(fn, actualSource.binding) &&
    identifierUsesAre(fn, actualSource.binding, [actualSource.identifier, canonicalArgument])
  );
}

function nativeWidthDirectSpawnVerified(
  fn: Record<string, unknown>,
  child: ReturnType<typeof namedCallInitializer>,
  candidate: Record<string, unknown> | undefined,
): candidate is Record<string, unknown> {
  if (!child || !candidate || child.call !== candidate) return false;
  const statements = functionStatements(fn);
  const statement = statements[child.index];
  const declarations = Array.isArray(statement?.declarations) ? statement.declarations : [];
  return (
    astNodes(fn).filter(bunSpawnSyncCall).length === 1 &&
    // 承認済みの root 定義以外を spawn 前に許すと、終了文や別 initializer で実行を迂回できる。
    child.index === 2 &&
    statements
      .slice(0, child.index)
      .every(
        (prefix) =>
          prefix.type === "VariableDeclaration" &&
          prefix.kind === "const" &&
          Array.isArray(prefix.declarations) &&
          prefix.declarations.length === 1,
      ) &&
    statement?.type === "VariableDeclaration" &&
    statement.kind === "const" &&
    declarations.length === 1
  );
}

function nativeWidthSpawnArgumentsVerified(
  call: Record<string, unknown>,
  repository: ReturnType<typeof namedCallInitializer>,
  actualSource: ReturnType<typeof namedCallInitializer>,
  recipe = "native-width-hash-facts",
): boolean {
  if (!repository || !actualSource) return false;
  const args = Array.isArray(call.arguments) ? call.arguments : [];
  const command = object(object(args[0])?.expression);
  const argv = Array.isArray(command?.elements) ? command.elements : [];
  const values = argv.map((item) => object(object(item)?.expression));
  const options = object(object(args[1])?.expression);
  const properties = Array.isArray(options?.properties) ? options.properties.map(object) : [];
  const cwd = properties.find((item) => object(item?.key)?.value === "cwd");
  const timeout = properties.find((item) => object(item?.key)?.value === "timeout");
  return (
    args.length === 2 &&
    command?.type === "ArrayExpression" &&
    argv.length === 4 &&
    argv.every((item) => !object(item)?.spread) &&
    literal(values[0]) === "just" &&
    literal(values[1]) === recipe &&
    identifierKey(values[2]) === repository.binding &&
    identifierKey(values[3]) === actualSource.binding &&
    options?.type === "ObjectExpression" &&
    properties.length === 2 &&
    identifierKey(cwd?.value) === repository.binding &&
    object(timeout?.value)?.type === "NumericLiteral" &&
    object(timeout?.value)?.value === 120000
  );
}

function nativeWidthHashCall(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Record<string, unknown> | undefined {
  if (context?.name !== NATIVE_WIDTH_HASH_CALLER) return undefined;
  const fn = functionNamed(module, "inspectNativeWidthHashFacts");
  if (
    !fn ||
    astNodes(module).filter(
      (node) =>
        node.type === "FunctionDeclaration" &&
        identifierKey(node.identifier) === identifierKey(fn.identifier),
    ).length !== 1
  )
    return undefined;
  const path = defaultImportBinding(module, "node:path");
  const fs = defaultImportBinding(module, "node:fs");
  const sourceRoot = parameterKey(fn, 0);
  const repository = namedCallInitializer(fn, "repositoryRoot", path?.key, "resolve");
  const actualSource = namedCallInitializer(fn, "actualSourceRoot", fs?.key, "realpathSync");
  const child = namedCallInitializer(fn, "child", "Bun:1", "spawnSync");
  const calls = astNodes(fn).filter(bunSpawnSyncCall);
  const call = calls.length === 1 && child && child.call === calls[0] ? child.call : undefined;
  const sourcePath = repository?.args;
  const sourceCanonical = actualSource?.args;
  if (
    !repository ||
    !actualSource ||
    !child ||
    repository.index !== 0 ||
    actualSource.index !== 1 ||
    !nativeWidthModuleBindingsVerified(module, path, fs, repository, actualSource, child) ||
    !nativeWidthDirectSpawnVerified(fn, child, call)
  )
    return undefined;
  const args = Array.isArray(call.arguments) ? call.arguments : [];
  const command = object(object(args[0])?.expression);
  const argv = Array.isArray(command?.elements) ? command.elements : [];
  const argvValues = argv.map((item) => object(object(item)?.expression));
  const options = object(object(args[1])?.expression);
  const properties = Array.isArray(options?.properties) ? options.properties.map(object) : [];
  const cwd = properties.find((item) => object(item?.key)?.value === "cwd");
  if (
    sourcePath?.length !== 2 ||
    !importMetaDirectory(sourcePath[0]) ||
    literal(sourcePath[1]) !== "../.." ||
    !nativeWidthLocalBindingsVerified(
      fn,
      sourceRoot,
      sourceCanonical,
      repository,
      actualSource,
      argvValues,
      cwd,
    ) ||
    !nativeWidthSpawnArgumentsVerified(call, repository, actualSource)
  )
    return undefined;
  return call;
}

const CLASSIC_FACTS_CALLER = "scripts/runtime-bundles/runtime-unused-rust-classic.ts";
// 実readerと入力検証helperの有限ASTに限定し、未知の前処理や終了文を認定しない。
const CLASSIC_FACTS_AST = {
  manifestSchema: "4f3bc09febd0fa4ab07cc9d44be39c800247f103ba466ba29b1c606addcbab19",
  rangeSchema: "1669679b1b5636b2cb4afab69338d466a48184e6562763c207d76768160e4c4b",
  RUST_CLASSIC_INPUT_PATHS: "2dcb0b5a3637d13a17a99114d822bfa2341d3872abda04a6c63e91ac3d146dc7",
  factPaths: "9159a6ee11e1cb39f40082b3428a97b07a2f34b2678e8dd377a68571eb58c348",
  scriptEntry: "60b243937e96cddd057ad490114fb07bab5823963c86310fc11641821d5199f6",
  relativeIdentity: "51533ef50bf54562579d7cec2781eb1c865e62198d14dbed835675d9af901974",
  reportSchema: "32eb476cb80bb83d377219a546f42c04e12d484add95ca0b6c99493e2a2b0e74",
  physicalInputs: "3b0dcedcca61b01578f9248e5a98d1b226450761e74420e71daeb0af46d5d97c",
  validateRange: "bbcb60566a7321003ebcdba19185b038a5a0900fba8b45a19e17c75292d68377",
  validateOutput: "4db5e1eb75453021da952a2da27e3a9cd5ad9cc0baa71434b424e2ae72cfe6f0",
  inspectRustClassicScriptFacts: "effab2aca90d69ebdc300603e05fc8dd888d8b92b007c33cd2d4c215cf7d9d02",
} as const;

function factsDeclarationsVerified(
  module: Record<string, unknown>,
  expectedNodes: Readonly<Record<string, string>>,
): boolean {
  const body = Array.isArray(module.body) ? module.body.map(object) : [];
  const declarations = body.map((node) =>
    node?.type === "ExportDeclaration" ? object(node.declaration) : node,
  );
  for (const [name, expected] of Object.entries(expectedNodes)) {
    const nodes = declarations.filter((node) =>
      node?.type === "FunctionDeclaration"
        ? object(node.identifier)?.value === name
        : node?.type === "VariableDeclaration" &&
          Array.isArray(node.declarations) &&
          node.declarations.length === 1 &&
          object(object(node.declarations[0])?.id)?.value === name,
    );
    if (nodes.length !== 1 || canonicalNativeUnitAstDigest(nodes[0]) !== expected) return false;
  }
  return true;
}

function classicSchemaEntriesVerified(
  module: Record<string, unknown>,
  variables: readonly (Record<string, unknown> | undefined)[],
): boolean {
  const entryCalls = [
    ["runtimeEntry", "mermaid-runtime.min.js", "MERMAID_RUNTIME"],
    ["vendorEntry", "mermaid.min.js", "bundle"],
    ["pluginEntry", "mermaid-zenuml.min.js", "zenuml_bundle"],
    ["zenumlEntry", "zenuml-runtime.min.js", "ZENUML_RUNTIME"],
    ["renderEntry", "render-mermaid.js", "request_json"],
  ];
  const entryFunction = functionNamed(module, "scriptEntry");
  for (const [name, file, binding] of entryCalls) {
    const declaration = variables.find(
      (node) =>
        Array.isArray(node?.declarations) &&
        object(object(node.declarations[0])?.id)?.value === name,
    );
    const list = Array.isArray(declaration?.declarations) ? declaration.declarations : [];
    const args = callArguments(object(list[0])?.init, identifierKey(entryFunction?.identifier));
    if (
      !args ||
      args.length !== (name === "renderEntry" ? 3 : 2) ||
      literal(args[0]) !== file ||
      literal(args[1]) !== binding ||
      (name === "renderEntry" && object(args[2])?.value !== true)
    )
      return false;
  }
  return (
    !!importKey(module, "zod", "z") &&
    !!importKey(module, "./runtime-unused-origin", "physicalUnusedSource") &&
    !!importKey(module, "./runtime-unused-source-map", "Lines")
  );
}

function classicFactsShapesVerified(module: Record<string, unknown>): boolean {
  const body = Array.isArray(module.body) ? module.body.map(object) : [];
  const declarations = body.map((node) =>
    node?.type === "ExportDeclaration" ? object(node.declaration) : node,
  );
  if (!factsDeclarationsVerified(module, CLASSIC_FACTS_AST)) return false;
  const variableNames = [
    "RUST_CLASSIC_INPUT_PATHS",
    "factPaths",
    "rangeSchema",
    "runtimeEntry",
    "vendorEntry",
    "pluginEntry",
    "zenumlEntry",
    "renderEntry",
    "manifestSchema",
    "reportSchema",
  ];
  const variables = declarations.filter((node) => node?.type === "VariableDeclaration");
  if (
    variables.length !== variableNames.length ||
    variables.some(
      (node) =>
        !Array.isArray(node?.declarations) ||
        node.declarations.length !== 1 ||
        !variableNames.includes(String(object(object(node.declarations[0])?.id)?.value)),
    )
  )
    return false;
  if (!classicSchemaEntriesVerified(module, variables)) return false;
  const effects = declarations.filter(
    (node) =>
      node &&
      ![
        "ImportDeclaration",
        "FunctionDeclaration",
        "VariableDeclaration",
        "TsTypeAliasDeclaration",
      ].includes(String(node.type)),
  );
  return effects.length === 0;
}

function classicFactsCall(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Record<string, unknown> | undefined {
  if (context?.name !== CLASSIC_FACTS_CALLER || !classicFactsShapesVerified(module))
    return undefined;
  const justfile = context.read("Justfile");
  if (
    !justfile ||
    !guardedFactsRecipeVerified(justfile, "mermaid-classic-script-facts") ||
    context.read(FACTS_CARGO_GUARD_SOURCE) === undefined
  )
    return undefined;
  const fn = functionNamed(module, "inspectRustClassicScriptFacts");
  if (!fn) return undefined;
  const path = defaultImportBinding(module, "node:path");
  const fs = defaultImportBinding(module, "node:fs");
  const repository = namedCallInitializer(fn, "repositoryRoot", fs?.key, "realpathSync");
  const actualSource = namedCallInitializer(fn, "actualSourceRoot", fs?.key, "realpathSync");
  const child = namedCallInitializer(fn, "child", "Bun:1", "spawnSync");
  const calls = astNodes(module).filter(bunSpawnSyncCall);
  if (
    !path ||
    !fs ||
    !repository ||
    !actualSource ||
    !child ||
    repository.index !== 0 ||
    actualSource.index !== 1 ||
    child.index !== 3 ||
    calls.length !== 1 ||
    calls[0] !== child.call ||
    !nativeWidthSpawnArgumentsVerified(
      child.call,
      repository,
      actualSource,
      "mermaid-classic-script-facts",
    )
  )
    return undefined;
  const allowed = (
    binding: string,
    imported: Record<string, unknown>,
    methods: readonly string[],
  ) => {
    const objects = astNodes(module)
      .flatMap((node) => {
        const callee = object(node.callee);
        return memberCallArguments(node, binding, String(object(callee?.property)?.value)) &&
          methods.includes(String(object(callee?.property)?.value))
          ? [object(callee?.object)]
          : [];
      })
      .filter((node): node is Record<string, unknown> => node !== undefined);
    return identifierUsesAre(module, binding, [imported, ...objects]);
  };
  if (
    !allowed(path.key, path.local, ["resolve", "join"]) ||
    !allowed(fs.key, fs.local, ["realpathSync", "lstatSync", "readFileSync"]) ||
    !identifierUsesAre(module, "Bun:1", [child.memberObject]) ||
    astNodes(module).some(
      (node) =>
        node.type === "Identifier" &&
        ["Reflect", "globalThis", "global", "window", "self"].includes(String(node.value)),
    )
  )
    return undefined;
  return child.call;
}

const MATHJAX_FACTS_CALLER = "scripts/runtime-bundles/runtime-unused-mathjax.ts";
// 既存MathJaxの一root producerと実13facts/入力・範囲validatorだけを拘束する。
const MATHJAX_FACTS_AST = {
  MATHJAX_RUST_FACTS_SCHEMA: "a603a05d5530071950bdd17700f8f9571c59ec09022c0c504cf7911a8e477a10",
  MATHJAX_RUST_FACT_IDS: "df56d3699a8db1732c9f1e8c741bc5a521cc865880e37d90b7f55500ee45665c",
  MATHJAX_RUST_FACT_PATHS: "a3c4ce80f8b89bb031fa984a32313c29cbe7381332fa2fe28f0f226e5a8370db",
  MATHJAX_RUST_INPUT_PATHS: "012e7a6ffe8646584099107cf701fc9dfddfda4a9e442a74a16999b163504cc2",
  rustFactRangeSchema: "1fab1e7e72ee9daba05111c8f8e2d7e65d7f618703261af7816fa65f8f595192",
  mathJaxRustFactsSchema: "72aea7ad0090d9dba0a6c96eb84b69d3b31a0a768a114dc4e6d0f39e34ee5127",
  validateMathJaxRustFacts: "d4312bf6f5c42a5dc697caa193f5aaa35d4c1fc2fb90baf4182e9f5081cc23ee",
  validateMathJaxRustFactRange: "68d6a7e42d70fdb78d7f703eaa0796217b6a41f2e8258c0f379f1ab0436eb1ce",
  parseMathJaxRustFactsOutput: "eb1c1d482ad062557db51596807cc13c1802818987c1e6b526c07a72a3c4bfec",
  inspectMathJaxRustFacts: "74e0bda24cf1301b2095e8278c1851180b2edd7aee709f61d4d2bfced16d339e",
  safeRelative: "a13a33316a5eac5fd7a1fabe62d02911f117688e460a65a6fe6469762f3d5e03",
  readSource: "e90b6e2f1699171640220d66e1237ed827a1dd443a200ad90e58ebc63673f3a3",
  readMathJaxOwnedAuditEvidence: "8bc6692319868129cef25b2fd04eaea2891e03e01cfce6e67f23de5ebd5ba8a3",
} as const;

function factsPathBindingVerified(module: Record<string, unknown>): boolean {
  const binding = defaultImportBinding(module, "node:path");
  if (!binding) return false;
  const nodes = astNodes(module);
  const calls = nodes.filter((node) => node.type === "CallExpression");
  const members = nodes.filter(
    (node) => node.type === "MemberExpression" && identifierKey(node.object) === binding.key,
  );
  const allowed = members.filter((member) => {
    const property = object(member.property);
    if (property?.type !== "Identifier") return false;
    if (property.value === "sep") return true;
    if (["posix", "win32"].includes(String(property.value)))
      return calls.some(
        (call) =>
          object(object(call.callee)?.object) === member &&
          ["isAbsolute", "normalize"].includes(
            String(object(object(call.callee)?.property)?.value),
          ),
      );
    return (
      ["resolve", "join", "relative", "isAbsolute"].includes(String(property.value)) &&
      calls.some((call) => call.callee === member)
    );
  });
  const writes = nodes.some(
    (node) =>
      ["AssignmentExpression", "UpdateExpression"].includes(String(node.type)) &&
      astNodes(node.type === "AssignmentExpression" ? node.left : node.argument).some(
        (target) => identifierKey(target) === binding.key,
      ),
  );
  return (
    !writes &&
    allowed.length === members.length &&
    identifierUsesAre(module, binding.key, [
      binding.local,
      ...allowed
        .map((member) => object(member.object))
        .filter((node): node is Record<string, unknown> => node !== undefined),
    ])
  );
}

function mathjaxFactsCall(
  module: Record<string, unknown>,
  context: SourceCodeContext | undefined,
): Record<string, unknown> | undefined {
  if (
    context?.name !== MATHJAX_FACTS_CALLER ||
    !factsDeclarationsVerified(module, MATHJAX_FACTS_AST) ||
    !factsPathBindingVerified(module)
  )
    return undefined;
  if (
    astNodes(module).some(
      (node) =>
        node.type === "Identifier" &&
        identifierKey(node) === `${String(node.value)}:1` &&
        ["Reflect", "globalThis", "global", "window", "self"].includes(String(node.value)),
    )
  )
    return undefined;
  const justfile = context.read("Justfile");
  if (
    !justfile ||
    !guardedFactsRecipeVerified(justfile, "mathjax-consumer-facts") ||
    context.read(FACTS_CARGO_GUARD_SOURCE) === undefined
  )
    return undefined;
  const fn = functionNamed(module, "inspectMathJaxRustFacts");
  if (!fn) return undefined;
  const fs = defaultImportBinding(module, "node:fs");
  const repository = namedCallInitializer(fn, "repositoryRoot", fs?.key, "realpathSync");
  const child = namedCallInitializer(fn, "child", "Bun:1", "spawnSync");
  const calls = astNodes(module).filter(bunSpawnSyncCall);
  if (
    !fs ||
    !repository ||
    !child ||
    repository.index !== 0 ||
    child.index !== 1 ||
    calls.length !== 1 ||
    calls[0] !== child.call ||
    !identifierUsesAre(module, "Bun:1", [child.memberObject])
  )
    return undefined;
  const fsCalls = astNodes(module)
    .flatMap((node) => {
      const callee = object(node.callee);
      return ["realpathSync", "lstatSync", "readFileSync"].some((method) =>
        memberCallArguments(node, fs.key, method),
      )
        ? [object(callee?.object)]
        : [];
    })
    .filter((node): node is Record<string, unknown> => node !== undefined);
  if (
    !identifierUsesAre(module, fs.key, [fs.local, ...fsCalls]) ||
    !importKey(module, "./runtime-unused-origin", "physicalUnusedSource") ||
    !importKey(module, "./runtime-unused-source-map", "Lines") ||
    !importKey(module, "zod", "z")
  )
    return undefined;
  return child.call;
}

const HTML_NATIVE_MEMBER = "crates/katana-render-runtime";
const HTML_NATIVE_MANIFEST = `${HTML_NATIVE_MEMBER}/Cargo.toml`;
const HTML_NATIVE_CONSUMERS: Array<{
  caller: string;
  function: string;
  additionalFunctions?: string[];
  selector: string | undefined;
  argvClass: NativeConsumerBinding["argvClass"];
  entrypoint: string;
  producer: string;
}> = [
  {
    caller: "scripts/html/dynamic-fidelity.ts",
    function: "captureKrrState",
    additionalFunctions: ["captureKrrCatalogCondition"],
    selector: "writes_fixture_feature_state_interactive",
    argvClass: "html-feature-lib-test",
    entrypoint: "src/lib.rs",
    producer: "src/renderer/backends/html_interactive/tests/feature_diagnostic.rs",
  },
  {
    caller: "scripts/html/toc-fidelity.ts",
    function: "runTocProbe",
    selector: "writes_fixture_toc_diagnostic_when_requested",
    argvClass: "html-toc-lib-test",
    entrypoint: "src/lib.rs",
    producer: "src/renderer/backends/html_interactive/tests/toc_diagnostic.rs",
  },
  {
    caller: "scripts/html/fidelity-suite.ts",
    function: "runProbe",
    selector: undefined,
    argvClass: "html-fidelity-example",
    entrypoint: "examples/html_fidelity_probe.rs",
    producer: "examples/probe/frame.rs",
  },
];

function nativeParameterKey(fn: Record<string, unknown>, index: number): string | undefined {
  const params = Array.isArray(fn.params) ? fn.params : [];
  const pat = object(object(params[index])?.pat);
  return identifierKey(pat?.type === "AssignmentPattern" ? pat.left : pat);
}

function nativeStringArgument(value: unknown, key: string | undefined, property?: string): boolean {
  const args = callArguments(value, "String:1");
  return (
    key !== undefined &&
    args?.length === 1 &&
    (property ? memberOf(args[0], key, property) : identifierKey(args[0]) === key)
  );
}

function nativeScrollArguments(value: unknown, key: string | undefined): boolean {
  const call = object(value);
  const args = Array.isArray(call?.arguments) ? call.arguments : [];
  const arrow = object(object(args[0])?.expression);
  const params = Array.isArray(arrow?.params) ? arrow.params : [];
  const properties = object(params[0])?.properties;
  const prop =
    Array.isArray(properties) && properties.length === 1 ? object(properties[0]) : undefined;
  const requested =
    prop?.type === "AssignmentPatternProperty" &&
    prop.value === null &&
    object(prop.key)?.value === "requested"
      ? identifierKey(prop.key)
      : undefined;
  const body = object(arrow?.body);
  const elements =
    body?.type === "ArrayExpression" && Array.isArray(body.elements) ? body.elements : [];
  return (
    call?.type === "CallExpression" &&
    memberOf(call.callee, key, "flatMap") &&
    args.length === 1 &&
    !object(args[0])?.spread &&
    arrow?.type === "ArrowFunctionExpression" &&
    arrow.async === false &&
    params.length === 1 &&
    requested !== undefined &&
    elements.length === 2 &&
    elements.every((element) => !object(element)?.spread) &&
    literal(object(elements[0])?.expression) === "--scroll-y" &&
    nativeStringArgument(object(elements[1])?.expression, requested)
  );
}

function nativeExampleArguments(elements: unknown[], fn: Record<string, unknown>): boolean {
  const parameter = (index: number) => nativeParameterKey(fn, index);
  const expression = (index: number) => object(elements[index])?.expression;
  return (
    [0, 1, 2, 3, 4].every((index) => parameter(index) !== undefined) &&
    elements.length === 12 &&
    elements.slice(0, 11).every((element) => !object(element)?.spread) &&
    memberOf(expression(0), parameter(0), "binary") &&
    ["--input", "--output", "--width", "--height", "--scale"].every(
      (flag, index) => literal(expression(index * 2 + 1)) === flag,
    ) &&
    identifierKey(expression(2)) === parameter(1) &&
    identifierKey(expression(4)) === parameter(2) &&
    nativeStringArgument(expression(6), parameter(3), "width") &&
    nativeStringArgument(expression(8), parameter(3), "height") &&
    literal(expression(10)) === "1" &&
    !!object(elements[11])?.spread &&
    nativeScrollArguments(expression(11), parameter(4))
  );
}

function nativeExampleEntrypoint(
  manifest: Record<string, unknown> | undefined,
  packageOptions: Record<string, unknown>,
  entrypoint: string,
): boolean {
  const examples = Array.isArray(manifest?.example) ? manifest.example : [];
  const explicit = examples
    .map(object)
    .filter((example) => example?.name === "html_fidelity_probe");
  return explicit.length === 0
    ? packageOptions.autoexamples !== false
    : explicit.length === 1 &&
        (explicit[0]?.path ?? "examples/html_fidelity_probe.rs") === entrypoint;
}

function nativeCargoEntrypoint(read: SourceCodeContext["read"], entrypoint: string): boolean {
  try {
    const root = object(Bun.TOML.parse(read("Cargo.toml") ?? ""));
    const workspace = object(root?.workspace);
    if (!Array.isArray(workspace?.members) || !workspace.members.includes(HTML_NATIVE_MEMBER))
      return false;
    if (
      Array.isArray(workspace.exclude) &&
      workspace.exclude.some(
        (name) => typeof name === "string" && new Bun.Glob(name).match(HTML_NATIVE_MEMBER),
      )
    )
      return false;
    const manifest = object(Bun.TOML.parse(read(HTML_NATIVE_MANIFEST) ?? ""));
    const packageOptions = object(manifest?.package);
    if (packageOptions?.name !== "katana-render-runtime") return false;
    if (entrypoint === "src/lib.rs") {
      const library = object(manifest?.lib);
      return (
        (library !== undefined || packageOptions.autolib !== false) &&
        (library?.path ?? "src/lib.rs") === entrypoint &&
        (library?.name ?? "katana_render_runtime") === "katana_render_runtime"
      );
    }
    return nativeExampleEntrypoint(manifest, packageOptions, entrypoint);
  } catch {
    return false;
  }
}

function nativeConsumerCall(
  module: Record<string, unknown>,
  source: string,
  processes: Map<string, ProcessBinding>,
  context?: SourceCodeContext,
): { calls: Set<Record<string, unknown>>; entrypoint: string } | undefined {
  const descriptor = HTML_NATIVE_CONSUMERS.find((entry) => entry.caller === context?.name);
  if (!descriptor || !context) return undefined;
  const functionNames = [descriptor.function, ...(descriptor.additionalFunctions ?? [])];
  const topLevel = Array.isArray(module.body) ? module.body : [];
  const declaredNames = topLevel
    .map((value) => {
      const node = object(value);
      return node?.type === "ExportDeclaration" ? object(node.declaration) : node;
    })
    .filter(
      (node) =>
        node?.type === "FunctionDeclaration" &&
        functionNames.includes(String(object(node.identifier)?.value)),
    );
  const functions = functionNames.map((name) => functionNamed(module, name));
  const callsByFunction = functions.map((fn) =>
    astNodes(fn).filter(
      (node) =>
        node.type === "CallExpression" &&
        processMethod(object(node.callee), processes)?.api === "bun" &&
        processMethods.includes(processMethod(object(node.callee), processes)?.method ?? ""),
    ),
  );
  const moduleCalls = astNodes(module).filter(
    (node) =>
      node.type === "CallExpression" &&
      processMethod(object(node.callee), processes)?.api === "bun" &&
      processMethods.includes(processMethod(object(node.callee), processes)?.method ?? ""),
  );
  if (
    declaredNames.length !== functionNames.length ||
    functions.some((fn) => !fn) ||
    callsByFunction.some((calls) => calls.length !== 1) ||
    moduleCalls.length !== functionNames.length
  )
    return undefined;
  const calls = callsByFunction.map((items) => items[0]);
  const valid = calls.every((call, index) =>
    nativeConsumerCallValid(
      module,
      functions[index],
      call,
      descriptor.selector,
      functionNames.length > 1,
      functionNames[index] === "captureKrrCatalogCondition",
    ),
  );
  const entrypoint = `${HTML_NATIVE_MEMBER}/${descriptor.entrypoint}`;
  const producer = `${HTML_NATIVE_MEMBER}/${descriptor.producer}`;
  const producerSource = context.read(producer);
  if (
    !valid ||
    !nativeCargoEntrypoint(context.read, descriptor.entrypoint) ||
    context.read(entrypoint) === undefined ||
    producerSource === undefined
  )
    return undefined;
  // このrecordは期待producerのsource登録であり、選択binaryやruntime guardの検証ではない。
  context.onNativeConsumerBinding?.({
    proofKind: "source-ownership-classification",
    artifactVerified: false,
    executionVerified: false,
    caller: context.name,
    callerSourceSha256: sourceDigest(source),
    function: descriptor.function,
    argvClass: descriptor.argvClass,
    swcVersion,
    manifest: HTML_NATIVE_MANIFEST,
    entrypoint,
    expectedProducerSources: [{ path: producer, sourceSha256: sourceDigest(producerSource) }],
  });
  return {
    calls: new Set(calls.filter((call): call is Record<string, unknown> => call !== undefined)),
    entrypoint,
  };
}

function nativeConsumerCallValid(
  module: Record<string, unknown>,
  fn: Record<string, unknown> | undefined,
  call: Record<string, unknown> | undefined,
  selector: string | undefined,
  checkFeatureEnvironment: boolean,
  catalog: boolean,
): boolean {
  if (!fn || !call || !memberOf(object(call.callee), "Bun:1", "spawnSync")) return false;
  const args = Array.isArray(call.arguments) ? call.arguments : [];
  const array = object(object(args[0])?.expression);
  const elements =
    array?.type === "ArrayExpression" && Array.isArray(array.elements) ? array.elements : [];
  if (args.some((arg) => object(arg)?.spread) || ![1, 2].includes(args.length)) return false;
  if (!selector) return nativeExampleArguments(elements, fn);
  const argvValid =
    nativeParameterKey(fn, 0) !== undefined &&
    elements.length === 3 &&
    elements.every((element) => !object(element)?.spread) &&
    identifierKey(object(elements[0])?.expression) === nativeParameterKey(fn, 0) &&
    literal(object(elements[1])?.expression) === selector &&
    literal(object(elements[2])?.expression) === "--nocapture";
  return (
    argvValid && (!checkFeatureEnvironment || nativeFeatureEnvironment(module, fn, call, catalog))
  );
}

function nativeMemberChain(value: unknown, parts: string[]): boolean {
  let node = object(value);
  if (parts.length < 2) return false;
  const properties: string[] = [];
  while (node?.type === "MemberExpression") {
    const property = object(node.property);
    if (property?.type !== "Identifier") return false;
    properties.push(String(property.value));
    node = object(node.object);
  }
  return (
    identifierKey(node) === parts[0] && properties.reverse().join(".") === parts.slice(1).join(".")
  );
}

function nativeFeatureValue(properties: Record<string, unknown>[], name: string): unknown {
  const found = properties.filter(
    (property) =>
      property.type === "KeyValueProperty" &&
      object(property.key)?.type === "Identifier" &&
      object(property.key)?.value === name,
  );
  return found.length === 1 ? found[0]?.value : undefined;
}

function nativeVariableKey(fn: Record<string, unknown>, name: string): string | undefined {
  for (const statement of functionStatements(fn)) {
    if (statement.type !== "VariableDeclaration") continue;
    const declarations = Array.isArray(statement.declarations) ? statement.declarations : [];
    for (const value of declarations) {
      const id = object(object(value)?.id);
      if (id?.value === name) return identifierKey(id);
    }
  }
  return undefined;
}

function nativeFeatureEnvironment(
  module: Record<string, unknown>,
  fn: Record<string, unknown>,
  call: Record<string, unknown>,
  catalog: boolean,
): boolean {
  const args = Array.isArray(call.arguments) ? call.arguments : [];
  const names = [
    "KRR_HTML_FEATURE_INPUT",
    "KRR_HTML_FEATURE_EXPRESSION",
    "KRR_HTML_FEATURE_NONCE",
    "KRR_HTML_FEATURE_CASE",
    "KRR_HTML_FEATURE_BINARY_SHA256",
    "KRR_HTML_FEATURE_WIDTH",
    "KRR_HTML_FEATURE_HEIGHT",
    "KRR_HTML_FEATURE_OUTPUT",
    ...(catalog ? ["KRR_HTML_FEATURE_OPERATION_JSON"] : []),
  ];
  const entries = nativeFeatureEnvironmentEntries(args[1], names);
  if (!entries) return false;
  const parameter = (index: number) => nativeParameterKey(fn, index);
  const request = parameter(4);
  const viewport = catalog ? [request ?? "", "viewport"] : [parameter(5) ?? ""];
  const stringPropertyMatches = (name: string, parts: string[]) => {
    const value = object(nativeFeatureValue(entries, name));
    const stringArgs = callArguments(value, "String:1");
    return stringArgs?.length === 1 && nativeMemberChain(stringArgs[0], parts);
  };
  const nonce = initializedCall(fn, "nonce", importKey(module, "node:crypto", "randomUUID"));
  const sourceSha = initializedCall(
    fn,
    "sourceSha256",
    importKey(module, "./reference-capture", "sha256File"),
  );
  const binarySha = initializedCall(
    fn,
    "binarySha256",
    importKey(module, "./reference-capture", "sha256File"),
  );
  const operationKey = nativeVariableKey(fn, "operation");
  const operation = object(nativeFeatureValue(entries, "KRR_HTML_FEATURE_OPERATION_JSON"));
  const operationCallee = object(operation?.callee);
  const operationArgs = Array.isArray(operation?.arguments) ? operation.arguments : [];
  const caseValue = object(nativeFeatureValue(entries, "KRR_HTML_FEATURE_CASE"));
  const caseMatches = catalog
    ? nativeMemberChain(caseValue, [request ?? "", "caseId"])
    : identifierKey(caseValue) === request;
  const checks = [
    identifierKey(nativeFeatureValue(entries, "KRR_HTML_FEATURE_INPUT")) === parameter(1),
    identifierKey(nativeFeatureValue(entries, "KRR_HTML_FEATURE_EXPRESSION")) === parameter(2),
    identifierKey(nativeFeatureValue(entries, "KRR_HTML_FEATURE_NONCE")) === nonce?.key,
    caseMatches,
    identifierKey(nativeFeatureValue(entries, "KRR_HTML_FEATURE_BINARY_SHA256")) === binarySha?.key,
    stringPropertyMatches("KRR_HTML_FEATURE_WIDTH", [...viewport, "width"]),
    stringPropertyMatches("KRR_HTML_FEATURE_HEIGHT", [...viewport, "height"]),
    identifierKey(nativeFeatureValue(entries, "KRR_HTML_FEATURE_OUTPUT")) === parameter(3),
    nonce?.args.length === 0,
    sourceSha?.args.length === 1 && identifierKey(sourceSha.args[0]) === parameter(1),
    binarySha?.args.length === 1 && identifierKey(binarySha.args[0]) === parameter(0),
    !catalog ||
      (memberOf(operationCallee, "JSON:1", "stringify") &&
        operationArgs.length === 1 &&
        !object(operationArgs[0])?.spread &&
        identifierKey(object(operationArgs[0])?.expression) === operationKey),
  ];
  return checks.every(Boolean);
}

function nativeFeatureEnvironmentEntries(
  argument: unknown,
  names: string[],
): Record<string, unknown>[] | undefined {
  const options = object(object(argument)?.expression);
  const optionProperties = Array.isArray(options?.properties)
    ? options.properties.map(object).filter((value): value is Record<string, unknown> => !!value)
    : [];
  const envProperty = object(optionProperties[0]);
  if (
    optionProperties.length !== 1 ||
    envProperty?.type !== "KeyValueProperty" ||
    object(envProperty.key)?.value !== "env"
  )
    return undefined;
  const env = object(envProperty.value);
  const properties = Array.isArray(env?.properties)
    ? env.properties.map(object).filter((value): value is Record<string, unknown> => !!value)
    : [];
  const spreads = properties.filter((property) => property.type === "SpreadElement");
  const entries = properties.filter((property) => property.type === "KeyValueProperty");
  const spreadSource = object(spreads[0]?.arguments);
  return properties.length === names.length + 1 &&
    spreads.length === 1 &&
    spreadSource?.type === "MemberExpression" &&
    identifierKey(spreadSource.object) === "process:1" &&
    object(spreadSource.property)?.value === "env" &&
    names.every((name) => nativeFeatureValue(entries, name) !== undefined)
    ? entries
    : undefined;
}

function registeredProcessRoute(
  module: Record<string, unknown>,
  source: string,
  context: SourceCodeContext | undefined,
): DomainProcessRoute | undefined {
  const mathjaxCall = mathjaxFactsCall(module, context);
  if (mathjaxCall) return { mathjaxFactsCall: mathjaxCall };
  const classicCall = classicFactsCall(module, context);
  if (classicCall) return { classicFactsCall: classicCall };
  const nativeWidthCall = nativeWidthHashCall(module, context);
  const nativeWidthConsumerCall = nativeWidthConsumerFactsCall(module, context);
  if (nativeWidthCall && nativeWidthConsumerCall)
    return { nativeWidthCall, nativeWidthConsumerCall };
  if (nativeWidthCall || nativeWidthConsumerCall) return undefined;
  const chromeFontProbe = chromeFontIdentityProbeCall(module, context);
  if (chromeFontProbe) return { chromeFontIdentityProbeCall: chromeFontProbe };
  const compressorCall = runtimeAssetCompressorUnitCall(module, source, context);
  return compressorCall ? { unitProgramCall: compressorCall } : undefined;
}

function domainProcessRoute(
  module: Record<string, unknown>,
  source: string,
  context?: SourceCodeContext,
): DomainProcessRoute {
  const factsRoute = registeredProcessRoute(module, source, context);
  if (factsRoute) return factsRoute;
  if (
    context?.name === "scripts/mermaid/native-cli-process.ts" &&
    context.importer === "scripts/mermaid/native-render-provenance.ts" &&
    nativeProducerVerified(context.read("scripts/mermaid/native-render-provenance.ts"))
  ) {
    const wrapper = functionNamed(module, "spawnNativeCli");
    const binary = parameterKey(wrapper, 0);
    return binary && unchangedParameter(wrapper, binary) ? { nativeBinary: binary } : {};
  }
  if (context) {
    const unitBinary = nativeUnitRoute(module, source, context);
    if (unitBinary) return { unitBinary };
  }
  if (context?.name === "scripts/mermaid/raster-browser-observation.ts") {
    const collector = collectorSourceVerified(module);
    const request = collector ? observerRouteVerified(module, collector) : undefined;
    if (collector && request) return { observerRequest: request, collector };
  }
  return {};
}

function domainProcessCode(
  node: Record<string, unknown>,
  domain: DomainProcessRoute,
): string[] | undefined {
  const args = Array.isArray(node.arguments) ? node.arguments : [];
  if (args.some((arg) => object(arg)?.spread)) return undefined;
  if (node === domain.chromeFontIdentityProbeCall) return [];
  const binary = domain.nativeBinary ?? domain.unitBinary;
  if (binary && identifierKey(object(args[0])?.expression) === binary) return [];
  if (!memberOf(object(args[0])?.expression, domain.observerRequest, "observerPython"))
    return undefined;
  const array = object(object(args[1])?.expression);
  const elements = Array.isArray(array?.elements) ? array.elements : [];
  return array?.type === "ArrayExpression" &&
    !object(elements[0])?.spread &&
    callArguments(object(elements[0])?.expression, domain.collector)?.length === 0
    ? ["!execute-local:browser-process-observe.py"]
    : undefined;
}

function importedCallCode(
  callee: Record<string, unknown> | undefined,
  first: unknown,
): string[] | undefined {
  if (!(callee?.type === "Import" || (callee?.type === "Identifier" && callee.value === "require")))
    return undefined;
  const name = literal(first);
  return nodeProcessModules.includes(name ?? "")
    ? [name ?? "!unresolved-import", "!unresolved-executable"]
    : [name ?? "!unresolved-import"];
}

function callCode(
  node: Record<string, unknown>,
  bindings: Set<string>,
  staticBindings: () => ResolvedStaticBindings,
  processes: Map<string, ProcessBinding>,
  domain: DomainProcessRoute,
): string[] {
  if (
    node === domain.nativeWidthCall ||
    node === domain.nativeWidthConsumerCall ||
    node === domain.classicFactsCall ||
    node === domain.mathjaxFactsCall ||
    node === domain.unitProgramCall ||
    node === domain.chromeFontIdentityProbeCall
  )
    return [];
  const callee = object(node.callee);
  const args = Array.isArray(node.arguments) ? node.arguments : [];
  const first = object(args[0])?.expression;
  const imported = importedCallCode(callee, first);
  if (imported) return imported;
  const binding = processMethod(callee, processes);
  const method = binding?.method;
  if (!processMethods.includes(String(method)))
    return binding?.api === "node" ? ["!unresolved-executable"] : [];
  if (args.some((arg) => object(arg)?.spread)) return ["!unresolved-executable"];
  if (
    binding?.api === "node" &&
    ["spawn", "spawnSync", "execFile", "execFileSync"].includes(String(method))
  ) {
    const resolved = domainProcessCode(node, domain);
    if (resolved !== undefined) return resolved;
    return execFileCode(args, bindings, staticBindings());
  }
  return childCode(first, bindings, staticBindings());
}

function directCallOf(
  node: Record<string, unknown>,
  parent: Record<string, unknown> | undefined,
): boolean {
  return parent?.type === "CallExpression" && parent.callee === node;
}

function nodeProcessEscaped(
  node: Record<string, unknown>,
  parent: Record<string, unknown> | undefined,
  grandparent: Record<string, unknown> | undefined,
  binding: ProcessBinding,
  processes: Map<string, ProcessBinding>,
): boolean {
  // importの束縛を別名・引数・computed accessへ渡す経路は静的な直接呼出しではない。
  if (binding.method !== "*")
    return !(directCallOf(node, parent) && processMethods.includes(binding.method));
  return !(
    parent?.type === "MemberExpression" &&
    parent.object === node &&
    directCallOf(parent, grandparent) &&
    processMethods.includes(processMethod(parent, processes)?.method ?? "")
  );
}

function isTypeofReference(
  node: Record<string, unknown>,
  parent: Record<string, unknown> | undefined,
): boolean {
  return (
    parent?.type === "UnaryExpression" && parent.operator === "typeof" && parent.argument === node
  );
}

function bunNamespaceEscaped(
  node: Record<string, unknown>,
  parent: Record<string, unknown> | undefined,
  processes: Map<string, ProcessBinding>,
): boolean {
  const key = identifierKey(node);
  const binding = processes.get(key ?? "");
  if (key !== "Bun:1" && !(binding?.api === "bun" && binding.method === "*")) return false;
  if (isTypeofReference(node, parent)) return false;
  // named member は既存の process method 検査へ渡し、namespace 全体の別名化は拒否する。
  return !(
    parent?.type === "MemberExpression" &&
    parent.object === node &&
    object(parent.property)?.type === "Identifier"
  );
}

function globalBunReference(node: Record<string, unknown>): boolean {
  if (node.type !== "MemberExpression" || identifierKey(node.object) !== "globalThis:1")
    return false;
  const property = object(node.property);
  return property?.type === "Identifier"
    ? property.value === "Bun"
    : property?.type === "Computed" && literal(property.expression) === "Bun";
}

function bunSpyArgument(
  value: unknown,
  spyOn: string,
  assigned: Set<string>,
): { key: string; target: object } | undefined {
  const variable = object(value);
  const key = identifierKey(variable?.id);
  const initializer = object(variable?.init);
  const implementation = object(initializer?.callee);
  const call = object(implementation?.object);
  const args = callArguments(call, spyOn);
  const mockArgs = Array.isArray(initializer?.arguments) ? initializer.arguments : [];
  if (
    !key ||
    assigned.has(key) ||
    initializer?.type !== "CallExpression" ||
    implementation?.type !== "MemberExpression" ||
    object(implementation.property)?.type !== "Identifier" ||
    object(implementation.property)?.value !== "mockImplementation" ||
    mockArgs.length !== 1 ||
    object(mockArgs[0])?.spread ||
    args?.length !== 2 ||
    identifierKey(args[0]) !== "Bun:1" ||
    literal(args[1]) !== "spawn"
  )
    return undefined;
  const target = object(args[0]);
  return target ? { key, target } : undefined;
}

function reviewedBunSpyArguments(module: Record<string, unknown>): Set<object> {
  const spyOn = importKey(module, "bun:test", "spyOn");
  const allowed = new Set<object>();
  if (!spyOn) return allowed;
  const assigned = assignedBindings(module);
  if (assigned.has(spyOn) || assigned.has("Bun:1")) return allowed;
  const nodes = astNodes(module);
  const restores = new Set(
    nodes
      .filter(
        (node) =>
          node.type === "CallExpression" &&
          memberOf(node.callee, identifierKey(object(node.callee)?.object), "mockRestore") &&
          Array.isArray(node.arguments) &&
          node.arguments.length === 0,
      )
      .map((node) => object(object(node.callee)?.object)),
  );
  const declarations = nodes
    .filter((node) => node.type === "VariableDeclaration" && node.kind === "const")
    .flatMap((node) => (Array.isArray(node.declarations) ? node.declarations : []));
  for (const value of declarations) {
    const variable = object(value);
    const candidate = bunSpyArgument(variable, spyOn, assigned);
    if (!candidate) continue;
    const uses = nodes.filter((node) => identifierKey(node) === candidate.key);
    if (uses.length === 2 && uses.every((node) => node === variable?.id || restores.has(node)))
      allowed.add(candidate.target);
  }
  return allowed;
}

function processApiEscaped(
  node: Record<string, unknown>,
  parent: Record<string, unknown> | undefined,
  grandparent: Record<string, unknown> | undefined,
  processes: Map<string, ProcessBinding>,
): boolean {
  if (bunNamespaceEscaped(node, parent, processes)) return true;
  if (globalBunReference(node)) return !isTypeofReference(node, parent);
  const binding = processes.get(identifierKey(node) ?? "");
  if (binding && (binding.api === "node" || processMethods.includes(binding.method)))
    return nodeProcessEscaped(node, parent, grandparent, binding, processes);
  return bunMemberEscaped(node, parent, processes);
}

function bunMemberEscaped(
  node: Record<string, unknown>,
  parent: Record<string, unknown> | undefined,
  processes: Map<string, ProcessBinding>,
): boolean {
  if (node.type !== "MemberExpression") return false;
  const method = processMethod(node, processes);
  const key = identifierKey(node.object);
  const namespace = processes.get(key ?? "");
  const computed = object(node.property)?.type === "Computed";
  const bunNamespace = key === "Bun:1" || (namespace?.api === "bun" && namespace.method === "*");
  if (method?.api !== "bun" && !(computed && bunNamespace)) return false;
  return (
    (computed || processMethods.includes(method?.method ?? "")) &&
    !(directCallOf(node, parent) && !computed)
  );
}

function declarationCode(node: Record<string, unknown>): string[] {
  if (node.type === "TsImportEqualsDeclaration") {
    const reference = object(node.moduleRef);
    if (reference?.type !== "TsExternalModuleReference") return [];
    const name = literal(reference.expression);
    if (!name) return ["!unresolved-import"];
    return !node.isTypeOnly && nodeProcessModules.includes(name)
      ? [name, "!unresolved-executable"]
      : [name];
  }
  if (
    !["ImportDeclaration", "ExportAllDeclaration", "ExportNamedDeclaration"].includes(
      String(node.type),
    )
  )
    return [];
  const name = literal(node.source);
  if (!name) return [];
  return node.type !== "ImportDeclaration" && !node.typeOnly && nodeProcessModules.includes(name)
    ? [name, "!unresolved-executable"]
    : [name];
}

function bindExternalCall(
  node: Record<string, unknown>,
  toolCall: Record<string, unknown> | undefined,
  module: Record<string, unknown>,
  source: string,
  context: SourceCodeContext | undefined,
): boolean {
  if (node !== toolCall || !context?.resolveExternalTool || !context.onExternalToolBinding)
    return false;
  const request: ExternalToolRequest = {
    kind: "typescript-bootstrap",
    caller: context.name,
    target: "node_modules/typescript/bin/tsc",
    executable: "process.execPath",
    argvClass: "typescript-show-config-safe-project",
    callerSourceSha256: sourceDigest(source),
    callerAstSha256: canonicalNativeUnitAstDigest(module),
    swcVersion,
  };
  try {
    const binding = context.resolveExternalTool(request);
    if (!binding || !externalToolBindingMatches(request, binding)) return false;
    context.onExternalToolBinding(binding);
    return true;
  } catch {
    return false;
  }
}

// 実 source の独立レビュー後にだけ固定し、未知の process 辺へ一般化しない。
const HOST_EVIDENCE_SOURCE_AST_SHA256: Record<string, string> = {
  "scripts/runtime-bundles/runtime-unused-native-width-hash.ts":
    "0e5850eca32f88bbf854a3056e903819e27199e8a1afde65fc451579a118ae5f",
  "scripts/runtime-bundles/runtime-rust-facts-host.ts":
    "f566a806f2058aa715aa15822981f8303d70cad4dc55547c84895dc0e4a8ccd5",
  "scripts/runtime-bundles/runtime-rust-facts-host-inputs.ts":
    "bcaf3428addb3de7f87054b0871bfc9eb3497f97aec0fd74ee7bc3e82aed99a6",
  "scripts/runtime-bundles/runtime-rust-facts-host-protocol.ts":
    "8dbc5f4231e9756d4e5f77cb91ab688e59459f288e2115660c273180fb62558e",
  "scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts":
    "a4d55d060af44d2711de44cdb04f6e8b3b353a9b2934ccc2fa37c0aece0bc971",
  "scripts/runtime-bundles/runtime-mathjax-audit-evidence-inputs.ts":
    "7a091a7f5af2d9b8e71617ca7dedb6a1dc51d6ad2400a8da1f3f07e0f5888b9c",
  "scripts/runtime-bundles/runtime-mathjax-audit-evidence-producer.ts":
    "7ebce6df5e2534548bc16efb72981e9af2f1f458ac6ec67109c5dccbbe8c05a2",
};
export function hostEvidenceSourceVerified(name: string, source: string): boolean {
  const expected = HOST_EVIDENCE_SOURCE_AST_SHA256[name];
  if (!expected || swcVersion !== "1.16.18") return false;
  try {
    const ast = parseSync(source, { syntax: "typescript", tsx: false });
    const normalized = JSON.stringify(ast, (key: string, value: unknown) =>
      ["span", "ctxt", "bodyCtxt", "raw"].includes(key) ? undefined : value,
    );
    return createHash("sha256").update(normalized).digest("hex") === expected;
  } catch {
    return false;
  }
}

export function sourceCode(source: string, context?: SourceCodeContext): string[] {
  return sourceCodeFromModule(
    source,
    context,
    parseSync(source, { syntax: "typescript", tsx: true }),
  );
}

function sourceCodeFromModule(
  source: string,
  context: SourceCodeContext | undefined,
  module: ReturnType<typeof parseSync>,
): string[] {
  const result: string[] = [];
  const moduleRecord = module as unknown as Record<string, unknown>;
  const bindings = nodePathBindings(moduleRecord);
  const processes = processBindings(moduleRecord);
  const preparedSource =
    context?.name !== undefined && hostEvidenceSourceVerified(context.name, source);
  if (
    context?.name &&
    Object.hasOwn(HOST_EVIDENCE_SOURCE_AST_SHA256, context.name) &&
    !preparedSource
  )
    result.push("!unresolved-executable");
  const spyArguments = reviewedBunSpyArguments(moduleRecord);
  const domain = domainProcessRoute(moduleRecord, source, context);
  if (
    context?.name === NATIVE_WIDTH_HASH_CALLER &&
    !domain.nativeWidthCall &&
    !domain.nativeWidthConsumerCall
  )
    result.push("!unresolved-executable");
  if (context?.name === CLASSIC_FACTS_CALLER && !domain.classicFactsCall)
    result.push("!unresolved-executable");
  if (context?.name === MATHJAX_FACTS_CALLER && !domain.mathjaxFactsCall)
    result.push("!unresolved-executable");
  const nativeCall = nativeConsumerCall(moduleRecord, source, processes, context);
  if (HTML_NATIVE_CONSUMERS.some((entry) => entry.caller === context?.name) && !nativeCall)
    result.push("!unresolved-executable");
  const toolCall = typeScriptQueryCall(moduleRecord, context);
  const biomeCommand = biomeContextCommand(moduleRecord, context);
  const duplicationExecutions = duplicationFixtureExecutions(moduleRecord, context);
  result.push(...duplicationExecutions.errors);
  const fixtureImports = new Map([
    ...semanticFixtureImports(moduleRecord, context),
    ...duplicationExecutions.calls,
  ]);
  const registeredTools = new Set([
    ...standardBiomeToolCommands(moduleRecord, context),
    ...gitSourceInventoryCalls(moduleRecord, context),
  ]);
  const standardBiomeCommand = biomeContextCommand(
    moduleRecord,
    context,
    "collectStandardUnusedRawBiome",
  );
  if (context?.name === "scripts/quality/owned-type-projects.ts" && !toolCall)
    result.push("!unresolved-executable");
  if (context?.name === "scripts/runtime-bundles/runtime-unused-check.ts" && !biomeCommand)
    result.push("!unresolved-executable");
  let staticContext:
    | { blocks: WeakMap<object, object>; bindings: Map<string, StaticStringBinding> }
    | undefined;
  const staticForCall = (node: Record<string, unknown>): ResolvedStaticBindings => {
    if (!staticContext) {
      const blocks = executionBlocks(moduleRecord);
      staticContext = { blocks, bindings: staticConstStringBindings(moduleRecord, blocks) };
    }
    return callStaticBindings(node, staticContext.bindings, staticContext.blocks);
  };
  const referencesForCall = (node: Record<string, unknown>): string[] => {
    if (fixtureImports.has(node)) return fixtureImports.get(node) ?? [];
    if (registeredTools.has(node)) return [];
    if (nativeCall?.calls.has(node)) return [nativeCall.entrypoint];
    if (bindExternalCall(node, toolCall, moduleRecord, source, context)) return [];
    if (node === biomeCommand?.call)
      return childCode(biomeCommand.array, bindings, staticForCall(node));
    if (node === standardBiomeCommand?.call) return [];
    const references = callCode(node, bindings, () => staticForCall(node), processes, domain);
    return preparedSource
      ? references.filter((name) => name !== "!unresolved-executable")
      : references;
  };
  const escaped = (
    node: Record<string, unknown>,
    parent?: Record<string, unknown>,
    grandparent?: Record<string, unknown>,
  ) =>
    !preparedSource &&
    !spyArguments.has(node) &&
    processApiEscaped(node, parent, grandparent, processes);
  const visit = (
    value: unknown,
    parent?: Record<string, unknown>,
    grandparent?: Record<string, unknown>,
  ): void => {
    if (Array.isArray(value)) {
      for (const child of value) visit(child, parent, grandparent);
      return;
    }
    const node = object(value);
    if (!node) return;
    result.push(...declarationCode(node));
    if (["ImportDeclaration", "TsTypeQuery"].includes(String(node.type))) return;
    if (escaped(node, parent, grandparent)) result.push("!unresolved-executable");
    if (node.type === "CallExpression") result.push(...referencesForCall(node));
    for (const child of Object.values(node)) visit(child, node, parent);
  };
  visit(module);
  return result;
}

export function justRecipes(source: string): Map<string, string> {
  const result = new Map<string, string>();
  let current = "";
  for (const line of source.split("\n")) {
    const header = /^([\w-]+)(?:\s+[^:]*)?:(?![=])\s*(.*)$/.exec(line);
    if (header?.[1]) {
      current = header[1];
      result.set(current, header[2] ? `just ${header[2]}` : "");
    } else if (/^\s+\S/.test(line) && current)
      result.set(current, `${result.get(current)}\n${line}`);
    else if (/^\S/.test(line) && !line.startsWith("#")) current = "";
  }
  return result;
}

function resolvedImport(
  name: string,
  importer: string | undefined,
  resolveCode: ManagedCodeResolver,
  errors: string[],
): string | undefined {
  try {
    const target = resolveCode(name, importer);
    if (!isCode(target) || target.startsWith("../") || path.posix.isAbsolute(target))
      throw new Error("resolved source is not repository-relative managed code");
    return target;
  } catch (error) {
    errors.push(
      `code resolution failed: ${name}: ${error instanceof Error ? error.message : String(error)}`,
    );
    return undefined;
  }
}

function dependencyOrDataImport(name: string, dependencies: Set<string>): boolean {
  const packageName = name.startsWith("@")
    ? name.split("/").slice(0, 2).join("/")
    : name.split("/")[0];
  return (
    name === "bun" ||
    isBuiltin(name) ||
    dependencies.has(packageName ?? "") ||
    /\.(json|png|svg|html)$/.test(name)
  );
}

function sourceTarget(
  name: string,
  importer: string | undefined,
  dependencies: Set<string>,
  errors: string[],
  resolveCode?: ManagedCodeResolver,
): string | undefined {
  const childImport = name.startsWith("!child-import:");
  const childPathError = childImportPathError(name);
  if (childPathError) {
    errors.push(childPathError);
    return undefined;
  }
  if (childImport) name = name.slice("!child-import:".length);
  const marker = /^!execute(?:-local)?:/.exec(name)?.[0];
  const execution = marker !== undefined;
  if (marker) name = name.slice(marker.length);
  if (!execution && name.startsWith("#") && resolveCode)
    return resolvedImport(name, importer, resolveCode, errors);
  if (!execution && dependencyOrDataImport(name, dependencies)) return undefined;
  if (!execution && !isCode(name) && !name.startsWith(".") && !name.startsWith("!")) {
    errors.push(`unknown code dependency: ${name}`);
    return undefined;
  }
  const target = path.posix.normalize(
    importer ? path.posix.join(path.posix.dirname(importer), name) : name.replace(/^\.\//, ""),
  );
  if (target.startsWith("../") || path.posix.isAbsolute(target) || target.startsWith("!")) {
    errors.push(`unresolved/external code: ${name}`);
    return undefined;
  }
  return execution || isCode(target) ? target : `${target}.ts`;
}

function childImportPathError(name: string): string | undefined {
  if (!name.startsWith("!child-import:")) return undefined;
  const pathName = name.slice("!child-import:".length);
  return pathName.startsWith(".") ||
    path.posix.isAbsolute(pathName) ||
    path.win32.isAbsolute(pathName)
    ? `unresolved/external code: child eval path ${pathName}`
    : undefined;
}

function dispatchedRecipes(
  name: string,
  body: string,
  policy: ContractPolicy,
  read: (name: string) => string | undefined,
  errors: string[],
): string[] {
  const dispatch = policy.recipeDispatch[name];
  if (!dispatch) return [];
  const selector = read(dispatch.source) ?? "";
  if (!body.includes(dispatch.source)) errors.push(`missing recipe dispatcher: ${name}`);
  return dispatch.recipes.filter((selected) => {
    if (selector.includes(`"${selected}"`)) return true;
    errors.push(`missing dispatched recipe: ${selected}`);
    return false;
  });
}

interface MoonTask {
  deps: string[];
  leaf: string | undefined;
  options: Record<string, unknown>;
  command: string[];
}

export const RUNTIME_UNUSED_REGRESSION_TESTS = [
  "scripts/runtime-bundles/runtime-fragment-loader_test.ts",
  "scripts/runtime-bundles/runtime-bundle-definitions_test.ts",
  "scripts/runtime-bundles/runtime-unused-check_test.ts",
  "scripts/runtime-bundles/runtime-bundle-tool-fingerprint_test.ts",
  "scripts/runtime-bundles/runtime-unused-mathjax_test.ts",
  "scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts",
  "scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
] as const;
export const RUNTIME_UNUSED_LEAF_IDS = [
  "runtime-unused-fragment-loader",
  "runtime-unused-bundle-definitions",
  "runtime-unused-check",
  "runtime-unused-tool-fingerprint",
  "runtime-unused-mathjax",
  "runtime-unused-native-width",
  "runtime-unused-rust-classic",
] as const;
export const RUNTIME_UNUSED_FINAL_VERIFY_COMMAND =
  "python3 scripts/release/runtime_script_cache_cli.py ensure-all --repo {{ quote(REPO_ROOT) }}";

export interface MoonSchedulerTask {
  deps: string[];
  leaf: string | null;
  options: Record<string, unknown>;
}

function moonDependencies(value: unknown, name: string, project: string): string[] {
  const deps = value ?? [];
  if (!Array.isArray(deps) || !deps.every((dep) => typeof dep === "string"))
    throw new Error(`non-required moon dependency ${name}`);
  const normalized = deps.map((dep: string) => dep.replace(new RegExp(`^(?:${project}|~):`), ""));
  if (normalized.some((dep) => dep.includes(":")))
    throw new Error(`external moon dependency ${name}`);
  return normalized;
}

function yamlRecord(source: string | undefined): Record<string, unknown> {
  const value = object(Bun.YAML.parse(source ?? ""));
  if (!value) throw new Error("missing/invalid moon YAML object");
  return value;
}

function moonTaskLeaf(task: Record<string, unknown>, name: string): string | undefined {
  if (task.command === "noop") return undefined;
  if (task.args !== undefined && (!Array.isArray(task.args) || task.args.length !== 0))
    throw new Error(`unsupported moon task arguments ${name}`);
  const command = task.command;
  if (
    !Array.isArray(command) ||
    (command.length !== 2 && command.length !== 3) ||
    command[0] !== "just" ||
    typeof command[1] !== "string" ||
    (command.length === 3 && typeof command[2] !== "string")
  )
    throw new Error(`unsupported moon task command ${name}`);
  return command[1];
}

function validateRuntimeUnusedScriptProducerTask(
  config: Record<string, unknown>,
  name: string,
  task: Record<string, unknown>,
): void {
  validateRuntimeBundleTaskKeys(task);
  const options = object(task.options);
  if (options?.cache !== "local") throw new Error("runtime unused producer cache differs");
  const id = name.endsWith("-producer") ? name.slice(0, -"-producer".length) : "";
  if (!(RUNTIME_UNUSED_LEAF_IDS as readonly string[]).includes(id))
    throw new Error(`unknown runtime unused producer profile ${name}`);
  if (
    !Array.isArray(task.command) ||
    task.command.length !== 3 ||
    task.command[0] !== "just" ||
    task.command[1] !== "runtime-unused-script-producer-run" ||
    task.command[2] !== id
  )
    throw new Error(`runtime unused producer command differs: ${id}`);
  const qualityInputs = object(config.tasks)?.["fmt-check"];
  const expectedInputs = object(qualityInputs)?.inputs;
  if (JSON.stringify(task.inputs) !== JSON.stringify(expectedInputs))
    throw new Error("runtime unused producer inputs differ");
  if (
    JSON.stringify(task.outputs) !==
    JSON.stringify([
      `tmp/quality/runtime-script-results/${id}.json`,
      `tmp/quality/runtime-script-results/${id}-current-group-001.xml`,
    ])
  )
    throw new Error("runtime unused producer outputs differ");
  if (
    JSON.stringify(task.checks) !==
    JSON.stringify([
      {
        check: "fingerprint",
        script: `just runtime-unused-script-cache-key ${id}`,
        hash: "stdout",
      },
    ])
  )
    throw new Error("runtime unused producer fingerprint differs");
}

function moonTaskOptionsValid(
  name: string,
  options: Record<string, unknown>,
  nativeCache: boolean,
): boolean {
  return (
    options.allowFailure === false &&
    options.expectFailure === false &&
    options.runInCI === "always" &&
    options.shell === false &&
    (options.cache === false ||
      nativeCache ||
      (name === "runtime-bundle-check" && options.cache === true) ||
      (RUNTIME_UNUSED_LEAF_IDS.some((id) => name === `${id}-producer`) &&
        options.cache === "local"))
  );
}

function moonTasks(config: Record<string, unknown>, project: string): Map<string, MoonTask> {
  const tasks = object(config.tasks);
  const defaults = object(config.taskOptions) ?? {};
  if (!tasks) throw new Error("moon tasks missing");
  return new Map(
    Object.entries(tasks).map(([name, value]) => {
      const task = object(value);
      if (!task) throw new Error(`invalid moon task ${name}`);
      if (task.options !== undefined && !object(task.options))
        throw new Error(`invalid moon task options ${name}`);
      const options: Record<string, unknown> = {
        allowFailure: false,
        expectFailure: false,
        ...defaults,
        ...object(task.options),
      };
      const nativeCache = nativeCoverageCacheTask(name, task);
      if (nativeCache) validateNativeCoverageCacheTask(config, name, task);
      if (name.endsWith("-producer")) validateRuntimeUnusedScriptProducerTask(config, name, task);
      if (!moonTaskOptionsValid(name, options, nativeCache))
        throw new Error(`unsafe moon task options ${name}`);
      if (name === "runtime-bundle-check") validateRuntimeBundleCacheTask(config, task);
      const normalized = moonDependencies(task.deps, name, project);
      const command = Array.isArray(task.command)
        ? task.command.filter((value): value is string => typeof value === "string")
        : [];
      return [name, { deps: normalized, leaf: moonTaskLeaf(task, name), options, command }];
    }),
  );
}

function validateRuntimeBundleCacheTask(
  config: Record<string, unknown>,
  task: Record<string, unknown>,
): void {
  validateRuntimeBundleTaskKeys(task);
  validateRuntimeBundleGlobalOptions(config);
  validateRuntimeBundleTaskOptions(task);
  validateRuntimeBundleCommand(task);
  validateRuntimeBundleInputs(task);
  validateRuntimeBundleFingerprint(task);
}

function validateRuntimeBundleTaskKeys(task: Record<string, unknown>): void {
  const allowedTaskKeys = new Set(["command", "deps", "inputs", "outputs", "checks", "options"]);
  if (Object.keys(task).some((key) => !allowedTaskKeys.has(key)))
    throw new Error("runtime bundle cache task contains an unknown setting");
}

function validateRuntimeBundleGlobalOptions(config: Record<string, unknown>): void {
  const defaults = object(config.taskOptions);
  if (defaults?.cache !== false || defaults?.runInCI !== "always" || defaults?.shell !== false)
    throw new Error("runtime bundle cache requires strict global task options");
}

function validateRuntimeBundleTaskOptions(task: Record<string, unknown>): void {
  const options = object(task.options);
  if (!options || Object.keys(options).sort().join("\0") !== "cache" || options.cache !== true)
    throw new Error("runtime bundle cache task options changed");
}

function validateRuntimeBundleCommand(task: Record<string, unknown>): void {
  if (
    !Array.isArray(task.command) ||
    task.command.length !== 2 ||
    task.command[0] !== "just" ||
    task.command[1] !== "runtime-bundle-check" ||
    (task.deps !== undefined && (!Array.isArray(task.deps) || task.deps.length !== 0))
  )
    throw new Error("runtime bundle cache command or dependencies changed");
}

function validateRuntimeBundleInputs(task: Record<string, unknown>): void {
  const expectedInputs = MOON_QUALITY_INPUTS;
  if (!Array.isArray(task.inputs) || task.inputs.join("\0") !== expectedInputs.join("\0"))
    throw new Error("runtime bundle cache inputs changed");
  if (task.outputs !== undefined && (!Array.isArray(task.outputs) || task.outputs.length !== 0))
    throw new Error("runtime bundle cache outputs must remain empty");
}

function validateRuntimeBundleFingerprint(task: Record<string, unknown>): void {
  const checks = task.checks;
  if (
    !Array.isArray(checks) ||
    checks.length !== 1 ||
    !object(checks[0]) ||
    Object.keys(object(checks[0]) ?? {})
      .sort()
      .join("\0") !== "check\0hash\0script" ||
    object(checks[0])?.check !== "fingerprint" ||
    object(checks[0])?.script !==
      "bun scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts" ||
    object(checks[0])?.hash !== "stdout"
  )
    throw new Error("runtime bundle cache fingerprint check changed");
}

function moonClosure(
  name: string,
  tasks: Map<string, MoonTask>,
  stack: string[] = [],
): Set<string> {
  if (stack.includes(name)) throw new Error(`moon dependency cycle ${name}`);
  const task = tasks.get(name);
  if (!task) throw new Error(`missing moon task ${name}`);
  return new Set([
    name,
    ...task.deps.flatMap((dep) => [...moonClosure(dep, tasks, [...stack, name])]),
  ]);
}

// 実rootとreview済みownerの一致に、既存Moon parserの同じ解釈を使う。
export function moonStaticTaskProjection(
  source: string,
  project: string,
  entry: string,
): Record<string, MoonSchedulerTask> {
  const tasks = moonTasks(yamlRecord(source), project);
  return Object.fromEntries(
    [...moonClosure(entry, tasks)].sort().map((name) => {
      const task = tasks.get(name);
      if (!task) throw new Error(`missing moon task ${name}`);
      return [
        name,
        { deps: [...new Set(task.deps)].sort(), leaf: task.leaf ?? null, options: task.options },
      ];
    }),
  );
}

function moonBootstrapOrder(
  policy: NonNullable<ContractPolicy["moon"]>,
  tasks: Map<string, MoonTask>,
  errors: string[],
): void {
  const foundation = "source-static-foundation",
    capacity = "capacity-admission",
    bootstrap = "rust-facts-host-prebuild";
  const independent = (policy.ciRequiredStatic ?? []).filter(
    (name) => !["biome", capacity, bootstrap].includes(name),
  );
  const foundationTasks = moonClosure(foundation, tasks);
  for (const name of independent)
    if (!foundationTasks.has(name) || tasks.get(name)?.leaf !== name)
      errors.push(`moon foundation leaf missing: ${name}`);
  if (foundationTasks.has("biome") || foundationTasks.has(bootstrap))
    errors.push("moon foundation requires host-dependent static task");
  if (foundationTasks.has(capacity))
    errors.push("moon foundation requires capacity admission downstream");
  for (const [before, after] of [
    [foundation, capacity],
    [capacity, bootstrap],
    [bootstrap, "biome"],
    [foundation, "source-static"],
    ["biome", "source-static"],
    [bootstrap, "mathjax-owned-evidence"],
  ] as const)
    if (!tasks.get(after)?.deps.includes(before))
      errors.push(`moon host bootstrap order bypass: ${before} -> ${after}`);
}
function moonHostEvidenceOrder(
  policy: NonNullable<ContractPolicy["moon"]>,
  tasks: Map<string, MoonTask>,
  errors: string[],
): void {
  if (!policy.requiredStatic.includes("rust-facts-host-prebuild")) return;
  moonBootstrapOrder(policy, tasks, errors);
  const sequence = [
    policy.barrier,
    "mathjax-owned-evidence",
    "runtime-asset-script-preflight",
    "runtime-classic-generation-evidence",
    "runtime-asset-script-test",
    "unit-test",
  ];
  for (let index = 1; index < sequence.length; index += 1) {
    const before = sequence[index - 1],
      after = sequence[index];
    if (!before || !after || !moonClosure(after, tasks).has(before))
      errors.push(`moon host evidence order bypass: ${before} -> ${after}`);
  }
}
function moonStaticLeaf(name: string, policy: NonNullable<ContractPolicy["moon"]>): string {
  return name === "biome" && policy.requiredStatic.includes("rust-facts-host-prebuild")
    ? "biome-run"
    : name;
}
function moonHeavyLeaf(name: string): string {
  const leaves: Record<string, string> = {
    "unit-test": "root-test-and-coverage",
    "performance-coverage": "performance-harness-coverage",
    coverage: "coverage-result-check",
  };
  return leaves[name] ?? name;
}

function moonCoverageRecipesVerified(
  recipes: Map<string, string>,
  justfile: string,
  errors: string[],
): void {
  const root = recipes.get("root-test-and-coverage") ?? "";
  const performance = recipes.get("performance-harness-coverage") ?? "";
  const reportLines = (recipe: string) =>
    recipe
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.includes(" llvm-cov ") && !line.includes(" llvm-cov clean "));
  const suffix = "{{TEST_THREAD_ARGS}}";
  const rootCommand =
    'CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-target" python3 scripts/build/cargo_capacity_guard.py --coverage-collection-domain root -- {{CARGO}} llvm-cov --workspace --all-targets --all-features --locked --json --summary-only --output-path "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage/root.json"' +
    suffix;
  const performanceCommand = (variant: "baseline" | "candidate") =>
    `CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-${variant}" python3 scripts/build/cargo_capacity_guard.py --coverage-collection-domain performance-${variant} -- {{CARGO}} llvm-cov --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --all-targets --no-default-features --features ${variant} --locked --json --output-path "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage/${variant}.json"` +
    suffix;
  if (!/^COVERAGE_MIN_LINES := env_var_or_default\("COVERAGE_MIN_LINES", "100"\)$/m.test(justfile))
    errors.push("root coverage threshold default differs");
  if (
    !/^COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default\("COVERAGE_MAX_UNCOVERED_LINES", "0"\)$/m.test(
      justfile,
    )
  )
    errors.push("root uncovered-line default differs");
  const rootReports = reportLines(root);
  if (rootReports.length !== 1 || rootReports[0] !== rootCommand)
    errors.push("root combined coverage producer differs");
  const performanceReports = reportLines(performance);
  if (
    performanceReports.length !== 2 ||
    ![performanceCommand("baseline"), performanceCommand("candidate")].every((command) =>
      performanceReports.includes(command),
    )
  )
    errors.push("performance coverage arms differ");
}

function moonCoverageGraphVerified(
  policy: NonNullable<ContractPolicy["moon"]>,
  tasks: Map<string, MoonTask>,
  recipes: Map<string, string>,
  justfile: string,
  errors: string[],
): void {
  if (!policy.requiredHeavy.includes("unit-test") || !policy.requiredHeavy.includes("coverage"))
    return;
  moonCoverageRecipesVerified(recipes, justfile, errors);
  const aggregate = (recipes.get("coverage-result-check") ?? "").trim();
  if (
    aggregate !==
    'python3 scripts/release/native_coverage_local_aggregate.py --repo "{{REPO_ROOT}}"'
  )
    errors.push("moon coverage aggregate validation differs");
  if (tasks.get("coverage")?.deps.join("\0") !== "performance-coverage")
    errors.push("moon coverage producer dependency differs");
  if (!moonClosure("coverage", tasks).has("unit-test"))
    errors.push("moon coverage unit-test dependency missing");
  if (!moonClosure("unit-test", tasks).has("runtime-asset-script-test"))
    errors.push("moon unit-test asset script dependency missing");
  moonRuntimeUnusedGraphVerified(tasks, recipes, errors);
  const assetAggregate = tasks.get("runtime-asset-script-test");
  if (
    assetAggregate === undefined ||
    assetAggregate.leaf !== "runtime-asset-script-aggregate-run" ||
    assetAggregate.deps.join("\0") !==
      ["runtime-asset-script-other", "runtime-unused-script-regression"].join("\0")
  )
    errors.push("moon asset script aggregate differs");
}

function moonRuntimeUnusedGraphVerified(
  tasks: Map<string, MoonTask>,
  recipes: Map<string, string>,
  errors: string[],
): void {
  if (tasks.has("runtime-unused-script-regression-producer"))
    errors.push("legacy runtime unused producer task remains");
  const sharedPrefix: ReadonlyArray<readonly [string, string]> = [
    ["runtime-asset-script-preflight", "mathjax-owned-evidence"],
    ["runtime-classic-generation-evidence", "runtime-asset-script-preflight"],
    ["runtime-asset-script-other", "runtime-classic-generation-evidence"],
  ];
  for (const [task, dependency] of sharedPrefix)
    if (tasks.get(task)?.deps.join("\0") !== dependency)
      errors.push(`moon asset script dependency differs: ${task} -> ${dependency}`);
  RUNTIME_UNUSED_LEAF_IDS.forEach((id, index) => {
    moonRuntimeUnusedProfileVerified(id, index, tasks, recipes, errors);
  });
  const final = tasks.get("runtime-unused-script-regression");
  if (
    !final ||
    final.deps.join("\0") !== `${RUNTIME_UNUSED_LEAF_IDS.at(-1) ?? ""}-producer` ||
    final.command.join("\0") !== "just\0runtime-unused-script-final-verify-run"
  )
    errors.push("runtime unused final verification differs");
  if (
    recipes.get("runtime-unused-script-final-verify-run")?.trim() !==
    RUNTIME_UNUSED_FINAL_VERIFY_COMMAND
  )
    errors.push("runtime unused final verification recipe differs");
  const compatibility = `bun test --path-ignore-patterns 'tmp/**' ${RUNTIME_UNUSED_REGRESSION_TESTS.map(
    (test) => `./${test}`,
  ).join(" ")}`;
  if (recipes.get("runtime-unused-script-regression-run")?.trim() !== compatibility)
    errors.push("runtime unused compatibility recipe differs");
}

function moonRuntimeUnusedProfileVerified(
  id: string,
  index: number,
  tasks: Map<string, MoonTask>,
  recipes: Map<string, string>,
  errors: string[],
): void {
  const test = RUNTIME_UNUSED_REGRESSION_TESTS[index];
  const producer = tasks.get(`${id}-producer`);
  const previous =
    index === 0
      ? "runtime-classic-generation-evidence"
      : `${RUNTIME_UNUSED_LEAF_IDS[index - 1]}-producer`;
  if (!producer || !test || !previous) {
    errors.push(`runtime unused profile missing: ${id}`);
    return;
  }
  if (producer.deps.join("\0") !== previous)
    errors.push(`runtime unused producer dependency differs: ${id}`);
  if (producer.command.join("\0") !== `just\0runtime-unused-script-producer-run\0${id}`)
    errors.push(`runtime unused producer command differs: ${id}`);
  if (recipes.get(`${id}-run`)?.trim() !== `bun test --path-ignore-patterns 'tmp/**' ./${test}`)
    errors.push(`runtime unused leaf recipe differs: ${id}`);
}

export function biomeBootstrapDispatchVerified(recipes: ReadonlyMap<string, string>): boolean {
  return (
    recipes.get("biome")?.trim() === "bun run moon exec krr:biome" &&
    recipes
      .get("biome-run")
      ?.trim()
      .split("\n")
      .map((line) => line.trim())
      .join("\n") ===
      'bun scripts/runtime-bundles/runtime-rust-facts-host.ts verify {{quote(REPO_ROOT)}} {{quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json")}}\nbun scripts/runtime-bundles/runtime-unused-check.ts --gate'
  );
}

function moonBarrier(
  policy: NonNullable<ContractPolicy["moon"]>,
  tasks: Map<string, MoonTask>,
  errors: string[],
  rust: ContractPolicy["rust"],
): Set<string> {
  const reachable = moonClosure(policy.entry, tasks);
  const staticTasks = moonClosure(policy.barrier, tasks);
  for (const name of policy.requiredStatic) {
    if (!staticTasks.has(name) || tasks.get(name)?.leaf !== moonStaticLeaf(name, policy))
      errors.push(`moon static leaf missing: ${name}`);
  }
  for (const name of policy.requiredHeavy) {
    const task = tasks.get(name);
    if (!reachable.has(name) || !moonHeavyTaskIsValid(name, task))
      errors.push(`moon heavy leaf missing: ${name}`);
    else if (!moonClosure(name, tasks).has(policy.barrier))
      errors.push(`moon static barrier bypass: ${name}`);
  }
  moonHostEvidenceOrder(policy, tasks, errors);
  // staticへの合流だけでは、共有Cargo出力を使うlintの先行開始を防げない。
  if (
    rust &&
    policy.ciEntry &&
    (policy.ciEntry === rust.lintRecipe || !moonClosure(rust.lintRecipe, tasks).has(policy.ciEntry))
  )
    errors.push(`moon cheap static barrier bypass: ${rust.lintRecipe}`);
  return reachable;
}

function moonHeavyTaskIsValid(name: string, task: MoonTask | undefined): boolean {
  return name === "runtime-asset-script-test"
    ? task?.leaf === "runtime-asset-script-aggregate-run"
    : task?.leaf === moonHeavyLeaf(name);
}

function moonCommand(words: string[], target: string): boolean {
  const start = words[0] === "moon" ? 1 : words.slice(0, 3).join(" ") === "bun run moon" ? 3 : -1;
  if (start < 0) return false;
  const args = words.slice(start);
  if (args[0] === "--concurrency") args.splice(0, 2);
  return (
    args.join("\0") === ["exec", target, "--on-failure", "continue", "--no-actions"].join("\0")
  );
}

function ciCommands(job: Record<string, unknown> | undefined): string[] {
  const steps = Array.isArray(job?.steps) ? job.steps : [];
  if (
    !job ||
    job["continue-on-error"] ||
    /always\(|failure\(|cancelled\(/.test(String(job.if ?? ""))
  )
    throw new Error("unsafe/missing CI job");
  return steps.flatMap((value) => {
    const step = object(value);
    if (typeof step?.run !== "string") return [];
    if (step["continue-on-error"] || /always\(|failure\(|cancelled\(/.test(String(step.if ?? ""))) {
      if (
        /^(?:just\s|bun run moon\s|moon\s|bun scripts\/html\/harness-contract-cli|cargo (?:build|test|run)\s)/m.test(
          step.run,
        )
      )
        throw new Error("unsafe CI run step");
      return [];
    }
    return step.run
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean);
  });
}

function ciStaticTasks(
  moon: NonNullable<ContractPolicy["moon"]>,
  job: Record<string, unknown> | undefined,
  commands: string[],
  tasks: Map<string, MoonTask>,
): Set<string> {
  const target = `${moon.project}:${moon.ciEntry ?? moon.entry}`;
  const entryIndex = commands.findIndex((command) => moonCommand(shellWords(command), target));
  if (entryIndex < 0) throw new Error("CI moon entry registration missing");
  const preflightIndex = commands.indexOf("bun scripts/html/harness-contract-cli.ts");
  if (preflightIndex < 0 || preflightIndex >= entryIndex)
    throw new Error("CI contract preflight ordering missing");
  const upstreamSteps = job?.steps;
  if (
    Array.isArray(upstreamSteps) &&
    upstreamSteps.some(
      (step) =>
        object(step)?.if !== undefined &&
        typeof object(step)?.run === "string" &&
        String(object(step)?.run).includes(target),
    )
  )
    throw new Error("conditional CI static entry");
  const staticTasks = moonClosure(moon.ciEntry ?? moon.entry, tasks);
  for (const name of moon.ciRequiredStatic ?? []) {
    if (!staticTasks.has(name) || tasks.get(name)?.leaf !== moonStaticLeaf(name, moon))
      throw new Error(`CI static leaf missing: ${name}`);
  }
  return staticTasks;
}

function ciDownstreamLeaves(
  moon: NonNullable<ContractPolicy["moon"]>,
  downstream: Record<string, unknown> | undefined,
  recipes: Map<string, string>,
): string[] {
  const needs = typeof downstream?.needs === "string" ? [downstream.needs] : downstream?.needs;
  if (!Array.isArray(needs) || !needs.includes(moon.ciJob))
    throw new Error("CI required static job dependency missing");
  const downstreamCommands = ciCommands(downstream);
  const leaves = downstreamCommands.flatMap((command) => {
    const words = shellWords(command);
    return words[0] === "just" && recipes.has(words[1] ?? "") ? [words[1] ?? ""] : [];
  });
  const firstHeavy = downstreamCommands.findIndex((command) => {
    const words = shellWords(command);
    return (
      (words[0] === "just" &&
        (moon.requiredHeavy.includes(words[1] ?? "") ||
          /(?:test|coverage|fidelity|interaction|package-check)$/.test(words[1] ?? ""))) ||
      (words[0] === "cargo" && ["build", "test", "run"].includes(words[1] ?? ""))
    );
  });
  for (const name of moon.ciBeforeHeavy ?? []) {
    const index = downstreamCommands.indexOf(`just ${name}`);
    if (index < 0 || firstHeavy < 0 || index >= firstHeavy)
      throw new Error(`CI static ordering missing: ${name}`);
  }
  return leaves;
}

function ciMoonLeaves(
  moon: NonNullable<ContractPolicy["moon"]>,
  workflow: Record<string, unknown>,
  tasks: Map<string, MoonTask>,
  recipes: Map<string, string>,
  fullLeaves: string[],
  bridgeRecipe: string,
): string[] {
  const jobs = object(workflow.jobs);
  const job = object(jobs?.[moon.ciJob]);
  const commands = ciCommands(job);
  if (!moon.downstreamJob) {
    const target = `${moon.project}:${moon.entry}`;
    if (
      commands.some((command) => moonCommand(shellWords(command), target)) ||
      commands.includes(`just ${bridgeRecipe}`)
    )
      return fullLeaves;
    throw new Error("CI moon entry registration missing");
  }
  const staticTasks = ciStaticTasks(moon, job, commands, tasks);
  const leaves = ciDownstreamLeaves(moon, object(jobs?.[moon.downstreamJob]), recipes);
  return [...leaves, ...[...staticTasks].flatMap((name) => tasks.get(name)?.leaf ?? [])];
}

// 実producer・consumerをreviewしたepochに固定し、呼出文字列だけの偽装を拒否する。
const MOON_TERMINAL_SOURCES = {
  "scripts/hooks/run_parallel_checks.py":
    "5b1ba155f4705bff35d3a43cac746fa57a979d54b28ec58f3ce22d09181ce6db",
  "scripts/html/harness-contract-cli.ts":
    "1113e5b23e98a1c84d654dff6a60e964f554261aa46ffa825abfeb84501bb304",
} as const;

function moonTerminalSourcesRegistered(
  policy: ContractPolicy,
  read: (name: string) => string | undefined,
  root: string,
): boolean {
  return (
    policy.localBridge.source === "scripts/hooks/run_parallel_checks.py" &&
    root.includes(policy.localBridge.source) &&
    Object.entries(MOON_TERMINAL_SOURCES).every(([name, digest]) => {
      const source = read(name);
      // checkout時のCRLFだけを正規化し、Windowsでも同じreview済み意味へ束縛する。
      return (
        source !== undefined &&
        createHash("sha256").update(source.replaceAll("\r\n", "\n")).digest("hex") === digest
      );
    })
  );
}

function localMoonRegistered(
  policy: ContractPolicy,
  read: (name: string) => string | undefined,
  bridge: string,
  root: string,
  errors: string[],
): boolean {
  const moon = policy.moon;
  if (!moon) throw new Error("moon policy missing");
  if (moon.project === "krr" && moon.entry === "coverage") {
    const registered = moonTerminalSourcesRegistered(policy, read, root);
    if (!registered) errors.push("local terminal report producer binding differs");
    return registered;
  }
  const quoted = [...bridge.matchAll(/["']([^"'\n]+)["']/g)].map((match) => match[1] ?? "");
  const start = quoted.findIndex(
    (word, index) => word === "bun" && quoted[index + 1] === "run" && quoted[index + 2] === "moon",
  );
  const words = quoted.slice(start, start + 9);
  if (words[3] === "--concurrency" && words[4] === "exec") words.splice(4, 0, "JOBS");
  const registered =
    root.includes(policy.localBridge.source) &&
    start >= 0 &&
    bridge.includes("str(jobs)") &&
    moonCommand(words, `${moon.project}:${moon.entry}`);
  if (!registered) errors.push("local moon bridge entry missing");
  if (
    moon.downstreamJob &&
    (!bridge.includes('"scripts/html/harness-contract-cli.ts"') ||
      !bridge.includes("if preflight.returncode != 0:") ||
      !bridge.includes("return preflight.returncode") ||
      bridge.indexOf('"scripts/html/harness-contract-cli.ts"') > bridge.indexOf('"moon"'))
  )
    errors.push("local contract preflight missing");
  return registered;
}

function moonBootstrapLeaves(
  moon: NonNullable<ContractPolicy["moon"]>,
  recipes: Map<string, string>,
  errors: string[],
): string[] {
  if (!moon.requiredStatic.includes("rust-facts-host-prebuild")) return [];
  if (biomeBootstrapDispatchVerified(recipes)) return ["biome"];
  errors.push("Biome bootstrap dispatch differs");
  return [];
}

function moonRoots(
  policy: ContractPolicy,
  read: (name: string) => string | undefined,
  recipes: Map<string, string>,
  errors: string[],
): { local: string[]; ci: string[] } {
  const moon = policy.moon;
  if (!moon) throw new Error("moon policy missing");
  const workspace = yamlRecord(read(moon.workspace));
  if (
    object(workspace.projects)?.[moon.project] !== "." ||
    workspace.versionConstraint !== "=2.6.2"
  )
    throw new Error("moon project/version binding differs");
  const tasks = moonTasks(yamlRecord(read(moon.config)), moon.project);
  moonCoverageGraphVerified(moon, tasks, recipes, read("Justfile") ?? "", errors);
  const reachable = moonBarrier(moon, tasks, errors, policy.rust);
  const leaves = [...reachable].flatMap((name) => tasks.get(name)?.leaf ?? []);
  leaves.push(...moonBootstrapLeaves(moon, recipes, errors));
  if (
    reachable.has("runtime-asset-script-test") &&
    recipes.get("runtime-asset-script-test")?.trim() !==
      "bun run moon exec krr:runtime-asset-script-test"
  )
    errors.push("runtime script Moon wrapper differs");
  if (reachable.has("runtime-asset-script-test")) leaves.push("runtime-asset-script-test");
  const root = recipes.get(policy.localBridge.recipe) ?? "";
  const bridge = read(policy.localBridge.source) ?? "";
  const localRegistered = localMoonRegistered(policy, read, bridge, root, errors);
  const workflow = yamlRecord(read(policy.ciWorkflow));
  let ci: string[] = [];
  try {
    ci = ciMoonLeaves(moon, workflow, tasks, recipes, leaves, policy.localBridge.recipe);
  } catch (error) {
    errors.push(error instanceof Error ? error.message : String(error));
  }
  return { local: localRegistered ? leaves : [], ci };
}

function recipeRoots(
  policy: ContractPolicy,
  read: (name: string) => string | undefined,
  recipes: Map<string, string>,
  errors: string[],
): { local: string[]; ci: string[] } {
  if (policy.moon) {
    try {
      return moonRoots(policy, read, recipes, errors);
    } catch (error) {
      errors.push(
        `moon contract invalid: ${error instanceof Error ? error.message : String(error)}`,
      );
      return { local: [], ci: [] };
    }
  }
  const bridge = read(policy.localBridge.source) ?? "";
  const root = recipes.get(policy.localBridge.recipe) ?? "";
  if (!root.includes(policy.localBridge.source)) errors.push("local bridge is not invoked");
  for (const recipe of policy.localRecipes) {
    if (!bridge.includes(`Lane("assets", "${recipe}")`))
      errors.push(`local bridge missing ${recipe}`);
  }
  const ci = [...(read(policy.ciWorkflow) ?? "").matchAll(/^\s*run:\s*just\s+(.+)$/gm)].flatMap(
    (match) => shellWords(match[1] ?? "").filter((word) => recipes.has(word)),
  );
  return { local: policy.localRecipes, ci };
}

export function executionGraph(
  policy: ContractPolicy,
  read: (name: string) => string | undefined,
  scripts: Record<string, string>,
  dependencies: Set<string>,
  inventory: string[] = [],
  resolveCode?: ManagedCodeResolver,
  resolveExternalTool?: ExternalToolResolver,
): ExecutionGraph {
  const recipes = justRecipes(read("Justfile") ?? "");
  const errors: string[] = [];
  const roots = recipeRoots(policy, read, recipes, errors);
  const unitExecutableBindings: Array<UnitExecutableBinding & { lane: "local" | "ci" }> = [];
  const externalToolBindings: Array<ExternalToolBinding & { lane: "local" | "ci" }> = [];
  const nativeConsumerBindings: Array<NativeConsumerBinding & { lane: "local" | "ci" }> = [];
  const parsedModules = new Map<string, ReturnType<typeof parseSync>>();
  const parseModule = (source: string): ReturnType<typeof parseSync> => {
    const cached = parsedModules.get(source);
    if (cached !== undefined) return cached;
    const parsed = parseSync(source, { syntax: "typescript", tsx: true });
    parsedModules.set(source, parsed);
    return parsed;
  };
  const collect = (
    roots: string[],
    lane: "local" | "ci",
  ): { code: Set<string>; recipes: Set<string> } => {
    const code = new Set<string>();
    const seen = new Set<string>();
    const sourceEdges = new Set<string>();
    const walkSource = (name: string, importer?: string, importerSource?: string): void => {
      const target = sourceTarget(name, importer, dependencies, errors, resolveCode);
      if (!target) return;
      const edge = `${target}\0${importer ?? ""}`;
      if (sourceEdges.has(edge)) return;
      sourceEdges.add(edge);
      code.add(target);
      if (!policy.codeRoots.some((root) => target.startsWith(root))) return;
      const analyzed = managedReferences(
        target,
        importer,
        importerSource,
        read,
        errors,
        (binding) => unitExecutableBindings.push({ ...binding, lane }),
        resolveExternalTool,
        (binding) => externalToolBindings.push({ ...binding, lane }),
        (binding) => nativeConsumerBindings.push({ ...binding, lane }),
        parseModule,
      );
      if (
        target === "scripts/runtime-bundles/runtime-rust-facts-host.ts" &&
        hostEvidenceSourceVerified(target, analyzed.source ?? "")
      )
        ["rust-facts-host-build", "rust-facts-host-metadata"].forEach(walkRecipe);
      analyzed.references.forEach((reference) => {
        const referenceParent = referenceImporter(reference, target);
        walkSource(
          reference,
          referenceParent,
          referenceParent === target ? analyzed.source : undefined,
        );
      });
    };
    const walkRecipe = (name: string): void => {
      if (seen.has(name)) return;
      seen.add(name);
      const body = recipes.get(name);
      if (body === undefined) {
        errors.push(`missing recipe: ${name}`);
        return;
      }
      if (name === "biome" && biomeBootstrapDispatchVerified(recipes))
        ["rust-facts-host-prebuild", "biome-run"].forEach(walkRecipe);
      if (
        name === "runtime-asset-script-test" &&
        body.trim() === "bun run moon exec krr:runtime-asset-script-test"
      ) {
        [
          "runtime-asset-script-preflight-run",
          "runtime-classic-generation-evidence",
          "runtime-asset-script-other-run",
          "runtime-unused-script-verify-run",
          "runtime-unused-script-producer-run",
        ].forEach(walkRecipe);
      }
      const words = shellWords(body);
      words.forEach((word, index) => {
        if (word === "just" && recipes.has(words[index + 1] ?? ""))
          walkRecipe(words[index + 1] ?? "");
      });
      dispatchedRecipes(name, body, policy, read, errors).forEach(walkRecipe);
      commandCode(words, scripts)
        .map((file) => file.replace(/^\.\//, ""))
        .flatMap((file) => {
          if (file.includes("*")) return inventory.filter((name) => new Bun.Glob(file).match(name));
          if (file === "scripts/html")
            return inventory.filter((name) => /[_.]test\.ts$/.test(name));
          return [file];
        })
        .filter(
          (file) =>
            name.startsWith("html-") ||
            policy.codeRoots.some((root) => file.startsWith(root)) ||
            file.startsWith("tmp/") ||
            file.startsWith("../") ||
            path.isAbsolute(file),
        )
        .forEach((file) => {
          walkSource(`!execute:${file}`);
        });
    };
    roots.forEach(walkRecipe);
    return { code, recipes: seen };
  };
  const local = collect(roots.local, "local");
  const ci = collect(roots.ci, "ci");
  return {
    local: local.code,
    ci: ci.code,
    errors,
    localRecipes: local.recipes,
    ciRecipes: ci.recipes,
    unitExecutableBindings,
    externalToolBindings,
    nativeConsumerBindings,
  };
}

function referenceImporter(reference: string, target: string): string | undefined {
  return reference.startsWith(".") ||
    reference.startsWith("#") ||
    reference.startsWith("!execute-local:")
    ? target
    : undefined;
}

function managedReferences(
  target: string,
  importer: string | undefined,
  importerSource: string | undefined,
  read: (name: string) => string | undefined,
  errors: string[],
  onUnitBinding: (binding: UnitExecutableBinding) => void,
  resolveExternalTool: ExternalToolResolver | undefined,
  onExternalToolBinding: (binding: ExternalToolBinding) => void,
  onNativeConsumerBinding: (binding: NativeConsumerBinding) => void,
  parseModule: (source: string) => ReturnType<typeof parseSync>,
): { source?: string; references: string[] } {
  const source = read(target);
  if (source === undefined) {
    errors.push(`missing code: ${target}`);
    return { references: [] };
  }
  if (!/\.[cm]?[jt]sx?$/.test(target)) return { source, references: [] };
  return {
    source,
    references: sourceCodeFromModule(
      source,
      {
        name: target,
        ...(importer ? { importer } : {}),
        ...(importerSource !== undefined ? { importerSource } : {}),
        read,
        onUnitBinding,
        ...(resolveExternalTool ? { resolveExternalTool } : {}),
        onExternalToolBinding,
        onNativeConsumerBinding,
      },
      parseModule(source),
    ),
  };
}
