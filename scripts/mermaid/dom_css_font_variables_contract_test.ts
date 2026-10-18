import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";
import { createContext, runInContext } from "node:vm";
import { MermaidRuntimeBundleDefinition } from "#mermaid/runtime_definition";
import { RuntimeFragmentLoader } from "../runtime-bundles/runtime-fragment-loader";

const root = path.resolve(import.meta.dir, "../..");
const definition = MermaidRuntimeBundleDefinition.create();
const currentRuntime = new RuntimeFragmentLoader(root).loadDefinition(definition);
const cssLibraryBindings = [
  "katanaParseCssDeclarations",
  "katanaCssVariableReferences",
  "katanaSubstituteCssVariables",
  "katanaValidCssFontFamily",
  "katanaCssWideKeyword",
];

function runtimeContext() {
  const bundle = new URL(
    "../../crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mermaid-runtime.min.js",
    import.meta.url,
  );
  const libraryContext = createContext({});
  runInContext(readFileSync(bundle, "utf8"), libraryContext);
  const libraries: Record<string, unknown> = {};
  for (const name of cssLibraryBindings) {
    const value: unknown = runInContext(name, libraryContext);
    if (typeof value !== "function") throw new TypeError(`CSS library binding is missing: ${name}`);
    libraries[name] = value;
  }
  // 既存bundleはCSS library境界だけに使い、DOMとresolverは現canonical raw sourceを評価する。
  const context = createContext(libraries);
  runInContext(currentRuntime, context);
  return context;
}

function probe<T>(script: string): T {
  return runInContext(`(() => { ${script} })()`, runtimeContext()) as T;
}

test("font variables compute at their declaration element before inheritance", () => {
  const actual = probe<string[]>(`
    const parent = document.createElement("div");
    parent.style.cssText = "--base: Noto Sans; --family: var(--base); font-family: var(--family)";
    document.body.appendChild(parent);
    const child = document.createElement("span");
    child.style.cssText = "--base: sans-serif; font-family: var(--family)";
    parent.appendChild(child);
    const own = document.createElement("span");
    own.style.cssText = "--base: sans-serif; --family: var(--base); font-family: var(--family)";
    parent.appendChild(own);
    return [getComputedStyle(parent).fontFamily, getComputedStyle(child).fontFamily,
      getComputedStyle(own).fontFamily, getComputedStyle(child).getPropertyValue("font-family")];
  `);
  expect(actual).toEqual(["Noto Sans", "Noto Sans", "sans-serif", "Noto Sans"]);
});

test("setProperty rejects invalid font-family values without changing the declaration", () => {
  const actual = probe<{ value: string; priority: string; computed: string }>(`
    const node = document.createElement("span");
    node.style.setProperty("font-family", '"trebuchet ms", verdana, arial, sans-serif', "important");
    node.style.setProperty("font-family", '"trebuchet ms", verdana, arial, sans-serif;');
    return { value: node.style.getPropertyValue("font-family"),
      priority: node.style.getPropertyPriority("font-family"), computed: getComputedStyle(node).fontFamily };
  `);
  expect(actual).toEqual({
    value: '"trebuchet ms", verdana, arial, sans-serif',
    priority: "important",
    computed: '"trebuchet ms", verdana, arial, sans-serif',
  });
});

test("Mermaid rendering inherits font families from the raw producer body environment", async () => {
  const context = runtimeContext();
  runInContext(
    `
      const bodyFont = document.createElement("span");
      document.body.appendChild(bodyFont);
      const invalid = document.createElement("span");
      invalid.style.setProperty("font-family", '"trebuchet ms", verdana, arial, sans-serif;');
      document.body.appendChild(invalid);
      const empty = document.createElement("span");
      document.body.appendChild(empty);
      const inherited = document.createElement("span");
      inherited.style.fontFamily = "inherit";
      document.body.appendChild(inherited);
      const unset = document.createElement("span");
      unset.style.fontFamily = "unset";
      document.body.appendChild(unset);
      const explicit = document.createElement("span");
      explicit.style.fontFamily = '"trebuchet ms", verdana, arial, sans-serif';
      document.body.appendChild(explicit);
      const initial = document.createElement("span");
      initial.style.fontFamily = "initial";
      document.body.appendChild(initial);
      globalThis.__fontFamiliesAtInitialize = null;
      globalThis.mermaid = {
        initialize() {
          globalThis.__fontFamiliesAtInitialize = [bodyFont, invalid, empty, inherited, unset, explicit, initial]
            .map((node) => getComputedStyle(node).fontFamily);
        },
        async render() {
          throw new Error("stop after observing host fonts");
        },
      };
    `,
    context,
  );

  let renderError = "";
  try {
    await (runInContext(
      `katanaRunMermaidRuntime({
        source: "graph TD; A-->B", diagramType: "flowchart", svgId: "font-inheritance",
        theme: "dark", background: "#000", fill: "#111", text: "#fff",
        stroke: "#fff", arrow: "#fff",
      })`,
      context,
    ) as Promise<unknown>);
  } catch (error) {
    renderError = String(error);
  }

  expect(renderError).toContain("stop after observing host fonts");
  const actual = JSON.parse(
    runInContext("JSON.stringify(globalThis.__fontFamiliesAtInitialize)", context) as string,
  ) as string[];
  const bodyFamily = '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
  expect(actual).toEqual([
    bodyFamily,
    bodyFamily,
    bodyFamily,
    bodyFamily,
    bodyFamily,
    '"trebuchet ms", verdana, arial, sans-serif',
    "trebuchet ms, verdana, arial, sans-serif",
  ]);
});

