import { expect, test } from "bun:test";
import fs from "node:fs";
import {
  isPatchedRuntimeAsset,
  RuntimeAssetCatalog,
  RuntimeAssetCatalogSource,
  RuntimeAssetChecksum,
  RuntimeAssetPaths,
  runtimeAssetFreshnessEntries,
} from "./runtime-asset-common";

test("Mermaid patched asset keeps upstream version separate from artifact version", () => {
  const mermaid = RuntimeAssetCatalog.byKind("mermaid");
  const source = mermaid.source;
  if (source === undefined) throw new Error("Mermaid patched source metadata is missing");
  expect(mermaid.version).toBe("12.3.0-krr.pr6823.1");
  expect(source.upstreamVersion).toBe("12.3.0");
  expect(source.immutableUrl).toBe(
    "https://raw.githubusercontent.com/HiroyukiFuruno/katana-render-runtime/v0.4.24/crates/katana-render-runtime/vendor/mermaid/12.3.0-krr.pr6823.1/mermaid.min.js",
  );
  expect(isPatchedRuntimeAsset(mermaid)).toBe(true);
  expect(mermaid.downloadUrl(mermaid.version)).toBe(source.immutableUrl);
});

test("runtime asset path は version を含む", () => {
  const mermaid = RuntimeAssetCatalog.byKind("mermaid");
  const zenuml = RuntimeAssetCatalog.byKind("mermaid-zenuml");
  const zenumlCore = RuntimeAssetCatalog.byKind("zenuml-core");
  const drawio = RuntimeAssetCatalog.byKind("drawio");
  const mathjax = RuntimeAssetCatalog.byKind("mathjax");
  const plantuml = RuntimeAssetCatalog.byKind("plantuml");

  expect(RuntimeAssetPaths.assetFile(mermaid)).toBe(
    `crates/katana-render-runtime/vendor/mermaid/${mermaid.version}/mermaid.min.js`,
  );
  expect(RuntimeAssetPaths.assetFile(zenuml)).toBe(
    `crates/katana-render-runtime/vendor/mermaid-zenuml/${zenuml.version}/mermaid-zenuml.min.js`,
  );
  expect(RuntimeAssetPaths.assetFile(zenumlCore)).toBe(
    `crates/katana-render-runtime/vendor/zenuml-core/${zenumlCore.version}/zenuml.js`,
  );
  expect(RuntimeAssetPaths.assetFile(drawio)).toBe(
    `crates/katana-render-runtime/vendor/drawio/${drawio.version}/drawio.min.js`,
  );
  expect(RuntimeAssetPaths.compressedAssetFile(drawio)).toBe(
    "crates/katana-render-runtime-assets/assets/drawio.min.js.br",
  );
  expect(RuntimeAssetPaths.compressedAssetFile(drawio, "33.0.0")).toBe(
    "crates/katana-render-runtime-assets/assets/drawio.min.js.br",
  );
  expect(RuntimeAssetPaths.assetFile(mathjax)).toBe(
    `crates/katana-render-runtime/vendor/mathjax/${mathjax.version}/tex-svg.js`,
  );
  expect(RuntimeAssetPaths.assetFile(plantuml)).toBe(
    `crates/katana-render-runtime/vendor/plantuml/${plantuml.version}/plantuml.jar`,
  );
  expect(RuntimeAssetPaths.justVersionVariable(zenuml)).toBe("MERMAID_ZENUML_JS_VERSION");
  expect(RuntimeAssetPaths.justVersionVariable(zenumlCore)).toBe("ZENUML_CORE_JS_VERSION");
  expect(RuntimeAssetPaths.justVersionVariable(plantuml)).toBe("PLANTUML_JAR_VERSION");
});

test("runtime asset checksum は固定ファイルと一致する", () => {
  const mermaid = RuntimeAssetCatalog.byKind("mermaid");
  const zenuml = RuntimeAssetCatalog.byKind("mermaid-zenuml");
  const zenumlCore = RuntimeAssetCatalog.byKind("zenuml-core");
  const drawio = RuntimeAssetCatalog.byKind("drawio");
  const mathjax = RuntimeAssetCatalog.byKind("mathjax");
  const plantuml = RuntimeAssetCatalog.byKind("plantuml");

  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(mermaid))).toBe(
    mermaid.checksum,
  );
  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(zenuml))).toBe(
    zenuml.checksum,
  );
  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(zenumlCore))).toBe(
    zenumlCore.checksum,
  );
  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(drawio))).toBe(
    drawio.checksum,
  );
  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(mathjax))).toBe(
    mathjax.checksum,
  );
  expect(RuntimeAssetChecksum.readChecksumFile(RuntimeAssetPaths.checksumFile(plantuml))).toBe(
    plantuml.checksum,
  );
});

