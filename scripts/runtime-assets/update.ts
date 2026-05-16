import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DrawioWarExtractor } from "./drawio-war-extractor";
import {
  RuntimeAssetCatalog,
  RuntimeAssetCatalogSource,
  RuntimeAssetChecksum,
  type RuntimeAssetDefinition,
  RuntimeAssetPaths,
} from "./runtime-asset-common";

class RuntimeAssetDownloader {
  async download(definition: RuntimeAssetDefinition, version: string): Promise<void> {
    const target = RuntimeAssetPaths.assetFile(definition, version);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    if (definition.kind === "drawio") {
      await this.downloadDrawio(definition.downloadUrl(version), target);
      return;
    }
    await this.downloadFile(definition.downloadUrl(version), target);
  }

  private async downloadFile(url: string, target: string) {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`Failed to fetch ${url}: ${response.status}`);
    }
    const bytes = Buffer.from(await response.arrayBuffer());
    fs.writeFileSync(target, bytes);
  }

  private async downloadDrawio(url: string, target: string) {
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "kdr-drawio-"));
    const archive = path.join(tempDir, "draw.war");
    try {
      await this.downloadFile(url, archive);
      new DrawioWarExtractor().extract(archive, target, url);
    } finally {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
  }
}

export class RuntimeSourceUpdater {
  update(definition: RuntimeAssetDefinition, version: string, checksum: string) {
    this.updateRust(definition, version, checksum);
    this.updateMermaidZenumlReferences(definition, version);
    this.updateJustfile(definition, version);
    this.updateScriptCatalog(definition, version, checksum);
  }

  private updateRust(definition: RuntimeAssetDefinition, version: string, checksum: string) {
    const sourcePath = RuntimeAssetPaths.runtimeAssetsRust();
    let source = fs.readFileSync(sourcePath, "utf8");
    source = this.replaceConst(source, definition.rustVersionConst, version);
    source = this.replaceConst(source, definition.rustChecksumConst, checksum);
    source = this.replaceConst(
      source,
      definition.rustDownloadConst,
      definition.releasePageUrl(version),
    );
    if (definition.kind !== "mermaid-zenuml") {
      source = this.replaceVendorAssetVersion(source, definition, version);
    }
    fs.writeFileSync(sourcePath, source, "utf8");
  }

  private updateMermaidZenumlReferences(definition: RuntimeAssetDefinition, version: string) {
    if (definition.kind !== "mermaid-zenuml") {
      return;
    }
    this.updateVendorReference(RuntimeAssetPaths.mermaidRuntimeScriptsRust(), definition, version);
    this.updateVendorReference(RuntimeAssetPaths.mermaidDiagramUpdateScript(), definition, version);
  }

  private updateVendorReference(
    sourcePath: string,
    definition: RuntimeAssetDefinition,
    version: string,
  ) {
    const source = fs.readFileSync(sourcePath, "utf8");
    const updated = this.replaceVendorAssetVersion(source, definition, version);
    fs.writeFileSync(sourcePath, updated, "utf8");
  }

  private updateJustfile(definition: RuntimeAssetDefinition, version: string) {
    const justfile = RuntimeAssetPaths.justfile();
    const constName = RuntimeAssetPaths.justVersionVariable(definition);
    const pattern = new RegExp(`${constName} := "[^"]+"`);
    const source = fs
      .readFileSync(justfile, "utf8")
      .replace(pattern, `${constName} := "${version}"`);
    fs.writeFileSync(justfile, source, "utf8");
  }

  private updateScriptCatalog(
    definition: RuntimeAssetDefinition,
    version: string,
    checksum: string,
  ) {
    const catalogPath = RuntimeAssetPaths.runtimeAssetCommon();
    const source = fs.readFileSync(catalogPath, "utf8");
    const updated = RuntimeAssetCatalogSource.updatePinnedAsset(
      source,
      definition.kind,
      version,
      checksum,
    );
    fs.writeFileSync(catalogPath, updated, "utf8");
  }

  replaceConst(source: string, constName: string, value: string): string {
    const replacement =
      value.length > 56
        ? `pub const ${constName}: &str =\n    "${value}";`
        : `pub const ${constName}: &str = "${value}";`;
    const pattern = new RegExp(`pub const ${constName}: &str =\\s*"[^"]+";`);
    return source.replace(pattern, replacement);
  }

  replaceVendorAssetVersion(
    source: string,
    definition: RuntimeAssetDefinition,
    version: string,
  ): string {
    const kind = this.escapePattern(definition.kind);
    const fileName = this.escapePattern(definition.fileName);
    const pattern = new RegExp(`(vendor/${kind}/)[^/]+(/${fileName})`);
    if (!pattern.test(source)) {
      throw new Error(`Runtime asset include path not found: ${definition.kind}`);
    }
    return source.replace(pattern, `$1${version}$2`);
  }

  private escapePattern(value: string): string {
    return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }
}

class UpdateCommand {
  constructor(
    private definition: RuntimeAssetDefinition,
    private version: string,
  ) {}

  async run() {
    await new RuntimeAssetDownloader().download(this.definition, this.version);
    const checksum = RuntimeAssetChecksum.writeChecksumFile(this.definition, this.version);
    new RuntimeSourceUpdater().update(this.definition, this.version, checksum);
    console.log(`updated ${RuntimeAssetPaths.assetFile(this.definition, this.version)}`);
    console.log(`sha256=${checksum}`);
  }
}

const CliOptions = {
  command(argv: string[]): UpdateCommand {
    if (argv.length !== 2) {
      throw new Error(
        "Usage: bun run scripts/runtime-assets/update.ts <mermaid|mermaid-zenuml|drawio> <version>",
      );
    }
    return new UpdateCommand(
      RuntimeAssetCatalog.byKind(CliOptions.argAt(argv, 0)),
      CliOptions.argAt(argv, 1),
    );
  },

  argAt(argv: string[], index: number): string {
    const value = argv.at(index);
    if (value === undefined) {
      throw new Error("Runtime asset update argument is missing");
    }
    return value;
  },
};

if (import.meta.main) {
  await CliOptions.command(process.argv.slice(2)).run();
}
