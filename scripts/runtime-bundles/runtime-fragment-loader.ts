import fs from "node:fs";
import path from "node:path";
import type { RuntimeFragment, RuntimeFragmentTransform } from "#shared/runtime_bundle";

export class RuntimeFragmentLoader {
  constructor(private readonly root: string) {}

  load(fragment: RuntimeFragment): string {
    const sourcePath = path.join(this.root, fragment.sourcePath);
    const source = fs.readFileSync(sourcePath, "utf8");
    return [`/* ${fragment.label} */`, this.transform(source, fragment.transform), ""].join("\n");
  }

  private transform(source: string, transform: RuntimeFragmentTransform): string {
    if (transform === "verbatim") {
      return source;
    }
    if (transform === "drawio-runtime-adapter-without-installer") {
      return this.replaceRequiredLine(source, "katanaInstallDrawioRuntimeAdapter();", "");
    }
    return this.replaceRequiredLine(source, "katanaRenderDrawioSvg();", "");
  }

  private replaceRequiredLine(source: string, needle: string, replacement: string): string {
    if (!source.includes(needle)) {
      throw new Error(`Runtime bundle transform target was not found: ${needle}`);
    }
    return source.replace(needle, replacement);
  }
}
