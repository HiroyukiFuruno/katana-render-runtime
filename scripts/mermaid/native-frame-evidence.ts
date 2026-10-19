import fs from "node:fs";
import path from "node:path";
import { DOMParser } from "@xmldom/xmldom";
import { z } from "zod";
import { assertExactlyOneReportedInputSha } from "../html/reference-capture";
import { DiagramTheme } from "./diagram_theme";
import { fileSha256 } from "./mermaid-pair-manifest";
import { rasterCaptureHtml } from "./rasterize-svg-dir";
import { MagickOps } from "./reference_image_ops";
import type { ReferencePair } from "./reference_pair_repository";
import {
  isPaintProjectedReferenceTargetSlug,
  PAINT_PROJECTED_REFERENCE_TARGET_SLUGS,
  PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA,
} from "./reference_svg_geometry";
import {
  type PaintProjectedReferenceRun,
  scorePaintProjectedReferenceTargets,
} from "./reference_svg_geometry_scoring";

export const NATIVE_FRAME_EVIDENCE_SCHEMA = "krr-mermaid-native-frame-evidence-v1" as const;
export const NATIVE_FRAME_TARGETS = PAINT_PROJECTED_REFERENCE_TARGET_SLUGS;

const digest = z.string().regex(/^[a-f0-9]{64}$/);
const filePin = z.object({ path: z.string().min(1), sha256: digest }).strict();
const frame = z
  .object({
    width: z.number().int().positive(),
    height: z.number().int().positive(),
    scale: z.number().positive(),
    scrollY: z.number().finite().nonnegative(),
  })
  .strict();

const rowSchema = z
  .object({
    slug: z.string().min(1),
    inputSvg: filePin,
    sourcePng: filePin,
    wrapperHtml: filePin,
    png: filePin,
    stdout: filePin,
    stderr: filePin,
    exitEvidence: filePin,
    outputDirectory: z.string().min(1),
    frame,
    argv: z.array(z.string().min(1)).min(1),
    exitCode: z.literal(0),
  })
  .strict();

export const nativeFrameEvidenceSchema = z
  .object({
    schema: z.literal(NATIVE_FRAME_EVIDENCE_SCHEMA),
    language: z.enum(["en", "ja"]),
    probeBinary: filePin,
    runtime: filePin,
    sourceManifest: filePin,
    config: filePin,
    fontContext: filePin,
    rows: z.array(rowSchema),
  })
  .strict();

export type NativeFrameEvidence = z.infer<typeof nativeFrameEvidenceSchema>;

export interface NativeFrameCaptureOptions {
  language: "en" | "ja";
  sourceSvgDirectory: string;
  sourcePngDirectory: string;
  outputDirectory: string;
  probeBinary: { path: string; sha256: string };
  runtime: { path: string; sha256: string };
  sourceManifest: { path: string; sha256: string };
  config: { path: string; sha256: string };
  fontContext: { path: string; sha256: string };
  root: string;
  scale: number;
  scrollY: number;
}

export function captureNativeFrameEvidence(
  options: NativeFrameCaptureOptions,
): NativeFrameEvidence {
  const normalized = {
    ...options,
    sourceSvgDirectory: path.resolve(options.sourceSvgDirectory),
    sourcePngDirectory: path.resolve(options.sourcePngDirectory),
    outputDirectory: path.resolve(options.outputDirectory),
    root: path.resolve(options.root),
  };
  fs.mkdirSync(normalized.outputDirectory, { recursive: true, mode: 0o700 });
  const targets =
    normalized.language === "ja" ? PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA : NATIVE_FRAME_TARGETS;
  const rows = targets.map((slug) => captureRow(normalized, slug));
  const evidence: NativeFrameEvidence = {
    schema: NATIVE_FRAME_EVIDENCE_SCHEMA,
    language: normalized.language,
    probeBinary: normalized.probeBinary,
    runtime: normalized.runtime,
    sourceManifest: normalized.sourceManifest,
    config: normalized.config,
    fontContext: normalized.fontContext,
    rows,
  };
  const evidencePath = path.join(options.outputDirectory, "native-frame-evidence.json");
  fs.writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, {
    flag: "wx",
    mode: 0o600,
  });
  verifyNativeFrameEvidence(evidence, normalized.language);
  return evidence;
}

