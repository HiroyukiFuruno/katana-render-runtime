import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DiagramTheme } from "./diagram_theme";
import { makeOnePixelPng } from "./mermaid_pair_test_support";
import { fileSha256 } from "./mermaid-pair-manifest";
import type { NativeFrameEvidence } from "./native-frame-evidence";
import {
  NATIVE_FRAME_TARGETS,
  nativeFrameCaptureWrapper,
  parseNativeFrameEvidence,
  scoreNativeFramePaintProjection,
  verifyNativeFrameEvidence,
} from "./native-frame-evidence";
import { PlaywrightLoader } from "./official-renderer";
import type { PageHandle } from "./official-renderer-types";
import { assignStrictSvg, resizeSvgCapture } from "./raster-capture-observation";
import { rasterCaptureHtml } from "./rasterize-svg-dir";
import { MagickOps } from "./reference_image_ops";
import { PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA } from "./reference_svg_geometry";

function fixture(language: "en" | "ja" = "en", requestedSlugs?: string[]) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-native-frame-evidence-"));
  const write = (name: string, body: string) => {
    const file = path.join(root, name);
    fs.writeFileSync(file, body, { flag: "wx", mode: 0o600 });
    return { path: file, sha256: fileSha256(file) };
  };
  const probeBinary = write("probe", "probe");
  const runtime = write("runtime", "runtime");
  const sourceManifest = write("source-manifest.json", "manifest");
  const config = write("config.json", "config");
  const fontContext = write("font-context.json", "font-context");
  const validPng = makeOnePixelPng();
  const targets =
    requestedSlugs ??
    (language === "ja" ? PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA : NATIVE_FRAME_TARGETS);
  const rows = targets.map((slug) => ({
    slug,
    inputSvg: write(`${slug}.svg`, `<svg id="${slug}"/>`),
    sourcePng: write(`${slug}-source.png`, "placeholder"),
    wrapperHtml: write(`${slug}.html`, "placeholder"),
    png: write(`${slug}.png`, `png:${slug}`),
    stdout: write(`${slug}.stdout`, "png=fixture width=1 height=1 scale=1 scroll_y=0\n"),
    stderr: write(`${slug}.stderr`, ""),
    exitEvidence: write(`${slug}.exit.json`, '{"exitCode":0,"signal":null,"error":null}\n'),
    outputDirectory: root,
    frame: { width: 1, height: 1, scale: 1, scrollY: 0 },
    argv: [
      probeBinary.path,
      "--input",
      path.join(root, `${slug}.html`),
      "--output",
      root,
      "--width",
      "1",
      "--height",
      "1",
      "--scale",
      "1",
      "--scroll-y",
      "0",
    ],
    exitCode: 0 as const,
  }));
  for (const row of rows) {
    const outputDirectory = path.join(root, row.slug);
    fs.mkdirSync(outputDirectory, { mode: 0o700 });
    const framesDirectory = path.join(outputDirectory, "frames");
    fs.mkdirSync(framesDirectory, { mode: 0o700 });
    row.outputDirectory = framesDirectory;
    row.argv[4] = framesDirectory;
    const svg = fs.readFileSync(row.inputSvg.path, "utf8"),
      wrapper = nativeFrameCaptureWrapper(svg, { width: 1, height: 1 });
    fs.writeFileSync(row.wrapperHtml.path, wrapper, { flag: "w" });
    row.wrapperHtml.sha256 = fileSha256(row.wrapperHtml.path);
    fs.writeFileSync(
      row.stdout.path,
      `source_sha256=${row.wrapperHtml.sha256}\npng=fixture width=1 height=1 scale=1 scroll_y=0\n`,
      { flag: "w" },
    );
    row.stdout.sha256 = fileSha256(row.stdout.path);
    const pngPath = path.join(framesDirectory, "1x1-scale-1-scroll-0.png");
    const png = validPng;
    fs.writeFileSync(pngPath, png);
    row.png = { path: pngPath, sha256: fileSha256(pngPath) };
    const sourcePngPath = path.join(root, `${row.slug}-source.png`);
    fs.writeFileSync(sourcePngPath, png);
    row.sourcePng = { path: sourcePngPath, sha256: fileSha256(sourcePngPath) };
  }
  return {
    root,
    value: {
      schema: "krr-mermaid-native-frame-evidence-v1" as const,
      language,
      probeBinary,
      runtime,
      sourceManifest,
      config,
      fontContext,
      rows,
    },
  };
}

