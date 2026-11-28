import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
  isClassReferenceTarget,
  validateGeneratedClassReference,
  validateScoredClassReferencePair,
} from "./class-reference-gate";

const fixtures = {
  ja: {
    slug: "03-02-class-diagram",
    svg: "../../tests/fixtures/mermaid/structural-regression/ja/03-02-class-diagram.svg",
    source: "../../tests/fixtures/mermaid/ja/03-02-class-diagram.md",
  },
  en: {
    slug: "03-02-class-diagram-inheritance",
    svg: "../../tests/fixtures/mermaid/structural-regression/en/03-02-class-diagram-inheritance.svg",
    source: "../../tests/fixtures/mermaid/en/03-02-class-diagram-inheritance.md",
  },
} as const;

function fixtureSvg(language: keyof typeof fixtures): string {
  return readFileSync(new URL(fixtures[language].svg, import.meta.url), "utf8");
}

function fixtureSource(language: keyof typeof fixtures): string {
  const markdown = readFileSync(new URL(fixtures[language].source, import.meta.url), "utf8");
  const match = markdown.match(/(?:^|\n)(`{3,}|~{3,})mermaid[^\n]*\n([\s\S]*?)\n\1/m);
  if (match?.[2] === undefined) throw new Error(`Mermaid source missing for ${language}`);
  return match[2].trim();
}

describe("class reference gate", () => {
  test("accepts matching English and Japanese target SVG pairs", () => {
    for (const language of ["en", "ja"] as const) {
      const svg = fixtureSvg(language);
      const result = validateScoredClassReferencePair(fixtures[language].slug, svg, svg);

      expect(result?.language).toBe(language);
      expect(result?.scope).toBe("class-inheritance-text-anchors");
      expect(result?.report.nodes).toHaveLength(4);
      expect(result?.report.edges).toBe(3);
    }
  });

  test("rejects when both sides contain the same broken SVG", () => {
    expect(() =>
      validateScoredClassReferencePair(
        fixtures.en.slug,
        '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
        '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
      ),
    ).toThrow("class reference gate rejected");
  });

  test("rejects a broken official SVG even when the candidate is valid", () => {
    expect(() =>
      validateScoredClassReferencePair(fixtures.en.slug, "<not-svg/>", fixtureSvg("en")),
    ).toThrow("official:");
  });

  test("rejects a broken candidate SVG even when the official is valid", () => {
    expect(() =>
      validateScoredClassReferencePair(fixtures.en.slug, fixtureSvg("en"), "<not-svg/>"),
    ).toThrow("candidate:");
  });

  test("rejects a missing official or candidate SVG", () => {
    expect(() =>
      validateScoredClassReferencePair(fixtures.en.slug, null, fixtureSvg("en")),
    ).toThrow("missing official SVG");
    expect(() =>
      validateScoredClassReferencePair(fixtures.en.slug, fixtureSvg("en"), null),
    ).toThrow("missing candidate SVG");
  });

  test("gates generation only for the exact target slugs and original render source", () => {
    const source = fixtureSource("en");
    const svg = fixtureSvg("en");
    const result = validateGeneratedClassReference({
      slug: fixtures.en.slug,
      diagramType: "classdiagram",
      source,
      renderSource: source,
      rawSvg: svg,
      restoredSvg: svg,
    });
    expect(result?.language).toBe("en");
    expect(
      validateGeneratedClassReference({
        slug: "some-other-classdiagram",
        diagramType: "classdiagram",
        source,
        renderSource: source,
        rawSvg: "bad svg",
        restoredSvg: "bad svg",
      }),
    ).toBeNull();
    expect(isClassReferenceTarget(fixtures.ja.slug)).toBe(true);
    expect(isClassReferenceTarget("some-other-classdiagram")).toBe(false);
  });

  test("rejects target generation when type, source identity, or raw stage is invalid", () => {
    const source = fixtureSource("en");
    const svg = fixtureSvg("en");
    const input = {
      slug: fixtures.en.slug,
      diagramType: "flowchart",
      source,
      renderSource: source,
      rawSvg: svg,
      restoredSvg: svg,
    };
    expect(() => validateGeneratedClassReference(input)).toThrow(
      "expected classDiagram source type",
    );
    expect(() =>
      validateGeneratedClassReference({
        ...input,
        diagramType: "classdiagram",
        renderSource: "changed",
      }),
    ).toThrow("render source differs from fixture");
    expect(() =>
      validateGeneratedClassReference({
        ...input,
        diagramType: "classdiagram",
        restoredSvg: "different",
      }),
    ).toThrow("raw/restored SVG differ");
  });
});
