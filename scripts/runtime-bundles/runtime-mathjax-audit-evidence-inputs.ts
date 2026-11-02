import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { z } from "zod";
import { RUNTIME_UNUSED_REGRESSION_TESTS } from "../html/harness-contract-graph";
import { fileSha256 } from "../mermaid/mermaid-pair-manifest";
import { canonicalRegularFile, type FileRecord } from "../mermaid/native-build-receipt";
import { runtimeUnusedImportScan } from "./runtime-unused-import-scan";
import { MATHJAX_SOURCE_PATH } from "./runtime-unused-mathjax";
export const source = "scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts";
export const owned = "mathjax-owned-raw";
export const unicode = "mathjax-unicode-unused";
export const suffix =
  '\nconst displayed = "😀";\nconst unused_Ω = "😀";\nconsole.log(displayed);\n';
export function record(file: string): FileRecord {
  const canonical = canonicalRegularFile(file, "MathJax evidence input");
  return { path: canonical, sha256: fileSha256(canonical), size: fs.statSync(canonical).size };
}

export function environment() {
  return Object.fromEntries(
    Object.entries(process.env)
      .filter(
        ([key, value]) =>
          value !== undefined && /^(PATH$|NODE_|BUN_|BIOME_|SWC_|ROLLUP_|npm_config_)/.test(key),
      )
      .sort(([left], [right]) => left.localeCompare(right)),
  );
}

function mathJaxInputTree(root: string, tree: string) {
  const files: string[] = [];
  const directory = path.join(root, tree);
  if (fs.lstatSync(directory).isSymbolicLink())
    throw new Error("MathJax input tree contains symlink");
  for (const name of fs.readdirSync(directory, { recursive: true, encoding: "utf8" })) {
    const file = path.join(directory, name);
    const stat = fs.lstatSync(file);
    if (stat.isSymbolicLink()) throw new Error("MathJax input tree contains symlink");
    if (
      stat.isFile() &&
      !(tree === "scripts/runtime-bundles" && /(?:_test|\.test)\.ts$/.test(name))
    )
      files.push(file);
  }
  return files;
}

export function runtimeUnusedImportInputs(root: string) {
  return runtimeUnusedImportScan(root, RUNTIME_UNUSED_REGRESSION_TESTS, canonicalRegularFile);
}

export function verifyRuntimeUnusedImportPins(
  imported: ReturnType<typeof runtimeUnusedImportInputs>,
  records: FileRecord[],
): void {
  const pins = new Set(records.map((entry) => entry.path));
  for (const file of [...imported.sources, ...imported.tools])
    if (!pins.has(file)) throw new Error(`Runtime unused import is not physically pinned: ${file}`);
}

export function inputs(root: string): FileRecord[] {
  if (
    process.env.BIOME_BINARY ||
    process.env.SWC_BINARY_PATH ||
    process.env.NODE_OPTIONS ||
    process.env.NODE_PATH
  )
    throw new Error("MathJax evidence rejects unpinned tool override");
  const imported = runtimeUnusedImportInputs(root);
  const files = [
    "package.json",
    "bun.lock",
    "biome.json",
    "tsconfig.json",
    "node_modules/@mathjax/src/package.json",
    "node_modules/@mathjax/mathjax-newcm-font/package.json",
    MATHJAX_SOURCE_PATH,
    "scripts/mermaid/native-build-receipt.ts",
    "scripts/mermaid/mermaid-pair-manifest.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/shared/runtime_bundle.ts",
  ].map((name) => path.join(root, name));
  files.push(...imported.sources, ...imported.dedicatedTools);
  const trees = [
    "scripts/runtime-bundles",
    "node_modules/@mathjax/src/mjs",
    "node_modules/@mathjax/mathjax-newcm-font/mjs",
    "node_modules/@rollup",
    "node_modules/@swc",
    "node_modules/@biomejs",
    ..."rollup terser zod acorn acorn-loose acorn-walk source-map-js magic-string estree-walker picomatch resolve is-core-module deepmerge commondir fdir p-map @jridgewell @types/resolve @types/estree is-reference is-module smob es-errors hasown function-bind path-parse supports-preserve-symlinks-flag commander source-map-support source-map buffer-from"
      .split(" ")
      .map((name) => `node_modules/${name}`),
  ];
  if (fs.existsSync(path.join(root, "node_modules/fsevents"))) trees.push("node_modules/fsevents");
  for (const tree of trees) files.push(...mathJaxInputTree(root, tree));
  const bunx = Bun.which("bunx");
  const node = Bun.which("node");
  if (!bunx || !node) throw new Error("MathJax evidence requires actual bunx/node");
  files.push(fs.realpathSync(process.execPath), fs.realpathSync(bunx), fs.realpathSync(node));
  for (const name of ["bunfig.toml", ".npmrc"])
    if (fs.existsSync(path.join(root, name))) files.push(path.join(root, name));
  const records = [...new Set(files)].sort().map(record);
  verifyMathJaxToolDependencies(records);
  verifyRuntimeUnusedImportPins(imported, records);
  return records;
}