test("setProperty accepts quoted, generic, CSS-wide, variable and escaped font families", () => {
  const actual = probe<string[]>(`
    const quoted = document.createElement("span");
    quoted.style.setProperty("font-family", '"A;B", sans-serif');
    const generic = document.createElement("span");
    generic.style.setProperty("font-family", "sans-serif");
    const cssWide = document.createElement("span");
    cssWide.style.setProperty("font-family", "inherit");
    const variable = document.createElement("span");
    variable.style.setProperty("--Family", "Noto Sans");
    variable.style.setProperty("font-family", "var(--Family, serif)");
    const escaped = document.createElement("span");
    escaped.style.setProperty("font-family", "Noto\\ Sans");
    return [quoted, generic, cssWide, variable, escaped].map((node) => node.style.getPropertyValue("font-family"));
  `);
  expect(actual).toEqual([
    '"A;B", sans-serif',
    "sans-serif",
    "inherit",
    "var(--Family, serif)",
    "Noto Sans",
  ]);
});

test("setProperty rejects malformed font variables without erasing the last valid value", () => {
  const actual = probe<{ value: string; priority: string }>(`
    const node = document.createElement("span");
    node.style.setProperty("font-family", "Noto Sans", "important");
    node.style.setProperty("font-family", "var(not-custom, serif)");
    return { value: node.style.getPropertyValue("font-family"), priority: node.style.getPropertyPriority("font-family") };
  `);
  expect(actual).toEqual({ value: "Noto Sans", priority: "important" });
});

test("font variables respect local scopes, priority and case-sensitive names", () => {
  const actual = probe<string[]>(`
    const style = document.createElement("style");
    style.textContent = ".scope { --Family: Noto Sans !important; }";
    document.body.appendChild(style);
    const parent = document.createElement("div");
    parent.setAttribute("class", "scope");
    parent.style.cssText = "--Family: serif; font-family: var(--Family)";
    document.body.appendChild(parent);
    const child = document.createElement("span");
    child.style.cssText = "--Family: sans-serif; font-family: var(--Family)";
    parent.appendChild(child);
    const sibling = document.createElement("span");
    sibling.style.cssText = "font-family: var(--Family, serif)";
    document.body.appendChild(sibling);
    const wrongCase = document.createElement("span");
    wrongCase.style.cssText = "font-family: var(--family, monospace)";
    parent.appendChild(wrongCase);
    return [parent, child, sibling, wrongCase].map((node) => getComputedStyle(node).fontFamily);
  `);
  expect(actual).toEqual(["Noto Sans", "sans-serif", "serif", "monospace"]);
});

test("font variable fallback supports lists, nested variables and quoted commas", () => {
  const actual = probe<string[]>(`
    const node = document.createElement("span");
    document.body.appendChild(node);
    node.style.cssText = '--other: "A,B"; font-family: var(--missing, var(--other), sans-serif)';
    const nested = getComputedStyle(node).fontFamily;
    node.style.fontFamily = '"var(--literal)", sans-serif';
    const literal = getComputedStyle(node).fontFamily;
    node.style.cssText = '--family: initial; font-family: var(--family, Noto Sans, sans-serif)';
    return [nested, literal, getComputedStyle(node).fontFamily];
  `);
  expect(actual).toEqual([
    '"A,B",sans-serif',
    '"var(--literal)",sans-serif',
    "Noto Sans,sans-serif",
  ]);
});

