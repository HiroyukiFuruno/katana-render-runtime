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
    version: "12.0.0",
    checksum: "28fca7ae6ebc7ed7bb63bde63136a74bfef14f296a57e403657eeb8b32836073",
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
    version: "1.0.0",
    checksum: "f75662869d22aac58870e52370fe4c46b89b9710bf4b03df93b3227b5e18e730",
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
    version: "4.3.0",
    checksum: "7e48bc62d5ef65d2f30747da6dc7da01b7175ff9c637dda066a4b92c2d8bdf4d",
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
    version: "31.4.5",
    checksum: "ec8b66e744a9e5dbf3870129c297385595f55a548f75b952f209548d49610536",
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
    version: "1.2026.8",
    checksum: "1057dd8b346bed26a48ffebe6054e16fc785dda7c91f37f6c19030a4aab8a942",
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
