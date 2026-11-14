import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { JsonValue } from "./dynamic-state";
import { sha256File } from "./reference-capture";

type JsonObject = { [key: string]: JsonValue };

const destinations = [
  "ch1",
  "s11",
  "s12",
  "s13",
  "s14",
  "s15",
  "ch2",
  "s21",
  "s22",
  "s23",
  "s24",
  "s25",
  "s26",
  "s27",
  "s28",
  "changelog",
];

function fixture(): { root: string; binary: string; args: string[]; cleanup: () => void } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-toc-evidence-"));
  const source = path.join(root, "source.html");
  fs.writeFileSync(source, "<!doctype html><title>fixture</title>");
  const binary = process.execPath;
  const binarySha = sha256File(binary);
  for (const [width, height] of [
    [1280, 900],
    [960, 720],
    [620, 900],
  ] as const) {
    const krrDirectory = path.join(root, `krr-${width}`);
    const chromeDirectory = path.join(root, `chrome-${width}`);
    const png = path.join(root, `${width}x${height}.png`);
    const image = Bun.spawnSync(["magick", "-size", `${width}x${height}`, "xc:white", png]);
    if (image.exitCode !== 0) throw new Error("failed to create fixture PNG");
    fs.mkdirSync(krrDirectory, { recursive: true });
    fs.mkdirSync(chromeDirectory, { recursive: true });
    const krrEntries = destinations.map((destination) => ({
      destination,
      capturePath: png,
      executionNonce: `nonce-${width}`,
      scrollY: 0,
      targetTop: 0,
      targetHeight: 20,
      targetVisible: true,
      pointer: { x: 1, y: 1 },
    }));
    const chromeEntries = destinations.map((destination) => ({
      href: `#${destination}`,
      targetId: destination,
      actualScrollY: 0,
      requestedScrollY: 0,
      targetTop: 0,
      targetVisible: true,
      capturePath: png,
    }));
    fs.writeFileSync(
      path.join(krrDirectory, "toc-report.json"),
      JSON.stringify({
        htmlSha256: sha256File(source),
        executionNonce: `nonce-${width}`,
        viewport: { width, height, scale: 1 },
        entries: krrEntries,
      }),
    );
    fs.writeFileSync(
      path.join(chromeDirectory, "scroll-coverage.json"),
      JSON.stringify({
        htmlPath: source,
        htmlSha256: sha256File(source),
        viewport: { width, height, scale: 1 },
        toc: chromeEntries,
      }),
    );
  }
  const manifest = path.join(root, "manifest.json");
  fs.writeFileSync(
    manifest,
    JSON.stringify({
      kind: "toc_pointer_diagnostic",
      source_html_sha256: sha256File(source),
      runtime_binary: { path: binary, sha256: binarySha },
      cases: [1280, 960, 620].map((width) => {
        const report = path.join(root, `krr-${width}`, "toc-report.json");
        return { report, report_sha256: sha256File(report), entries: 16 };
      }),
    }),
  );
  return {
    root,
    binary,
    args: [
      "--krr-root",
      root,
      "--chrome-root",
      root,
      "--krr-prefix",
      "krr",
      "--chrome-prefix",
      "chrome",
      "--binary-sha256",
      binarySha,
      "--binary-path",
      binary,
      "--manifest",
      manifest,
      "--output",
      path.join(root, "result.json"),
    ],
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function run(args: string[]): ReturnType<typeof Bun.spawnSync> {
  return Bun.spawnSync(["bun", path.join(import.meta.dir, "compare-toc-evidence.ts"), ...args], {
    cwd: process.cwd(),
  });
}

function mutateReport(root: string, width: number, mutate: (value: JsonObject) => void): void {
  const file = path.join(root, `krr-${width}`, "toc-report.json");
  const value = JSON.parse(fs.readFileSync(file, "utf8")) as JsonObject;
  mutate(value);
  fs.writeFileSync(file, JSON.stringify(value));
  const manifestPath = path.join(root, "manifest.json");
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as JsonObject;
  const cases = manifest.cases as JsonObject[];
  const entry = cases.find((item) => item.report === file);
  if (entry === undefined) throw new Error("fixture manifest entry missing");
  entry.report_sha256 = sha256File(file);
  fs.writeFileSync(manifestPath, JSON.stringify(manifest));
}

function firstEntry(value: JsonObject): JsonObject {
  const entry = (value.entries as JsonObject[]).at(0);
  if (entry === undefined) throw new Error("fixture entry missing");
  return entry;
}

test("accepts a complete 48-entry evidence fixture while keeping acceptance false", () => {
  const value = fixture();
  try {
    const result = run(value.args);
    if (result.exitCode !== 0)
      throw new Error(
        `${new TextDecoder().decode(result.stderr)}${new TextDecoder().decode(result.stdout)}`,
      );
    const report = JSON.parse(fs.readFileSync(path.join(value.root, "result.json"), "utf8"));
    expect(report.count).toBe(48);
    expect(report.failures).toEqual([]);
    expect(report.acceptance).toBe(false);
    expect(report.performanceVerified).toBe(false);
  } finally {
    value.cleanup();
  }
}, 30_000);

test.each([
  [
    "viewport scale",
    (root: string) => mutateReport(root, 1280, (v) => ((v.viewport as JsonObject).scale = 2)),
  ],
  [
    "PNG dimensions",
    (root: string) =>
      mutateReport(root, 1280, (v) => (firstEntry(v).capturePath = path.join(root, "31x32.png"))),
  ],
  [
    "target top missing",
    (root: string) => mutateReport(root, 1280, (v) => delete firstEntry(v).targetTop),
  ],
  [
    "visible self declaration",
    (root: string) => mutateReport(root, 1280, (v) => (firstEntry(v).targetVisible = false)),
  ],
  [
    "target height missing",
    (root: string) => mutateReport(root, 1280, (v) => delete firstEntry(v).targetHeight),
  ],
  [
    "target entirely above viewport",
    (root: string) => mutateReport(root, 1280, (v) => (firstEntry(v).targetTop = -20)),
  ],
  [
    "pointer on viewport boundary",
    (root: string) =>
      mutateReport(root, 1280, (v) => ((firstEntry(v).pointer as JsonObject).x = 1280)),
  ],
  [
    "pointer outside viewport",
    (root: string) =>
      mutateReport(root, 1280, (v) => ((firstEntry(v).pointer as JsonObject).x = 1281)),
  ],
  [
    "required destination replacement",
    (root: string) => mutateReport(root, 1280, (v) => (firstEntry(v).destination = "unexpected")),
  ],
] as const)("rejects %s evidence", (_name, mutate) => {
  const value = fixture();
  try {
    if (_name === "PNG dimensions") {
      const image = Bun.spawnSync([
        "magick",
        "-size",
        "31x32",
        "xc:white",
        path.join(value.root, "31x32.png"),
      ]);
      expect(image.exitCode).toBe(0);
    }
    mutate(value.root);
    expect(run(value.args).exitCode).not.toBe(0);
  } finally {
    value.cleanup();
  }
});

test("rejects a binary SHA mismatch before reading evidence", () => {
  const value = fixture();
  try {
    const args = [...value.args];
    args[args.indexOf("--binary-sha256") + 1] = "0".repeat(64);
    expect(run(args).exitCode).not.toBe(0);
  } finally {
    value.cleanup();
  }
});

test("rejects a manifest that repeats an unrelated report", () => {
  const value = fixture();
  try {
    const file = path.join(value.root, "manifest.json");
    const manifest = JSON.parse(fs.readFileSync(file, "utf8")) as JsonObject;
    const cases = manifest.cases as JsonObject[];
    const first = cases.at(0);
    if (first === undefined) throw new Error("fixture manifest is empty");
    manifest.cases = [first, first, first];
    fs.writeFileSync(file, JSON.stringify(manifest));
    expect(run(value.args).exitCode).not.toBe(0);
  } finally {
    value.cleanup();
  }
});
