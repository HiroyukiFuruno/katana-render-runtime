import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";

const diagramConsumerFixtures = [
  {
    file: "mermaid-class-diagram.html",
    metadata:
      "Independent Mermaid SVG; source=tests/fixtures/mermaid/ja/03-02-class-diagram.md; accepted=mermaid-current-owned-raw117-r9-execution-r1/raw117/ja/output/03-02-class-diagram.svg; license=KRR repository MIT",
    svgSha256: "6fa7e568294faed8a36fb767c2ec96c7d6db408bb0f23c30e15c48ebecdcfaef",
  },
  {
    file: "drawio-class-diagram.html",
    metadata:
      "Independent Draw.io SVG; source=tests/fixtures/drawio/official/templates/software/class_1.drawio; upstream=jgraph/drawio-diagrams@7b70ed6f96738fed4044a25a314733f8544f076e; license=Apache-2.0",
    svgSha256: "03c3dd46a29706011faae890098ef38ff4310618d7527dac7ada3dd35df40255",
  },
];
const svgBegin = "<!-- reference-svg-begin -->\n";
const svgEnd = "\n<!-- reference-svg-end -->";

function assertDiagramConsumerSource(
  source: string,
  metadata: string,
  expectedSha256: string,
): void {
  if (!source.includes(`<!-- ${metadata}; sha256=${expectedSha256} -->`))
    throw new Error("diagram source provenance changed");
  const begin = source.indexOf(svgBegin);
  const end = source.indexOf(svgEnd);
  if (
    begin < 0 ||
    end <= begin ||
    source.lastIndexOf(svgBegin) !== begin ||
    source.lastIndexOf(svgEnd) !== end
  )
    throw new Error("diagram SVG boundaries missing or duplicated");
  const svg = source.slice(begin + svgBegin.length, end);
  if (createHash("sha256").update(svg, "utf8").digest("hex") !== expectedSha256)
    throw new Error("diagram SVG bytes changed");
}

const fixture = fs.readFileSync(
  new URL("../../tests/fixtures/html/anonymized-requirements.html", import.meta.url),
  "utf8",
);

test("anonymized requirements retains its document layout and executable resources", () => {
  expect(fixture.match(/<\/?[a-zA-Z][^>]*>/g)?.length).toBe(1207);
  const hashes = {
    style: "02eded3ec6cf174f1ef53726f94d644133a09934d1e4a317984e29e1c6cb7e49",
    script: "cecc469558ca2e4f587d67b10a79abef2b0f1531dbd51933d7c241beba02f93e",
  };
  for (const [tag, expected] of Object.entries(hashes)) {
    const body = fixture.match(new RegExp(`<${tag}[^>]*>([\\s\\S]*?)</${tag}>`))?.[1];
    expect(body).toBeDefined();
    expect(
      createHash("sha256")
        .update(body ?? "")
        .digest("hex"),
    ).toBe(expected);
  }
});

test("supplemental diagram HTML preserves independent SVG provenance and bytes", () => {
  for (const item of diagramConsumerFixtures) {
    const source = fs.readFileSync(
      new URL(`../../tests/fixtures/html-diagram-consumer/${item.file}`, import.meta.url),
      "utf8",
    );
    expect(() => assertDiagramConsumerSource(source, item.metadata, item.svgSha256)).not.toThrow();
    expect(() =>
      assertDiagramConsumerSource(
        source.replace("<svg", '<svg data-mutated="true"'),
        item.metadata,
        item.svgSha256,
      ),
    ).toThrow("SVG bytes changed");
    expect(() =>
      assertDiagramConsumerSource(
        source.replace(item.metadata, "altered"),
        item.metadata,
        item.svgSha256,
      ),
    ).toThrow("provenance changed");
  }
});

test("anonymized requirements retains the ordered table of contents and all destinations", () => {
  const navigation = fixture.match(/<nav\b[^>]*>([\s\S]*?)<\/nav>/)?.[1] ?? "";
  const destinations = Array.from(navigation.matchAll(/href="#([^"]+)"/g), (match) => match[1]);
  expect(destinations).toEqual([
    "ch1",
    "s11",
    "s12",
    "s13",
    "s14",
    "s15",
    "ch2",
    "s21",
    "s22",
    "s23",
    "s24",
    "s25",
    "s26",
    "s27",
    "s28",
    "changelog",
  ]);
  for (const destination of destinations) {
    expect(fixture.match(new RegExp(`id="${destination}"`, "g"))?.length).toBe(1);
  }
  expect(navigation.match(/class="toc-item sub"/g)?.length).toBe(13);
});
