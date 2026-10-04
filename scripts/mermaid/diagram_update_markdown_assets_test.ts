import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { Fixture } from "./diagram_update_fixtures";
import { MarkdownReferenceAssets } from "./diagram_update_markdown_assets";

test("skip-errors preserves references captured before an invalid fixture render", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-assets-"));
  try {
    const source = path.join(root, "official");
    const target = path.join(root, "markdown");
    fs.mkdirSync(source);
    fs.mkdirSync(target);
    const fixture = fixtureFor(root);
    const svg = path.join(target, `${fixture.slug}.svg`);
    const png = path.join(source, `${fixture.slug}.png`);
    fs.writeFileSync(svg, "existing-svg", "utf8");

    const assets = new MarkdownReferenceAssets(source, target);
    const referencesBeforeRender = assets.capture(fixture);
    expect(referencesBeforeRender.files.size).toBe(1);

    expect(assets.skipFailedFixture(fixture, referencesBeforeRender)).toBe(false);
    expect(fs.readFileSync(svg, "utf8")).toBe("existing-svg");
    expect(fs.existsSync(png)).toBe(false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("skip-errors removes partial assets only when no reference existed before render", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-assets-"));
  try {
    const source = path.join(root, "official");
    const target = path.join(root, "markdown");
    fs.mkdirSync(source);
    fs.mkdirSync(target);
    const fixture = fixtureFor(root);
    const assets = new MarkdownReferenceAssets(source, target);
    const referencesBeforeRender = assets.capture(fixture);
    expect(referencesBeforeRender.files.size).toBe(0);

    fs.writeFileSync(path.join(source, `${fixture.slug}.svg`), "partial-svg", "utf8");
    fs.writeFileSync(path.join(target, `${fixture.slug}.png`), "partial-png", "utf8");
    expect(assets.skipFailedFixture(fixture, referencesBeforeRender)).toBe(true);
    expect(assets.hasReferences(fixture)).toBe(false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("skip-errors restores shared references after a late failure following capture", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-assets-"));
  try {
    const output = path.join(root, "official");
    fs.mkdirSync(output);
    const fixture = fixtureFor(root);
    const svg = path.join(output, `${fixture.slug}.svg`);
    const png = path.join(output, `${fixture.slug}.png`);
    fs.writeFileSync(svg, Buffer.from([0, 255, 11]));
    fs.writeFileSync(png, Buffer.from([137, 80, 78, 71, 0, 255]));

    const assets = new MarkdownReferenceAssets(output, output);
    const referencesBeforeRender = assets.capture(fixture);
    fs.writeFileSync(svg, "new-svg", "utf8");
    fs.writeFileSync(png, "new-png", "utf8");
    try {
      throw new Error("injected writer failure after capture and asset sync");
    } catch {
      expect(assets.skipFailedFixture(fixture, referencesBeforeRender)).toBe(false);
    }

    expect(fs.readFileSync(svg)).toEqual(Buffer.from([0, 255, 11]));
    expect(fs.readFileSync(png)).toEqual(Buffer.from([137, 80, 78, 71, 0, 255]));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("skip-errors restores both directories after markdown sync and successful sync updates both", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-assets-"));
  try {
    const source = path.join(root, "official");
    const target = path.join(root, "markdown-assets");
    fs.mkdirSync(source);
    fs.mkdirSync(target);
    const fixture = fixtureFor(root);
    const sourceSvg = path.join(source, `${fixture.slug}.svg`);
    const sourcePng = path.join(source, `${fixture.slug}.png`);
    const targetSvg = path.join(target, `${fixture.slug}.svg`);
    const targetPng = path.join(target, `${fixture.slug}.png`);
    const previousAssets: Array<[string, string]> = [
      [sourceSvg, "old-source-svg"],
      [sourcePng, "old-source-png"],
      [targetSvg, "old-target-svg"],
      [targetPng, "old-target-png"],
    ];
    for (const [file, content] of previousAssets) {
      fs.writeFileSync(file, content, "utf8");
    }

    const assets = new MarkdownReferenceAssets(source, target);
    const referencesBeforeRender = assets.capture(fixture);
    fs.writeFileSync(sourceSvg, "new-svg", "utf8");
    fs.writeFileSync(sourcePng, "new-png", "utf8");
    assets.sync(fixture);
    try {
      throw new Error("injected writer failure after markdown asset sync");
    } catch {
      expect(assets.skipFailedFixture(fixture, referencesBeforeRender)).toBe(false);
    }
    expect(fs.readFileSync(sourceSvg, "utf8")).toBe("old-source-svg");
    expect(fs.readFileSync(sourcePng, "utf8")).toBe("old-source-png");
    expect(fs.readFileSync(targetSvg, "utf8")).toBe("old-target-svg");
    expect(fs.readFileSync(targetPng, "utf8")).toBe("old-target-png");

    fs.writeFileSync(sourceSvg, "successful-svg", "utf8");
    fs.writeFileSync(sourcePng, "successful-png", "utf8");
    assets.sync(fixture);
    expect(fs.readFileSync(targetSvg, "utf8")).toBe("successful-svg");
    expect(fs.readFileSync(targetPng, "utf8")).toBe("successful-png");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

function fixtureFor(root: string): Fixture {
  const filePath = path.join(root, "29-empty.md");
  const markdown = "# 29. Empty\n";
  return {
    fileName: "29-empty.md",
    filePath,
    slug: "29-empty",
    title: "29. Empty",
    markdown,
    source: "",
    fenceEnd: markdown.length,
  };
}
