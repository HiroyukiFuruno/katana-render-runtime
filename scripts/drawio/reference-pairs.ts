import fs from "node:fs";
import path from "node:path";
import type { CliParsedOptions } from "../mermaid/reference_compare_options";
import type { ReferencePair } from "../mermaid/reference_pair_repository";

type ReferencePairDirectories = Pick<CliParsedOptions, "officialDir" | "katanaDir">;

export class DrawioReferencePairRepository {
  constructor(private directories: ReferencePairDirectories) {}

  list(): ReferencePair[] {
    const officialPngSlugs = this.regularFileSlugs(this.directories.officialDir, ".png");
    if (officialPngSlugs.length === 0) {
      throw new Error("Draw.io comparison has no official PNG fixtures");
    }

    const officialSvgSlugs = this.regularFileSlugs(this.directories.officialDir, ".svg");
    const candidatePngSlugs = this.regularFileSlugs(this.directories.katanaDir, ".png");
    this.requireSameSlugs(officialPngSlugs, officialSvgSlugs, "official SVG");
    this.requireSameSlugs(officialPngSlugs, candidatePngSlugs, "candidate PNG");

    return officialPngSlugs.map((slug) => ({
      slug,
      officialPath: path.join(this.directories.officialDir, `${slug}.png`),
      katanaPath: path.join(this.directories.katanaDir, `${slug}.png`),
      officialSvgPath: path.join(this.directories.officialDir, `${slug}.svg`),
      katanaSvgPath: path.join(this.directories.katanaDir, `${slug}.svg`),
    }));
  }

  private regularFileSlugs(directory: string, extension: ".png" | ".svg"): string[] {
    const slugs = fs
      .readdirSync(directory, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith(extension))
      .map((entry) => entry.name.slice(0, -extension.length))
      .sort();
    if (slugs.some((slug) => slug.length === 0)) {
      throw new Error(`Draw.io ${extension} inventory contains an empty slug`);
    }
    return slugs;
  }

  private requireSameSlugs(expected: string[], actual: string[], role: string) {
    const expectedSet = new Set(expected);
    const actualSet = new Set(actual);
    const missing = expected.filter((slug) => !actualSet.has(slug));
    const extra = actual.filter((slug) => !expectedSet.has(slug));
    if (missing.length > 0 || extra.length > 0) {
      throw new Error(
        `Draw.io ${role} slug mismatch; missing=${missing.join(",")}; extra=${extra.join(",")}`,
      );
    }
  }
}
