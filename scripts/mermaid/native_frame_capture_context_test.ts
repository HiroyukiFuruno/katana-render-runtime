import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { FontEnvironment } from "../html/catalog-capture-producer-context";
import { fileSha256 } from "./mermaid-pair-manifest";
import {
  assertNativeFrameFontContext,
  type NativeFrameCaptureConfig,
  type NativeFrameContextPin,
  readNativeFrameCaptureConfig,
  readNativeFrameFontContext,
} from "./native-frame-capture-context";

function fixture() {
  const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-frame-context-"));
  const directory = fs.realpathSync(temporaryDirectory);
  const write = (name: string, value: unknown): NativeFrameContextPin => {
    const file = path.join(directory, name);
    fs.writeFileSync(file, `${JSON.stringify(value)}\n`, { flag: "wx", mode: 0o600 });
    return { path: file, sha256: fileSha256(file) };
  };
  return { directory, write };
}

function withFixture(run: (temp: ReturnType<typeof fixture>) => void): void {
  const temp = fixture();
  try {
    run(temp);
  } finally {
    fs.rmSync(temp.directory, { recursive: true, force: true });
  }
}

function validConfig(): NativeFrameCaptureConfig {
  return {
    schema: "krr-mermaid-native-frame-capture-config-v1",
    theme: "dark",
    frameSizeSource: "candidate-png",
    scale: 1,
    scrollY: 0,
  };
}

function validFontContext(): FontEnvironment {
  return {
    schema: "krr-font-environment-v1",
    cacheable: true,
    platform: "Linux",
    inventorySha256: "a".repeat(64),
    recordCount: 3,
    missReasons: [],
    pythonVersion: "3.12.0",
    pythonExecutableSha256: "b".repeat(64),
  };
}

test("reads only the exact native frame capture config", () => {
  withFixture((temp) => {
    const defaultsPath = new URL("./native-frame-capture-defaults.json", import.meta.url);
    const defaults = JSON.parse(fs.readFileSync(defaultsPath, "utf8")) as unknown;
    expect(defaults).toEqual(validConfig());
    const good = temp.write("config.json", defaults);
    expect(readNativeFrameCaptureConfig(good)).toEqual(validConfig());
    const invalidConfigs = [
      "arbitrary text",
      { ...validConfig(), theme: "light" },
      { ...validConfig(), scale: 2 },
      { ...validConfig(), scrollY: 1 },
      { ...validConfig(), extra: true },
    ];
    for (const [index, changed] of invalidConfigs.entries()) {
      const pin = temp.write(`invalid-${index}.json`, changed);
      expect(() => readNativeFrameCaptureConfig(pin)).toThrow();
    }
    expect(() => readNativeFrameCaptureConfig({ ...good, sha256: "Z".repeat(64) })).toThrow();
    expect(() => readNativeFrameCaptureConfig({ ...good, sha256: "a".repeat(64) })).toThrow();
  });
});

test("requires a cacheable canonical eight-field font context", () => {
  withFixture((temp) => {
    const context = validFontContext();
    const pin = temp.write("font-context.json", context);
    expect(readNativeFrameFontContext(pin)).toEqual(context);
    expect(() =>
      readNativeFrameFontContext(temp.write("arbitrary.json", "font context")),
    ).toThrow();
    expect(() =>
      readNativeFrameFontContext(
        temp.write("not-cacheable.json", { ...context, cacheable: false }),
      ),
    ).toThrow();
    expect(() =>
      readNativeFrameFontContext(temp.write("extra-field.json", { ...context, extra: true })),
    ).toThrow();
    expect(() => readNativeFrameFontContext({ ...pin, sha256: "invalid" })).toThrow();
  });
});

test("matches the actual snapshot independent of JSON field order", () => {
  withFixture((temp) => {
    const actual = validFontContext();
    const reordered = {
      pythonExecutableSha256: actual.pythonExecutableSha256,
      pythonVersion: actual.pythonVersion,
      missReasons: actual.missReasons,
      recordCount: actual.recordCount,
      inventorySha256: actual.inventorySha256,
      platform: actual.platform,
      cacheable: actual.cacheable,
      schema: actual.schema,
    };
    const pin = temp.write("font-context.json", reordered);
    expect(() => assertNativeFrameFontContext(pin, actual)).not.toThrow();
    expect(() =>
      assertNativeFrameFontContext(pin, { ...actual, inventorySha256: "c".repeat(64) }),
    ).toThrow();
  });
});
