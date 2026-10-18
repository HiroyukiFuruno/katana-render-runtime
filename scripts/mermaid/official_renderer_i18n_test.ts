import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { runInNewContext } from "node:vm";
import { FixtureRepository } from "./diagram_update_fixtures";
import { PlaywrightLoader } from "./official-renderer";
import { resolveOfficialRendererAssetInputs } from "./official-renderer-asset-inputs";
import { MermaidRuntimeScripts } from "./official-renderer-i18n";
import type { BrowserHandle, MermaidWindow, PageHandle } from "./official-renderer-types";

const runtimeScriptNames = MermaidRuntimeScripts.paths().map((it) => path.basename(it));

test("公式参照生成は Mermaid normalizer の依存ファイルを利用側より先に読む", () => {
  expectLoadedBefore("svg_path_bezier_bounds.js", "svg_path_bounds.js");
  expect(runtimeScriptNames).not.toContain("svg_class_layout_fixes.js");
  expect(runtimeScriptNames).toContain("svg_class_fixes.js");
  expectLoadedBefore("svg_flowchart_fixture_layout_fixes.js", "svg_normalize.js");
  expectLoadedBefore("svg_block_fixture_layout_fixes.js", "svg_normalize.js");
  expectLoadedBefore("svg_er_fixture_layout_fixes.js", "svg_normalize.js");
  expectLoadedBefore("svg_c4_context_full_fixture_layout.js", "svg_review_fixes.js");
  expectLoadedBefore("svg_mindmap_layout_fixes.js", "svg_diagram_normalizers.js");
  expectLoadedBefore("svg_requirement_localized_fixes.js", "svg_visual_fixes.js");
  expectLoadedBefore("svg_sequence_activation_fixes.js", "svg_normalize.js");
});

function expectLoadedBefore(dependency: string, consumer: string) {
  expect(runtimeScriptNames).toContain(dependency);
  expect(runtimeScriptNames).toContain(consumer);
  expect(runtimeScriptNames.indexOf(dependency)).toBeLessThan(runtimeScriptNames.indexOf(consumer));
}

test.each([
  "sankey-beta\n原料,加工,10\n加工,製品,8\n",
  "sankey-beta\nsource,target,10\n",
  "---\ntitle: 日本語\n---\nsankey-beta\n原料,製品,10\n",
])("patched Sankey parser receives the original source unchanged: %s", (source) => {
  expectSourcePreserved(source);
});

test.each([
  "19-01-requirement-diagram",
  "19-02-requirement-diagram",
  "26-01-wardley-beta",
  "26-02-wardley-beta",
  "27-02-xy",
])("quoted Japanese source reaches the vendor unchanged: %s", (slug) => {
  const fixture = new FixtureRepository("tests/fixtures/mermaid/ja").read(`${slug}.md`);
  expectSourcePreserved(fixture.source);
});

function expectSourcePreserved(source: string) {
  const scripts = MermaidRuntimeScripts.paths()
    .filter((name) => /source_i18n_(context|normalize)\.js$/.test(name))
    .map((name) => fs.readFileSync(name, "utf8"))
    .join("\n");
  expect(
    runInNewContext(`${scripts}\nkatanaNormalizeMermaidSourceI18n(source)`, { source }),
  ).toEqual({
    source,
    replacements: [],
  });
}

function expectOnlyDoubleQuoteInsertions(negative: string, positive: string) {
  const negativeCharacters = Array.from(negative);
  const positiveCharacters = Array.from(positive);
  let negativeIndex = 0;
  for (const character of positiveCharacters) {
    if (negativeCharacters[negativeIndex] === character) {
      negativeIndex += 1;
    } else {
      expect(character).toBe('"');
    }
  }
  expect(negativeIndex).toBe(negativeCharacters.length);
}

test.each([
  ["quote insertion", "label", '"label"', true],
  ["emoji quote insertion", "😀", '"😀"', true],
  ["existing quote with additional quote", '"😀"', '""😀"', true],
  ["empty source", "", "", true],
  ["original quote deletion", 'label "x"', "label x", false],
  ["character replacement", "label x", "label y", false],
  ["coordinate numeric change", "x: 1", "x: 2", false],
  ["trailing character omission", "label", "labe", false],
  ["emoji non-quote insertion", "😀", "x😀", false],
] as const)("quote-only guard: %s", (_name, negative, positive, accepted) => {
  const check = () => expectOnlyDoubleQuoteInsertions(negative, positive);
  if (accepted) {
    expect(check).not.toThrow();
  } else {
    expect(check).toThrow();
  }
});

test("default reference asset uses the same versioned vendor bytes as the product", () => {
  const cwd = path.resolve(import.meta.dir, "../..");
  const inputs = resolveOfficialRendererAssetInputs([], {}, cwd, "/unused-home");
  expect(inputs.mermaidJs).toBe(
    path.join(
      cwd,
      "crates/katana-render-runtime/vendor/mermaid/12.2.0-krr.pr6823.1/mermaid.min.js",
    ),
  );
});

type RawMermaidFixture = {
  slug: string;
  positive: string;
  negative: string;
  negativeSha256: string;
  expectedTitle: string;
};

type RawMermaidResult = {
  positive: { accepted: boolean; error: string };
  positiveSource: string;
  positiveTitleText: string;
  negative: { accepted: boolean; error: string };
};

type MermaidParser = NonNullable<MermaidWindow["mermaid"]> & {
  parse(source: string): Promise<unknown>;
  render(id: string, source: string): Promise<{ svg: string }>;
};

