import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { z } from "zod";
import { canonicalRegularFile } from "../mermaid/native-build-receipt";
import {
  hostEnvironmentDigest,
  hostFileRecord,
  hostInputsSchema,
} from "./runtime-rust-facts-host-protocol";

export { hostFileRecord } from "./runtime-rust-facts-host-protocol";
export type RustFactsHostInputs = z.infer<typeof hostInputsSchema>;

const metadataSchema = z.object({
  packages: z.array(z.object({ id: z.string(), name: z.string(), manifest_path: z.string() })),
  resolve: z.object({
    nodes: z.array(z.object({ id: z.string(), dependencies: z.array(z.string()) })),
  }),
  target_directory: z.string(),
});

export function parseRustFactsHostMetadata(bytes: Buffer): unknown {
  const value: unknown = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  return metadataSchema.parse(value);
}

function packageRoots(root: string, value: unknown): string[] {
  const metadata = metadataSchema.parse(value);
  if (metadata.target_directory !== path.join(root, "target"))
    throw new Error("Rust facts host metadata target differs");
  const host = metadata.packages.filter(
    (item) =>
      item.name === "kdr-linter" &&
      item.manifest_path === path.join(root, "crates/kdr-linter/Cargo.toml"),
  );
  if (host.length !== 1 || !host[0]) throw new Error("Rust facts host package is not unique");
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const packages = new Map(metadata.packages.map((item) => [item.id, item]));
  if (nodes.size !== metadata.resolve.nodes.length || packages.size !== metadata.packages.length)
    throw new Error("Rust facts host dependency metadata is ambiguous");
  const queue = [host[0].id];
  const visited = new Set<string>();
  const roots: string[] = [];
  for (const id of queue) {
    if (visited.has(id)) continue;
    visited.add(id);
    const node = nodes.get(id);
    const item = packages.get(id);
    if (!node || !item) throw new Error("Rust facts host dependency metadata is incomplete");
    roots.push(path.dirname(canonicalRegularFile(item.manifest_path, "host dependency manifest")));
    queue.push(...node.dependencies);
  }
  return [...new Set(roots)].sort();
}

function sourceFiles(directory: string): string[] {
  if (
    !path.isAbsolute(directory) ||
    fs.realpathSync(directory) !== directory ||
    !fs.lstatSync(directory).isDirectory()
  )
    throw new Error("unsafe host source directory");
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .sort((a, b) => a.name.localeCompare(b.name))
    .flatMap((item) => {
      const file = path.join(directory, item.name);
      if (item.isSymbolicLink()) throw new Error("host source closure contains a symlink");
      return item.isDirectory() ? sourceFiles(file) : [canonicalRegularFile(file, "host source")];
    });
}

function configurationFiles(root: string): string[] {
  const bases = [process.env.CARGO_HOME ?? path.join(os.homedir(), ".cargo")];
  const ancestors: string[] = [];
  let directory = root;
  for (;;) {
    bases.push(path.join(directory, ".cargo"));
    ancestors.push(directory);
    const parent = path.dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  const files = [...new Set(bases)]
    .flatMap((base) => ["config", "config.toml"].map((name) => path.join(base, name)))
    .filter((file) => fs.lstatSync(file, { throwIfNoEntry: false }) !== undefined);
  files.push(
    ...ancestors
      .flatMap((ancestor) =>
        ["rust-toolchain", "rust-toolchain.toml"].map((name) => path.join(ancestor, name)),
      )
      .filter((file) => fs.lstatSync(file, { throwIfNoEntry: false }) !== undefined),
  );
  const settings = path.join(
    process.env.RUSTUP_HOME ?? path.join(os.homedir(), ".rustup"),
    "settings.toml",
  );
  if (fs.lstatSync(settings, { throwIfNoEntry: false }) !== undefined) files.push(settings);
  return files;
}

function toolLocation(root: string, name: keyof RustFactsHostInputs["tools"]): string | null {
  const options = { cwd: root, PATH: process.env.PATH ?? "" };
  if (name === "bun") return process.execPath;
  if (name === "rtk" && process.env.RTK !== undefined)
    return process.env.RTK === "" ? null : Bun.which(process.env.RTK, options);
  return Bun.which(name, options);
}

function currentTools(
  root: string,
  prepared?: RustFactsHostInputs["tools"],
): RustFactsHostInputs["tools"] {
  const names = ["just", "cargo", "rustc", "rtk", "python3", "bun"] as const;
  return hostInputsSchema.shape.tools.parse(
    Object.fromEntries(
      names.map((name) => {
        const located = toolLocation(root, name);
        if (!located) {
          if (name === "rtk" && !process.env.RTK) return [name, null];
          throw new Error(`missing Rust facts host tool: ${name}`);
        }
        const launcher = hostFileRecord(fs.realpathSync(located));
        let file = prepared?.[name]?.executable.path ?? launcher.path;
        const rustup = Bun.which("rustup", { cwd: root, PATH: process.env.PATH ?? "" });
        if (
          !prepared &&
          ["cargo", "rustc"].includes(name) &&
          rustup &&
          hostFileRecord(fs.realpathSync(rustup)).sha256 === launcher.sha256
        )
          file = execFileSync(fs.realpathSync(rustup), ["which", name], {
            encoding: "utf8",
            cwd: root,
          }).trim();
        return [name, { launcher, executable: hostFileRecord(file) }];
      }),
    ),
  );
}

export function currentRustFactsHostInputs(
  root: string,
  metadata: unknown,
  preparedTools?: RustFactsHostInputs["tools"],
): RustFactsHostInputs {
  if (!path.isAbsolute(root) || fs.realpathSync(root) !== root || !fs.lstatSync(root).isDirectory())
    throw new Error("Rust facts host root is not canonical");
  const authority = [
    "Cargo.toml",
    "Cargo.lock",
    "Justfile",
    "moon.yml",
    "package.json",
    "bun.lock",
    "scripts/runtime-bundles/runtime-rust-facts-host.ts",
    "scripts/runtime-bundles/runtime-rust-facts-host-inputs.ts",
    "scripts/runtime-bundles/runtime-rust-facts-host-protocol.ts",
    "scripts/runtime-bundles/runtime-unused-mathjax.ts",
    "scripts/runtime-bundles/runtime-unused-native-width-hash.ts",
    "scripts/runtime-bundles/runtime-unused-rust-classic.ts",
    "scripts/html/harness-contract-graph.ts",
    "scripts/html/harness-contract-policy.json",
    "scripts/quality/owned-validation-contract.ts",
  ];
  const files = [
    ...packageRoots(root, metadata).flatMap(sourceFiles),
    // guard/lock/toml_loaderとvendored _tomliはPython sourceを使い、.pyc生成物は入力ではない。
    ...sourceFiles(path.join(root, "scripts/build")).filter((file) => file.endsWith(".py")),
    ...sourceFiles(path.join(root, "scripts/release")).filter((file) => file.endsWith(".py")),
    ...authority.map((relative) => path.join(root, relative)),
    ...configurationFiles(root),
  ];
  return hostInputsSchema.parse({
    files: [...new Set(files)].sort().map(hostFileRecord),
    tools: currentTools(root, preparedTools),
    environmentSha256: hostEnvironmentDigest(root),
  });
}
