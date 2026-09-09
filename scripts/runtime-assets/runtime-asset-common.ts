import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

export type RuntimeAssetKind =
  | "mermaid"
  | "mermaid-zenuml"
  | "zenuml-core"
  | "drawio"
  | "mathjax"
  | "plantuml";

export interface RuntimeAssetDefinition {
  readonly kind: RuntimeAssetKind;
  readonly displayName: string;
  readonly version: string;
  readonly checksum: string;
  readonly fileName: string;
  readonly rustVersionConst: string;
  readonly rustChecksumConst: string;
  readonly rustDownloadConst: string;
  readonly latestUrl: string;
  releasePageUrl(version: string): string;
  downloadUrl(version: string): string;
}

const DEFINITIONS: RuntimeAssetDefinition[] = [
  {
    kind: "mermaid",
    displayName: "Mermaid.js",
    version: "11.17.2",
    checksum: "581ed7d74bd9048d0e3a91363927d72ef22942d7722546b27f7cc29e35390eb8",
    fileName: "mermaid.min.js",
    rustVersionConst: "MERMAID_JS_VERSION",
    rustChecksumConst: "MERMAID_JS_CHECKSUM",
    rustDownloadConst: "MERMAID_DOWNLOAD_URL",
    latestUrl: "https://registry.npmjs.org/mermaid/latest",
    releasePageUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/mermaid@${version}/dist/mermaid.min.js`,
    downloadUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/mermaid@${version}/dist/mermaid.min.js`,
  },
  {
    kind: "mermaid-zenuml",
    displayName: "Mermaid ZenUML",
    version: "0.2.3",
    checksum: "28eeec88021d9e9728df4d005ff723a3d71da29a21dbcfa2a628232c35ef2ab6",
    fileName: "mermaid-zenuml.min.js",
    rustVersionConst: "MERMAID_ZENUML_JS_VERSION",
    rustChecksumConst: "MERMAID_ZENUML_JS_CHECKSUM",
    rustDownloadConst: "MERMAID_ZENUML_DOWNLOAD_URL",
    latestUrl: "https://registry.npmjs.org/@mermaid-js/mermaid-zenuml/latest",
    releasePageUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid-zenuml@${version}/dist/mermaid-zenuml.min.js`,
    downloadUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid-zenuml@${version}/dist/mermaid-zenuml.min.js`,
  },
  {
    kind: "zenuml-core",
    displayName: "ZenUML Core",
    version: "4.2.1",
    checksum: "554828af031325ae41887e9ec26468f2fa89cb29e8bf096d51e4909d72e69cdf",
    fileName: "zenuml.js",
    rustVersionConst: "ZENUML_CORE_JS_VERSION",
    rustChecksumConst: "ZENUML_CORE_JS_CHECKSUM",
    rustDownloadConst: "ZENUML_CORE_DOWNLOAD_URL",
    latestUrl: "https://registry.npmjs.org/@zenuml/core/latest",
    releasePageUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/@zenuml/core@${version}/dist/zenuml.js`,
    downloadUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/@zenuml/core@${version}/dist/zenuml.js`,
  },
  {
    kind: "drawio",
    displayName: "Draw.io",
    version: "31.4.2",
    checksum: "3810b944463d342daa438e0bf7793addbcc96531269c4aea26907142b1a38d0a",
    fileName: "drawio.min.js",
    rustVersionConst: "DRAWIO_JS_VERSION",
    rustChecksumConst: "DRAWIO_JS_CHECKSUM",
    rustDownloadConst: "DRAWIO_DOWNLOAD_URL",
    latestUrl: "https://api.github.com/repos/jgraph/drawio/releases/latest",
    releasePageUrl: (version: string) =>
      `https://github.com/jgraph/drawio/releases/tag/v${version}`,
    downloadUrl: (version: string) =>
      `https://github.com/jgraph/drawio/releases/download/v${version}/draw.war`,
  },
  {
    kind: "mathjax",
    displayName: "MathJax",
    version: "4.1.3",
    checksum: "23c036deccc0f2374834a47e4032e452419f3ac027bf17e17c104e2746b19f4c",
    fileName: "tex-svg.js",
    rustVersionConst: "MATHJAX_JS_VERSION",
    rustChecksumConst: "MATHJAX_JS_CHECKSUM",
    rustDownloadConst: "MATHJAX_DOWNLOAD_URL",
    latestUrl: "https://registry.npmjs.org/mathjax/latest",
    releasePageUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/mathjax@${version}/tex-svg.js`,
    downloadUrl: (version: string) => `https://cdn.jsdelivr.net/npm/mathjax@${version}/tex-svg.js`,
  },
  {
    kind: "plantuml",
    displayName: "PlantUML JAR",
    version: "1.2026.7",
    checksum: "1eb8cd1d0253227f3652586bc3b53cb3d5cfe69b5dcca41ce9b92ab1ce4f58ff",
    fileName: "plantuml.jar",
    rustVersionConst: "PLANTUML_JAR_VERSION",
    rustChecksumConst: "PLANTUML_JAR_CHECKSUM",
    rustDownloadConst: "PLANTUML_DOWNLOAD_URL",
    latestUrl:
      "https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/maven-metadata.xml",
    releasePageUrl: (version: string) =>
      `https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/${version}/plantuml-lgpl-${version}.jar`,
    downloadUrl: (version: string) =>
      `https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/${version}/plantuml-lgpl-${version}.jar`,
  },
];

