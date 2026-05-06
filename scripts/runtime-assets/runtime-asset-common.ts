import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

export type RuntimeAssetKind = "mermaid" | "drawio";

export interface RuntimeAssetDefinition {
  readonly kind: RuntimeAssetKind;
  readonly displayName: string;
  readonly version: string;
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
    version: "3.3.1",
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
    kind: "drawio",
    displayName: "Draw.io",
    version: "29.7.10",
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
];

export class RuntimeAssetCatalog {
  static all(): RuntimeAssetDefinition[] {
    return DEFINITIONS;
  }

  static byKind(kind: string): RuntimeAssetDefinition {
    const definition = DEFINITIONS.find((it) => it.kind === kind);
    if (definition === undefined) {
      throw new Error(`Unknown runtime asset: ${kind}`);
    }
    return definition;
  }
}

export class RuntimeAssetPaths {
  static vendorDir(definition: RuntimeAssetDefinition, version = definition.version): string {
    return path.join(
      "crates",
      "katana-canvas-forge",
      "vendor",
      definition.kind,
      version,
    );
  }

  static assetFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return path.join(RuntimeAssetPaths.vendorDir(definition, version), definition.fileName);
  }

  static checksumFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return `${RuntimeAssetPaths.assetFile(definition, version)}.sha256`;
  }

  static runtimeAssetsRust(): string {
    return path.join(
      "crates",
      "katana-canvas-forge",
      "src",
      "markdown",
      "runtime_assets.rs",
    );
  }

  static justfile(): string {
    return "Justfile";
  }
}

export class RuntimeAssetChecksum {
  static digestFile(filePath: string): string {
    const content = fs.readFileSync(filePath);
    return crypto.createHash("sha256").update(content).digest("hex");
  }

  static writeChecksumFile(definition: RuntimeAssetDefinition, version: string): string {
    const assetFile = RuntimeAssetPaths.assetFile(definition, version);
    const checksum = RuntimeAssetChecksum.digestFile(assetFile);
    const checksumFile = RuntimeAssetPaths.checksumFile(definition, version);
    fs.writeFileSync(checksumFile, `${checksum}  ${definition.fileName}\n`, "utf8");
    return checksum;
  }
}
