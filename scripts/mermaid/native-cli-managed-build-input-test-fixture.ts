import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { sha256 } from "./mermaid-pair-manifest";
import { digestFileRecords, digestStringRecord } from "./native-build-receipt";
import { writeFile } from "./native-classic-capture-test-fixture";
import { managedOwnedSchema } from "./native-cli-managed-build-schema";

function inventory(source: string, name: string): string[] {
  const block = source.match(new RegExp(`const ${name} = \\[([\\s\\S]*?)\\];`));
  if (block?.[1] === undefined) throw new Error("Managed reader fixture inventory is absent");
  return [...block[1].matchAll(/"([^"\n]+)"/g)].map((match) => match[1] ?? "");
}

export function createManagedReaderInputs() {
  // 全reader入口の単体data graph。Cargo/native/コピーしたTSを実行しない。
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "managed-reader-unit-")));
  const token = "f".repeat(64);
  const save = (name: string, bytes: string) => {
    const file = path.join(root, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    return writeFile(file, bytes);
  };
  const json = (name: string, value: unknown) => save(name, JSON.stringify(value));
  const literalSource = fs.readFileSync(
    path.join(process.cwd(), "scripts/mermaid/native-cli-managed-inputs.ts"),
    "utf8",
  );
  const control = inventory(literalSource, "controlNames").map((name) =>
    save(name, fs.readFileSync(path.join(process.cwd(), name), "utf8")),
  );
  save(".gitignore", "tmp/\ntarget/\n");
  save("Cargo.lock", "version = 4\n");
  save("Cargo.toml", '[workspace]\nmembers=["crates/katana-render-runtime-cli"]\n');
  const copyHostSourceTree = (directory: string) => {
    const source = path.join(process.cwd(), directory);
    const visit = (current: string) => {
      for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
        const file = path.join(current, entry.name);
        if (entry.isDirectory()) visit(file);
        else if (entry.isFile() && file.endsWith(".py")) {
          const relative = path.relative(process.cwd(), file);
          if (!fs.existsSync(path.join(root, relative)))
            save(relative, `unit Rust facts host input: ${relative}\n`);
        }
      }
    };
    visit(source);
  };
  copyHostSourceTree("scripts/build");
  copyHostSourceTree("scripts/release");
  for (const name of [
    "crates/kdr-linter/Cargo.toml",
    "crates/kdr-linter/src/bin/mathjax_consumer_facts.rs",
    "dependency/Cargo.toml",
    "dependency/src/lib.rs",
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
  ]) {
    if (!fs.existsSync(path.join(root, name)))
      save(name, `unit Rust facts host input: ${name}\n`);
  }
  const manifest = save(
    "crates/katana-render-runtime-cli/Cargo.toml",
    '[package]\nname="katana-render-runtime-cli"\n',
  );
  const main = save("crates/katana-render-runtime-cli/src/main.rs", "fn main() {}\n");
  const runtime = save("runtime.js", "unit runtime data");
  execFileSync("git", ["init", "--quiet"], { cwd: root });
  execFileSync("git", ["add", "."], { cwd: root });
  const names = execFileSync("git", ["ls-files", "-z"], { cwd: root })
    .toString()
    .split("\0")
    .filter(Boolean);
  const files = names
    .map((name) => {
      const raw = fs.readFileSync(path.join(root, name));
      return { path: path.join(root, name), sha256: sha256(raw), size: raw.length };
    })
    .sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path)));
  const source = {
    schema: "krr-native-build-source-manifest-v1",
    digest: digestFileRecords(files),
    files,
  };
  const sourcePin = json(`tmp/${token}/source-manifest.json`, source);
  const expectedSource = json("tmp/expected-source.json", source);
  const tools = Object.fromEntries(
    ["cargo", "rustc", "rtk", "just", "git", "bash"].map((name) => [
      name,
      save(`tmp/toolchain/${name}`, `unexecuted unit ${name} data`),
    ]),
  );
  const python = process.env.PATH?.split(path.delimiter)
    .map((base) => path.join(base, "python3"))
    .find((file) => fs.existsSync(file) && (fs.statSync(file).mode & 0o111) !== 0);
  if (python === undefined) throw new Error("Unit PATH python3 unavailable");
  const toolPin = (file: string) => {
    const raw = fs.readFileSync(fs.realpathSync(file));
    return { path: fs.realpathSync(file), sha256: sha256(raw), size: raw.length };
  };
  tools.python3 = toolPin(python);
  tools.bun = toolPin(process.execPath);
  tools.lsof = toolPin(process.platform === "darwin" ? "/usr/sbin/lsof" : "/usr/bin/lsof");
  const git = process.env.PATH?.split(path.delimiter)
    .map((base) => path.join(base, "git"))
    .find((file) => fs.existsSync(file) && (fs.statSync(file).mode & 0o111) !== 0);
  if (git === undefined) throw new Error("Unit actual Git unavailable");
  tools.git = toolPin(git);
  const tool = (name: string) => {
    const pin = tools[name];
    if (pin === undefined) throw new Error("unit tool missing");
    return pin;
  };
  const protectedPin = save("tmp/protected", "unit protected bytes"),
    protectedStat = fs.statSync(protectedPin.path);
  const refs = [
    { ...protectedPin, dev: protectedStat.dev, ino: protectedStat.ino, mode: protectedStat.mode },
  ];
  const registry = json("tmp/approved-registry.json", {
    schema: "krr-protected-refs-successor-v1",
    physicalRefCount: refs.length,
    refs,
  });
  const policy = {
    metric: "unique-inode-allocated-bytes",
    metric_version: 1,
    baseline_bytes: 2363047936,
    factor: 2,
    threshold_bytes: 4726095872,
    target: path.join(root, "target"),
  };
  const state = json("tmp/.cargo-capacity-guard.json", policy),
    protectedEnv = JSON.stringify(refs.map((pin) => pin.path));
  const capacity = {
    state,
    protectedRegistry: registry,
    refs,
    capacityPolicy: policy,
    environmentValue: protectedEnv,
    environmentSha256: sha256(Buffer.from(protectedEnv)),
  };
  const environment = {
    PATH: path.dirname(tool("cargo").path) + path.delimiter + process.env.PATH,
    HOME: path.join(root, "tmp/home"),
    PWD: root,
    _: python,
    SHLVL: "0",
    RUSTFLAGS: "-D warnings",
    CARGO_CAPACITY_PROTECTED_REFS: protectedEnv,
    ...Object.fromEntries(inventory(literalSource, "emptySelectors").map((name) => [name, ""])),
  };
  const bases: string[] = [];
  for (let base = root; ; base = path.dirname(base)) {
    bases.push(base);
    if (base === path.dirname(base)) break;
  }
  const chains = bases.flatMap((base) => [
    path.join(base, "rust-toolchain"),
    path.join(base, "rust-toolchain.toml"),
  ]);
  const absentConfigs = [
    ...new Set([
      ...bases.flatMap((base) => [
        path.join(base, ".cargo/config"),
        path.join(base, ".cargo/config.toml"),
      ]),
      path.join(environment.HOME, ".cargo/config"),
      path.join(environment.HOME, ".cargo/config.toml"),
    ]),
  ];
  const snapshot = {
    source: files,
    tools,
    control,
    environment,
    environmentSha256: digestStringRecord(environment),
    capacityAdmission: capacity,
    toolchainFiles: chains.filter((file) => fs.existsSync(file)).map(toolPin),
    toolchainAbsent: chains.filter((file) => !fs.existsSync(file)),
  };
  const owned = (
    role: "build" | "metadata" | "tool-query",
    name: string,
    argv: string[],
    pid: number,
    label: string,
    bytes: string,
    env: { [key: string]: string } = environment,
    time?: { startedNs: string; endedNs: string },
  ) => {
    const folder = `tmp/${token}/lifecycle/${name}`;
    const stdout = save(`${folder}/stdout.raw`, bytes),
      stderr = save(`${folder}/stderr.raw`, "");
    const startedNs = time?.startedNs ?? (role === "build" ? "30" : "10");
    const endedNs = time?.endedNs ?? (role === "build" ? "50" : "20");
    const observation = managedOwnedSchema.parse({
      argv,
      stdout,
      stderr,
      label,
      cwd: root,
      attemptToken: token,
      environment: env,
      environmentSha256: digestStringRecord(env),
      pid,
      actualExit: 0,
      startedNs,
      endedNs,
      reaped: true,
      errors: [],
    });
    const lifecycle = json(`${folder}/lifecycle.json`, {
      schema: "krr-performance-command-lifecycle-v1",
      argv,
      role,
      cwd: root,
      identity: {
        pid,
        parentPid: 99,
        sessionId: pid,
        processGroupId: pid,
        spawnObservedMonotonic: 1,
        observationLimit: "trusted dedicated session/group; non-atomic PID observations",
      },
      runLimitSeconds: { build: 1800, metadata: 120, "tool-query": 30 }[role],
      cleanupLimitSeconds: 10,
      elapsedSeconds: 0.1,
      groupClosed: true,
      rawFdsClosed: true,
      descendantClosureProved: false,
      nativeRawFdClosure: {
        schema: "krr-native-raw-fd-closure-v1",
        toolBefore: tool("lsof"),
        toolAfter: tool("lsof"),
        argv: [tool("lsof").path, "-nP", "-FpgfatiD", "--", stdout.path, stderr.path],
        cwd: root,
        attemptToken: token,
        environment: env,
        environmentSha256: digestStringRecord(env),
        identity: {
          pid: pid + 10000,
          parentPid: 99,
          sessionId: pid + 10000,
          processGroupId: pid + 10000,
        },
        actualExit: 1,
        startedNs: String(BigInt(startedNs) + 1n),
        endedNs: String(BigInt(startedNs) + 2n),
        reaped: true,
        groupClosed: true,
        rawFdsClosed: true,
        runLimitSeconds: 1,
        cleanupLimitSeconds: 10,
        stdout: save(`${folder}/native-fd-stdout.raw`, ""),
        stderr: save(`${folder}/native-fd-stderr.raw`, ""),
        rawIdentities: [stdout, stderr].map((pin) => {
          const stat = fs.statSync(pin.path);
          return { device: stat.dev, inode: stat.ino };
        }),
        holders: [],
        descendantClosureProved: false,
      },
      exitCode: 0,
      failure: null,
      cleanupFailure: null,
      exitedLeaderSignalObservations: [],
      raw: [stdout, stderr].map(({ path, sha256 }) => ({ path, sha256 })),
    });
    return { observation, lifecycle };
  };
  return {
    root,
    token,
    save,
    json,
    source,
    sourcePin,
    expectedSource,
    tool,
    registry,
    policy,
    capacity,
    environment,
    absentConfigs,
    snapshot,
    owned,
    manifest,
    main,
    runtime,
    control,
  };
}

export function duplicateManagedFixtureJson(bytes: string, key: string, escaped = false): string {
  const value = JSON.parse(bytes) as { [key: string]: unknown };
  const spelling = escaped
    ? `"\\u${key.charCodeAt(0).toString(16).padStart(4, "0")}${key.slice(1)}"`
    : JSON.stringify(key);
  return `{${spelling}:${JSON.stringify(value[key])},${bytes.slice(1)}`;
}
