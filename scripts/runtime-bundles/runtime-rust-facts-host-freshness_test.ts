import { afterAll, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { rustFactsHostInputDifferences, verifyRustFactsHost } from "./runtime-rust-facts-host";
import { fixture as hostFixture } from "./runtime-rust-facts-host_test";
import { currentRustFactsHostInputs, hostFileRecord } from "./runtime-rust-facts-host-inputs";
import {
  hostEnvironmentDiagnostics,
  hostEnvironmentDigest,
} from "./runtime-rust-facts-host-protocol";

const fixtureParents = new Set<string>();
afterAll(() => {
  for (const directory of fixtureParents) fs.rmSync(directory, { recursive: true, force: true });
});
function fixture(nested = false) {
  const value = hostFixture(nested);
  fixtureParents.add(nested ? path.dirname(path.dirname(value.root)) : path.dirname(value.root));
  return value;
}

function repin(f: ReturnType<typeof fixture>) {
  f.value.inputs = currentRustFactsHostInputs(f.root, f.metadata, f.value.inputs.tools);
  fs.writeFileSync(f.receipt, JSON.stringify(f.value));
}

function input(f: ReturnType<typeof fixture>, relative: string, value: string) {
  const file = path.join(f.root, relative);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, value);
  return file;
}

test.each(["tmp", "target", "__pycache__", ".git"])(
  "compiled package input under src/%s remains in freshness closure",
  (name) => {
    const f = fixture();
    const file = input(f, `dependency/src/${name}/part.rs`, "pub const VALUE: u8 = 1;\n");
    input(f, "dependency/src/lib.rs", `#[path = "${name}/part.rs"] mod part;\n`);
    repin(f);
    expect(f.value.inputs.files).toContainEqual(hostFileRecord(file));
    expect(() => verifyRustFactsHost(f.root, f.receipt)).not.toThrow();
    fs.writeFileSync(file, "pub const VALUE: u8 = 2;\n");
    expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("stale");
  },
);

test.each(["tmp", "target", "__pycache__", ".git"])(
  "new compiled package input under src/%s invalidates prepared host",
  (name) => {
    const f = fixture();
    input(f, `dependency/src/${name}/part.rs`, "pub const VALUE: u8 = 1;\n");
    expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("stale");
  },
);

test.each([
  ["rust-toolchain", 1],
  ["rust-toolchain.toml", 1],
  ["rust-toolchain", 2],
  ["rust-toolchain.toml", 2],
] as const)(
  "ancestor %s at depth %s create, change and removal are distinct input changes",
  (name, depth) => {
    const f = fixture(true);
    const ancestor = depth === 1 ? path.dirname(f.root) : path.dirname(path.dirname(f.root));
    const file = path.join(ancestor, name);
    fs.writeFileSync(
      file,
      name.endsWith("toml") ? '[toolchain]\nchannel = "stable"\n' : "stable\n",
    );
    try {
      expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("stale");
      repin(f);
      expect(f.value.inputs.files).toContainEqual(hostFileRecord(file));
      fs.appendFileSync(file, "\n# changed\n");
      expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("stale");
      repin(f);
      fs.unlinkSync(file);
      expect(() => verifyRustFactsHost(f.root, f.receipt)).toThrow("stale");
    } finally {
      if (fs.existsSync(file)) fs.unlinkSync(file);
    }
  },
);

function toolProxy(rustup: string, name: "cargo" | "rustc", kind: string) {
  const file = path.join(path.dirname(rustup), name);
  if (kind === "symlink") fs.symlinkSync(rustup, file);
  else if (kind === "hardlink") fs.linkSync(rustup, file);
  else if (kind === "copy") fs.copyFileSync(rustup, file);
  else fs.writeFileSync(file, `direct system ${name}`, { mode: 0o755 });
}

