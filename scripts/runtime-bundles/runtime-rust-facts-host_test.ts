import { afterAll, expect, spyOn, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  defaultRustFactsHostReceipt,
  prepareRustFactsHost,
  type RustFactsHostReceipt,
  runRustFactsHost,
  runRustFactsHostPair,
  rustFactsHostFingerprint,
  rustFactsHostReceiptSchema,
  verifyRustFactsHost,
} from "./runtime-rust-facts-host";
import {
  currentRustFactsHostInputs,
  hostFileRecord,
  parseRustFactsHostMetadata,
} from "./runtime-rust-facts-host-inputs";
import {
  createRustFactsHostAttemptDirectory,
  hostBuildOutputRecord,
  hostEnvironmentDigest,
  publishRustFactsHostBinary,
  rustFactsHostArtifactName,
} from "./runtime-rust-facts-host-protocol";
import { hostFixtureInputs } from "./runtime-rust-facts-host-test-fixture";

const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "rust-facts-host-unit-")));
afterAll(() => fs.rmSync(directory, { recursive: true, force: true }));
const authorities = [
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
  "scripts/build/cargo_capacity_guard.py",
  "scripts/release/toml_loader.py",
  "crates/kdr-linter/Cargo.toml",
  "crates/kdr-linter/src/bin/mathjax_consumer_facts.rs",
  "dependency/Cargo.toml",
  "dependency/src/lib.rs",
];

