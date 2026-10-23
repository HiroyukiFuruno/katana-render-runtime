import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import type {
  ExternalToolBinding,
  ExternalToolRequest,
  ExternalToolResolver,
} from "../html/harness-contract-graph";
import { shellWords } from "../html/harness-contract-graph";

import {
  inspectMathJaxDeclarations,
  MATHJAX_DECL_PROFILE,
  MATHJAX_TYPE_PROFILE,
} from "./mathjax-declaration-contract";

const strings = z.array(z.string());
const compilerRoot = path.resolve(import.meta.dir, "../..");
type CompilerPin = ExternalToolBinding["inputPins"][number];
const reviewedCompilerFiles: Record<string, string> = {
  "node_modules/typescript/package.json":
    "3722b30210616a13a3213ded11575ba6b2dbab10c32a5ef67afca8513e27017e",
  "node_modules/typescript/bin/tsc":
    "2219f428a7e55aaf1f7ad85b9b0f0cf5078aeb76ccc9a7c6036c92d48f492ffd",
  "node_modules/typescript/lib/tsc.js":
    "cbdfbf11c26ed00dfc073155e316a42d6d6d8a387be61006b82fa9aa93ac572e",
  "node_modules/typescript/lib/getExePath.js":
    "96c10fd3a18e165d7323de8e314ac8d755ad96811f9fbc304a5b3ba4e05285cc",
};
// registry identityはreview済みlock集合へ固定し、installed native bytesの公式認証とは区別する。
const reviewedCompilerLockSha256 =
  "869183e1acb21db3debfe1285ec74d8dd4a3ed4f3e82edc2a8ae82e76bc32dbc";

