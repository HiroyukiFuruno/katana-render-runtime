import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  assertInputUnchanged,
  assertNoFailedAssets,
  compareImages,
  failedAssetUrls,
  parseMetricOutput,
  parseOptions,
  scoreFromErrors,
} from "./reference-capture";

describe("HTML Chrome fidelity harness", () => {
  test("rejects changed pixels and mismatched dimensions using actual image metrics", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-score-regression-"));
    try {
      const original = path.join(directory, "original.png");
      const changed = path.join(directory, "changed.png");
      const resized = path.join(directory, "resized.png");
      for (const [size, color, output] of [
        ["32x32", "white", original],
        ["32x32", "black", changed],
        ["33x32", "white", resized],
      ] as const) {
        const result = Bun.spawnSync(["magick", "-size", size, `xc:${color}`, output]);
        expect(result.exitCode).toBe(0);
      }
      expect(compareImages(original, original, 99).score).toBe(100);
      const altered = compareImages(original, changed, 99);
      expect(altered.passed).toBe(false);
      expect(altered.score).toBeLessThan(99);
      expect(() => compareImages(original, resized, 99)).toThrow("PNG dimensions differ");
    } finally {
      fs.rmSync(directory, { recursive: true });
    }
  });
  test("parses ImageMagick normalized metrics", () => {
    expect(parseMetricOutput("0 (0.0125)\n")).toBe(0.0125);
  });

  test("rejects changed HTML input and failed asset captures", () => {
    expect(() => assertInputUnchanged("before", "after")).toThrow("HTML input changed");
    expect(() => assertInputUnchanged("same", "same")).not.toThrow();
    expect(() => assertNoFailedAssets(["404 /font.woff2"])).toThrow("asset loading failed");
    expect(() => assertNoFailedAssets([])).not.toThrow();
  });

  test("preserves the strict and interaction image completion policies", () => {
    const partialImage = {
      images: [{ currentSrc: "file:///image.png", complete: false, naturalWidth: 120 }],
      fonts: [],
    };
    expect(failedAssetUrls(partialImage, "complete")).toEqual(["file:///image.png"]);
    expect(failedAssetUrls(partialImage, "natural-width")).toEqual([]);

    const brokenImage = {
      images: [{ currentSrc: "file:///broken.png", complete: true, naturalWidth: 0 }],
      fonts: [],
    };
    expect(failedAssetUrls(brokenImage, "complete")).toEqual(["file:///broken.png"]);
    expect(failedAssetUrls(brokenImage, "natural-width")).toEqual(["file:///broken.png"]);
    expect(failedAssetUrls({ images: [], fonts: ["example-font"] }, "natural-width")).toEqual([
      "example-font",
    ]);
  });

  test("uses the minimum RMSE/MAE score without image normalization", () => {
    const report = scoreFromErrors(0.01, 0.03, 99);
    expect(report.rmseScore).toBe(99);
    expect(report.maeScore).toBe(97);
    expect(report.score).toBe(97);
    expect(report.passed).toBe(false);
  });

  test("parses explicit viewport and capture mode", () => {
    const options = parseOptions([
      "--html",
      "/tmp/input.html",
      "--krr",
      "/tmp/krr.png",
      "--width",
      "1440",
      "--height",
      "900",
      "--scale",
      "2",
      "--mode",
      "scroll",
      "--scroll-y",
      "120",
    ]);
    expect(options.width).toBe(1440);
    expect(options.height).toBe(900);
    expect(options.scale).toBe(2);
    expect(options.mode).toBe("scroll");
    expect(options.scrollY).toBe(120);
  });

  test("rejects invalid dimensions, score thresholds, and unrecognized arguments", () => {
    const base = ["--html", "/tmp/input.html", "--krr", "/tmp/krr.png"];
    expect(() => parseOptions([...base, "--width", "1280.5"])).toThrow();
    expect(() => parseOptions([...base, "--min-score", "101"])).toThrow();
    expect(() => parseOptions([...base, "--unsupported-option", "value"])).toThrow();
    expect(() => parseOptions([...base, "--height", "NaN"])).toThrow();
  });
});
