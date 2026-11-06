import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileSha256 } from "../mermaid/mermaid-pair-manifest";
import { digestFileRecords, type FileRecord } from "../mermaid/native-build-receipt";
import { collectRuntimeBundleToolFingerprint } from "../runtime-bundles/runtime-bundle-tool-fingerprint";
import {
  assertCatalogProducerContext,
  type CatalogCaptureProducerContext,
  catalogProducerContextChanges,
  sameCatalogProducerContext,
} from "./catalog-capture-producer-context";

function fixtureContext(
  directory: string,
  role: "before" | "after",
): CatalogCaptureProducerContext {
  const sourceFile: FileRecord = {
    path: process.execPath,
    sha256: fileSha256(process.execPath),
    size: fs.statSync(process.execPath).size,
  };
  const sourceManifestPath = path.join(directory, `source-${role}.json`);
  const sourceManifest = {
    schema: "krr-native-build-source-manifest-v1",
    digest: digestFileRecords([sourceFile]),
    files: [sourceFile],
  };
  fs.writeFileSync(sourceManifestPath, `${JSON.stringify(sourceManifest)}\n`, { flag: "wx" });
  return {
    schema: "krr-html84-capture-producer-context-v1",
    role,
    sourceManifest: {
      path: sourceManifestPath,
      sha256: fileSha256(sourceManifestPath),
      digest: sourceManifest.digest,
    },
    tools: collectRuntimeBundleToolFingerprint(),
    playwrightFiles: [],
    os: { release: "test", version: "test" },
    fontEnvironment: {
      schema: "krr-font-environment-v1",
      cacheable: true,
      platform: "test",
      inventorySha256: "1".repeat(64),
      recordCount: 1,
      missReasons: [],
      pythonVersion: "3.14.3",
      pythonExecutableSha256: "2".repeat(64),
    },
  };
}

test("catalog producer context verifies source, tools and font identities", () => {
  const outputDirectory = fs.mkdtempSync(path.resolve("tmp/catalog-producer-context-test-"));
  try {
    const before = fixtureContext(outputDirectory, "before");
    expect(before.sourceManifest.digest).toMatch(/^[0-9a-f]{64}$/);
    expect(assertCatalogProducerContext(before)).toEqual(before);
    expect(sameCatalogProducerContext(before, { ...before, role: "after" })).toBe(true);
    expect(
      sameCatalogProducerContext(before, {
        ...before,
        os: { ...before.os, version: `${before.os.version}-changed` },
      }),
    ).toBe(false);
    expect(
      catalogProducerContextChanges(before, {
        ...before,
        fontEnvironment: {
          ...before.fontEnvironment,
          inventorySha256: "0".repeat(64),
        },
      }),
    ).toEqual(["fontEnvironment"]);
    const { fontEnvironment: _fontEnvironment, ...legacyContext } = before;
    expect(() => assertCatalogProducerContext(legacyContext)).toThrow(
      /invalid catalog capture producer context/,
    );
    const { pythonExecutableSha256: _pythonSha, ...unsupportedFont } = before.fontEnvironment ?? {};
    expect(() =>
      assertCatalogProducerContext({
        ...before,
        fontEnvironment: {
          ...unsupportedFont,
          cacheable: false,
          inventorySha256: "unavailable",
          recordCount: 0,
          missReasons: ["unsupported_host_profile"],
        },
      }),
    ).not.toThrow();
  } finally {
    fs.rmSync(outputDirectory, { recursive: true, force: true });
  }
});
