import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { SourceMapGenerator } from "source-map-js";
import { RuntimeBundleDefinition } from "#shared/runtime_bundle";
import { verifyNativeClassicCapture } from "../mermaid/native-classic-capture";
import { withFixture, writeFile } from "../mermaid/native-classic-capture-test-fixture";
import {
  bindClassicOwnedBytes,
  ClassicCaptureProgramOrigin,
  classicUnusedMetadata,
  readClassicUnusedBindings,
} from "./runtime-unused-classic-binding";
import {
  readUnusedClassicPrograms,
  type UnusedClassicProgram,
} from "./runtime-unused-classic-capture";
import type { UnusedDiagnostic } from "./runtime-unused-origin";

function diagnostic(program: UnusedClassicProgram, start: number, end: number): UnusedDiagnostic {
  return {
    category: "unit/unused",
    severity: "warning",
    message: "physical range",
    location: {
      path: program.file.path,
      start: { line: 1, column: start },
      end: { line: 1, column: end },
    },
  };
}

test("absence remains not-provided and does not close any consumer", () => {
  expect(readClassicUnusedBindings({})).toEqual([]);
  expect(classicUnusedMetadata("/repository", [], [], [])).toEqual([
    {
      mode: "normal",
      status: "not-provided",
      actualCaptureBytesBound: false,
      acceptedConsumerClosure: false,
      scopeComplete: false,
    },
    {
      mode: "zenuml",
      status: "not-provided",
      actualCaptureBytesBound: false,
      acceptedConsumerClosure: false,
      scopeComplete: false,
    },
  ]);
});

test.each(["const marker=1;", "/*😀𐐀é*/ const marker=1;\r\n", "\ufeffconst marker=1;"])(
  "per-program physical map retains Unicode/BOM/CRLF bytes: %s",
  (code) => {
    withFixture((f) => {
      const file = writeFile(path.join(f.root, "physical.js"), code);
      const program = {
        index: 0,
        name: "physical.js",
        file,
        source: { code, sha256: file.sha256 },
      };
      const origin = new ClassicCaptureProgramOrigin(program);
      const start = code.indexOf("marker");
      const column = Array.from(code.slice(0, start)).length + 1;
      expect(origin.resolve(diagnostic(program, column, column + 6))).toEqual({
        sourcePath: file.path,
        sourceSha256: file.sha256,
        start,
        end: start + 6,
      });
      const map = JSON.parse(origin.map) as { sources: string[]; sourcesContent: string[] };
      expect(map.sources).toEqual([file.path]);
      expect(map.sourcesContent).toEqual([code]);
    });
  },
);

test.each(["source", "content", "offset", "unmapped", "duplicate"])(
  "physical map rejects forged %s",
  (kind) => {
    withFixture((f) => {
      const code = "const marker=1;";
      const file = writeFile(path.join(f.root, "physical.js"), code);
      const program = {
        index: 0,
        name: "physical.js",
        file,
        source: { code, sha256: file.sha256 },
      };
      const origin = new ClassicCaptureProgramOrigin(program);
      const map = JSON.parse(origin.map) as {
        sources: string[];
        sourcesContent: string[];
        mappings: string;
      };
      if (kind === "source") map.sources = ["other.js"];
      if (kind === "content") map.sourcesContent = ["const otherx=1;"];
      if (kind === "unmapped") map.mappings = "";
      if (kind === "duplicate") map.sources.push(file.path);
      if (kind === "offset") {
        const generator = new SourceMapGenerator();
        generator.setSourceContent(file.path, code);
        generator.addMapping({
          generated: { line: 1, column: 0 },
          original: { line: 1, column: 1 },
          source: file.path,
        });
        map.mappings = JSON.parse(generator.toString()).mappings as string;
      }
      expect(() => {
        const changed = new ClassicCaptureProgramOrigin(program, JSON.stringify(map));
        changed.resolve(diagnostic(program, 7, 13));
      }).toThrow();
    });
  },
);

test("physical origin rejects cross-program, reversed, out-of-range and changed bytes", () => {
  withFixture((f) => {
    const code = "const marker=1;";
    const file = writeFile(path.join(f.root, "physical.js"), code);
    const program = { index: 0, name: "physical.js", file, source: { code, sha256: file.sha256 } };
    const origin = new ClassicCaptureProgramOrigin(program);
    const cross = diagnostic(program, 7, 13);
    cross.location.path = "other.js";
    for (const item of [cross, diagnostic(program, 13, 7), diagnostic(program, 7, 40)])
      expect(() => origin.resolve(item)).toThrow();
    writeFile(file.path, "const changed=1;");
    expect(() => origin.resolve(diagnostic(program, 7, 13))).toThrow();
  });
});

test.each([false, true])(
  "normal/Zen keeps scripts separate and vendor roles outside raw fragment maps: %s",
  (zen) => {
    withFixture((f) => {
      const envelope = verifyNativeClassicCapture(f.envelope, f.artifactDir, f.source);
      const saved = readUnusedClassicPrograms(envelope, f.artifactDir, f.source);
      const origins = saved.programs.map((program) => new ClassicCaptureProgramOrigin(program));
      expect(origins).toHaveLength(zen ? 5 : 4);
      for (const origin of origins) {
        expect((JSON.parse(origin.map) as { sources: string[] }).sources).toEqual([
          origin.program.file.path,
        ]);
        expect(origin.program.source.code).toBe(fs.readFileSync(origin.program.file.path, "utf8"));
      }
    }, zen);
  },
);

test.each([
  "valid",
  "different capture",
  "changed checked bytes",
  "different output",
  "module entry",
  "changed generated file",
])("owned generated bytes require canonical checked identity: %s", (kind) => {
  withFixture((f) => {
    const name = "mermaid-runtime.min.js";
    const directory = path.join(
      f.root,
      "crates/katana-render-runtime/src/markdown/diagram_runtime/generated",
    );
    fs.mkdirSync(directory, { recursive: true });
    const content = "var unit=1;";
    const generated = writeFile(path.join(directory, name), content);
    const file = writeFile(
      path.join(f.root, "captured.js"),
      kind === "different capture" ? "var unit=2;" : content,
    );
    const program = { index: 0, name, file, source: { code: content, sha256: file.sha256 } };
    const definition = new RuntimeBundleDefinition(
      "mermaid",
      name,
      [],
      "",
      kind === "module entry" ? "entry.ts" : null,
    );
    const bundle = {
      definition,
      outputPath: kind === "different output" ? file.path : generated.path,
      content: kind === "changed checked bytes" ? "var unit=3;" : content,
      checksum: generated.sha256,
    };
    if (kind === "changed generated file") writeFile(generated.path, "var unit=4;");
    if (kind === "valid")
      expect(bindClassicOwnedBytes(f.root, program, [bundle]).checksum).toBe(file.sha256);
    else expect(() => bindClassicOwnedBytes(f.root, program, [bundle])).toThrow();
  });
});
