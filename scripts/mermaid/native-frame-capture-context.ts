import fs from "node:fs";
import { isDeepStrictEqual } from "node:util";
import { z } from "zod";
import { type FontEnvironment, isFontEnvironment } from "../html/catalog-capture-producer-context";
import { sha256 } from "./mermaid-pair-manifest";
import { canonicalRegularFile } from "./native-build-receipt";
import { physicalFileIdentity } from "./raster-comparison-binding";
import { sameRasterFileIdentity } from "./raster-production-binding";

const filePinSchema = z
  .object({
    path: z.string().min(1),
    sha256: z.string().regex(/^[a-f0-9]{64}$/),
  })
  .strict();
const captureConfigSchema = z
  .object({
    schema: z.literal("krr-mermaid-native-frame-capture-config-v1"),
    theme: z.literal("dark"),
    frameSizeSource: z.literal("candidate-png"),
    scale: z.literal(1),
    scrollY: z.literal(0),
  })
  .strict();

export type NativeFrameCaptureConfig = z.infer<typeof captureConfigSchema>;
export type NativeFrameContextPin = z.infer<typeof filePinSchema>;

const fontEnvironmentKeys = [
  "cacheable",
  "inventorySha256",
  "missReasons",
  "platform",
  "pythonExecutableSha256",
  "pythonVersion",
  "recordCount",
  "schema",
].sort();

export function readNativeFrameCaptureConfig(pin: NativeFrameContextPin): NativeFrameCaptureConfig {
  return captureConfigSchema.parse(readPinnedJson(pin, "native frame capture config"));
}

export function readNativeFrameFontContext(pin: NativeFrameContextPin): FontEnvironment {
  return parseNativeFrameFontContext(readPinnedJson(pin, "native frame font context"));
}

export function assertNativeFrameFontContext(
  pin: NativeFrameContextPin,
  actual: FontEnvironment,
): void {
  if (!isDeepStrictEqual(readNativeFrameFontContext(pin), parseNativeFrameFontContext(actual))) {
    throw new Error("Native frame font context differs from the captured environment");
  }
}

function parseNativeFrameFontContext(value: unknown): FontEnvironment {
  if (
    !isFontEnvironment(value) ||
    value.cacheable !== true ||
    Object.keys(value).sort().join("\0") !== fontEnvironmentKeys.join("\0")
  ) {
    throw new Error("Native frame font context must be a cacheable canonical font environment");
  }
  return value;
}

function readPinnedJson(pinValue: NativeFrameContextPin, label: string): unknown {
  const pin = filePinSchema.parse(pinValue);
  canonicalRegularFile(pin.path, label);
  const before = physicalFileIdentity(pin.path);
  if (before.sha256 !== pin.sha256) throw new Error(`${label} SHA differs from its pin`);
  const bytes = fs.readFileSync(pin.path);
  const after = physicalFileIdentity(pin.path);
  if (
    sha256(bytes) !== pin.sha256 ||
    !sameRasterFileIdentity(before, after) ||
    after.sha256 !== pin.sha256
  ) {
    throw new Error(`${label} changed while being read`);
  }
  return JSON.parse(bytes.toString("utf8")) as unknown;
}
