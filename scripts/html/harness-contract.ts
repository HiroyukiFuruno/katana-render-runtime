import { createHash } from "node:crypto";
import path from "node:path";
import {
  type ContractPolicy,
  type ExecutionGraph,
  type ExternalToolResolver,
  externalToolBindingMatches,
  isCode,
  justRecipes,
  shellWords,
  sourceCode,
} from "./harness-contract-graph";

export interface HarnessSnapshot {
  root: string;
  policy: ContractPolicy;
  graph: ExecutionGraph;
  codeFiles: string[];
  declaredCode: string[];
  tracked: Set<string>;
  biomeIncludes: string[];
  typeIncludes: string[];
  typeExcludes: string[];
  authorityFiles?: string[];
  javaScriptTyped?: (name: string) => boolean;
  externalToolResolver?: ExternalToolResolver;
  scopeMatches?: { biome: Record<string, number>; types: Record<string, number> };
  read: (name: string) => string | undefined;
}

function covered(file: string, includes: string[], excludes: string[] = []): boolean {
  return (
    includes.some((pattern) => !pattern.startsWith("!") && new Bun.Glob(pattern).match(file)) &&
    ![
      ...includes.filter((pattern) => pattern.startsWith("!")).map((pattern) => pattern.slice(1)),
      ...excludes,
    ].some((pattern) => new Bun.Glob(pattern).match(file))
  );
}

function staticScopeErrors(name: string, snapshot: HarnessSnapshot): string[] {
  if (name.endsWith(".py"))
    return pythonRegistered(name, snapshot)
      ? []
      : [`Python Ruff + strict type check registration unavailable: ${name}`];
  if (name.endsWith(".rs"))
    return rustRegistered(name, snapshot)
      ? []
      : [`Rust workspace/Clippy registration unavailable: ${name}`];
  if (!/\.[cm]?[jt]sx?$/.test(name))
    return [`required static checks unavailable for code language: ${name}`];
  const errors: string[] = [];
  if (!covered(name, snapshot.biomeIncludes)) errors.push(`lint scope missing: ${name}`);
  if (/\.[cm]?jsx?$/.test(name)) {
    try {
      if (snapshot.javaScriptTyped?.(name) !== true) errors.push(`type scope missing: ${name}`);
    } catch (error) {
      errors.push(`TypeScript project authority unavailable: ${name}: ${String(error)}`);
    }
  } else if (!covered(name, snapshot.typeIncludes, snapshot.typeExcludes))
    errors.push(`type scope missing: ${name}`);
  return errors;
}

function recipeRegistered(name: string, snapshot: HarnessSnapshot): boolean {
  return (
    snapshot.graph.localRecipes?.has(name) === true && snapshot.graph.ciRecipes?.has(name) === true
  );
}

function pythonRegistered(name: string, snapshot: HarnessSnapshot): boolean {
  const policy = snapshot.policy.python;
  if (!policy || !name.startsWith("scripts/")) return false;
  const recipes = justRecipes(snapshot.read("Justfile") ?? "");
  const lint = shellWords(recipes.get(policy.lintRecipe) ?? "");
  const types = shellWords(recipes.get(policy.typeRecipe) ?? "");
  const lintCommand = [
    "uvx",
    "--from",
    `ruff==${policy.ruff}`,
    "ruff",
    "check",
    "--select",
    "F,E4,E7,E9,B,UP",
    "scripts",
  ];
  const typeCommand = ["uvx", "--from", `mypy==${policy.mypy}`, "mypy", "--strict", "scripts"];
  return (
    lint.join("\0") === lintCommand.join("\0") &&
    types.join("\0") === typeCommand.join("\0") &&
    recipeRegistered(policy.lintRecipe, snapshot) &&
    recipeRegistered(policy.typeRecipe, snapshot)
  );
}

function rustRegistered(name: string, snapshot: HarnessSnapshot): boolean {
  const policy = snapshot.policy.rust;
  if (!policy || !recipeRegistered(policy.lintRecipe, snapshot)) return false;
  const workspace = Bun.TOML.parse(snapshot.read(policy.workspace) ?? "") as Record<
    string,
    unknown
  >;
  const value = workspace.workspace as { members?: unknown; exclude?: unknown } | undefined;
  const members = Array.isArray(value?.members) ? value.members : [];
  const excludes = Array.isArray(value?.exclude) ? value.exclude : [];
  if (
    excludes.some(
      (pattern) => typeof pattern === "string" && new Bun.Glob(`${pattern}/**`).match(name),
    )
  )
    return false;
  const member = members.find(
    (entry) =>
      typeof entry === "string" && name.startsWith(`${entry}/`) && !name.includes("/target/"),
  );
  if (typeof member !== "string") return false;
  const manifest = Bun.TOML.parse(snapshot.read(`${member}/Cargo.toml`) ?? "") as Record<
    string,
    unknown
  >;
  if (!manifest.package) return false;
  if (!rustTarget(name, member, manifest) || snapshot.read(name) === undefined) return false;
  const lint = shellWords(
    justRecipes(snapshot.read("Justfile") ?? "").get(policy.lintRecipe) ?? "",
  );
  return (
    ["clippy", "--workspace", "--all-targets", "--all-features"].every((word) =>
      lint.includes(word),
    ) &&
    lint.some((word, index) => word === "-D" && lint[index + 1] === "warnings") &&
    !lint.some((word) => ["--exclude", "--no-default-features"].includes(word))
  );
}