export const RuntimeAssetCatalog = {
  all(): RuntimeAssetDefinition[] {
    return DEFINITIONS;
  },

  byKind(kind: string): RuntimeAssetDefinition {
    const definition = DEFINITIONS.find((it) => it.kind === kind);
    if (definition === undefined) {
      throw new Error(`Unknown runtime asset: ${kind}`);
    }
    return definition;
  },
};

export const RuntimeAssetCatalogSource = {
  updatePinnedAsset(
    source: string,
    kind: RuntimeAssetKind,
    version: string,
    checksum: string,
  ): string {
    let updated = RuntimeAssetCatalogSource.replaceString(source, kind, "version", version);
    updated = RuntimeAssetCatalogSource.replaceString(updated, kind, "checksum", checksum);
    return updated;
  },

  replaceString(
    source: string,
    kind: RuntimeAssetKind,
    propertyName: "version" | "checksum",
    value: string,
  ): string {
    const blockPattern = new RegExp(`(\\{\\n\\s+kind: "${kind}",[\\s\\S]*?\\n\\s+\\},)`);
    const block = source.match(blockPattern)?.[1];
    if (block === undefined) {
      throw new Error(`Runtime asset catalog entry not found: ${kind}`);
    }
    const propertyPattern = new RegExp(`(\\s+${propertyName}: )"[^"]+"`);
    if (!propertyPattern.test(block)) {
      throw new Error(`Runtime asset catalog property not found: ${kind}.${propertyName}`);
    }
    return source.replace(block, block.replace(propertyPattern, `$1"${value}"`));
  },
};

export const RuntimeAssetPaths = {
  vendorDir(definition: RuntimeAssetDefinition, version = definition.version): string {
    return path.join("crates", "katana-render-runtime", "vendor", definition.kind, version);
  },

  assetFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return path.join(RuntimeAssetPaths.vendorDir(definition, version), definition.fileName);
  },

  checksumFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return `${RuntimeAssetPaths.assetFile(definition, version)}.sha256`;
  },

  compressedAssetFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return `${RuntimeAssetPaths.assetFile(definition, version)}.br`;
  },

  justVersionVariable(definition: RuntimeAssetDefinition): string {
    if (definition.kind === "plantuml") {
      return "PLANTUML_JAR_VERSION";
    }
    return `${definition.kind.toUpperCase().replaceAll("-", "_")}_JS_VERSION`;
  },

  runtimeAssetsRust(): string {
    return path.join("crates", "katana-render-runtime", "src", "markdown", "runtime_assets.rs");
  },

  plantumlAssetRust(): string {
    return path.join(
      "crates",
      "katana-render-runtime",
      "src",
      "markdown",
      "plantuml_renderer",
      "asset.rs",
    );
  },

  rendererCargoToml(): string {
    return path.join("crates", "katana-render-runtime", "Cargo.toml");
  },

  mermaidDiagramUpdateScript(): string {
    return path.join("scripts", "mermaid", "diagram-update.ts");
  },

  justfile(): string {
    return "Justfile";
  },

  runtimeAssetCommon(): string {
    return path.join("scripts", "runtime-assets", "runtime-asset-common.ts");
  },
};

export const RuntimeAssetChecksum = {
  digestFile(filePath: string): string {
    const content = fs.readFileSync(filePath);
    return crypto.createHash("sha256").update(content).digest("hex");
  },

  readChecksumFile(filePath: string): string {
    const content = fs.readFileSync(filePath, "utf8").trim();
    const checksum = content.split(/\s+/).at(0);
    if (checksum === undefined) {
      throw new Error(`Runtime asset checksum file is empty: ${filePath}`);
    }
    return checksum;
  },

  writeChecksumFile(definition: RuntimeAssetDefinition, version: string): string {
    const assetFile = RuntimeAssetPaths.assetFile(definition, version);
    const checksum = RuntimeAssetChecksum.digestFile(assetFile);
    const checksumFile = RuntimeAssetPaths.checksumFile(definition, version);
    fs.writeFileSync(checksumFile, `${checksum}  ${definition.fileName}\n`, "utf8");
    return checksum;
  },
};
