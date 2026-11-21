import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";

function probe<T>(script: string, runtime = "mermaid"): T {
  const bundle = new URL(
    `../../crates/katana-render-runtime/src/markdown/diagram_runtime/generated/${runtime}-runtime.min.js`,
    import.meta.url,
  );
  const context = createContext({});
  runInContext(readFileSync(bundle, "utf8"), context);
  return runInContext(`(() => { ${script} })()`, context) as T;
}

for (const runtime of ["drawio", "zenuml"]) {
  test(`${runtime} shared DOM installs the declaration parser independently`, () => {
    const actual = probe<{ weight: string; priority: string }>(
      `const node = document.createElement("span");
       node.style.cssText = "FONT-WEIGHT: 700 !IMPORTANT";
       return { weight: node.style.getPropertyValue("font-weight"), priority: node.style.getPropertyPriority("font-weight") };`,
      runtime,
    );
    expect(actual).toEqual({ weight: "700", priority: "important" });
  });
}

test("inline CSS separates important priority and keeps nested value delimiters", () => {
  const actual = probe<Record<string, string>>(`
    const node = document.createElement("span");
    node.setAttribute("style", 'font-weight: 700 !IMPORTANT; font-weight: 400; font-family: "A;B"; background-image: url("a;b:c");');
    return { weight: node.style.getPropertyValue("font-weight"),
      priority: node.style.getPropertyPriority("font-weight"),
      family: node.style.getPropertyValue("font-family"),
      image: node.style.getPropertyValue("background-image") };
  `);
  expect(actual.weight).toBe("700");
  expect(actual.priority).toBe("important");
  expect(actual.family).toBe('"A;B"');
  expect(actual.image).toContain("a;b:c");
});

test("CSS declaration parser recovers invalid entries and comments around priority", () => {
  const actual = probe<{ weight: string; priority: string }>(`
    const node = document.createElement("span");
    node.style.cssText = "broken; font-weight: 700 /* value */ ! /* priority */ important; color: red";
    return { weight: node.style.getPropertyValue("font-weight"), priority: node.style.getPropertyPriority("font-weight") };
  `);
  expect(actual.weight).toBe("700");
  expect(actual.priority).toBe("important");
});

test("priority survives clone and serialization and is removed with its value", () => {
  const actual = probe<{ before: string; cloned: string; removed: string; after: string }>(`
    const node = document.createElement("span");
    node.style.setProperty("font-weight", "700", "important");
    const clone = node.cloneNode();
    const restored = document.createElement("span");
    restored.style.cssText = clone.style.cssText;
    const before = restored.style.getPropertyPriority("font-weight");
    const removed = restored.style.removeProperty("font-weight");
    return { before, cloned: clone.style.getPropertyPriority("font-weight"), removed,
      after: restored.style.getPropertyPriority("font-weight") };
  `);
  expect(actual).toEqual({ before: "important", cloned: "important", removed: "700", after: "" });
});

test("setProperty replaces priority explicitly and rejects invalid priority", () => {
  const actual = probe<{ unchanged: string; changed: string; priority: string }>(`
    const node = document.createElement("span");
    node.style.setProperty("font-weight", "700", "important");
    node.style.setProperty("font-weight", "900", "invalid");
    const unchanged = node.style.fontWeight;
    node.style.setProperty("font-weight", "400");
    return { unchanged, changed: node.style.fontWeight, priority: node.style.getPropertyPriority("font-weight") };
  `);
  expect(actual).toEqual({ unchanged: "700", changed: "400", priority: "" });
});

