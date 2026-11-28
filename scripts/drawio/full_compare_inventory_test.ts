import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { assertFreshDrawioOutput } from "./svg-capture";

interface CompareRecipes {
  fullDomains: string[];
  ciDomain: string;
  ciHasBaseline: boolean;
}

function recipeBody(source: string, header: string, nextHeader: string): string {
  const start = source.indexOf(header);
  const end = source.indexOf(nextHeader, start);
  if (start < 0 || end < 0) {
    throw new Error(`Unsupported Draw.io recipe layout: ${header}`);
  }
  return source.slice(start, end);
}

function readCompareRecipes(source: string): CompareRecipes {
  const full = recipeBody(
    source,
    "drawio-compare-full min_score='99':",
    "# Render Draw.io fixtures",
  );
  const list = full.match(/printf '%s\\n' \\\n([\s\S]*?)\n\s*\| xargs/);
  if (list?.[1] === undefined) {
    throw new Error("Unsupported Draw.io full-domain list syntax");
  }
  const fullDomains = [...list[1].matchAll(/^\s*"\$root\/([^"\n]+)" \\$/gm)].map(
    (match) => match[1] ?? "",
  );
  if (fullDomains.length === 0 || fullDomains.some((domain) => domain.length === 0)) {
    throw new Error("Draw.io full-domain list is empty or malformed");
  }

  const ci = recipeBody(source, "drawio-compare-ci min_score='99':", "# Compare basic Draw.io");
  const ciCalls = [
    ...ci.matchAll(
      /just drawio-compare-prebuilt (tests\/fixtures\/drawio\/[^\s]+) "\{\{min_score\}\}" "\$output" (tests\/fixtures\/drawio\/[^\s]+)/g,
    ),
  ];
  const ciCall = ciCalls[0];
  if (ciCalls.length !== 1 || ciCall?.[1] === undefined || ciCall[2] === undefined) {
    throw new Error("Unsupported Draw.io CI compare command");
  }
  if (
    !/run_root="\$\(mktemp -d tmp\/krr-drawio-ci\.XXXXXX\)"/.test(ci) ||
    !/output="\$run_root\/representative"/.test(ci)
  ) {
    throw new Error("Draw.io CI output must be a new representative child directory");
  }
  const prebuilt = recipeBody(
    source,
    "drawio-compare-prebuilt fixtures min_score='99'",
    "# Compare representative Draw.io",
  );
  if (
    !prebuilt.includes("candidate-svg-rasterize.ts") ||
    !prebuilt.includes("reference-compare.ts") ||
    !prebuilt.includes('candidate-svg-rasterize.ts --assert-output "{{output}}"') ||
    !full.includes('"$3/$output_slug"')
  ) {
    throw new Error("Draw.io comparison recipes do not use the fresh output guard and score path");
  }
  return {
    fullDomains,
    ciDomain: path.posix.relative("tests/fixtures/drawio", ciCall[1]),
    ciHasBaseline: ciCall[2] === "tests/fixtures/drawio/representative/score-baseline.json",
  };
}

function fixtureDomains(root: string): string[] {
  const domains: string[] = [];
  const visit = (directory: string) => {
    const entries = fs.readdirSync(directory, { withFileTypes: true });
    if (entries.some((entry) => entry.isFile() && entry.name.endsWith(".drawio"))) {
      domains.push(path.relative(root, directory).split(path.sep).join("/"));
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        visit(path.join(directory, entry.name));
      }
    }
  };
  visit(root);
  return domains.sort();
}

test("the actual full and CI recipes cover every Draw.io fixture domain exactly once", () => {
  const repositoryRoot = path.resolve(".");
  const fixtureRoot = path.join(repositoryRoot, "tests/fixtures/drawio");
  const recipes = readCompareRecipes(
    fs.readFileSync(path.join(repositoryRoot, "Justfile"), "utf8"),
  );
  const discovered = fixtureDomains(fixtureRoot);
  const combined = [...recipes.fullDomains, recipes.ciDomain];
  const fullCount = recipes.fullDomains.reduce(
    (total, domain) =>
      total +
      fs
        .readdirSync(path.join(fixtureRoot, domain))
        .filter((fileName) => fileName.endsWith(".drawio")).length,
    0,
  );
  const ciCount = fs
    .readdirSync(path.join(fixtureRoot, recipes.ciDomain))
    .filter((fileName) => fileName.endsWith(".drawio")).length;

  expect(new Set(combined).size).toBe(combined.length);
  expect([...combined].sort()).toEqual(discovered);
  expect(fullCount).toBe(610);
  expect(recipes.ciDomain).toBe("representative");
  expect(ciCount).toBe(20);
  expect(fullCount + ciCount).toBe(630);
  expect(recipes.ciHasBaseline).toBe(true);
  expect(fs.existsSync(path.join(fixtureRoot, "representative/score-baseline.json"))).toBe(true);
});

test("the prebuilt recipe accepts a new CI child output and rejects an existing one", () => {
  const repositoryRoot = path.resolve(".");
  const recipes = readCompareRecipes(
    fs.readFileSync(path.join(repositoryRoot, "Justfile"), "utf8"),
  );
  const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "drawio-ci-output-"));
  const output = path.join(runRoot, recipes.ciDomain);
  try {
    expect(fs.existsSync(runRoot)).toBe(true);
    expect(fs.existsSync(output)).toBe(false);
    expect(() => assertFreshDrawioOutput(output)).not.toThrow();
    fs.mkdirSync(output);
    expect(() => assertFreshDrawioOutput(output)).toThrow(
      `Draw.io comparison output already exists: ${output}`,
    );
  } finally {
    fs.rmSync(runRoot, { recursive: true, force: true });
  }
});
