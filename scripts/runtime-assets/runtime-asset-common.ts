import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

export type RuntimeAssetKind = "mermaid" | "mermaid-zenuml" | "drawio" | "plantuml";

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
    version: "11.15.0",
    checksum: "70137e77bb273bb2ef972b86e8b0400cca8be53cb25bfc45911a186dc98665de",
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
    kind: "drawio",
    displayName: "Draw.io",
    version: "30.0.2",
    checksum: "0435d7a829549490482d576a37556224fa190d538610c96908632e5cda7c601f",
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
    kind: "plantuml",
    displayName: "PlantUML JAR",
    version: "1.2026.4",
    checksum: "1783d4569855f2f0a17e65bd192add377c7f2b5e3e1781b65dc94d084de98699",
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
    return path.join("crates", "katana-diagram-renderer", "vendor", definition.kind, version);
  },

  assetFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return path.join(RuntimeAssetPaths.vendorDir(definition, version), definition.fileName);
  },

  checksumFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return `${RuntimeAssetPaths.assetFile(definition, version)}.sha256`;
  },

  justVersionVariable(definition: RuntimeAssetDefinition): string {
    if (definition.kind === "plantuml") {
      return "PLANTUML_JAR_VERSION";
    }
    return `${definition.kind.toUpperCase().replaceAll("-", "_")}_JS_VERSION`;
  },

  runtimeAssetsRust(): string {
    return path.join("crates", "katana-diagram-renderer", "src", "markdown", "runtime_assets.rs");
  },

  plantumlAssetRust(): string {
    return path.join(
      "crates",
      "katana-diagram-renderer",
      "src",
      "markdown",
      "plantuml_renderer",
      "asset.rs",
    );
  },

  rendererCargoToml(): string {
    return path.join("crates", "katana-diagram-renderer", "Cargo.toml");
  },

  mermaidRuntimeScriptsRust(): string {
    return path.join(
      "crates",
      "katana-diagram-renderer",
      "src",
      "markdown",
      "mermaid_renderer",
      "js_runtime_scripts.rs",
    );
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