test("stylesheet important beats normal inline while important inline wins ties", () => {
  const actual = probe<{ stylesheet: string; inline: string }>(`
    const style = document.createElement("style");
    style.textContent = ".label { font-weight: 700 !important; font-weight: 400; } .label { font-weight: 900 !important; } .label { font-weight: 300; }";
    document.body.appendChild(style);
    const node = document.createElement("span");
    node.setAttribute("class", "label");
    node.style.fontWeight = "400";
    document.body.appendChild(node);
    const stylesheet = getComputedStyle(node).getPropertyValue("font-weight");
    node.style.setProperty("font-weight", "700", "important");
    return { stylesheet, inline: getComputedStyle(node).getPropertyValue("font-weight") };
  `);
  expect(actual).toEqual({ stylesheet: "900", inline: "700" });
});

test("empty value removes priority even when the requested priority is invalid", () => {
  const actual = probe<{ value: string; priority: string }>(`
    const node = document.createElement("span");
    node.style.setProperty("font-weight", "700", "important");
    node.style.setProperty("font-weight", "", "invalid");
    return { value: node.style.getPropertyValue("font-weight"), priority: node.style.getPropertyPriority("font-weight") };
  `);
  expect(actual).toEqual({ value: "", priority: "" });
});

test("case-sensitive custom property names survive serialization", () => {
  const actual = probe<{ value: string; priority: string; otherCase: string; length: number }>(`
    const node = document.createElement("span");
    node.style.cssText = "--Brand: red !important";
    const restored = document.createElement("span");
    restored.style.cssText = node.style.cssText;
    return { value: restored.style.getPropertyValue("--Brand"), priority: restored.style.getPropertyPriority("--Brand"),
      otherCase: restored.style.getPropertyValue("--brand"), length: restored.style.length };
  `);
  expect(actual).toEqual({ value: "red", priority: "important", otherCase: "", length: 1 });
});

test("replacing cssText removes dynamic properties as well as their priorities", () => {
  const actual = probe<{ style: string; weight: string; priority: string }>(`
    const node = document.createElement("span");
    node.style.cssText = "font-style: italic; font-weight:700!important";
    node.style.cssText = "color: red";
    return { style: node.style.getPropertyValue("font-style"), weight: node.style.getPropertyValue("font-weight"),
      priority: node.style.getPropertyPriority("font-weight") };
  `);
  expect(actual).toEqual({ style: "", weight: "", priority: "" });
});

test("declaration cache reuses immutable results and evicts older inputs", () => {
  const actual = probe<{ reused: boolean; immutable: boolean; evicted: boolean }>(`
    const first = globalThis.katanaParseCssDeclarations("font-weight:700!important");
    const second = globalThis.katanaParseCssDeclarations("font-weight:700!important");
    for (let i = 0; i < 257; i++) globalThis.katanaParseCssDeclarations("--entry:" + i);
    const later = globalThis.katanaParseCssDeclarations("font-weight:700!important");
    return { reused: first === second, immutable: Object.isFrozen(first) && Object.isFrozen(first[0]), evicted: later !== first };
  `);
  expect(actual).toEqual({ reused: true, immutable: true, evicted: true });
});

test("an important parent weight is inherited without overriding a child's normal run", () => {
  const actual = probe<number[]>(`
    globalThis.__katanaMermaidDiagramType = "flowchart";
    const weights = [];
    globalThis.__katanaMeasureText = (_text, _family, _size, weight) => { weights.push(weight); return 0; };
    const ns = "http://www.w3.org/2000/svg";
    const parent = document.createElementNS(ns, "g");
    parent.style.cssText = "font-weight: 700 !important";
    const child = document.createElementNS(ns, "text");
    child.style.fontWeight = "400";
    child.textContent = "AAA";
    parent.appendChild(child);
    const inherited = document.createElementNS(ns, "text");
    inherited.textContent = "AAA";
    parent.appendChild(inherited);
    const normal = document.createElementNS(ns, "text");
    normal.textContent = "AAA";
    normal.style.fontWeight = "400";
    normal.getComputedTextLength();
    child.getComputedTextLength();
    inherited.getComputedTextLength();
    return weights;
  `);
  expect(actual).toEqual([400, 400, 700]);
});