function write(root: string, name: string, text: string): string {
  const file = path.join(root, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  return file;
}

export function fixture(nested = false) {
  fs.mkdirSync(directory, { recursive: true });
  const parent = fs.mkdtempSync(path.join(directory, "source-"));
  const root = nested ? path.join(parent, "child/repository") : parent;
  fs.mkdirSync(root, { recursive: true });
  for (const name of authorities) write(root, name, `fixture source: ${name}`);
  write(
    root,
    "Justfile",
    [
      "rust-facts-host-build root=REPO_ROOT:",
      '    @CARGO_TARGET_DIR={{quote(root + "/target")}} {{CARGO_BUILD}} build --locked -p kdr-linter --bin mathjax_consumer_facts',
      "rust-facts-host-metadata root=REPO_ROOT:",
      '    @CARGO_TARGET_DIR={{quote(root + "/target")}} {{CARGO}} metadata --locked --format-version 1',
    ].join("\n"),
  );
  const { metadata, tools } = hostFixtureInputs(root);
  const inputs = currentRustFactsHostInputs(root, metadata, tools);
  const proofDirectory = "tmp/quality/rust-facts-host/current/unit-proof";
  const processProof = (label: string) => {
    const value = {
      argv: [tools.just.executable.path, label, root],
      cwd: root,
      exitCode: 0,
      signal: null,
      error: null,
      stdout: hostFileRecord(
        write(root, `${proofDirectory}/${label}.stdout`, JSON.stringify(metadata)),
      ),
      stderr: hostFileRecord(write(root, `${proofDirectory}/${label}.stderr`, "")),
    };
    write(root, `${proofDirectory}/${label}.process.json`, JSON.stringify(value));
    return value;
  };
  const buildOutput = hostBuildOutputRecord(
    write(
      root,
      `target/debug/mathjax_consumer_facts${process.platform === "win32" ? ".exe" : ""}`,
      "unit fixture; never executed",
    ),
  );
  const receipt = defaultRustFactsHostReceipt(root);
  const attempt = "00000000-0000-4000-8000-000000000001";
  createRustFactsHostAttemptDirectory(root, receipt, attempt);
  const binary = publishRustFactsHostBinary(root, receipt, attempt, buildOutput);
  const value: RustFactsHostReceipt = {
    schema: "krr-rust-facts-host/v2",
    status: "prepared",
    root,
    attempt: "00000000-0000-4000-8000-000000000001",
    inputs,
    buildOutput,
    binary,
    metadataBefore: processProof("rust-facts-host-metadata"),
    build: processProof("rust-facts-host-build"),
    metadataAfter: processProof("rust-facts-host-metadata"),
  };
  fs.writeFileSync(receipt, JSON.stringify(value));
  return { root, receipt, value, metadata };
}

function publishFixtureExecutable(f: ReturnType<typeof fixture>, executable: string) {
  const target = f.value.buildOutput.path;
  fs.writeFileSync(target, executable);
  fs.chmodSync(target, 0o755);
  const buildOutput = hostBuildOutputRecord(target);
  const binary = publishRustFactsHostBinary(f.root, f.receipt, f.value.attempt, buildOutput);
  f.value.buildOutput = buildOutput;
  f.value.binary = binary;
  fs.writeFileSync(f.receipt, JSON.stringify(f.value));
  return { target, binary };
}

test("physical unit receipt validation does not claim an actual host build or execution", () => {
  const f = fixture();
  expect(verifyRustFactsHost(f.root, f.receipt)).toEqual(f.value);
  expect(defaultRustFactsHostReceipt(f.root)).toBe(f.receipt);
  expect(f.value.inputs.files.some((file) => file.path.endsWith("dependency/src/lib.rs"))).toBe(
    true,
  );
});

test("host fingerprint is stable and binds verified inputs and binary bytes", () => {
  const f = fixture();
  const first = rustFactsHostFingerprint(f.root, f.receipt);
  expect(first).toMatch(/^[a-f0-9]{64}$/);
  expect(rustFactsHostFingerprint(f.root, f.receipt)).toBe(first);
  const changed = structuredClone(f.value);
  changed.inputs.environmentSha256 = "0".repeat(64);
  fs.writeFileSync(f.receipt, JSON.stringify(changed));
  expect(() => rustFactsHostFingerprint(f.root, f.receipt)).toThrow();
});

test("host fingerprint CLI emits the stable schema", () => {
  const f = fixture();
  const child = Bun.spawnSync(
    [
      process.execPath,
      "scripts/runtime-bundles/runtime-rust-facts-host.ts",
      "fingerprint",
      f.root,
      f.receipt,
    ],
    {
      cwd: path.resolve(import.meta.dir, "../.."),
      stdout: "pipe",
      stderr: "pipe",
    },
  );
  expect(child.exitCode).toBe(0);
  expect(JSON.parse(Buffer.from(child.stdout).toString("utf8"))).toMatchObject({
    schema: "krr-rust-facts-host-fingerprint-v1",
    fingerprint: expect.stringMatching(/^[a-f0-9]{64}$/),
  });
});

test.each([
  ["owned source changed", "crates/kdr-linter/src/bin/mathjax_consumer_facts.rs"],
  ["transitive source changed", "dependency/src/lib.rs"],
  ["capacity guard changed", "scripts/build/cargo_capacity_guard.py"],
  ["guard imported source changed", "scripts/release/toml_loader.py"],
  ["lock changed", "Cargo.lock"],
  ["validator changed", "scripts/runtime-bundles/runtime-rust-facts-host-inputs.ts"],
])("prepared host rejects %s", (_name, source) => {
  const f = fixture();
  fs.appendFileSync(path.join(f.root, source), "\nchanged");
  expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("stale");
});

test.each(["dependency/src/added.rs", "crates/kdr-linter/src/added.rs", ".cargo/config.toml"])(
  "prepared host rejects a new input %s",
  (source) => {
    const f = fixture();
    write(f.root, source, "new input");
    expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("stale");
  },
);

test.each([
  [
    "failed build",
    (value: RustFactsHostReceipt) => {
      value.build.exitCode = 1;
    },
  ],
  [
    "signalled build",
    (value: RustFactsHostReceipt) => {
      value.build.signal = "SIGKILL";
    },
  ],
  [
    "spawn error",
    (value: RustFactsHostReceipt) => {
      value.build.error = "spawn failed";
    },
  ],
  [
    "wrong command",
    (value: RustFactsHostReceipt) => {
      value.build.argv.push("--release");
    },
  ],
  [
    "wrong cwd",
    (value: RustFactsHostReceipt) => {
      value.build.cwd = directory;
    },
  ],
  [
    "wrong binary",
    (value: RustFactsHostReceipt) => {
      value.binary.path = value.build.stdout.path;
    },
  ],
  [
    "legacy receipt schema",
    (value: RustFactsHostReceipt) => {
      Reflect.set(value, "schema", "krr-rust-facts-host/v1");
    },
  ],
  [
    "wrong build output",
    (value: RustFactsHostReceipt) => {
      value.buildOutput.path = value.build.stdout.path;
    },
  ],
  [
    "artifact mode",
    (value: RustFactsHostReceipt) => {
      value.binary.mode ^= 0o111;
    },
  ],
  [
    "artifact hash",
    (value: RustFactsHostReceipt) => {
      value.binary.sha256 = "0".repeat(64);
    },
  ],
  [
    "tool SHA",
    (value: RustFactsHostReceipt) => {
      value.inputs.tools.cargo.executable.sha256 = "0".repeat(64);
    },
  ],
  [
    "environment SHA",
    (value: RustFactsHostReceipt) => {
      value.inputs.environmentSha256 = "0".repeat(64);
    },
  ],
  [
    "replayed input set",
    (value: RustFactsHostReceipt) => {
      value.inputs.files.pop();
    },
  ],
])("prepared host rejects %s", (_name, mutate) => {
  const f = fixture();
  mutate(f.value);
  fs.writeFileSync(f.receipt, JSON.stringify(f.value));
  expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow();
});

test.each(["metadata", "binary"])("prepared host rejects changed physical %s", (kind) => {
  const f = fixture();
  fs.appendFileSync(
    kind === "binary" ? f.value.binary.path : f.value.metadataBefore.stdout.path,
    "changed",
  );
  expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("physical proof changed");
});

test("prepared host rejects a changed executable mode on the immutable copy", () => {
  const f = fixture();
  fs.chmodSync(f.value.binary.path, f.value.binary.mode ^ 0o111);
  expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("mode changed");
});

test("receipt cannot rebind a changed copy mode while retaining producer mode", () => {
  if (process.platform === "win32") return;
  const f = fixture();
  const changedMode = f.value.binary.mode ^ 0o100;
  fs.chmodSync(f.value.binary.path, changedMode);
  f.value.binary.mode = changedMode;
  fs.writeFileSync(f.receipt, JSON.stringify(f.value));
  expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("root/binary differs");
});

test("run-only rejects missing receipt and cannot fall back to compilation", () => {
  const f = fixture();
  const missing = `${f.receipt}.missing`;
  const attemptsBefore = fs
    .readdirSync(path.dirname(f.receipt))
    .filter((name) => name.startsWith("attempt-")).length;
  expect(() => runRustFactsHost(f.root, "classic", f.root, missing)).toThrow();
  expect(
    fs.readdirSync(path.dirname(f.receipt)).filter((name) => name.startsWith("attempt-")),
  ).toHaveLength(attemptsBefore);
});

test("preparation rejects a changed build recipe before any process starts", () => {
  const f = fixture();
  fs.appendFileSync(path.join(f.root, "Justfile"), "\n    cargo build\n");
  expect(() => prepareRustFactsHost(f.root, f.receipt)).toThrow("recipe changed");
});

test("strict receipt and physical readers reject unknown fields, symlinks, and invalid metadata", () => {
  const f = fixture();
  expect(() => rustFactsHostReceiptSchema.parse({ ...f.value, accepted: true })).toThrow();
  const link = path.join(f.root, "linked.rs");
  fs.symlinkSync(f.value.binary.path, link);
  expect(() => hostFileRecord(link)).toThrow("canonical regular file");
  expect(() => parseRustFactsHostMetadata(Buffer.from([0xff]))).toThrow();
  expect(() => parseRustFactsHostMetadata(Buffer.from("{}"))).toThrow();
  expect(() =>
    currentRustFactsHostInputs(
      f.root,
      { ...f.metadata, resolve: { nodes: [] } },
      f.value.inputs.tools,
    ),
  ).toThrow("incomplete");
});

test("content-addressed host artifact name preserves the Windows executable suffix", () => {
  expect(rustFactsHostArtifactName("a".repeat(64), "win32")).toBe(
    `mathjax_consumer_facts-${"a".repeat(64)}.exe`,
  );
  expect(rustFactsHostArtifactName("a".repeat(64), "darwin").endsWith(".exe")).toBe(false);
});

test("receipt rejects a symlink at its content-addressed artifact path", () => {
  const f = fixture();
  const moved = `${f.value.binary.path}.saved`;
  fs.renameSync(f.value.binary.path, moved);
  fs.symlinkSync(moved, f.value.binary.path);
  expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow();
});

test("artifact capture rejects a changed target output after its identity was recorded", () => {
  const f = fixture();
  const artifactDirectory = path.dirname(f.value.binary.path);
  const entriesBefore = fs.readdirSync(artifactDirectory);
  fs.appendFileSync(f.value.buildOutput.path, "changed after build output snapshot");
  expect(() =>
    publishRustFactsHostBinary(f.root, f.receipt, f.value.attempt, f.value.buildOutput),
  ).toThrow("changed during capture");
  expect(fs.readdirSync(artifactDirectory)).toEqual(entriesBefore);
});

test("artifact publication never overwrites an existing content-addressed copy", () => {
  const f = fixture();
  const before = fs.readFileSync(f.value.binary.path);
  const entriesBefore = fs.readdirSync(path.dirname(f.value.binary.path));
  expect(() =>
    publishRustFactsHostBinary(f.root, f.receipt, f.value.attempt, f.value.buildOutput),
  ).toThrow("artifact already exists");
  expect(fs.readFileSync(f.value.binary.path)).toEqual(before);
  expect(fs.readdirSync(path.dirname(f.value.binary.path))).toEqual(entriesBefore);
});

test("exclusive copy preserves a late-created content-addressed destination", () => {
  const f = fixture();
  const target = f.value.buildOutput.path;
  fs.appendFileSync(target, "distinct artifact digest");
  const buildOutput = hostBuildOutputRecord(target);
  const expected = path.join(
    path.dirname(f.receipt),
    `attempt-${f.value.attempt}`,
    `mathjax_consumer_facts-${buildOutput.sha256}${process.platform === "win32" ? ".exe" : ""}`,
  );
  const sentinel = Buffer.from("preexisting destination");
  const originalCopy = fs.copyFileSync;
  const copySpy = spyOn(fs, "copyFileSync").mockImplementation((source, destination, flags) => {
    fs.writeFileSync(destination, sentinel, { flag: "wx" });
    return originalCopy(source, destination, flags);
  });
  try {
    expect(() =>
      publishRustFactsHostBinary(f.root, f.receipt, f.value.attempt, buildOutput),
    ).toThrow();
    expect(expected).toBeTruthy();
  } finally {
    copySpy.mockRestore();
  }
  expect(fs.readFileSync(expected)).toEqual(sentinel);
});

test("host command runs the verified immutable copy after the target output changes", () => {
  const f = fixture();
  const executable = "#!/bin/sh\nprintf 'immutable host ran\\n'\n";
  const { target, binary } = publishFixtureExecutable(f, executable);
  fs.writeFileSync(target, "later Cargo output replaced the mutable target\n");

  const result = runRustFactsHost(f.root, "mathjax", f.root, f.receipt);
  expect(result.proof.exitCode).toBe(0);
  expect(result.stdout).toEqual(Buffer.from("immutable host ran\n"));
  expect(result.proof.argv[0]).toBe(binary.path);
  expect(fs.readFileSync(binary.path, "utf8")).toBe(executable);
});

test("Mach-O facts mode passes a canonical manifest file to the verified host", () => {
  const f = fixture();
  const executable = '#!/bin/sh\nprintf "%s\\n" "$@"\n';
  const { binary } = publishFixtureExecutable(f, executable);
  const manifest = path.join(f.root, "macho-input.json");
  fs.writeFileSync(manifest, '{"schema":"krr-macho-noso-input/v1","files":[]}\n');

  const result = runRustFactsHost(f.root, "macho-noso", manifest, f.receipt);
  expect(result.proof.argv).toEqual([binary.path, "--macho-noso", manifest]);
  expect(result.stdout.toString("utf8")).toContain("--macho-noso\n");
  expect(result.proof.exitCode).toBe(0);
});

test("paired host command runs native-width then classic with separate physical proofs", () => {
  const f = fixture();
  const executable = `#!/bin/sh
if [ "$1" = "--native-width-hash" ]; then printf '{"mode":"native-width"}\\n'; else printf '{"mode":"classic"}\\n'; fi
`;
  const { binary } = publishFixtureExecutable(f, executable);

  const result = runRustFactsHostPair(f.root, f.root, f.receipt);
  expect(JSON.parse(result.nativeWidth.stdout.toString("utf8"))).toEqual({ mode: "native-width" });
  expect(JSON.parse(result.classic.stdout.toString("utf8"))).toEqual({ mode: "classic" });
  expect(result.nativeWidth.proof.argv).toEqual([binary.path, "--native-width-hash", f.root]);
  expect(result.classic.proof.argv).toEqual([binary.path, "--mermaid-classic-scripts", f.root]);
  expect(result.nativeWidth.proof.stdout.path).not.toBe(result.classic.proof.stdout.path);
  expect(result.nativeWidth.proof.stderr.path).not.toBe(result.classic.proof.stderr.path);
  expect(result.nativeWidth.proof.exitCode).toBe(0);
  expect(result.classic.proof.exitCode).toBe(0);
});

test("paired host command preserves a failed child's exit, stderr, and error JSON", () => {
  const f = fixture();
  const executable = `#!/bin/sh
if [ "$1" = "--native-width-hash" ]; then printf '{"error":"native-width rejected"}\\n'; printf 'native-width stderr\\n' >&2; exit 7; fi
printf '{"mode":"classic"}\\n'
`;

  publishFixtureExecutable(f, executable);
  const result = runRustFactsHostPair(f.root, f.root, f.receipt);
  expect(result.nativeWidth.proof.exitCode).toBe(7);
  expect(result.nativeWidth.stdout.toString("utf8")).toContain("native-width rejected");
  expect(result.nativeWidth.stderr.toString("utf8")).toContain("native-width stderr");
  expect(result.classic.proof.exitCode).toBe(0);
});

test.each(["empty", "invalid JSON"])(
  "pair CLI preserves stderr and child exit without a wrapper for %s native stdout",
  (outputKind) => {
    const f = fixture();
    const nativeOutput = outputKind === "empty" ? "" : "invalid-json";
    const executable = `#!/bin/sh
if [ "$1" = "--native-width-hash" ]; then printf '${nativeOutput}'; printf 'native-width stderr\\n' >&2; exit 7; fi
printf '{"mode":"classic"}\\n'
`;
    publishFixtureExecutable(f, executable);
    const child = Bun.spawnSync(
      [
        process.execPath,
        "scripts/runtime-bundles/runtime-rust-facts-host.ts",
        "run",
        f.root,
        "native-width-classic",
        f.root,
        f.receipt,
      ],
      { cwd: path.resolve(import.meta.dir, "../.."), stdout: "pipe", stderr: "pipe" },
    );
    expect(child.exitCode).toBe(7);
    expect(Buffer.from(child.stdout)).toEqual(Buffer.alloc(0));
    expect(Buffer.from(child.stderr).toString("utf8")).toContain("native-width stderr");
    if (outputKind === "invalid JSON")
      expect(Buffer.from(child.stderr).toString("utf8")).toContain("invalid-json");
  },
);

test("paired host command rejects a stale receipt before either child starts", () => {
  const f = fixture();
  const attemptsBefore = fs
    .readdirSync(path.dirname(f.receipt))
    .filter((name) => name.startsWith("attempt-")).length;
  fs.appendFileSync(path.join(f.root, "Cargo.lock"), "\nstale");
  expect(() => runRustFactsHostPair(f.root, f.root, f.receipt)).toThrow("stale");
  expect(
    fs.readdirSync(path.dirname(f.receipt)).filter((name) => name.startsWith("attempt-")),
  ).toHaveLength(attemptsBefore);
});

test("target environment preserves canonical default and rejects a different target", () => {
  const previous = process.env.CARGO_TARGET_DIR;
  try {
    delete process.env.CARGO_TARGET_DIR;
    const implicit = hostEnvironmentDigest(directory);
    process.env.CARGO_TARGET_DIR = path.join(directory, "target");
    expect(hostEnvironmentDigest(directory)).toBe(implicit);
    process.env.CARGO_TARGET_DIR = path.join(directory, "other-target");
    expect(() => hostEnvironmentDigest(directory)).toThrow("target override");
  } finally {
    if (previous === undefined) delete process.env.CARGO_TARGET_DIR;
    else process.env.CARGO_TARGET_DIR = previous;
  }
});

test("a successful summary cannot replace a failed physical process observation", () => {
  const f = fixture();
  const failed = { ...f.value.build, exitCode: 1 };
  const file = path.basename(failed.stdout.path, ".stdout");
  fs.writeFileSync(
    path.join(path.dirname(failed.stdout.path), `${file}.process.json`),
    JSON.stringify(failed),
  );
  expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("process observation differs");
});
