import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";

const CHANNELS = 4;
const CHANNEL_THRESHOLD = 1;

export type Rgba = [number, number, number, number];

export interface ReadRgbaPngResult {
  pixels: Uint8Array;
  sha256: string;
}

export function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

export function pixelsDiffer(actual: ArrayLike<number>, expected: ArrayLike<number>): boolean {
  return rgbaPixelsDiffer(actual, expected, 0, 0);
}

export function countRgbaDifferences(
  actual: Uint8Array,
  expected: Uint8Array,
  width: number,
  height: number,
): number {
  const requiredLength = width * height * CHANNELS;
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width <= 0 ||
    height <= 0 ||
    actual.length !== requiredLength ||
    expected.length !== requiredLength
  ) {
    throw new Error("decoded PNG dimensions are invalid");
  }
  let differences = 0;
  for (let offset = 0; offset < requiredLength; offset += CHANNELS) {
    if (rgbaPixelsDiffer(actual, expected, offset, offset)) differences += 1;
  }
  return differences;
}

export function rgbaAt(image: Uint8Array, x: number, y: number, width: number): Rgba {
  const offset = (y * width + x) * CHANNELS;
  if (
    !Number.isSafeInteger(x) ||
    !Number.isSafeInteger(y) ||
    !Number.isSafeInteger(width) ||
    x < 0 ||
    x >= width ||
    y < 0 ||
    width <= 0 ||
    offset + CHANNELS > image.length
  ) {
    throw new Error("RGBA coordinate is outside the decoded image");
  }
  return [
    image[offset] ?? 0,
    image[offset + 1] ?? 0,
    image[offset + 2] ?? 0,
    image[offset + 3] ?? 0,
  ];
}

export function readStableRgbaPng(
  file: string,
  width: number,
  height: number,
  expectedSha256?: string,
): ReadRgbaPngResult {
  const originalSha256 = sha256(fs.readFileSync(file));
  if (expectedSha256 !== undefined && originalSha256 !== expectedSha256) {
    throw new Error("PNG bytes differ from their evidence pin");
  }
  const pixels = decodeRgbaPng(file, width, height);
  const finalSha256 = sha256(fs.readFileSync(file));
  if (finalSha256 !== originalSha256) throw new Error("PNG changed during diagnosis");
  return { pixels, sha256: finalSha256 };
}

function rgbaPixelsDiffer(
  actual: ArrayLike<number>,
  expected: ArrayLike<number>,
  actualOffset: number,
  expectedOffset: number,
): boolean {
  if (actual[actualOffset + 3] === 0 && expected[expectedOffset + 3] === 0) return false;
  for (let channel = 0; channel < CHANNELS; channel += 1) {
    if (
      Math.abs((actual[actualOffset + channel] ?? 0) - (expected[expectedOffset + channel] ?? 0)) >
      CHANNEL_THRESHOLD
    ) {
      return true;
    }
  }
  return false;
}

function decodeRgbaPng(file: string, width: number, height: number): Uint8Array {
  let dimensions: string;
  try {
    dimensions = execFileSync("magick", ["identify", "-format", "%w %h", file], {
      encoding: "utf8",
    });
  } catch {
    throw new Error("PNG dimensions could not be read");
  }
  if (dimensions !== `${width} ${height}`) throw new Error("PNG dimensions differ from inventory");
  let pixels: Uint8Array;
  try {
    pixels = execFileSync("magick", [file, "-depth", "8", "rgba:-"], {
      maxBuffer: width * height * CHANNELS + 1024,
    });
  } catch {
    throw new Error("PNG could not be decoded as inventory-sized RGBA8");
  }
  if (pixels.length !== width * height * CHANNELS) {
    throw new Error("decoded PNG byte count differs from inventory dimensions");
  }
  return pixels;
}
