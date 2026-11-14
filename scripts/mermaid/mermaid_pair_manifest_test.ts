import { expect, spyOn, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { makeOnePixelPng } from "./mermaid_pair_test_support";
import {
  publishMermaidPair,
  requireStrictRenderedPair,
  verifyMermaidPair,
} from "./mermaid-pair-manifest";

const PNG = makeOnePixelPng();

test("publishes a decoded SVG/PNG pair with the manifest as the final binding", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-pair-"));
  try {
    const svg = Buffer.from("<svg></svg>");
    const manifest = publishMermaidPair(input(root, svg, svg, PNG));
    expect(manifest.png).toEqual({ width: 1, height: 1 });
    expect(verifyMermaidPair(root, "fixture", "official")).toEqual(manifest);
    expect(JSON.parse(fs.readFileSync(path.join(root, "fixture.pair.json"), "utf8"))).toEqual(
      manifest,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("records rendered-byte mismatch and rejects it as a strict pair", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-pair-"));
  try {
    const source = Buffer.from("<svg><br /></svg>");
    const rendered = Buffer.from("<svg><br></svg>");
    const manifest = publishMermaidPair(input(root, source, rendered, PNG));
    expect(manifest.renderedSvgMatchesSource).toBe(false);
    expect(verifyMermaidPair(root, "fixture", "official")).toEqual(manifest);
    expect(() => requireStrictRenderedPair(manifest)).toThrow("rendered SVG bytes differ");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("rejects a tampered PNG and a legacy pair with no manifest", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-pair-"));
  try {
    const svg = Buffer.from("<svg></svg>");
    publishMermaidPair(input(root, svg, svg, PNG));
    fs.appendFileSync(path.join(root, "fixture.png"), Buffer.from([0]));
    expect(() => verifyMermaidPair(root, "fixture", "official")).toThrow(
      "does not match its files",
    );
    fs.unlinkSync(path.join(root, "fixture.pair.json"));
    expect(() => verifyMermaidPair(root, "fixture", "official")).toThrow("incomplete or unsafe");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("rechecks bound runtime asset bytes when a pair is consumed", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-pair-"));
  const runtimeAsset = path.join(root, "runtime.js");
  try {
    const svg = Buffer.from("<svg></svg>");
    fs.writeFileSync(runtimeAsset, "runtime-v1");
    publishMermaidPair({
      ...input(root, svg, svg, PNG),
      runtimeAssets: [
        {
          path: runtimeAsset,
          sha256: createHash("sha256").update("runtime-v1").digest("hex"),
        },
      ],
    });
    fs.writeFileSync(runtimeAsset, "runtime-v2");
    expect(() => verifyMermaidPair(root, "fixture", "official")).toThrow(
      "runtime asset hash changed",
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("invalid PNG input does not alter an existing committed pair", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-pair-"));
  try {
    const svg = Buffer.from("<svg></svg>");
    publishMermaidPair(input(root, svg, svg, PNG));
    const before = ["fixture.svg", "fixture.rendered.svg", "fixture.png", "fixture.pair.json"].map(
      (name) => fs.readFileSync(path.join(root, name)),
    );
    expect(() =>
      publishMermaidPair(input(root, Buffer.from("new"), Buffer.from("new"), Buffer.from("bad"))),
    ).toThrow();
    expect(
      ["fixture.svg", "fixture.rendered.svg", "fixture.png", "fixture.pair.json"].map((name) =>
        fs.readFileSync(path.join(root, name)),
      ),
    ).toEqual(before);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a late manifest-publish failure rolls back both assets and the previous marker", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-pair-"));
  try {
    const oldSvg = Buffer.from("<svg>old</svg>");
    publishMermaidPair(input(root, oldSvg, oldSvg, PNG));
    const before = ["fixture.svg", "fixture.rendered.svg", "fixture.png", "fixture.pair.json"].map(
      (name) => fs.readFileSync(path.join(root, name)),
    );
    const rename = fs.renameSync.bind(fs);
    let failedMarkerCommit = false;
    const renameSpy = spyOn(fs, "renameSync").mockImplementation((source, target) => {
      if (
        !failedMarkerCommit &&
        String(source).includes(".publish-") &&
        String(target).endsWith("fixture.pair.json")
      ) {
        failedMarkerCommit = true;
        throw new Error("injected manifest commit failure");
      }
      return rename(source, target);
    });
    try {
      const newSvg = Buffer.from("<svg>new</svg>");
      expect(() => publishMermaidPair(input(root, newSvg, newSvg, PNG))).toThrow(
        "injected manifest commit failure",
      );
    } finally {
      renameSpy.mockRestore();
    }
    expect(
      ["fixture.svg", "fixture.rendered.svg", "fixture.png", "fixture.pair.json"].map((name) =>
        fs.readFileSync(path.join(root, name)),
      ),
    ).toEqual(before);
    verifyMermaidPair(root, "fixture", "official");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

function input(directory: string, sourceSvg: Buffer, renderedSvg: Buffer, png: Buffer) {
  return {
    directory,
    slug: "fixture",
    role: "official" as const,
    sourceSvg,
    renderedSvg,
    png,
    provenance: {
      path: "embedded:test",
      sha256: createHash("sha256").update(JSON.stringify("provenance")).digest("hex"),
      document: "provenance",
    },
    config: { viewport: [1, 1] },
    runtimeAssets: [],
  };
}