test.each(["symlink", "hardlink", "copy", "system"])(
  "prepare resolves %s tools in repository cwd",
  (kind) => {
    const f = fixture();
    input(f, "package.json", '{"type":"commonjs"}\n');
    const caller = input(f, "caller/location.txt", "caller");
    const tools = path.join(f.root, "proxy-bin");
    fs.mkdirSync(tools);
    const rustup = path.join(tools, "rustup");
    fs.writeFileSync(
      rustup,
      '#!/usr/bin/env node\nconst path = require("node:path");\nrequire("node:fs").appendFileSync(path.join(process.cwd(), "which-calls.txt"), process.argv[3] + "\\n");\nprocess.stdout.write(path.join(process.cwd(), "toolchain", process.argv[3]) + "\\n");\n',
    );
    fs.chmodSync(rustup, 0o755);
    for (const name of ["cargo", "rustc"] as const) {
      toolProxy(rustup, name, kind);
      input(f, `toolchain/${name}`, `selected root tool ${name}`);
      input(f, `caller/toolchain/${name}`, `wrong caller tool ${name}`);
    }
    const previousPath = process.env.PATH,
      previousCwd = process.cwd();
    try {
      process.env.PATH = `${tools}${path.delimiter}${previousPath ?? ""}`;
      process.chdir(path.dirname(caller));
      const prepared = currentRustFactsHostInputs(f.root, f.metadata);
      for (const name of ["cargo", "rustc"] as const)
        expect(prepared.tools[name].executable.path).toBe(
          kind === "system" ? path.join(tools, name) : path.join(f.root, `toolchain/${name}`),
        );
      const calls = path.join(f.root, "which-calls.txt");
      const before = fs.existsSync(calls) ? fs.readFileSync(calls, "utf8") : "";
      if (kind !== "symlink") fs.unlinkSync(rustup);
      const verified = currentRustFactsHostInputs(f.root, f.metadata, prepared.tools);
      expect(verified.tools.cargo.executable).toEqual(prepared.tools.cargo.executable);
      expect(fs.existsSync(calls) ? fs.readFileSync(calls, "utf8") : "").toBe(before);
    } finally {
      process.chdir(previousCwd);
      if (previousPath === undefined) delete process.env.PATH;
      else process.env.PATH = previousPath;
    }
  },
);

const selectedRtk = Bun.which("rtk");
test.each(["", ...(selectedRtk ? [selectedRtk] : [])])(
  "Just CARGO default and explicit route agree for RTK '%s'",
  (rtk) => {
    const previousCargo = process.env.CARGO,
      previousRtk = process.env.RTK;
    try {
      process.env.RTK = rtk;
      delete process.env.CARGO;
      const implicit = hostEnvironmentDigest(path.dirname(import.meta.dir));
      process.env.CARGO = rtk === "" ? "cargo" : `${rtk} cargo`;
      expect(hostEnvironmentDigest(path.dirname(import.meta.dir))).toBe(implicit);
      process.env.CARGO = "cargo --config other.toml";
      expect(() => hostEnvironmentDigest(path.dirname(import.meta.dir))).toThrow("override");
    } finally {
      if (previousCargo === undefined) delete process.env.CARGO;
      else process.env.CARGO = previousCargo;
      if (previousRtk === undefined) delete process.env.RTK;
      else process.env.RTK = previousRtk;
    }
  },
);

test("host environment diagnostics expose selected names and value digests only", () => {
  const root = path.resolve(import.meta.dir, "../..");
  const environment = {
    RTK: "",
    PATH: "/diagnostic/path",
    HOME: "/diagnostic/home",
    CARGO_PRIVATE_VALUE: "private-marker",
    UNSELECTED_PRIVATE_VALUE: "must-not-appear",
  };
  const diagnostic = hostEnvironmentDiagnostics(root, environment);
  expect(diagnostic.environmentSha256).toBe(hostEnvironmentDigest(root, environment));
  expect(diagnostic.selected.map(({ name }) => name)).toEqual([
    "CARGO",
    "CARGO_PRIVATE_VALUE",
    "CARGO_TARGET_DIR",
    "HOME",
    "PATH",
    "RTK",
    "RUSTFLAGS",
  ]);
  const serialized = JSON.stringify(diagnostic);
  expect(serialized).not.toContain("/diagnostic/path");
  expect(serialized).not.toContain("/diagnostic/home");
  expect(serialized).not.toContain("private-marker");
  expect(serialized).not.toContain("must-not-appear");
  expect(
    diagnostic.selected.every(
      ({ valueSha256 }) => valueSha256 === null || /^[a-f0-9]{64}$/u.test(valueSha256),
    ),
  ).toBe(true);
});

test("missing repository root fails before any tool selection", () => {
  const f = fixture();
  expect(() =>
    currentRustFactsHostInputs(path.join(f.root, "missing"), f.metadata, f.value.inputs.tools),
  ).toThrow();
});

test("stale input diagnostics expose field names and digests only", () => {
  const f = fixture();
  const expected = f.value.inputs;
  const actual = structuredClone(expected);
  actual.environmentSha256 = "a".repeat(64);
  actual.tools.cargo.executable.sha256 = "b".repeat(64);
  const firstFile = actual.files[0];
  if (!firstFile) throw new Error("host fixture has no file inputs");
  firstFile.sha256 = "c".repeat(64);

  const differences = rustFactsHostInputDifferences(expected, actual);
  expect(differences.map(({ field }) => field)).toEqual([
    "environmentSha256",
    "tools.cargo.executable",
    "files[000000]",
  ]);
  expect(
    differences.every(
      ({ expectedSha256, actualSha256 }) =>
        (expectedSha256 === null || /^[a-f0-9]{64}$/u.test(expectedSha256)) &&
        (actualSha256 === null || /^[a-f0-9]{64}$/u.test(actualSha256)),
    ),
  ).toBe(true);
  expect(JSON.stringify(differences)).not.toContain(firstFile.path);
});