test("cycles are invalid even in unused fallbacks and invalid font values inherit", () => {
  const actual = probe<string[]>(`
    const parent = document.createElement("div");
    parent.style.fontFamily = "Noto Sans";
    document.body.appendChild(parent);
    const child = document.createElement("span");
    parent.appendChild(child);
    child.style.cssText = "--a: var(--b); --b: var(--a, serif); font-family: var(--a, sans-serif)";
    const cycleFallback = getComputedStyle(child).fontFamily;
    child.style.fontFamily = "var(--a)";
    const inherited = getComputedStyle(child).fontFamily;
    child.style.cssText = "--defined: serif; --a: var(--defined, var(--a)); font-family: var(--a, sans-serif)";
    const unusedFallbackCycle = getComputedStyle(child).fontFamily;
    child.style.fontFamily = "var(--missing,)";
    return [cycleFallback, inherited, unusedFallbackCycle, getComputedStyle(child).fontFamily];
  `);
  expect(actual).toEqual(["sans-serif", "Noto Sans", "sans-serif", "Noto Sans"]);
});

test("escaped CSS identifiers are decoded through the declaration and value parser", () => {
  const actual = probe<string[]>(`
    const node = document.createElement("span");
    document.body.appendChild(node);
    const slash = String.fromCharCode(92);
    node.style.cssText = '--F' + slash + '61 mily: Noto Sans; font-family: v' + slash + '61 r(--F' + slash + '61 mily)';
    return [node.style.getPropertyValue('--Family'), getComputedStyle(node).fontFamily];
  `);
  expect(actual).toEqual(["Noto Sans", "Noto Sans"]);
});

test("overlapping dependency cycles invalidate every component member", () => {
  const actual = probe<string[]>(`
    const node = document.createElement("span");
    document.body.appendChild(node);
    node.style.cssText = '--a:var(--b) var(--c);--b:var(--a);--c:var(--b,serif);font-family:var(--a,var(--c,monospace))';
    const fromA = getComputedStyle(node).fontFamily;
    node.style.fontFamily = 'var(--c,var(--a,monospace))';
    return [fromA, getComputedStyle(node).fontFamily];
  `);
  expect(actual).toEqual(["monospace", "monospace"]);
});

test("a substituted invalid font family inherits instead of using var fallback", () => {
  const actual = probe<string[]>(`
    const parent = document.createElement("div");
    parent.style.fontFamily = 'Noto Sans';
    document.body.appendChild(parent);
    const child = document.createElement("span");
    parent.appendChild(child);
    child.style.cssText = '--family:16px;font-family:var(--family,Arial)';
    const invalidDimension = getComputedStyle(child).fontFamily;
    child.style.cssText = '--family:serif,;font-family:var(--family,Arial)';
    return [invalidDimension, getComputedStyle(child).fontFamily];
  `);
  expect(actual).toEqual(["Noto Sans", "Noto Sans"]);
});

test("escaped var fallbacks preserve separate family identifiers and nested branches", () => {
  const actual = probe<string[]>(`
    const node = document.createElement("span");
    document.body.appendChild(node);
    const escapedVar = 'v' + String.fromCharCode(92) + '61 r';
    node.style.cssText = 'font-family:' + escapedVar + '(--missing,Noto Sans,serif)';
    const words = getComputedStyle(node).fontFamily;
    node.style.fontFamily = escapedVar + '(--missing,' + escapedVar + '(--missing2,Noto Sans),serif)';
    return [words, getComputedStyle(node).fontFamily];
  `);
  expect(actual).toEqual(["Noto Sans,serif", "Noto Sans,serif"]);
});

