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
    const hadReferencesBeforeRender = assets.hasReferences(fixture);
    expect(hadReferencesBeforeRender).toBe(true);

    expect(assets.skipFailedFixture(fixture, hadReferencesBeforeRender)).toBe(false);
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
    const hadReferencesBeforeRender = assets.hasReferences(fixture);
    expect(hadReferencesBeforeRender).toBe(false);

    fs.writeFileSync(path.join(source, `${fixture.slug}.svg`), "partial-svg", "utf8");
    fs.writeFileSync(path.join(target, `${fixture.slug}.png`), "partial-png", "utf8");
    expect(assets.skipFailedFixture(fixture, hadReferencesBeforeRender)).toBe(true);
    expect(assets.hasReferences(fixture)).toBe(false);
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
