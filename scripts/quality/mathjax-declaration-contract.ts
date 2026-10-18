import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { z } from "zod";

export const MATHJAX_TYPE_PROFILE = "scripts/quality/tsconfig-owned-native-mathjax.json";
export const MATHJAX_DECL_PROFILE = "scripts/quality/tsconfig-owned-native-mathjax-decls.json";
export const MATHJAX_PROVENANCE = "scripts/quality/mathjax-declaration-provenance.json";
export const MATHJAX_RUNTIME =
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime/render_mathjax.js";
export const MATHJAX_ABI =
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime/host_mathjax.d.ts";
// 公式型と注釈境界のレビュー後だけ更新し、検査対象から期待値を自動生成しない。
const PROVENANCE_SHA256 = "46d28ceaba5f4f59cdcd5a74e349e9e4e354271a37ee0cd80c3ca2052ce325d0";
const digestSchema = z.string().regex(/^[a-f0-9]{64}$/);
const provenanceSchema = z.strictObject({
  schema: z.literal(1),
  library: z.strictObject({
    name: z.literal("@mathjax/src"),
    version: z.literal("4.1.3"),
    packageSHA256: digestSchema,
    lockEntrySHA256: digestSchema,
  }),
  compiler: z.strictObject({
    version: z.literal("7.0.2"),
    packageSHA256: digestSchema,
    lockEntrySHA256: digestSchema,
    libs: z.array(z.strictObject({ name: z.string(), sha256: digestSchema })).length(69),
  }),
  owned: z.strictObject({
    runtimeSHA256: digestSchema,
    abiSHA256: digestSchema,
    consumerConfigSHA256: digestSchema,
    companionConfigSHA256: digestSchema,
    rustRequestSHA256: digestSchema,
    rustScriptsSHA256: digestSchema,
  }),
  declarations: z.array(z.strictObject({ path: z.string(), sha256: digestSchema })).length(77),
});

export interface MathJaxTypeProject {
  name: string;
  files: ReadonlySet<string>;
  options: Readonly<Record<string, unknown>>;
}

function sha(source: string): string {
  return createHash("sha256").update(source).digest("hex");
}
function samePaths(actual: Iterable<string>, expected: Iterable<string>): boolean {
  return [...actual].sort().join("\0") === [...expected].sort().join("\0");
}
function requireBoundary(valid: boolean, reason: string): void {
  if (!valid) throw new Error(reason);
}
function profileFlags(project: MathJaxTypeProject, consumer: boolean): void {
  const options = project.options;
  for (const flag of [
    "strict",
    "noImplicitAny",
    "strictNullChecks",
    "noUncheckedIndexedAccess",
    "exactOptionalPropertyTypes",
    "useUnknownInCatchVariables",
    "noUnusedLocals",
    "noUnusedParameters",
    "noEmit",
  ])
    requireBoundary(options[flag] === true, `MathJax strict option missing: ${flag}`);
  for (const flag of ["allowJs", "checkJs", "skipLibCheck"])
    requireBoundary(options[flag] === consumer, `MathJax profile option mismatch: ${flag}`);
  requireBoundary(
    JSON.stringify(options.lib) === '["es2024"]' && JSON.stringify(options.types) === "[]",
    "MathJax profile must use ES2024/types[] without DOM",
  );
}
function lockEntry(source: string, name: string): string {
  const prefix = `"${name}": [`;
  const rows = source
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.startsWith(prefix));
  requireBoundary(rows.length === 1, `missing or ambiguous Bun lock entry: ${name}`);
  const row = rows[0];
  if (!row) throw new Error("missing Bun lock entry");
  const entry: unknown = JSON.parse(row.slice(row.indexOf(":") + 1).replace(/,$/, ""));
  return sha(JSON.stringify(entry));
}
const installedRoot = fs.realpathSync(path.resolve(import.meta.dir, "../.."));
function installed(name: string): string {
  const file = path.join(installedRoot, name);
  requireBoundary(fs.realpathSync(file) === file, `symlinked compiler input: ${name}`);
  return fs.readFileSync(file, "utf8");
}
function compilerFiles(root: string, profile: string): string[] {
  const result = execFileSync(
    "bunx",
    [
      "--no-install",
      "tsc",
      "--listFilesOnly",
      "--pretty",
      "false",
      "--project",
      path.join(root, profile),
    ],
    { cwd: installedRoot, encoding: "utf8" },
  );
  return result.split(/\r?\n/).filter(Boolean);
}
function closure(
  root: string,
  inputs: string[],
  read: (name: string) => string,
  vendor: ReadonlyMap<string, string>,
  libs: ReadonlyMap<string, string>,
  pins: string[],
): Set<string> {
  const owned = new Set<string>();
  const foundVendor = new Set<string>();
  const foundLibs = new Set<string>();
  const nativeLib = path.join(
    installedRoot,
    "node_modules/@typescript",
    `typescript-${process.platform}-${process.arch}`,
    "lib",
  );
  for (const file of inputs) {
    requireBoundary(path.isAbsolute(file), "nonphysical compiler input");
    const relative = path.relative(root, file).replaceAll(path.sep, "/");
    let source: string;
    if (path.dirname(file) === nativeLib) {
      const name = path.basename(file);
      source = installed(path.relative(installedRoot, file));
      requireBoundary(libs.get(name) === sha(source), `unknown or changed compiler lib: ${name}`);
      foundLibs.add(name);
    } else {
      source = read(relative);
      if (vendor.has(relative)) {
        requireBoundary(
          vendor.get(relative) === sha(source),
          `changed official MathJax declaration: ${relative}`,
        );
        foundVendor.add(relative);
      } else {
        requireBoundary(
          !relative.startsWith("node_modules/"),
          `unapproved external declaration: ${relative}`,
        );
        requireBoundary(
          relative === MATHJAX_RUNTIME || relative.endsWith(".d.ts"),
          `unexpected owned compiler input: ${relative}`,
        );
        if (relative.endsWith(".d.ts")) owned.add(relative);
      }
    }
    pins.push(`${file}\0${sha(source)}\n`);
  }
  requireBoundary(samePaths(foundLibs, libs.keys()), "incomplete compiler ES declaration closure");
  if (vendor.size)
    requireBoundary(
      samePaths(foundVendor, vendor.keys()),
      "MathJax official declaration closure changed",
    );
  return owned;
}

