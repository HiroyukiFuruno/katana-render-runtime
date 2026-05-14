import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
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
      const extracted = spawnSync("unzip", ["-p", archive, "js/app.min.js"], {
        encoding: "buffer",
      });
      if (extracted.status !== 0) {
        throw new Error(`Failed to extract js/app.min.js from ${url}`);
      }
      fs.writeFileSync(target, extracted.stdout);
    } finally {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
  }
}

class RuntimeSourceUpdater {
  update(definition: RuntimeAssetDefinition, version: string, checksum: string) {
    this.updateRust(definition, version, checksum);
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
    source = this.replaceIncludePath(source, definition, version);
    fs.writeFileSync(sourcePath, source, "utf8");
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

  private replaceConst(source: string, constName: string, value: string): string {
    const replacement =
      value.length > 56
        ? `pub const ${constName}: &str =\n    "${value}";`
        : `pub const ${constName}: &str = "${value}";`;
    const pattern = new RegExp(`pub const ${constName}: &str =(?:\\n\\s*)?"[^"]+";`);
    return source.replace(pattern, replacement);
  }

  private replaceIncludePath(
    source: string,
    definition: RuntimeAssetDefinition,
    version: string,
  ): string {
    const pattern = new RegExp(
      `include_bytes!\\("\\.\\./\\.\\./vendor/${definition.kind}/[^/]+/${definition.fileName}"\\)`,
    );
    const replacement = `include_bytes!("../../vendor/${definition.kind}/${version}/${definition.fileName}")`;
    return source.replace(pattern, replacement);
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

await CliOptions.command(process.argv.slice(2)).run();