function captureRow(
  options: NativeFrameCaptureOptions,
  slug: string,
): NativeFrameEvidence["rows"][number] {
  const svgPath = path.join(options.sourceSvgDirectory, `${slug}.svg`),
    sourcePngPath = path.join(options.sourcePngDirectory, `${slug}.png`),
    svg = fs.readFileSync(svgPath, "utf8"),
    frame = pngDimensions(sourcePngPath),
    rowDirectory = path.join(options.outputDirectory, slug),
    outputDirectory = path.join(rowDirectory, "frames"),
    wrapperPath = path.join(rowDirectory, "input.html");
  fs.mkdirSync(rowDirectory, { recursive: true, mode: 0o700 });
  fs.mkdirSync(outputDirectory, { recursive: true, mode: 0o700 });
  fs.writeFileSync(wrapperPath, nativeFrameCaptureWrapper(svg, frame), {
    flag: "wx",
    mode: 0o600,
  });
  const argv = [
    options.probeBinary.path,
    "--input",
    wrapperPath,
    "--output",
    outputDirectory,
    "--width",
    String(frame.width),
    "--height",
    String(frame.height),
    "--scale",
    String(options.scale),
    "--scroll-y",
    String(options.scrollY),
  ];
  const result = Bun.spawnSync(argv, {
    cwd: options.root,
    env: process.env,
  });
  const stdoutPath = path.join(rowDirectory, "probe.stdout.raw"),
    stderrPath = path.join(rowDirectory, "probe.stderr.raw"),
    exitPath = path.join(rowDirectory, "probe.exit.json");
  fs.writeFileSync(stdoutPath, result.stdout ?? "", { flag: "wx", mode: 0o600 });
  fs.writeFileSync(stderrPath, result.stderr ?? "", { flag: "wx", mode: 0o600 });
  fs.writeFileSync(
    exitPath,
    `${JSON.stringify({ exitCode: result.exitCode, signal: result.signalCode ?? null, error: null })}\n`,
    { flag: "wx", mode: 0o600 },
  );
  if (result.exitCode !== 0 || result.signalCode !== undefined) {
    throw new Error(`Native frame probe failed: ${slug}`);
  }
  const pngPath = path.join(outputDirectory, pngName(frame));
  return {
    slug,
    inputSvg: pin(svgPath),
    sourcePng: pin(sourcePngPath),
    wrapperHtml: pin(wrapperPath),
    png: pin(pngPath),
    stdout: pin(stdoutPath),
    stderr: pin(stderrPath),
    exitEvidence: pin(exitPath),
    outputDirectory,
    frame,
    argv,
    exitCode: 0,
  };
}

export function nativeFrameCaptureWrapper(
  svg: string,
  dimensions: { width: number; height: number },
): string {
  const naturalSize = svgViewBoxSize(svg),
    diagramStyle =
      naturalSize === undefined
        ? ""
        : ` style="width:${naturalSize.width}px;height:${naturalSize.height}px"`;
  return rasterCaptureHtml(DiagramTheme.parse("dark"))
    .replace(
      '<div id="capture">',
      `<div id="capture" style="width:${dimensions.width}px;height:${dimensions.height}px">`,
    )
    .replace('<div id="diagram"></div>', `<div id="diagram"${diagramStyle}>${svg}</div>`);
}

function svgViewBoxSize(svg: string): { width: number; height: number } | undefined {
  const document = new DOMParser({
      onError: (level, message) => {
        throw new Error(`Native frame SVG parse ${level}: ${message}`);
      },
    }).parseFromString(svg, "image/svg+xml"),
    root = document.documentElement;
  if (
    document.doctype !== null ||
    root === null ||
    root.localName !== "svg" ||
    (root.namespaceURI !== null && root.namespaceURI !== "http://www.w3.org/2000/svg") ||
    document.getElementsByTagName("parsererror").length > 0
  ) {
    throw new Error("Native frame input requires one well-formed SVG root");
  }
  const viewBox = root.getAttribute("viewBox");
  if (viewBox === null) return undefined;
  const values = viewBox
    .trim()
    .split(/[\s,]+/)
    .map(Number);
  if (
    values.length !== 4 ||
    values.some((value) => !Number.isFinite(value)) ||
    values[2] === undefined ||
    values[3] === undefined ||
    values[2] <= 0 ||
    values[3] <= 0
  ) {
    throw new Error("Native frame input has an invalid SVG viewBox");
  }
  return { width: values[2], height: values[3] };
}