export function inspectMathJaxDeclarations(
  root: string,
  projects: readonly MathJaxTypeProject[],
  read: (name: string) => string,
): { inputDigest: string; issues: string[] } {
  const pins: string[] = [];
  const observed = new Map<string, string>();
  const source = (name: string): string => {
    const value = read(name);
    const prior = observed.get(name);
    requireBoundary(prior === undefined || prior === sha(value), `MathJax input changed: ${name}`);
    observed.set(name, sha(value));
    return value;
  };
  try {
    const consumer = projects.find((project) => project.name === MATHJAX_TYPE_PROFILE);
    const companion = projects.find((project) => project.name === MATHJAX_DECL_PROFILE);
    requireBoundary(
      consumer !== undefined && companion !== undefined,
      "MathJax paired profiles are missing",
    );
    if (!consumer || !companion) throw new Error("MathJax paired profiles are missing");
    profileFlags(consumer, true);
    profileFlags(companion, false);
    requireBoundary(
      samePaths(consumer.files, [MATHJAX_RUNTIME, MATHJAX_ABI]),
      "MathJax consumer source set changed",
    );
    const manifest = source(MATHJAX_PROVENANCE);
    requireBoundary(sha(manifest) === PROVENANCE_SHA256, "unreviewed MathJax provenance");
    const provenance = provenanceSchema.parse(JSON.parse(manifest));
    requireBoundary(
      sha(source(MATHJAX_RUNTIME)) === provenance.owned.runtimeSHA256,
      "unreviewed MathJax runtime annotations or API use",
    );
    requireBoundary(
      sha(source(MATHJAX_ABI)) === provenance.owned.abiSHA256,
      "unreviewed Rust MathJax ABI declaration",
    );
    requireBoundary(
      sha(source(MATHJAX_TYPE_PROFILE)) === provenance.owned.consumerConfigSHA256 &&
        sha(source(MATHJAX_DECL_PROFILE)) === provenance.owned.companionConfigSHA256,
      "unreviewed MathJax profile configuration",
    );
    requireBoundary(
      sha(source("crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime.rs")) ===
        provenance.owned.rustRequestSHA256 &&
        sha(
          source(
            "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime_scripts.rs",
          ),
        ) === provenance.owned.rustScriptsSHA256,
      "Rust MathJax request/call origin changed",
    );
    const pkg = source("node_modules/@mathjax/src/package.json");
    requireBoundary(
      sha(pkg) === provenance.library.packageSHA256,
      "MathJax package identity changed",
    );
    requireBoundary(
      sha(installed("node_modules/typescript/package.json")) === provenance.compiler.packageSHA256,
      "compiler package identity changed",
    );
    const lock = source("bun.lock");
    requireBoundary(
      lockEntry(lock, "@mathjax/src") === provenance.library.lockEntrySHA256 &&
        lockEntry(lock, "typescript") === provenance.compiler.lockEntrySHA256,
      "MathJax/compiler lock entry changed",
    );
    const vendor = new Map(provenance.declarations.map((file) => [file.path, file.sha256]));
    const libs = new Map(provenance.compiler.libs.map((file) => [file.name, file.sha256]));
    requireBoundary(vendor.size === 77 && libs.size === 69, "duplicate declaration provenance");
    const own = closure(root, compilerFiles(root, consumer.name), source, vendor, libs, pins);
    const companionOwned = closure(
      root,
      compilerFiles(root, companion.name),
      source,
      new Map(),
      libs,
      pins,
    );
    requireBoundary(
      samePaths(own, companion.files) && samePaths(own, companionOwned),
      "ALL owned declarations must have equal strict companion coverage",
    );
    requireBoundary(
      projects.some(
        (project) =>
          project.name !== consumer.name &&
          project.name !== companion.name &&
          project.options.skipLibCheck !== true &&
          Array.isArray(project.options.lib) &&
          project.options.lib.includes("es2024"),
      ),
      "compiler ES libraries lack ordinary strict coverage",
    );
    for (const name of observed.keys()) source(name);
    pins.push(...[...observed].map(([name, hash]) => `${name}\0${hash}\n`));
    return { inputDigest: sha(pins.sort().join("")), issues: [] };
  } catch (error) {
    return {
      inputDigest: sha(pins.sort().join("")),
      issues: [error instanceof Error ? error.message : String(error)],
    };
  }
}