const cwd = path.resolve(import.meta.dir, "../..");
const mermaidJs = path.join(
  cwd,
  "crates/katana-render-runtime/vendor/mermaid/12.2.0-krr.pr6823.1/mermaid.min.js",
);
const positiveFixtures = new FixtureRepository(path.join(cwd, "tests/fixtures/mermaid/ja"));
const negativeFixtures = new FixtureRepository(
  path.join(cwd, "tests/fixtures/mermaid/invalid-unquoted-ja"),
);
const rawMermaidFixtures: RawMermaidFixture[] = [
  {
    slug: "26-01-wardley-beta",
    negativeSha256: "958bdca709eb10d6e23c1f2509cfdc0340760b905e22721c061f135ccb8bbe93",
    expectedTitle: "レンダラー導入",
  },
  {
    slug: "26-02-wardley-beta",
    negativeSha256: "24b6c5c7148fd22af8f0eda840b807362d25f32138ff4e937ec0be52c82f1873",
    expectedTitle: "ティーショップ",
  },
  {
    slug: "27-02-xy",
    negativeSha256: "8c6c662c1fd634a8a0ba6eac02ff02d5a8766521ba2a652d45a616a7f4ef7f67",
    expectedTitle: "",
  },
  {
    slug: "19-01-requirement-diagram",
    negativeSha256: "e2ccccb15d98ead24df9a3b53c576452dee7ee456456bf2b929070a21d93b3bb",
    expectedTitle: "",
  },
  {
    slug: "19-02-requirement-diagram",
    negativeSha256: "982fb983fb017713ad03eb405f0c6c6a6de4eb39bbfdffdf551404da6e73d6a6",
    expectedTitle: "",
  },
].map((fixture) => ({
  ...fixture,
  positive: positiveFixtures.read(`${fixture.slug}.md`).source,
  negative: negativeFixtures.read(`${fixture.slug}.md`).source,
}));
const rawMermaidTestCases = rawMermaidFixtures.map((fixture) => [fixture.slug, fixture] as const);

let browser: BrowserHandle | undefined;
let page: PageHandle | undefined;
let renderIndex = 0;

async function closeRawMermaidBrowser(): Promise<void> {
  const activePage = page;
  page = undefined;
  const activeBrowser = browser;
  browser = undefined;
  try {
    await activePage?.close();
  } finally {
    await activeBrowser?.close();
  }
}

async function evaluateRawMermaidFixture(fixture: RawMermaidFixture): Promise<RawMermaidResult> {
  const activePage = page;
  if (activePage === undefined) throw new Error("Mermaid browser page was not initialized");
  const id = `raw-parser-regression-${renderIndex++}`;
  return activePage.evaluate(
    async ({ positive, negative, renderId }) => {
      const mermaid = (window as MermaidWindow).mermaid as MermaidParser | undefined;
      if (mermaid === undefined) throw new Error("Mermaid runtime was not installed");
      const parse = async (source: string) => {
        try {
          await mermaid.parse(source);
          return { accepted: true, error: "" };
        } catch (error) {
          return {
            accepted: false,
            error: error instanceof Error ? error.message : String(error),
          };
        }
      };
      const positiveResult = await parse(positive);
      const rendered = await mermaid.render(renderId, positive);
      const document = new DOMParser().parseFromString(rendered.svg, "image/svg+xml");
      const positiveTitleText = [...document.querySelectorAll("text.wardley-title")]
        .map((element) => element.textContent ?? "")
        .join("");
      const negativeResult = await parse(negative);
      return {
        positive: positiveResult,
        positiveSource: positive,
        positiveTitleText,
        negative: negativeResult,
      };
    },
    { positive: fixture.positive, negative: fixture.negative, renderId: id },
  );
}

describe("raw Mermaid parser", () => {
  beforeAll(async () => {
    try {
      const { chromium } = await new PlaywrightLoader().load();
      browser = await chromium.launch({ headless: true });
      page = await browser.newPage({
        viewport: { width: 1520, height: 845 },
        deviceScaleFactor: 1,
      });
      await page.setContent("<!doctype html><html><body></body></html>", { waitUntil: "load" });
      await page.addScriptTag({ path: mermaidJs });
    } catch (error) {
      try {
        await closeRawMermaidBrowser();
      } catch (closeError) {
        throw new AggregateError([error, closeError], "Mermaid browser setup and cleanup failed");
      }
      throw error;
    }
  });

  afterAll(async () => {
    await closeRawMermaidBrowser();
  });

  test.each(rawMermaidTestCases)(
    "keeps the original Japanese syntax cases rejected: %s",
    async (slug, fixture) => {
      const result = await evaluateRawMermaidFixture(fixture);
      expect(result.positive.accepted, `${slug} quoted fixture`).toBe(true);
      expect(result.positiveSource, `${slug} vendor input`).toBe(fixture.positive);
      expectOnlyDoubleQuoteInsertions(fixture.negative, fixture.positive);
      expect(result.positiveTitleText, `${slug} rendered title`).toBe(fixture.expectedTitle);
      expect(result.negative.accepted, `${slug} original fixture`).toBe(false);
      expect(result.negative.error, `${slug} original parser error`).not.toBe("");
      const negativePath = path.join(
        cwd,
        "tests/fixtures/mermaid/invalid-unquoted-ja",
        `${slug}.md`,
      );
      expect(
        createHash("sha256").update(fs.readFileSync(negativePath)).digest("hex"),
        `${slug} original fixture SHA-256`,
      ).toBe(fixture.negativeSha256);
    },
  );
});