function fileSha256(file: string): string {
  return createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function compilerInputPins(): CompilerPin[] {
  const names = [
    ...Object.keys(reviewedCompilerFiles),
    `node_modules/@typescript/typescript-${process.platform}-${process.arch}/package.json`,
    `node_modules/@typescript/typescript-${process.platform}-${process.arch}/lib/tsc${process.platform === "win32" ? ".exe" : ""}`,
  ];
  return names.map((name) => {
    const physicalPath = safeFile(compilerRoot, name);
    return { name, physicalPath, sha256: fileSha256(physicalPath) };
  });
}

function verifyCompilerInputs(pins: CompilerPin[]): void {
  for (const input of pins) {
    const physical = safeFile(compilerRoot, input.name);
    if (physical !== input.physicalPath || fileSha256(physical) !== input.sha256)
      throw new Error(`TypeScript compiler input changed: ${input.name}`);
  }
}

function compilerLockIdentities(root: string): ExternalToolBinding["lockIdentities"] {
  const selected = `@typescript/typescript-${process.platform}-${process.arch}`;
  const pkg = z
    .object({ devDependencies: z.object({ typescript: z.literal("7.0.2") }) })
    .parse(JSON.parse(fs.readFileSync(safeFile(root, "package.json"), "utf8")));
  const lock = z
    .object({
      workspaces: z.object({
        "": z.object({ devDependencies: z.object({ typescript: z.string() }) }),
      }),
      packages: z.record(z.string(), z.unknown()),
    })
    .parse(Bun.JSONC.parse(fs.readFileSync(safeFile(root, "bun.lock"), "utf8")));
  const rows = Object.entries(lock.packages)
    .filter(([name]) => name === "typescript" || name.startsWith("@typescript/typescript-"))
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  if (
    lock.workspaces[""].devDependencies.typescript !== pkg.devDependencies.typescript ||
    createHash("sha256").update(JSON.stringify(rows)).digest("hex") !== reviewedCompilerLockSha256
  )
    throw new Error("unreviewed TypeScript registry identity");
  return ["typescript", selected].map((name) => {
    const row = z
      .tuple([
        z.literal(`${name}@7.0.2`),
        z.literal(""),
        z.record(z.string(), z.unknown()),
        z.string().regex(/^sha512-[A-Za-z0-9+/]{86}==$/),
      ])
      .parse(lock.packages[name]);
    return { name, version: "7.0.2", defaultRegistry: true, integrity: row[3] };
  });
}

function compilerPackageIdentity(): void {
  const selected = `@typescript/typescript-${process.platform}-${process.arch}`;
  const pkg = z
    .object({
      name: z.literal("typescript"),
      version: z.literal("7.0.2"),
      bin: z.object({ tsc: z.literal("./bin/tsc") }).strict(),
      imports: z.object({ "#getExePath": z.literal("./lib/getExePath.js") }),
      optionalDependencies: z.record(z.string(), z.string()),
    })
    .parse(
      JSON.parse(
        fs.readFileSync(safeFile(compilerRoot, "node_modules/typescript/package.json"), "utf8"),
      ),
    );
  if (pkg.optionalDependencies[selected] !== "7.0.2")
    throw new Error("unknown TypeScript native package");
  z.object({
    name: z.literal(selected),
    version: z.literal("7.0.2"),
    os: z.tuple([z.literal(process.platform)]),
    cpu: z.tuple([z.literal(process.arch)]),
  }).parse(
    JSON.parse(
      fs.readFileSync(safeFile(compilerRoot, `node_modules/${selected}/package.json`), "utf8"),
    ),
  );
}

function compilerRuntime(): ExternalToolBinding["runtime"] {
  const version = Bun.version;
  const physical = fs.realpathSync(process.execPath);
  if (
    physical !== process.execPath ||
    !fs.lstatSync(physical).isFile() ||
    fs.lstatSync(process.execPath).isSymbolicLink() ||
    process.versions.bun !== "1.4.4" ||
    version !== process.versions.bun
  )
    throw new Error("unverified TypeScript Bun runtime");
  return { kind: "bun", version, path: physical, sha256: fileSha256(physical) };
}

function compilerRequestMatches(root: string, request: ExternalToolRequest): boolean {
  return (
    request.kind === "typescript-bootstrap" &&
    request.caller === "scripts/quality/owned-type-projects.ts" &&
    request.target === "node_modules/typescript/bin/tsc" &&
    request.executable === "process.execPath" &&
    request.argvClass === "typescript-show-config-safe-project" &&
    fs.realpathSync(root) === compilerRoot
  );
}

function compilerToolSnapshot(root: string, request: ExternalToolRequest): ExternalToolBinding {
  const inputPins = compilerInputPins();
  for (const input of inputPins)
    if (reviewedCompilerFiles[input.name] && reviewedCompilerFiles[input.name] !== input.sha256)
      throw new Error(`unreviewed TypeScript compiler input: ${input.name}`);
  for (const name of ["package.json", "bun.lock", request.caller]) {
    const physicalPath = safeFile(root, name);
    inputPins.push({ name, physicalPath, sha256: fileSha256(physicalPath) });
  }
  compilerPackageIdentity();
  const lockIdentities = compilerLockIdentities(root);
  return {
    ...request,
    packageName: "typescript",
    packageVersion: "7.0.2",
    verification: "observed-local-pins",
    artifactVerified: false,
    inputPins,
    runtime: compilerRuntime(),
    lockIdentities,
  };
}

export function typeScriptToolBinding(root: string): ExternalToolResolver {
  let snapshot: ExternalToolBinding | undefined;
  return (request) => {
    if (!compilerRequestMatches(root, request)) return undefined;
    snapshot ??= compilerToolSnapshot(root, request);
    verifyCompilerInputs(snapshot.inputPins);
    const runtime = compilerRuntime();
    if (JSON.stringify(runtime) !== JSON.stringify(snapshot.runtime))
      throw new Error("TypeScript Bun runtime changed");
    if (
      request.callerSourceSha256 !== snapshot.callerSourceSha256 ||
      request.callerAstSha256 !== snapshot.callerAstSha256 ||
      request.swcVersion !== snapshot.swcVersion ||
      fileSha256(safeFile(root, request.caller)) !== request.callerSourceSha256
    )
      return undefined;
    return {
      ...snapshot,
      inputPins: snapshot.inputPins.map((pin) => ({ ...pin })),
      runtime: { ...snapshot.runtime },
      lockIdentities: snapshot.lockIdentities.map((identity) => ({ ...identity })),
    };
  };
}
export const typeConfiguration = z.object({
  files: strings.default([]),
  include: strings.optional(),
  compilerOptions: z.object({
    strict: z.boolean().optional(),
    noImplicitAny: z.boolean().optional(),
    strictNullChecks: z.boolean().optional(),
    noUnusedLocals: z.boolean().optional(),
    noUnusedParameters: z.boolean().optional(),
    useUnknownInCatchVariables: z.boolean().optional(),
    allowJs: z.boolean().optional(),
    checkJs: z.boolean().optional(),
    skipLibCheck: z.boolean().optional(),
    skipDefaultLibCheck: z.boolean().optional(),
    noCheck: z.boolean().optional(),
    noEmit: z.boolean().optional(),
    noUncheckedIndexedAccess: z.boolean().optional(),
    exactOptionalPropertyTypes: z.boolean().optional(),
    types: strings.optional(),
    lib: strings.optional(),
  }),
});
export function safeRelative(name: string): boolean {
  return (
    name.length > 0 &&
    !name.includes("\\") &&
    !name.includes("\0") &&
    !path.posix.isAbsolute(name) &&
    !path.win32.isAbsolute(name) &&
    path.posix.normalize(name) === name &&
    !name.split("/").some((part) => part === "." || part === "..")
  );
}
export function safeFile(root: string, name: string): string {
  if (!safeRelative(name)) throw new Error(`unsafe repository-relative source: ${name}`);
  let current = root;
  for (const part of name.split("/")) {
    current = path.join(current, part);
    if (fs.lstatSync(current).isSymbolicLink())
      throw new Error(`source path contains symlink: ${name}`);
  }
  const physical = fs.realpathSync(current);
  if (!fs.lstatSync(current).isFile() || !physical.startsWith(`${root}${path.sep}`))
    throw new Error(`source is not an in-repository regular file: ${name}`);
  return physical;
}
export function typeProjectNames(pkg: { scripts: { typecheck?: string | undefined } }): string[] {
  const commands = pkg.scripts.typecheck?.split(/\s*&&\s*/) ?? [];
  if (commands.length === 0) throw new Error("missing package typecheck script");
  return commands.map(typeProject);
}
function typeProject(command: string): string {
  const words = shellWords(command);
  if (/[;|`$<>]/.test(command) || words[0] !== "tsc" || !words.includes("--noEmit"))
    throw new Error("unsupported TypeScript checking command");
  let project = "tsconfig.json";
  let selected = false;
  for (let index = 1; index < words.length; index += 1) {
    const word = words[index];
    if (word === "--noEmit") continue;
    if ((word !== "-p" && word !== "--project") || selected || !words[index + 1])
      throw new Error("unsupported TypeScript checking option");
    project = words[index + 1] ?? "";
    selected = true;
    index += 1;
  }
  return project;
}
function typeSelectionPaths(name: string, base: string, patterns: string[]): void {
  for (const pattern of patterns) {
    if (
      path.posix.isAbsolute(pattern) ||
      path.win32.isAbsolute(pattern) ||
      !safeRelative(path.posix.join(base, pattern))
    )
      throw new Error(`external TypeScript selection: ${name}`);
  }
}

export function typeAuthorities(root: string, name: string, seen = new Set<string>()): string[] {
  if (seen.has(name)) throw new Error(`cyclic TypeScript config: ${name}`);
  seen.add(name);
  safeFile(root, name);
  const authorities = [name];
  const config = z
    .object({
      extends: z.union([z.string(), strings]).optional(),
      files: strings.optional(),
      include: strings.optional(),
      exclude: strings.optional(),
    })
    .parse(JSON.parse(fs.readFileSync(safeFile(root, name), "utf8")));
  const base = path.posix.dirname(name);
  typeSelectionPaths(name, base, [
    ...(config.files ?? []),
    ...(config.include ?? []),
    ...(config.exclude ?? []),
  ]);
  const parents = typeof config.extends === "string" ? [config.extends] : (config.extends ?? []);
  for (const parent of parents) {
    if (!parent.startsWith(".")) throw new Error(`unsupported package TypeScript config: ${name}`);
    const selected = path.posix.join(base, parent.endsWith(".json") ? parent : `${parent}.json`);
    authorities.push(...typeAuthorities(root, selected, new Set(seen)));
  }
  return authorities;
}

export function queryTypeProject(root: string, name: string) {
  const authorities = typeAuthorities(root, name);
  const config = safeFile(root, name);
  safeFile(path.resolve(import.meta.dir, "../.."), "node_modules/typescript/bin/tsc");
  // 標準型検査と同じコンパイラに選択と継承を解決させる。
  const parsed = typeConfiguration.parse(
    JSON.parse(
      execFileSync(
        process.execPath,
        [
          path.join(import.meta.dir, "../../node_modules/typescript/bin/tsc"),
          "--showConfig",
          "--project",
          config,
        ],
        {
          cwd: path.resolve(import.meta.dir, "../.."),
          encoding: "utf8",
        },
      ),
    ),
  );
  const files = parsed.files.map((file) =>
    path.relative(root, path.resolve(path.dirname(config), file)).replaceAll(path.sep, "/"),
  );
  for (const file of files) safeFile(root, file);
  return {
    name,
    files: new Set(files),
    options: parsed.compilerOptions,
    include: parsed.include ?? [],
    authorities,
  };
}

export function strictJavaScriptProject(
  options: z.infer<typeof typeConfiguration>["compilerOptions"],
): boolean {
  return (
    options.strict === true &&
    options.noImplicitAny !== false &&
    options.strictNullChecks !== false &&
    options.noUnusedLocals === true &&
    options.noUnusedParameters === true &&
    options.useUnknownInCatchVariables === true &&
    options.noUncheckedIndexedAccess === true &&
    options.exactOptionalPropertyTypes === true &&
    options.allowJs === true &&
    options.checkJs === true &&
    options.noEmit === true &&
    options.skipLibCheck !== true &&
    options.skipDefaultLibCheck !== true &&
    options.noCheck !== true
  );
}

export function javaScriptTypeReader(
  root: string,
  pkg: { scripts: { typecheck?: string | undefined } },
  tracked: ReadonlySet<string>,
) {
  const names = typeProjectNames(pkg);
  const inputs = new Map<string, string>();
  const projects = new Map<string, ReturnType<typeof queryTypeProject>>();
  const digest = (name: string) => fileSha256(safeFile(root, name));
  const pin = (name: string) => {
    const current = digest(name);
    const previous = inputs.get(name);
    if (previous !== undefined && current !== previous)
      throw new Error(`TypeScript snapshot input changed: ${name}`);
    inputs.set(name, current);
  };
  for (const name of ["package.json", "bun.lock"]) pin(name);
  const compilerPins = compilerInputPins();
  const verifyInputs = () => {
    verifyCompilerInputs(compilerPins);
    for (const name of inputs.keys()) pin(name);
  };
  const project = (name: string) => {
    const prior = projects.get(name);
    if (prior) return prior;
    for (const authority of typeAuthorities(root, name)) {
      if (!tracked.has(authority)) throw new Error(`untracked TypeScript authority: ${authority}`);
      pin(authority);
    }
    const current = queryTypeProject(root, name);
    for (const authority of current.authorities) pin(authority);
    projects.set(name, current);
    return current;
  };
  const checked = (selected: ReturnType<typeof queryTypeProject>) => {
    if (strictJavaScriptProject(selected.options)) return true;
    if (
      selected.name !== MATHJAX_TYPE_PROFILE ||
      selected.options.skipLibCheck !== true ||
      !strictJavaScriptProject({ ...selected.options, skipLibCheck: false }) ||
      !names.includes(MATHJAX_DECL_PROFILE)
    )
      return false;
    return (
      inspectMathJaxDeclarations(root, names.map(project), (name) =>
        fs.readFileSync(safeFile(root, name), "utf8"),
      ).issues.length === 0
    );
  };
  return (file: string): boolean => {
    verifyInputs();
    pin(file);
    let result = false;
    for (const name of names) {
      const selected = project(name);
      if (!selected.files.has(file)) continue;
      result = checked(selected);
      break;
    }
    verifyInputs();
    return result;
  };
}