test("runtime asset catalog source は対象 asset の固定値だけ更新する", () => {
  const source = [
    "const DEFINITIONS: RuntimeAssetDefinition[] = [",
    "  {",
    '    kind: "mermaid",',
    '    version: "3.3.1",',
    '    checksum: "old-mermaid-checksum",',
    "  },",
    "  {",
    '    kind: "drawio",',
    '    version: "29.7.10",',
    '    checksum: "old-drawio-checksum",',
    "  },",
    "];",
  ].join("\n");

  const updated = RuntimeAssetCatalogSource.updatePinnedAsset(
    source,
    "mermaid",
    "4.0.0",
    "new-mermaid-checksum",
  );

  expect(updated).toContain('kind: "mermaid",\n    version: "4.0.0",');
  expect(updated).toContain('checksum: "new-mermaid-checksum",');
  expect(updated).toContain('kind: "drawio",\n    version: "29.7.10",');
  expect(updated).toContain('checksum: "old-drawio-checksum",');
});

test("runtime asset freshness AST reads the six direct catalog entries and upstream version", () => {
  const source = fs.readFileSync(new URL("./runtime-asset-common.ts", import.meta.url), "utf8");
  const entries = runtimeAssetFreshnessEntries(source);

  expect(entries).toHaveLength(6);
  expect(entries.map(({ kind }) => kind)).toEqual([
    "mermaid",
    "mermaid-zenuml",
    "zenuml-core",
    "drawio",
    "mathjax",
    "plantuml",
  ]);
  expect(entries[0]).toEqual({
    kind: "mermaid",
    version: "12.3.0",
    latestUrl: "https://registry.npmjs.org/mermaid/latest",
  });
  expect(entries[1]?.version).toBe("1.0.1");
});

test("runtime asset freshness AST ignores interface and nested source kind lookalikes", () => {
  const source = [
    'interface RuntimeAssetDefinition { source?: { kind: "patched-upstream"; upstreamVersion: string }; }',
    'const helper = { kind: "patched-upstream", upstreamVersion: "fake-helper" };',
    'function nested() { const DEFINITIONS = [{ kind: "nested-fake", version: "0", latestUrl: "fake" }]; }',
    "const DEFINITIONS = [",
    '  { kind: "catalog-item", version: "artifact-2.0", latestUrl: "https://example.test/latest",',
    '    source: { kind: "patched-upstream", upstreamVersion: "upstream-1.0" } },',
    "];",
  ].join("\n");

  expect(runtimeAssetFreshnessEntries(source)).toEqual([
    { kind: "catalog-item", version: "upstream-1.0", latestUrl: "https://example.test/latest" },
  ]);
});

test("runtime asset freshness AST fails closed for malformed or ambiguous catalog shapes", () => {
  const asset = (fields: string) => `{ kind: "sample", ${fields} }`;
  const catalog = (elements: string, prefix = "") =>
    `${prefix}\nconst DEFINITIONS = [${elements}];`;
  const valid = asset('version: "1.0", latestUrl: "https://example.test/latest"');
  const invalidSources = [
    ["empty table", catalog("")],
    ["missing required version", catalog(asset('latestUrl: "https://example.test/latest"'))],
    [
      "empty required version",
      catalog(asset('version: "", latestUrl: "https://example.test/latest"')),
    ],
    [
      "nonliteral version",
      catalog(asset('version: VERSION, latestUrl: "https://example.test/latest"')),
    ],
    ["spread asset", catalog(`...${valid}`)],
    [
      "spread field",
      catalog(asset('version: "1.0", latestUrl: "https://example.test/latest", ...extra')),
    ],
    [
      "duplicate key",
      catalog(asset('version: "1.0", version: "2.0", latestUrl: "https://example.test/latest"')),
    ],
    ["duplicate kind", catalog(`${valid}, ${valid}`)],
    [
      "patched upstream is nonliteral",
      catalog(
        asset(
          'version: "1.0", latestUrl: "https://example.test/latest", source: { kind: "patched-upstream", upstreamVersion: upstream }',
        ),
      ),
    ],
    [
      "patched source spread",
      catalog(
        asset(
          'version: "1.0", latestUrl: "https://example.test/latest", source: { ...base, kind: "patched-upstream", upstreamVersion: "1.0" }',
        ),
      ),
    ],
    ["DEFINITIONS missing", "const OTHER = [];"],
    ["nested-only DEFINITIONS", "function nested() { const DEFINITIONS = []; }"],
    ["DEFINITIONS ambiguous", catalog(valid, `const DEFINITIONS = [${valid}];`)],
    ["invalid syntax", 'const DEFINITIONS = [{ kind: "sample", version: "1", latestUrl: ];'],
  ] as const;

  for (const [label, source] of invalidSources) {
    expect(() => runtimeAssetFreshnessEntries(source), label).toThrow();
  }
});
