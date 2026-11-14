import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { makeOnePixelPng } from "./mermaid_pair_test_support";
import { publishMermaidPair, sha256 } from "./mermaid-pair-manifest";
import { ReferencePairRepository } from "./reference_pair_repository";

test("explicit fixture inventory reports empty fixtures and requires every nonempty pair", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-inventory-"));
  try {
    const fixturesDir = path.join(root, "fixtures");
    const officialDir = path.join(fixturesDir, "official-dark");
    const candidateDir = path.join(root, "candidate-png");
    const candidateSvgDir = path.join(root, "candidate-svg");
    for (const directory of [fixturesDir, officialDir, candidateDir, candidateSvgDir]) {
      fs.mkdirSync(directory, { recursive: true });
    }
    fs.writeFileSync(
      path.join(fixturesDir, "01-valid.md"),
      "# Valid\n\n```mermaid\nflowchart LR\nA-->B\n```\n",
    );
    fs.writeFileSync(path.join(fixturesDir, "02-empty.md"), "# Empty\n\n```mermaid\n\n```\n");
    const svg = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"></svg>');
    fs.writeFileSync(path.join(candidateSvgDir, "01-valid.svg"), svg);
    publishPair(officialDir, "01-valid", svg, "official");
    publishPair(candidateDir, "01-valid", svg, "candidate");
    const repository = new ReferencePairRepository({
      officialDir,
      katanaDir: candidateDir,
      katanaSvgDir: candidateSvgDir,
      outputDir: path.join(root, "out"),
      katanaCrop: null,
      baselineScoresPath: null,
      minScore: 99,
      theme: "dark",
      fixturesDir,
      fixtureGlob: "*.md",
    });
    const inventory = repository.inventory();
    expect(inventory.contract).toBe("explicit-fixture-directory-and-glob");
    expect(inventory.expected).toEqual(["01-valid"]);
    expect(inventory.skippedEmpty).toEqual([{ slug: "02-empty", reason: "empty Mermaid source" }]);
    expect(repository.list(inventory).map((pair) => pair.slug)).toEqual(["01-valid"]);

    fs.unlinkSync(path.join(candidateDir, "01-valid.png"));
    expect(() => repository.list(inventory)).toThrow("missing image");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

function publishPair(directory: string, slug: string, svg: Buffer, role: "official" | "candidate") {
  const png = makeOnePixelPng();
  const provenance = { source: "test fixture" };
  publishMermaidPair({
    directory,
    slug,
    role,
    sourceSvg: svg,
    renderedSvg: svg,
    png,
    provenance: {
      path: "embedded:test",
      sha256: sha256(Buffer.from(JSON.stringify(provenance))),
      document: provenance,
    },
    config: { viewport: [1, 1] },
    runtimeAssets: [],
  });
}