function pngDimensions(filePath: string): {
  width: number;
  height: number;
  scale: number;
  scrollY: number;
} {
  const png = fs.readFileSync(filePath);
  if (png.length < 24 || png[0] !== 0x89 || png.toString("ascii", 1, 4) !== "PNG") {
    throw new Error(`Source PNG is invalid: ${filePath}`);
  }
  return { width: png.readUInt32BE(16), height: png.readUInt32BE(20), scale: 1, scrollY: 0 };
}

function pin(filePath: string): { path: string; sha256: string } {
  return { path: path.resolve(filePath), sha256: fileSha256(filePath) };
}

export async function scoreNativeFramePaintProjection(
  pairs: readonly ReferencePair[],
  evidence: NativeFrameEvidence,
  comparisonDirectory: string,
  theme: "dark" | "light",
  magick: MagickOps,
): Promise<PaintProjectedReferenceRun> {
  const rows = new Map(evidence.rows.map((row) => [row.slug, row]));
  const nativePairs = pairs
    .filter((pair) => isPaintProjectedReferenceTargetSlug(pair.slug))
    .map((pair) => {
      const row = rows.get(pair.slug);
      if (row === undefined) throw new Error(`Missing native frame row: ${pair.slug}`);
      if (
        path.resolve(row.inputSvg.path) !== path.resolve(pair.katanaSvgPath) ||
        row.inputSvg.sha256 !== fileSha256(pair.katanaSvgPath)
      ) {
        throw new Error(`Native frame input SVG is not the current candidate: ${pair.slug}`);
      }
      if (
        path.resolve(row.sourcePng.path) !== path.resolve(pair.katanaPath) ||
        row.sourcePng.sha256 !== fileSha256(pair.katanaPath)
      ) {
        throw new Error(`Native frame source PNG is not the current candidate: ${pair.slug}`);
      }
      return { ...pair, katanaPath: row.png.path };
    });
  const expectedCount =
    evidence.language === "ja"
      ? PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA.length
      : NATIVE_FRAME_TARGETS.length;
  if (nativePairs.length !== expectedCount) {
    throw new Error("Native frame scoring does not contain the exact target inventory");
  }
  magick.prepare();
  return scorePaintProjectedReferenceTargets(nativePairs, comparisonDirectory, theme, magick);
}

export function parseNativeFrameEvidence(value: unknown): NativeFrameEvidence {
  return nativeFrameEvidenceSchema.parse(value);
}

export function verifyNativeFrameEvidence(
  value: unknown,
  expectedLanguage: "en" | "ja",
  expectedSlugs: readonly string[] = NATIVE_FRAME_TARGETS,
): NativeFrameEvidence {
  const evidence = parseNativeFrameEvidence(value);
  if (evidence.language !== expectedLanguage) throw new Error("Native frame language differs");
  const targets = expectedSlugs.filter(isPaintProjectedReferenceTargetSlug);
  const languageTargets =
    expectedLanguage === "ja" ? PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA : NATIVE_FRAME_TARGETS;
  if (targets.length !== languageTargets.length) {
    throw new Error("Native frame inventory must contain the exact C4/Tree target set");
  }
  const slugs = evidence.rows.map((row) => row.slug);
  if (
    slugs.length !== targets.length ||
    new Set(slugs).size !== slugs.length ||
    JSON.stringify(slugs) !== JSON.stringify(languageTargets)
  ) {
    throw new Error("Native frame inventory differs from the fixed C4/Tree target set");
  }
  verifyPin(evidence.probeBinary, "native frame probe binary");
  verifyPin(evidence.runtime, "native frame runtime");
  verifyPin(evidence.sourceManifest, "native frame source manifest");
  verifyPin(evidence.config, "native frame config");
  verifyPin(evidence.fontContext, "native frame font context");
  const decoder = new MagickOps(
    path.join(process.cwd(), "tmp", "native-frame-image-validation"),
    null,
    "dark",
  );
  for (const row of evidence.rows) verifyRow(row, evidence.probeBinary.path, decoder);
  return evidence;
}

function verifyRow(row: z.infer<typeof rowSchema>, probePath: string, decoder: MagickOps): void {
  verifyPin(row.inputSvg, `${row.slug} input SVG`);
  verifyPin(row.sourcePng, `${row.slug} source PNG`);
  verifyPin(row.wrapperHtml, `${row.slug} wrapper HTML`);
  verifyPin(row.png, `${row.slug} native PNG`);
  verifyPin(row.stdout, `${row.slug} probe stdout`);
  verifyPin(row.stderr, `${row.slug} probe stderr`);
  verifyPin(row.exitEvidence, `${row.slug} probe exit`);
  verifyWrapper(row);
  verifyProbe(row, probePath);
  verifyDecodedImages(row, decoder);
}