const independentCases: [string, string, string, string][] = [
  [
    "invalid-custom-var-grammar-inherits-parent",
    "--family:var(not-custom);font-family:var(--family,serif)",
    "--family:Noto Sans;font-family:Noto Sans",
    "Noto Sans",
  ],
  [
    "invalid-custom-var-grammar-retains-prior-valid",
    "--family:Noto Sans;--family:var(not-custom);font-family:var(--family,serif)",
    "font-family:monospace",
    "Noto Sans",
  ],
  [
    "escaped-leading-dashes-in-reference",
    "--family:Noto Sans;font-family:var(\\2d \\2d family,serif)",
    "font-family:monospace",
    "Noto Sans",
  ],
  [
    "escaped-leading-dashes-in-declaration",
    "\\2d \\2d Family:Noto Sans;font-family:var(--Family,serif)",
    "font-family:monospace",
    "Noto Sans",
  ],
  [
    "escaped-custom-css-wide-initial",
    "--family:\\69 nitial;font-family:var(--family,serif)",
    "--family:Noto Sans;font-family:Noto Sans",
    "serif",
  ],
  [
    "escaped-fallback-css-wide-inherit",
    "font-family:var(--missing,\\69 nherit)",
    "font-family:Noto Sans",
    "Noto Sans",
  ],
  [
    "empty-fallback-before-family",
    "font-family:var(--missing,) Noto Sans",
    "font-family:monospace",
    "Noto Sans",
  ],
  [
    "sibling-identifier-token-boundary",
    "--a:Noto;--b:Sans;font-family:var(--a)var(--b)",
    "font-family:monospace",
    "Noto Sans",
  ],
  [
    "fallback-token-boundary",
    "font-family:var(--missing,Noto Sans,serif)",
    "font-family:monospace",
    "Noto Sans,serif",
  ],
];
for (const [name, cssText, parentCss, expected] of independentCases) {
  test(name, () => {
    const actual = probe<string>(`
      const parent = document.createElement("div");
      parent.style.cssText = ${JSON.stringify(parentCss)};
      document.body.appendChild(parent);
      const child = document.createElement("span");
      child.style.cssText = ${JSON.stringify(cssText)};
      parent.appendChild(child);
      return getComputedStyle(child).fontFamily;
    `);
    expect(actual).toEqual(expected);
  });
}

test("font resolver is evaluated from the current canonical raw assembler", () => {
  const cssPath =
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/dom_css_style.js";
  expect(definition.fragments.filter((fragment) => fragment.sourcePath === cssPath)).toHaveLength(
    1,
  );
  const source = readFileSync(path.join(root, cssPath), "utf8");
  const match = source.match(/function katanaResolvedCssVariableValue\([\s\S]*?\n}/);
  if (!match) throw new Error("Current raw CSS variable resolver is missing");
  const evaluated: unknown = runInContext(
    "katanaResolvedCssVariableValue.toString()",
    runtimeContext(),
  );
  expect(evaluated).toBe(match[0]);
});

test.each([
  [
    "acyclic to self-cycle and back invalidates computed values",
    `const node = document.createElement("span");
     document.body.appendChild(node);
     node.style.cssText = "--a:serif;font-family:var(--a,monospace)";
     const acyclic = getComputedStyle(node).fontFamily;
     node.style.cssText = "--a:var(--a,serif);font-family:var(--a,monospace)";
     const cyclic = getComputedStyle(node).fontFamily;
     node.style.cssText = "--a:Noto Sans;font-family:var(--a,monospace)";
     return [acyclic, cyclic, getComputedStyle(node).fontFamily];`,
    ["serif", "monospace", "Noto Sans"],
  ],
  [
    "parent mutation and reparenting start a fresh resolution scope",
    `const parent = document.createElement("div");
     parent.style.fontFamily = "Noto Sans";
     document.body.appendChild(parent);
     const child = document.createElement("span");
     child.style.fontFamily = "var(--missing)";
     parent.appendChild(child);
     const first = getComputedStyle(child).fontFamily;
     parent.style.fontFamily = "serif";
     const changed = getComputedStyle(child).fontFamily;
     const other = document.createElement("div");
     other.style.fontFamily = "monospace";
     document.body.appendChild(other);
     other.appendChild(child);
     return [first, changed, getComputedStyle(child).fontFamily];`,
    ["Noto Sans", "serif", "monospace"],
  ],
  [
    "diamond dependencies retain shared values in either read-root order",
    `const node = document.createElement("span");
     document.body.appendChild(node);
     node.style.cssText = "--a:var(--b),var(--c);--b:var(--d);--c:var(--d);--d:serif;font-family:var(--a)";
     const first = getComputedStyle(node).fontFamily;
     node.style.fontFamily = "var(--c),var(--a)";
     const reversed = getComputedStyle(node).fontFamily;
     node.style.cssText = "--a:var(--b),var(--c);--b:var(--d);--c:var(--d);--d:var(--d);font-family:var(--a,monospace)";
     return [first, reversed, getComputedStyle(node).fontFamily];`,
    ["serif,serif", "serif,serif,serif", "monospace"],
  ],
] as const)("%s", (_name, script, expected) => {
  expect(probe<readonly string[]>(script)).toEqual(expected);
});