function rustTarget(name: string, member: string, manifest: Record<string, unknown>): boolean {
  const packageOptions = manifest.package as Record<string, unknown>;
  const library = manifest.lib as Record<string, unknown> | undefined;
  if (
    (library || packageOptions.autolib !== false) &&
    name === `${member}/${library?.path ?? "src/lib.rs"}`
  )
    return true;
  if (packageOptions.autobins !== false && name === `${member}/src/main.rs`) return true;
  return [
    ["bin", "src/bin"],
    ["test", "tests"],
    ["bench", "benches"],
    ["example", "examples"],
  ].some(([kind, directory]) => {
    const automatic =
      packageOptions[kind === "bin" ? "autobins" : `auto${directory}`] !== false &&
      new Bun.Glob(`${member}/${directory}/*.rs`).match(name);
    const targets = manifest[kind ?? ""];
    return (
      automatic ||
      (Array.isArray(targets) &&
        targets.some((target) => target.path && name === `${member}/${target.path}`))
    );
  });
}

function languageRegistered(name: string, snapshot: HarnessSnapshot): boolean {
  if (name.endsWith(".py")) return pythonRegistered(name, snapshot);
  return name.endsWith(".rs") && rustRegistered(name, snapshot);
}

export function declaredValidationCode(value: unknown): string[] {
  if (Array.isArray(value)) return value.flatMap(declaredValidationCode);
  if (value === null || typeof value !== "object") return [];
  return Object.entries(value).flatMap(([key, member]) => {
    if (/^(?:codePaths|executableCodePaths)$/.test(key)) {
      if (
        !Array.isArray(member) ||
        !member.every((name) => typeof name === "string" && isCode(name))
      )
        throw new Error(`invalid ${key}`);
      return member as string[];
    }
    if (
      /(?:caller|runner|driver|worker|helper|wrapper|collector|freezer|evaluator|analyzer|owned|r6f|capture)(?:Path)?$/i.test(
        key,
      )
    ) {
      if (typeof member !== "string") throw new Error(`invalid declared code ${key}`);
      return isCode(member) ? [member] : [];
    }
    return declaredValidationCode(member);
  });
}

function emptyScopeErrors(matches: HarnessSnapshot["scopeMatches"]): string[] {
  return Object.entries(matches ?? {}).flatMap(([kind, patterns]) =>
    Object.entries(patterns)
      .filter(([, count]) => count === 0)
      .map(([pattern]) => `empty managed ${kind} include: ${pattern}`),
  );
}

function externalToolErrors(snapshot: HarnessSnapshot): string[] {
  const errors: string[] = [];
  for (const { lane, ...binding } of snapshot.graph.externalToolBindings ?? []) {
    const request = {
      kind: binding.kind,
      caller: binding.caller,
      target: binding.target,
      executable: binding.executable,
      argvClass: binding.argvClass,
      callerSourceSha256: binding.callerSourceSha256,
      callerAstSha256: binding.callerAstSha256,
      swcVersion: binding.swcVersion,
    };
    try {
      const source = snapshot.read(binding.caller);
      const current = snapshot.externalToolResolver?.(request);
      if (
        binding.caller !== "scripts/quality/owned-type-projects.ts" ||
        binding.target !== "node_modules/typescript/bin/tsc" ||
        !snapshot.tracked.has(binding.caller) ||
        !snapshot.graph[lane].has(binding.caller) ||
        source === undefined ||
        createHash("sha256").update(source).digest("hex") !== binding.callerSourceSha256 ||
        !externalToolBindingMatches(request, binding) ||
        !current ||
        !externalToolBindingMatches(request, current) ||
        JSON.stringify(current) !== JSON.stringify(binding)
      )
        errors.push(`external tool binding changed/unverified: ${binding.target}`);
    } catch {
      errors.push(`external tool binding unavailable: ${binding.target}`);
    }
  }
  return errors;
}

export function checkHarnessContract(snapshot: HarnessSnapshot): string[] {
  const errors = [
    ...snapshot.graph.errors,
    ...externalToolErrors(snapshot),
    ...emptyScopeErrors(snapshot.scopeMatches),
    ...(snapshot.authorityFiles ?? [])
      .filter((file) => !snapshot.tracked.has(file))
      .map((file) => `untracked contract authority: ${file}`),
  ];
  const names = new Set([
    ...snapshot.codeFiles,
    ...snapshot.declaredCode,
    ...snapshot.graph.local,
    ...snapshot.graph.ci,
  ]);
  const normalize = (name: string): string =>
    path.relative(snapshot.root, path.resolve(snapshot.root, name)).replaceAll(path.sep, "/");
  const check = (raw: string): void => {
    const name = normalize(raw);
    if (name.startsWith("../") || !snapshot.policy.codeRoots.some((root) => name.startsWith(root)))
      errors.push(`misplaced code: ${raw}`);
    if (!snapshot.tracked.has(name)) errors.push(`untracked code: ${name}`);
    errors.push(...staticScopeErrors(name, snapshot));
    if (!snapshot.graph.local.has(name) && !languageRegistered(name, snapshot))
      errors.push(`local registration missing: ${name}`);
    if (!snapshot.graph.ci.has(name) && !languageRegistered(name, snapshot))
      errors.push(`CI registration missing: ${name}`);
  };
  for (const raw of names) check(raw);
  for (const raw of snapshot.declaredCode) {
    const name = normalize(raw);
    if (
      !snapshot.policy.codeRoots.some((root) => name.startsWith(root)) ||
      !/\.[cm]?[jt]sx?$/.test(name)
    )
      continue;
    const source = snapshot.read(name);
    if (source === undefined) {
      errors.push(`missing declared code: ${name}`);
      continue;
    }
    for (const reference of sourceCode(source)) {
      if (reference.startsWith(".") && isCode(reference))
        check(path.posix.join(path.posix.dirname(name), reference));
    }
  }
  return [...new Set(errors)].sort();
}
