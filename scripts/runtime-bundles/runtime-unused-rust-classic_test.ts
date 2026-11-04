import { afterAll, beforeAll, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { inspectRustClassicScriptFacts } from "./runtime-unused-rust-classic";

const repositoryRoot = fs.realpathSync(path.resolve(import.meta.dir, "../.."));
const temporaryRoot = fs.realpathSync(
  fs.mkdtempSync(path.join(os.tmpdir(), "rust-classic-adapter-managed-")),
);
let facts: ReturnType<typeof inspectRustClassicScriptFacts>;
// 実Rust facts変異の準備・拒否検証だけを既存の結合検査予算へ合わせる。純粋な境界assertionは5秒のままにする。
const RUST_FACTS_INTEGRATION_TIMEOUT_MS = 30_000;

beforeAll(() => {
  facts = inspectRustClassicScriptFacts(repositoryRoot);
});

afterAll(() => fs.rmSync(temporaryRoot, { recursive: true, force: true }));

function sha256(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function copiedSourceRoot(name: string): string {
  const sourceRoot = path.join(temporaryRoot, name);
  for (const [relativePath, expectedSha256] of Object.entries(facts.inputSha256)) {
    const source = path.join(repositoryRoot, relativePath);
    const target = path.join(sourceRoot, relativePath);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(source, target);
    expect(sha256(fs.readFileSync(target))).toBe(expectedSha256);
  }
  expect(Object.keys(facts.inputSha256)).toHaveLength(13);
  return fs.realpathSync(sourceRoot);
}

function replaceExactlyOnce(
  sourceRoot: string,
  relativePath: string,
  before: string,
  after: string,
) {
  const target = path.join(sourceRoot, relativePath);
  const source = fs.readFileSync(target, "utf8");
  const occurrences = source.split(before).length - 1;
  expect(occurrences).toBe(1);
  if (occurrences !== 1) throw new Error(`expected one mutation snippet in ${relativePath}`);
  fs.writeFileSync(target, source.replace(before, after));
}

function expectRustRejection(sourceRoot: string, diagnostic: string) {
  let failure: unknown;
  try {
    inspectRustClassicScriptFacts(sourceRoot);
  } catch (error) {
    failure = error;
  }
  expect(failure).toBeInstanceOf(Error);
  if (!(failure instanceof Error)) throw new Error("expected the public Rust adapter to reject");
  const marker = "Rust classic facts stdout: ";
  const markerIndex = failure.message.indexOf(marker);
  expect(markerIndex).toBeGreaterThanOrEqual(0);
  if (markerIndex < 0) throw new Error("Rust rejection report was not preserved by the adapter");
  const stdout = failure.message.slice(markerIndex + marker.length).split(/\r?\n/u, 1)[0];
  if (!stdout) throw new Error("Rust rejection report was empty");
  const report: unknown = JSON.parse(stdout);
  expect(report).toMatchObject({
    schema: "kdr-linter/mermaid-classic-script-facts/v1",
    status: "rejected",
    errors: [expect.stringContaining(diagnostic)],
  });
}

test("the public Rust adapter preserves the actual four- and five-script manifests", () => {
  expect(facts.schema).toBe("kdr-linter/mermaid-classic-script-facts/v1");
  expect(facts.status).toBe("verified");
  expect(facts.facts).toHaveLength(9);
  expect(Object.keys(facts.inputSha256)).toEqual(
    expect.arrayContaining([
      "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
      "crates/katana-render-runtime/src/markdown/runtime_resource_loader.rs",
      "crates/katana-render-runtime/src/renderer/backends/mermaid.rs",
      "crates/katana-render-runtime/src/markdown/mermaid_renderer/render_instance.rs",
      "crates/katana-render-runtime/src/resource_bytes.rs",
    ]),
  );
  expect(Object.keys(facts.inputSha256)).toHaveLength(13);
  expect(facts.manifest.normal.map((script) => script.name)).toEqual([
    "mermaid-runtime.min.js",
    "mermaid.min.js",
    "mermaid-zenuml.min.js",
    "render-mermaid.js",
  ]);
  expect(facts.manifest.zenuml.map((script) => script.name)).toEqual([
    "mermaid-runtime.min.js",
    "mermaid.min.js",
    "mermaid-zenuml.min.js",
    "zenuml-runtime.min.js",
    "render-mermaid.js",
  ]);
  expect(facts.acceptedConsumerClosure).toBe(false);
});

test.each([
  [
    "shared-realm loop order",
    "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
    "for script in scripts {",
    "for script in scripts.iter().rev() {",
    "mermaid-classic-realm:loop",
  ],
  [
    "source identity",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_scripts.rs",
    "../diagram_runtime/generated/mermaid-runtime.min.js",
    "../diagram_runtime/generated/zenuml-runtime.min.js",
    "mermaid-scripts:include-path",
  ],
  [
    "compile source",
    "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
    "ScriptCompiler::compile_with_origin(scope, source, origin_name)",
    "ScriptCompiler::compile_with_origin(scope, other_source, origin_name)",
    "v8-compile-order",
  ],
  [
    "run receiver",
    "crates/katana-render-runtime/src/markdown/diagram_js_runtime.rs",
    "script.run(scope)",
    "other_script.run(scope)",
    "v8-compile-order",
  ],
])(
  "the actual Rust producer rejects copied %s mutations",
  (_name, relativePath, before, after, diagnostic) => {
    const sourceRoot = copiedSourceRoot(String(_name).replaceAll(" ", "-"));
    replaceExactlyOnce(sourceRoot, String(relativePath), String(before), String(after));
    expectRustRejection(sourceRoot, String(diagnostic));
  },
  RUST_FACTS_INTEGRATION_TIMEOUT_MS,
);

test("the public adapter rejects a symlinked physical input before invoking Rust", () => {
  const sourceRoot = copiedSourceRoot("symlink-boundary");
  const relativePath = Object.keys(facts.inputSha256)[0];
  if (!relativePath) throw new Error("missing physical input fixture");
  const target = path.join(sourceRoot, relativePath);
  const outside = path.join(temporaryRoot, "symlink-target.rs");
  fs.copyFileSync(target, outside);
  fs.unlinkSync(target);
  fs.symlinkSync(outside, target);

  expect(() => inspectRustClassicScriptFacts(sourceRoot)).toThrow("canonical regular file");
});