function verifyWrapper(row: z.infer<typeof rowSchema>): void {
  const svgText = fs.readFileSync(row.inputSvg.path, "utf8"),
    wrapperText = fs.readFileSync(row.wrapperHtml.path, "utf8");
  if (!wrapperText.includes(svgText))
    throw new Error(`Native frame wrapper does not contain its input SVG: ${row.slug}`);
  if (
    wrapperText !==
    nativeFrameCaptureWrapper(svgText, { width: row.frame.width, height: row.frame.height })
  ) {
    throw new Error(`Native frame wrapper differs from the managed capture template: ${row.slug}`);
  }
}

function verifyProbe(row: z.infer<typeof rowSchema>, probePath: string): void {
  const expectedOutput = path.resolve(row.outputDirectory),
    expectedArgs = [
      probePath,
      "--input",
      row.wrapperHtml.path,
      "--output",
      expectedOutput,
      "--width",
      String(row.frame.width),
      "--height",
      String(row.frame.height),
      "--scale",
      String(row.frame.scale),
      "--scroll-y",
      String(row.frame.scrollY),
    ];
  if (JSON.stringify(row.argv) !== JSON.stringify(expectedArgs) || row.exitCode !== 0)
    throw new Error(`Native frame probe binding differs: ${row.slug}`);
  if (path.resolve(row.png.path) !== path.join(expectedOutput, pngName(row.frame)))
    throw new Error(`Native frame PNG path differs: ${row.slug}`);
  const sourceFrame = pngDimensions(row.sourcePng.path);
  if (sourceFrame.width !== row.frame.width || sourceFrame.height !== row.frame.height) {
    throw new Error(`Native frame source PNG dimensions differ: ${row.slug}`);
  }
  const exit = JSON.parse(fs.readFileSync(row.exitEvidence.path, "utf8")) as unknown;
  if (
    typeof exit !== "object" ||
    exit === null ||
    JSON.stringify(exit) !== JSON.stringify({ exitCode: 0, signal: null, error: null })
  ) {
    throw new Error(`Native frame probe did not record a clean exit: ${row.slug}`);
  }
  const stdout = fs.readFileSync(row.stdout.path, "utf8");
  assertExactlyOneReportedInputSha(stdout, row.wrapperHtml.sha256);
  if (
    !stdout.includes(`width=${row.frame.width}`) ||
    !stdout.includes(`height=${row.frame.height}`) ||
    !stdout.includes(`scale=${row.frame.scale}`) ||
    !stdout.includes(`scroll_y=${row.frame.scrollY}`)
  ) {
    throw new Error(`Native frame probe output does not bind the recorded frame: ${row.slug}`);
  }
}

function verifyDecodedImages(row: z.infer<typeof rowSchema>, decoder: MagickOps): void {
  const png = fs.readFileSync(row.png.path);
  if (png.length < 24 || png[0] !== 0x89 || png.toString("ascii", 1, 4) !== "PNG")
    throw new Error(`Native frame PNG is invalid: ${row.slug}`);
  if (png.readUInt32BE(16) !== row.frame.width || png.readUInt32BE(20) !== row.frame.height)
    throw new Error(`Native frame PNG dimensions differ: ${row.slug}`);
  for (const [label, filePath] of [
    ["source", row.sourcePng.path],
    ["native", row.png.path],
  ] as const) {
    const size = decoder.imageSize(filePath);
    if (size.width !== row.frame.width || size.height !== row.frame.height) {
      throw new Error(`Decoded ${label} PNG dimensions differ: ${row.slug}`);
    }
  }
}

function pngName(frame: { width: number; height: number; scale: number; scrollY: number }): string {
  const scale = String(frame.scale).replace(/\.0$/, "");
  return `${frame.width}x${frame.height}-scale-${scale}-scroll-${String(frame.scrollY)}.png`;
}

function verifyPin(pin: { path: string; sha256: string }, label: string): void {
  const resolved = path.resolve(pin.path);
  if (resolved !== pin.path || !fs.existsSync(resolved) || fileSha256(resolved) !== pin.sha256) {
    throw new Error(`${label} changed or is not an absolute regular file`);
  }
  if (!fs.lstatSync(resolved).isFile() || fs.lstatSync(resolved).isSymbolicLink()) {
    throw new Error(`${label} must be a regular file`);
  }
}
