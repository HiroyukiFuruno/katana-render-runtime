import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { verifyNativeClassicCapture } from "../mermaid/native-classic-capture";
import {
  program,
  syncManifest,
  withFixture,
  writeFile,
} from "../mermaid/native-classic-capture-test-fixture";
import {
  classicCapturedConsumer,
  readUnusedClassicCapture,
  readUnusedClassicPrograms,
  unusedClassicInputs,
} from "./runtime-unused-classic-capture";

test.each([false, true])(
  "physical classic reader preserves normal/Zen order and raw request: %s",
  (zen) => {
    withFixture((f) => {
      const envelope = verifyNativeClassicCapture(f.envelope, f.artifactDir, f.source);
      const saved = readUnusedClassicPrograms(envelope, f.artifactDir, f.source);
      expect(saved.programs.map((item) => item.name)).toEqual(
        envelope.capture.programs.map((item) => item.name),
      );
      expect(Buffer.from(saved.request.code)).toEqual(
        fs.readFileSync(envelope.capture.request.path),
      );
      for (const item of saved.programs) expect(item.source.sha256).toBe(item.file.sha256);
      expect(saved.programs[0]?.source.code).toContain("😀");
      const last = saved.programs.at(-1);
      if (!last) throw new Error("physical unit final program missing");
      expect(classicCapturedConsumer(last)).toMatch(/^[a-f0-9]{64}$/);
    }, zen);
  },
);

test("explicit classic input absence is not-provided and never selects an implicit path", () => {
  expect(unusedClassicInputs([])).toEqual({});
  expect(unusedClassicInputs(["--mermaid-js", "ignored.js"])).toEqual({});
  const input = {
    bindingsPath: "/attempt/render-bindings.json",
    fixturePath: "/fixtures/flow.md",
    slug: "flow",
    fenceLanguage: "mermaid" as const,
  };
  expect(unusedClassicInputs(["--classic-normal-input", JSON.stringify(input)])).toEqual({
    normal: input,
  });
  expect(
    unusedClassicInputs([
      "--classic-zenuml-input",
      JSON.stringify({ ...input, fenceLanguage: "zenuml" }),
    ]).zenuml?.fenceLanguage,
  ).toBe("zenuml");
});

test.each([
  ["missing value", ["--classic-normal-input"]],
  ["invalid JSON", ["--classic-normal-input", "{"]],
  ["wrong schema", ["--classic-normal-input", "{}"]],
  [
    "unknown field",
    [
      "--classic-normal-input",
      JSON.stringify({
        bindingsPath: "/a",
        fixturePath: "/b",
        slug: "c",
        fenceLanguage: "mermaid",
        extra: true,
      }),
    ],
  ],
  ["duplicate flag", ["--classic-normal-input", "{}", "--classic-normal-input", "{}"]],
] as const)("specified classic input rejects %s", (_name, argv) => {
  expect(() => unusedClassicInputs(argv)).toThrow();
});

test.each(["normal", "zenuml"] as const)(
  "valid %s input reaches duplicate flag rejection",
  (mode) => {
    const flag = `--classic-${mode}-input`;
    const value = JSON.stringify({
      bindingsPath: "/attempt/render-bindings.json",
      fixturePath: "/fixtures/flow.md",
      slug: "flow",
      fenceLanguage: mode === "normal" ? "mermaid" : "zenuml",
    });
    expect(() => unusedClassicInputs([flag, value, flag, value])).toThrow(
      `duplicate classic input: ${mode}`,
    );
  },
);

test.each(["missing", "relative", "symlink", "fixture-mode", "outer-receipt"])(
  "explicit native provenance fails closed for %s",
  (kind) => {
    withFixture((f) => {
      const fixturePath = path.join(f.root, "flow.md");
      const bindingsPath = path.join(f.root, "render-bindings.json");
      writeFile(fixturePath, "```mermaid\nflowchart LR\nA --> B\n```\n");
      writeFile(bindingsPath, "{}");
      const input = { bindingsPath, fixturePath, slug: "flow", fenceLanguage: "mermaid" as const };
      if (kind === "missing") input.bindingsPath = path.join(f.root, "absent.json");
      if (kind === "relative") input.bindingsPath = "render-bindings.json";
      if (kind === "symlink") {
        const link = path.join(f.root, "link.json");
        fs.symlinkSync(bindingsPath, link);
        input.bindingsPath = link;
      }
      expect(() =>
        readUnusedClassicCapture(input, kind === "fixture-mode" ? "zenuml" : "normal"),
      ).toThrow();
    });
  },
);

test.each(["call missing", "extra statement", "callee alias", "spread", "array request", "shadow"])(
  "SWC real call structure rejects %s",
  (kind) => {
    withFixture((f) => {
      const source = {
        "call missing": "void 0;",
        "extra statement": "katanaRunMermaidRuntime({}); void 0;",
        "callee alias": "other({});",
        spread: "katanaRunMermaidRuntime(...[]);",
        "array request": "katanaRunMermaidRuntime([]);",
        shadow: "let katanaRunMermaidRuntime; katanaRunMermaidRuntime({});",
      }[kind];
      if (!source) throw new Error("consumer unit mutation missing");
      const last = program(f, 3);
      const file = writeFile(last.file.path, source);
      expect(() =>
        classicCapturedConsumer({ ...last, file, source: { code: source, sha256: file.sha256 } }),
      ).toThrow();
    });
  },
);

test("physical reader detects same-sized script drift with refreshed manifest", () => {
  withFixture((f) => {
    const envelope = verifyNativeClassicCapture(f.envelope, f.artifactDir, f.source);
    const first = program(f, 0);
    writeFile(first.file.path, fs.readFileSync(first.file.path, "utf8").replace("unit0", "unit1"));
    syncManifest(f);
    expect(() => readUnusedClassicPrograms(envelope, f.artifactDir, f.source)).toThrow();
  });
});