export function verifyMathJaxToolDependencies(records: FileRecord[]) {
  const pins = new Set(records.map((entry) => entry.path));
  for (const manifest of records.filter(
    (entry) =>
      /\/node_modules\/(?:@[^/]+\/)?[^/]+\/package\.json$/.test(
        entry.path.split(path.sep).join("/"),
      ) && !entry.path.includes(`${path.sep}@mathjax${path.sep}`),
  )) {
    const metadata = z
      .object({
        dependencies: z.record(z.string(), z.string()).optional(),
        optionalDependencies: z.record(z.string(), z.string()).optional(),
      })
      .parse(JSON.parse(fs.readFileSync(manifest.path, "utf8")));
    for (const [name, optional] of [
      ...Object.keys(metadata.dependencies ?? {}).map((name) => [name, false] as const),
      ...Object.keys(metadata.optionalDependencies ?? {}).map((name) => [name, true] as const),
    ]) {
      const resolved = resolveMathJaxToolDependency(manifest.path, name, optional);
      if (resolved === null) continue;
      if (!pins.has(canonicalRegularFile(resolved, "MathJax resolved dependency")))
        throw new Error(`MathJax tool dependency is absent from physical pins: ${name}`);
    }
  }
}

function resolveMathJaxToolDependency(manifest: string, name: string, optional: boolean) {
  const resolver = createRequire(manifest);
  try {
    return resolver.resolve(`${name}/package.json`);
  } catch (error) {
    const code = z.object({ code: z.string() }).safeParse(error);
    if (
      code.success &&
      code.data.code === "MODULE_NOT_FOUND" &&
      optional &&
      !resolver.resolve.paths(name)?.some((directory) => fs.existsSync(path.join(directory, name)))
    )
      return null;
    if (!code.success || code.data.code !== "ERR_PACKAGE_PATH_NOT_EXPORTED") throw error;
    return nearestMathJaxToolManifest(resolver.resolve(name), name);
  }
}

function nearestMathJaxToolManifest(entry: string, name: string) {
  let directory = path.dirname(entry);
  for (;;) {
    const manifest = path.join(directory, "package.json");
    if (
      fs.existsSync(manifest) &&
      z.object({ name: z.string().optional() }).parse(JSON.parse(fs.readFileSync(manifest, "utf8")))
        .name === name
    )
      return manifest;
    if (directory === path.dirname(directory))
      throw new Error("MathJax tool dependency manifest is unresolved");
    directory = path.dirname(directory);
  }
}

export function artifactNames() {
  return [
    "inspection.json",
    "owned-audit.json",
    "biome-context-probe.json",
    ...[owned, unicode].flatMap((name) =>
      ["js", "json", "stdout", "stderr"].map((extension) => `${name}.${extension}`),
    ),
  ].sort();
}

export function diagnosticArgv(root: string, directory: string, name: string) {
  if (fs.realpathSync(root) !== root) throw new Error("noncanonical MathJax evidence root");
  return [
    "bunx",
    "--no-install",
    "biome",
    "lint",
    `--config-path=${path.join(directory, "biome-context-probe.json")}`,
    "--only=correctness/noUnusedVariables",
    "--only=correctness/noUnusedFunctionParameters",
    "--error-on-warnings",
    "--max-diagnostics=none",
    "--reporter=json",
    `--reporter-file=${path.join(directory, `${name}.json`)}`,
    path.join(directory, `${name}.js`),
  ];
}

export function verifyEvidenceDirectory(root: string, receiptRoot: string, directory: string) {
  const evidenceRoot = path.join(root, "tmp/quality/mathjax-audit-evidence/runs");
  if (
    receiptRoot !== fs.realpathSync(root) ||
    !directory.startsWith(`${evidenceRoot}${path.sep}`) ||
    fs.realpathSync(directory) !== directory ||
    fs.lstatSync(directory).isSymbolicLink()
  )
    throw new Error("MathJax evidence directory/root mismatch");
}