describe("native frame evidence", () => {
  test("sizes the native wrapper to the same natural viewBox as formal Chrome capture", async () => {
    const playwright = await new PlaywrightLoader().load(),
      browser = await playwright.chromium.launch({ headless: true }),
      page = await browser.newPage({
        viewport: { width: 1520, height: 845 },
        deviceScaleFactor: 1,
      });
    try {
      for (const target of nativeFrameSizingCases) await compareNativeAndFormalSizing(page, target);
      for (const viewBox of ["0 0 20", "0 0 0 20", "0 0 -1 20", "0 0 20 10 5"]) {
        await expectInvalidBrowserViewBox(page, viewBox);
      }
    } finally {
      await page.close();
      await browser.close();
    }
  });

  test("preserves no-viewBox fallback and rejects malformed natural dimensions", () => {
    const fallback = nativeFrameCaptureWrapper('<svg id="unit"/>', { width: 10, height: 8 });
    expect(fallback).toContain('<div id="diagram"><svg id="unit"/></div>');
    for (const viewBox of ["0 0 20", "0 0 0 20", "0 0 -1 20", "0 0 NaN 20"]) {
      expect(() =>
        nativeFrameCaptureWrapper(
          `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}"/>`,
          {
            width: 44,
            height: 44,
          },
        ),
      ).toThrow("invalid SVG viewBox");
    }
  });

  test("requires the exact C4 and TreeView target inventory and physical pins", () => {
    const data = fixture();
    try {
      expect(verifyNativeFrameEvidence(data.value, "en").rows).toHaveLength(8);
    } finally {
      fs.rmSync(data.root, { recursive: true, force: true });
    }
  });

  test("verifies the Japanese target inventory", () => {
    const data = fixture("ja");
    try {
      expect(verifyNativeFrameEvidence(data.value, "ja").rows).toHaveLength(8);
    } finally {
      fs.rmSync(data.root, { recursive: true, force: true });
    }
  });

  test("accepts full fixture evidence and rejects the old geometry-only sidecar", () => {
    const expected = [...NATIVE_FRAME_TARGETS, "native-extra-1"],
      complete = fixture("en", expected),
      geometryOnly = fixture();
    try {
      expect(verifyNativeFrameEvidence(complete.value, "en", expected).rows).toHaveLength(9);
      expect(() => verifyNativeFrameEvidence(complete.value, "en", expected.slice(0, -1))).toThrow(
        "Native frame inventory differs",
      );
      expect(() =>
        verifyNativeFrameEvidence(complete.value, "en", [...expected, expected[0] as string]),
      ).toThrow("Native frame inventory");
      expect(() => verifyNativeFrameEvidence(geometryOnly.value, "en", expected)).toThrow(
        "Native frame inventory differs",
      );
    } finally {
      fs.rmSync(complete.root, { recursive: true, force: true });
      fs.rmSync(geometryOnly.root, { recursive: true, force: true });
    }
  });

  test("rejects duplicate or missing target rows", () => {
    const data = fixture();
    try {
      const changed: NativeFrameEvidence = {
        ...data.value,
        rows: data.value.rows.slice(),
      };
      const duplicate = changed.rows[0];
      if (duplicate === undefined) throw new Error("missing native frame fixture");
      changed.rows.pop();
      changed.rows.push({ ...duplicate });
      expect(() => verifyNativeFrameEvidence(changed, "en")).toThrow(
        "Native frame inventory differs",
      );
    } finally {
      fs.rmSync(data.root, { recursive: true, force: true });
    }
  });

  test("rejects a changed PNG or probe binding", () => {
    const data = fixture();
    try {
      const changed: NativeFrameEvidence = {
        ...data.value,
        rows: data.value.rows.map((row) => ({ ...row })),
      };
      const first = changed.rows[0];
      if (first === undefined) throw new Error("missing native frame fixture");
      fs.writeFileSync(first.png.path, "tampered", { flag: "w" });
      expect(() => verifyNativeFrameEvidence(changed, "en")).toThrow("native PNG changed");
      const bindingData = fixture();
      const bindingChanged: NativeFrameEvidence = {
        ...bindingData.value,
        rows: bindingData.value.rows.map((row) => ({ ...row, argv: [...row.argv] })),
      };
      const bindingRow = bindingChanged.rows[0];
      if (bindingRow === undefined) throw new Error("missing native frame binding fixture");
      bindingRow.argv[0] = path.join(bindingData.root, "other-probe");
      expect(() => verifyNativeFrameEvidence(bindingChanged, "en")).toThrow(
        "Native frame probe binding differs",
      );
      fs.rmSync(bindingData.root, { recursive: true, force: true });
    } finally {
      fs.rmSync(data.root, { recursive: true, force: true });
    }
  });

  test("rejects a truncated PNG after the physical decoder check", () => {
    const data = fixture();
    try {
      const first = data.value.rows[0];
      if (first === undefined) throw new Error("missing native frame fixture");
      fs.writeFileSync(first.png.path, makeOnePixelPng().subarray(0, 24), { flag: "w" });
      first.png.sha256 = fileSha256(first.png.path);
      expect(() => verifyNativeFrameEvidence(data.value, "en")).toThrow("Command failed");
    } finally {
      fs.rmSync(data.root, { recursive: true, force: true });
    }
  });

  test("rejects probe output with a mismatched wrapper source SHA", () => {
    const data = fixture();
    try {
      const first = data.value.rows[0];
      if (first === undefined) throw new Error("missing native frame fixture");
      fs.writeFileSync(first.stdout.path, `${"source_sha256="}${"0".repeat(64)}\n`, {
        flag: "w",
      });
      first.stdout.sha256 = fileSha256(first.stdout.path);
      expect(() => verifyNativeFrameEvidence(data.value, "en")).toThrow("HTML input SHA changed");
    } finally {
      fs.rmSync(data.root, { recursive: true, force: true });
    }
  });

  test("parses only the native sidecar schema", () => {
    const data = fixture();
    try {
      expect(parseNativeFrameEvidence(data.value).schema).toBe(
        "krr-mermaid-native-frame-evidence-v1",
      );
      expect(() => parseNativeFrameEvidence({ ...data.value, accepted: true })).toThrow();
    } finally {
      fs.rmSync(data.root, { recursive: true, force: true });
    }
  });

  test("rejects sidecars whose input is not the current candidate pair", async () => {
    const data = fixture();
    try {
      const rows = new Map(data.value.rows.map((row) => [row.slug, row]));
      const pairs = NATIVE_FRAME_TARGETS.map((slug) => {
        const row = rows.get(slug);
        if (row === undefined) throw new Error(`missing fixture row: ${slug}`);
        return {
          slug,
          officialPath: row.sourcePng.path,
          katanaPath: row.sourcePng.path,
          officialSvgPath: row.inputSvg.path,
          katanaSvgPath: row.inputSvg.path,
        };
      });
      const first = pairs[0];
      if (first === undefined) throw new Error("missing native frame fixture");
      first.katanaSvgPath = pairs[1]?.katanaSvgPath ?? first.katanaSvgPath;
      await expect(
        scoreNativeFramePaintProjection(
          pairs,
          data.value,
          path.join(data.root, "comparison"),
          "dark",
          new MagickOps(path.join(data.root, "magick"), null, "dark"),
        ),
      ).rejects.toThrow("Native frame input SVG is not the current candidate");
    } finally {
      fs.rmSync(data.root, { recursive: true, force: true });
    }
  });
});

