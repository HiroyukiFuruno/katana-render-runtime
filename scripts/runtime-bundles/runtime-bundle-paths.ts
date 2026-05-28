import path from "node:path";

export class RuntimeBundlePaths {
  constructor(private readonly root: string) {}

  generatedPath(outputFile: string): string {
    return path.join(
      this.root,
      "crates",
      "katana-render-runtime",
      "src",
      "markdown",
      "diagram_runtime",
      "generated",
      outputFile,
    );
  }

  checksumManifestPath(): string {
    return this.generatedPath("runtime-bundles.sha256");
  }

  relative(target: string): string {
    return path.relative(this.root, target);
  }

  resolve(relative: string): string {
    return path.join(this.root, relative);
  }
}
