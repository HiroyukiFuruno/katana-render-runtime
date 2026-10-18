import fs from "node:fs";
import path from "node:path";
import type { Fixture } from "./diagram_update_fixtures";
import { verifyMermaidPair } from "./mermaid-pair-manifest";

const REFERENCE_IMAGE_EXTENSIONS = [".svg", ".rendered.svg", ".png", ".pair.json"] as const;

export class MarkdownReferenceAssets {
  constructor(
    private sourceDir: string,
    private targetDir: string,
  ) {}

  prepare() {
    fs.mkdirSync(this.targetDir, { recursive: true });
  }

  sync(fixture: Fixture) {
    verifyMermaidPair(this.sourceDir, fixture.slug, "official");
    const parent = fs.mkdtempSync(path.join(this.targetDir, `.pair-sync-${fixture.slug}-`));
    const staged = new Map<string, string>();
    const backups = new Map<string, string | null>();
    let committed = false;
    try {
      this.stagePair(fixture, parent, staged);
      verifyMermaidPair(parent, fixture.slug, "official");
      this.publishPair(fixture, parent, staged, backups);
      verifyMermaidPair(this.targetDir, fixture.slug, "official");
      committed = true;
    } catch (error) {
      const detail = error instanceof Error ? error.message : "unclassified reference-sync failure";
      try {
        rollbackAssets(backups, parent);
      } catch (rollbackError) {
        const rollbackDetail =
          rollbackError instanceof Error ? rollbackError.message : "unclassified rollback failure";
        throw new Error(
          `Mermaid reference sync failed; rollback failed and attempt retained at ${parent}: ${detail}; ${rollbackDetail}`,
        );
      }
      throw new Error(
        `Mermaid reference sync failed; previous pair restored and attempt retained at ${parent}: ${detail}`,
      );
    } finally {
      if (committed) fs.rmSync(parent, { recursive: true, force: true });
    }
  }

  private stagePair(fixture: Fixture, parent: string, staged: Map<string, string>) {
    for (const extension of REFERENCE_IMAGE_EXTENSIONS) {
      const target = this.targetPath(fixture, extension);
      const stage = path.join(parent, `${fixture.slug}${extension}`);
      fs.copyFileSync(this.sourcePath(fixture, extension), stage, fs.constants.COPYFILE_EXCL);
      staged.set(target, stage);
    }
  }

  private publishPair(
    fixture: Fixture,
    parent: string,
    staged: Map<string, string>,
    backups: Map<string, string | null>,
  ) {
    for (const extension of [".svg", ".rendered.svg", ".png", ".pair.json"] as const) {
      const target = this.targetPath(fixture, extension);
      backupAsset(target, path.join(parent, `${fixture.slug}.previous${extension}`), backups);
      const source = staged.get(target);
      if (source === undefined) throw new Error(`Missing staged Mermaid asset: ${target}`);
      if (extension !== ".pair.json") fs.renameSync(source, target);
    }
    const manifest = this.targetPath(fixture, ".pair.json");
    const stagedManifest = staged.get(manifest);
    if (stagedManifest === undefined) throw new Error(`Missing pair manifest: ${fixture.slug}`);
    fs.renameSync(stagedManifest, manifest);
  }

  hasReferences(fixture: Fixture): boolean {
    return this.outputPaths(fixture).some((outputPath) => fs.existsSync(outputPath));
  }

  capture(fixture: Fixture): ReferenceAssetsSnapshot {
    const files = new Map<string, Buffer>();
    for (const outputPath of this.outputPaths(fixture)) {
      if (fs.existsSync(outputPath)) {
        files.set(outputPath, fs.readFileSync(outputPath));
      }
    }
    return { files };
  }

  skipFailedFixture(fixture: Fixture, snapshot: ReferenceAssetsSnapshot): boolean {
    for (const outputPath of this.outputPaths(fixture)) {
      const previousContent = snapshot.files.get(outputPath);
      if (previousContent === undefined) {
        if (fs.existsSync(outputPath)) {
          fs.unlinkSync(outputPath);
        }
        continue;
      }
      fs.writeFileSync(outputPath, previousContent);
    }
    return snapshot.files.size === 0;
  }

  private outputPaths(fixture: Fixture): string[] {
    return [...new Set(this.extensionsFor(fixture).flatMap((it) => [it.source, it.target]))];
  }

  private extensionsFor(fixture: Fixture): ImagePathPair[] {
    return REFERENCE_IMAGE_EXTENSIONS.map((extension) => ({
      source: this.sourcePath(fixture, extension),
      target: this.targetPath(fixture, extension),
    }));
  }

  private sourcePath(fixture: Fixture, extension: string): string {
    return path.join(this.sourceDir, `${fixture.slug}${extension}`);
  }

  private targetPath(fixture: Fixture, extension: string): string {
    return path.join(this.targetDir, `${fixture.slug}${extension}`);
  }
}

function backupAsset(target: string, backup: string, backups: Map<string, string | null>) {
  if (!fs.existsSync(target)) {
    backups.set(target, null);
    return;
  }
  if (fs.lstatSync(target).isSymbolicLink()) {
    throw new Error(`Mermaid reference target must not be a symlink: ${target}`);
  }
  fs.renameSync(target, backup);
  backups.set(target, backup);
}

function rollbackAssets(backups: Map<string, string | null>, attemptDir: string) {
  for (const target of backups.keys()) {
    if (fs.existsSync(target)) {
      fs.renameSync(target, path.join(attemptDir, `failed-${path.basename(target)}`));
    }
  }
  for (const [target, backup] of backups) {
    if (backup !== null && fs.existsSync(backup)) fs.renameSync(backup, target);
  }
}

interface ImagePathPair {
  source: string;
  target: string;
}

export interface ReferenceAssetsSnapshot {
  files: Map<string, Buffer>;
}