const nativeFrameSizingCases = [
  {
    relativePath: "tests/fixtures/mermaid/en/official-dark/08-01-c4-context-simple.svg",
    viewBox: [0, -70, 1148, 441],
  },
  {
    relativePath: "tests/fixtures/mermaid/en/official-dark/22-01-tree-view-simple.svg",
    viewBox: [-0.5, 0, 124.828125, 232],
  },
] as const;

async function compareNativeAndFormalSizing(
  page: PageHandle,
  fixture: (typeof nativeFrameSizingCases)[number],
) {
  const svg = fs.readFileSync(path.join(process.cwd(), fixture.relativePath), "utf8"),
    frame = {
      width: Math.ceil(fixture.viewBox[2]) + 24,
      height: Math.ceil(fixture.viewBox[3]) + 24,
    };
  await page.setContent(nativeFrameCaptureWrapper(svg, frame), { waitUntil: "load" });
  const native = await readBrowserGeometry(page);
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  const loaded = await assignStrictSvg(page, svg);
  expect(loaded.assignedSvgSerialization).toBe(native.svgHtml);
  await resizeSvgCapture(page, true, "strictViewBox");
  const formal = await readBrowserGeometry(page);
  expect(native.capture).toEqual(frame);
  expect(native.diagram).toEqual(formal.diagram);
  expect(native.svgBox).toEqual(formal.svgBox);
  expect(native.capture).toEqual(formal.capture);
  expect(native.svgHtml).toBe(loaded.expectedSvgSerialization);
  expect(native.rootFontSize).toBe("16px");
}

function readBrowserGeometry(page: PageHandle) {
  return page.evaluate(() => {
    const capture = document.getElementById("capture") as HTMLElement,
      diagram = document.getElementById("diagram") as HTMLElement,
      svg = document.querySelector("#diagram svg") as SVGSVGElement,
      captureBox = capture.getBoundingClientRect(),
      diagramBox = diagram.getBoundingClientRect(),
      svgBox = svg.getBoundingClientRect();
    return {
      capture: { width: captureBox.width, height: captureBox.height },
      diagram: { width: diagramBox.width, height: diagramBox.height },
      svgBox: { width: svgBox.width, height: svgBox.height },
      svgHtml: svg.outerHTML,
      rootFontSize: getComputedStyle(svg).fontSize,
    };
  });
}

async function expectInvalidBrowserViewBox(page: PageHandle, viewBox: string) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}"/>`;
  expect(() => nativeFrameCaptureWrapper(svg, { width: 44, height: 44 })).toThrow(
    "invalid SVG viewBox",
  );
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  await assignStrictSvg(page, svg);
  await expect(resizeSvgCapture(page, true, "strictViewBox")).rejects.toThrow(
    "SVG viewBox is invalid",
  );
}
