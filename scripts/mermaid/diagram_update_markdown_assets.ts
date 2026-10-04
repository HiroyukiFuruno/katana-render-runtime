import fs from "node:fs";
import path from "node:path";
import type { Fixture } from "./diagram_update_fixtures";

const REFERENCE_IMAGE_EXTENSIONS = [".png", ".svg"] as const;

export class MarkdownReferenceAssets {
  constructor(
    private sourceDir: string,
    private targetDir: string,
  ) {}

  prepare() {
    fs.mkdirSync(this.targetDir, { recursive: true });
  }

  sync(fixture: Fixture) {
    for (const extension of REFERENCE_IMAGE_EXTENSIONS) {
      fs.copyFileSync(this.sourcePath(fixture, extension), this.targetPath(fixture, extension));
    }
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

interface ImagePathPair {
  source: string;
  target: string;
}

export interface ReferenceAssetsSnapshot {
  files: Map<string, Buffer>;
}
