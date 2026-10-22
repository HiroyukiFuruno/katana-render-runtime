import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createContext, runInContext } from "node:vm";
import { parseSync } from "@swc/core";
import { minify } from "terser";
import { z } from "zod";
import {
  MATHJAX_ABI,
  MATHJAX_DECL_PROFILE,
  MATHJAX_PROVENANCE,
  MATHJAX_RUNTIME,
  MATHJAX_TYPE_PROFILE,
} from "../quality/mathjax-declaration-contract";
import { containsRuntimeModuleSyntax } from "./runtime-bundle-checks";
import { terserReservedRuntimeEntryNames } from "./runtime-entry-names";

test("CSSと文字列内のimport/exportをモジュール構文として扱わない", () => {
  const source = String.raw`
    const css = \`@import url(\"font.woff2\");\`;
    const message = "export default is documentation";
  `;

  expect(containsRuntimeModuleSyntax(source)).toBeFalse();
});

test("静的および動的なモジュール構文を検出する", () => {
  expect(containsRuntimeModuleSyntax('import value from "dependency";')).toBeTrue();
  expect(containsRuntimeModuleSyntax("export const value = 1;")).toBeTrue();
  expect(containsRuntimeModuleSyntax('import("dependency");')).toBeTrue();
  expect(containsRuntimeModuleSyntax("import.meta.url;")).toBeTrue();
});

test("evalラッパー内の実コードだけをモジュール構文として検査する", () => {
  expect(
    containsRuntimeModuleSyntax(
      String.raw`globalThis.eval('const css=\`@import url("font.woff2");\`;');`,
    ),
  ).toBeFalse();
  expect(containsRuntimeModuleSyntax(`globalThis.eval('export const value=1;');`)).toBeTrue();
});

test("置換なしtemplate literalのeval payloadを検査する", () => {
  const templateWithSubstitution = "globalThis.eval(`export const $" + "{value}=1`);";

  expect(containsRuntimeModuleSyntax('globalThis.eval(`import "dependency"`);')).toBeTrue();
  expect(containsRuntimeModuleSyntax("globalThis.eval(`export const value=1;`);")).toBeTrue();
  expect(containsRuntimeModuleSyntax("globalThis.eval(`\\u0065xport const value=1;`);")).toBeTrue();
  expect(containsRuntimeModuleSyntax(templateWithSubstitution)).toBeFalse();
});

const root = path.resolve(import.meta.dir, "../..");
const utilityFiles = [
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_transform_bounds.js",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_bounds_attrs.js",
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_group_utils.js",
  "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/drawio_svg_shape_colors.js",
].map((file) => path.join(root, file));
const utilityConfig = path.join(root, "scripts/quality/tsconfig-owned-native-pure-utilities.json");

function compilerVersion(): string {
  const metadata: unknown = JSON.parse(
    fs.readFileSync(path.join(root, "node_modules/typescript/package.json"), "utf8"),
  );
  if (typeof metadata !== "object" || metadata === null || !("version" in metadata)) {
    throw new Error("TypeScript package version is missing");
  }
  if (typeof metadata.version !== "string") throw new Error("TypeScript version must be a string");
  return metadata.version;
}

test("実utility sourceとstrict設定に束縛したcompilerが誤入力とDOMを拒否する", () => {
  const version = spawnSync("rtk", ["proxy", "bunx", "--no-install", "tsc", "--version"], {
    cwd: root,
    encoding: "utf8",
  });
  expect(version.error).toBeUndefined();
  expect(version.status).toBe(0);
  expect(version.stdout.trim()).toBe(`Version ${compilerVersion()}`);
  const configBefore = fs.readFileSync(utilityConfig);
  const sourceBefore = utilityFiles.map((file) => fs.readFileSync(file));
  const config: unknown = JSON.parse(configBefore.toString("utf8"));
  expect(config).toMatchObject({
    compilerOptions: {
      lib: ["ES2024"],
      types: [],
      allowJs: true,
      checkJs: true,
      strict: true,
      noImplicitAny: true,
      strictNullChecks: true,
      noUncheckedIndexedAccess: true,
      exactOptionalPropertyTypes: true,
      useUnknownInCatchVariables: true,
      noUnusedLocals: true,
      noUnusedParameters: true,
    },
    files: utilityFiles.map((file) => path.relative(path.dirname(utilityConfig), file)),
  });
  const inputs = [
    {
      valid: true,
      diagnostic: "",
      source:
        'katanaAttrNumber("", "x"); katanaTranslate("")[0].toFixed(); katanaRewriteBalancedGroups("<g></g>", /<g>/g, group => group); KATANA_DRAWIO_SHAPE_COLOR_MAPS[1].set("fill|#000000", "#ededed");',
    },
    { valid: false, diagnostic: "TS2345", source: "katanaTranslate(17);" },
    {
      valid: false,
      diagnostic: "TS2322",
      source: 'const forced: number = katanaAttrNumber("", "x"); forced.toFixed();',
    },
    {
      valid: false,
      diagnostic: "TS2345",
      source: 'katanaRewriteBalancedGroups("", "pattern", group => group);',
    },
    {
      valid: false,
      diagnostic: "TS2322",
      source: 'katanaRewriteBalancedGroups("", /<g>/g, group => group.length);',
    },
    {
      valid: false,
      diagnostic: "TS2345",
      source: 'KATANA_DRAWIO_SHAPE_COLOR_MAPS[0].set("fill|#000000", 17);',
    },
    {
      valid: false,
      diagnostic: "TS2493",
      source: 'KATANA_DRAWIO_SHAPE_COLOR_MAPS[2].get("fill|#000000");',
    },
    {
      valid: false,
      diagnostic: "TS2322",
      source:
        'const state: KatanaBalancedGroupDepthState = { depth: 0, end: "-1" }; katanaCaptureGroupEnd(state, /g/);',
    },
    { valid: false, diagnostic: "TS2304", source: "document.body; window.location;" },
  ];
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-native-utility-types-"));
  try {
    const input = path.join(directory, "input.ts");
    const project = path.join(directory, "tsconfig.json");
    fs.writeFileSync(
      project,
      JSON.stringify({ extends: utilityConfig, files: [...utilityFiles, input] }),
    );
    for (const { valid, source, diagnostic } of inputs) {
      fs.writeFileSync(input, source);
      const result = spawnSync(
        "rtk",
        ["proxy", "bunx", "--no-install", "tsc", "--noEmit", "-p", project],
        { cwd: root, encoding: "utf8" },
      );
      expect(result.error).toBeUndefined();
      expect(result.status, source).toBe(valid ? 0 : 1);
      if (!valid) {
        expect(result.stdout, source).toContain("input.ts(");
        expect(result.stdout, source).toContain(diagnostic);
      }
    }
  } finally {
    fs.rmSync(directory, { recursive: true });
  }
  expect(fs.readFileSync(utilityConfig)).toEqual(configBefore);
  for (const [index, file] of utilityFiles.entries()) {
    const before = sourceBefore[index];
    if (before === undefined) throw new Error("Missing utility source snapshot");
    expect(fs.readFileSync(file)).toEqual(before);
  }
}, 30000);

test("実classicScript utilityの属性・入れ子group・paletteを純粋VMで確認する", () => {
  const context = createContext({});
  for (const file of utilityFiles) runInContext(fs.readFileSync(file, "utf8"), context);
  const result: unknown = runInContext(
    `
    const pattern = /<g\\b[^>]*>/g;
    const calls = [];
    const rewritten = katanaRewriteBalancedGroups("a<g><g>x</g></g>b<g>y</g>c", pattern,
      group => { calls.push(group); return "[" + group + "]"; });
    ({
      translated: katanaTranslate('transform="translate(2, -3.5)"'),
      untranslated: katanaTranslate(""),
      nonFinite: Number.isNaN(katanaTranslate('transform="translate(--, 3)"')[0]),
      number: katanaAttrNumber('<rect x="2.5" missing="no">', "x"),
      invalidNumber: katanaAttrNumber('<rect x="no">', "x"),
      missingNumber: katanaAttrNumber("", "x"),
      text: katanaAttrText('<rect title="text">', "title"),
      missingText: katanaAttrText("", "title"),
      rewritten, calls, lastIndex: pattern.lastIndex,
      unclosed: katanaRewriteBalancedGroups("a<g>x", /<g>/g, group => "[" + group + "]"),
      mapSizes: KATANA_DRAWIO_SHAPE_COLOR_MAPS.map(map => map.size),
      fill: KATANA_DRAWIO_SHAPE_COLOR_MAPS[1].get("fill|#000000"),
      stroke: KATANA_DRAWIO_SHAPE_COLOR_MAPS[1].get("stroke|#5e5e5e"),
      absent: KATANA_DRAWIO_SHAPE_COLOR_MAPS[0].get("fill|#000000"),
      sharedPalette: KATANA_DRAWIO_SHAPE_COLOR_MAPS[1] === KATANA_DRAWIO_AWS3D_DARK_COLOR,
    });
  `,
    context,
  );
  expect(result).toEqual({
    translated: [2, -3.5],
    untranslated: [0, 0],
    nonFinite: true,
    number: 2.5,
    invalidNumber: null,
    missingNumber: null,
    text: "text",
    missingText: null,
    rewritten: "a[<g><g>x</g></g>]b[<g>y</g>]c",
    calls: ["<g><g>x</g></g>", "<g>y</g>"],
    lastIndex: 0,
    unclosed: "a<g>x",
    mapSizes: [0, 8],
    fill: "#ededed",
    stroke: "#9c9c9c",
    absent: undefined,
    sharedPalette: true,
  });
});

test("実browser globalsのstorage clearは全keyを削除しgetterを空状態へ戻す", () => {
  const source = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/browser_globals.js",
  );
  const storageCases = [
    {
      entries: [
        ["first", "one"],
        ["second", "two"],
      ],
      before: [2, "first", "one"],
    },
    { entries: [["only", "value"]], before: [1, "only", "value"] },
    { entries: [], before: [0, null, null] },
  ];
  for (const storageCase of storageCases) {
    const context = createContext({ storageEntries: storageCase.entries });
    runInContext(fs.readFileSync(source, "utf8"), context);
    const result: unknown = runInContext(
      `for (const [key, value] of storageEntries) localStorage.setItem(key, value);
       const before = [localStorage.length, localStorage.key(0), localStorage.getItem(storageEntries[0]?.[0] ?? "")];
       localStorage.clear();
       JSON.stringify({ before, after: [localStorage.length, localStorage.key(0), localStorage.getItem("first"), localStorage.getItem("only")] });`,
      context,
    );
    expect(result).toBe(
      JSON.stringify({ before: storageCase.before, after: [0, null, null, null] }),
    );
  }
});

test("実Bezierとpath bounds utilityは極値と座標group stateを保持する", () => {
  const runtimeDirectory = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime",
  );
  const pathCases = [
    {
      tokens: "1 2 3 4 5 6",
      initialX: 0,
      initialY: 0,
      relative: false,
      pointCount: 3,
      points: [1, 2, 3, 4, 5, 6],
      state: [6, 5, 6],
    },
    {
      tokens: "1 2 3 4",
      initialX: 10,
      initialY: 20,
      relative: true,
      pointCount: 2,
      points: [11, 22, 13, 24],
      state: [4, 13, 24],
    },
  ];
  for (const pathCase of pathCases) {
    const caseContext = createContext({ pathCase });
    for (const name of ["svg_path_bezier_bounds.js", "svg_path_bounds.js"]) {
      runInContext(fs.readFileSync(path.join(runtimeDirectory, name), "utf8"), caseContext);
    }
    const result: unknown = runInContext(
      `(() => {
         const parser = katanaPathParser("M0 0 C100 0 100 100 0 100");
         while (parser.index < parser.tokens.length) katanaReadPathCommand(parser);
         const groups = katanaPathParser(pathCase.tokens);
         groups.x = pathCase.initialX;
         groups.y = pathCase.initialY;
         katanaReadPathPointGroup(groups, pathCase.relative, pathCase.pointCount);
         return JSON.stringify({
           box: katanaNumberPairsBox(parser.points),
           points: parser.points,
           groupPoints: groups.points,
           groupState: [groups.index, groups.x, groups.y],
         });
       })()`,
      caseContext,
    );
    expect(result).toBe(
      JSON.stringify({
        box: [0, 0, 75, 100],
        points: [0, 0, 75, 50, 0, 100],
        groupPoints: pathCase.points,
        groupState: pathCase.state,
      }),
    );
  }
});

test("実pie legendの色補正はroot属性と図形geometryを保持する", () => {
  const source = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_visual_fixes.js",
  );
  const context = createContext({});
  runInContext(fs.readFileSync(source, "utf8"), context);
  const cases = [
    { label: "Dogs", fill: "#111111" },
    { label: "Long label", fill: "#222222" },
    { label: "日本語の凡例", fill: "#333333" },
  ];
  for (const { label, fill } of cases) {
    const svg = `<svg width="100%" viewBox="0 0 546.96875 450" aria-roledescription="pie"><path fill="${fill}" class="pieCircle" d="M0 0 L186 0"/><g class="legend"><rect style="fill: old; stroke: old;"></rect><text>${label}</text></g></svg>`;
    const result: unknown = runInContext(`katanaNormalizePieSvg(${JSON.stringify(svg)})`, context);
    expect(result).toBe(
      `<svg width="100%" viewBox="0 0 546.96875 450" aria-roledescription="pie"><path fill="${fill}" class="pieCircle" d="M0 0 L186 0"/><g class="legend"><rect style="fill: ${fill}; stroke: ${fill};"></rect><text>${label}</text></g></svg>`,
    );
  }
});

function compileMathJaxProject(project: string) {
  return spawnSync(
    "rtk",
    ["proxy", "bunx", "--no-install", "tsc", "--noEmit", "--pretty", "false", "-p", project],
    { cwd: root, encoding: "utf8" },
  );
}
const mathJaxRuntime = path.join(root, MATHJAX_RUNTIME);
const mathJaxHost = path.join(root, MATHJAX_ABI);
function mathJaxOriginalBody(): string {
  const current = fs.readFileSync(mathJaxRuntime, "utf8");
  const restored = current
    .replace(/\/\*\*[\s\S]*?\*\//g, "")
    .replace(/\(error\)\.stack/g, "error.stack");
  // 型注釈と必要な括弧だけの差を、原演算のtoken列と既存minifierで検査する。
  return restored;
}
function errorResponseOperation(source: string): (error: unknown) => unknown {
  const marker = "  } catch (error) {";
  const begin = source.indexOf(marker) + marker.length;
  const end = source.indexOf("\n  }\n};", begin);
  if (begin < marker.length || end < begin) throw new Error("MathJax error body is missing");
  const program = new Function("error", source.slice(begin, end));
  return (error: unknown): unknown => program(error);
}

test("MathJax reviewed annotations preserve original runtime operations and actual minified fragment bytes", async () => {
  const source = fs.readFileSync(mathJaxRuntime, "utf8");
  const provenance = z
    .object({ owned: z.object({ runtimeSHA256: z.string() }) })
    .parse(JSON.parse(fs.readFileSync(path.join(root, MATHJAX_PROVENANCE), "utf8")));
  expect(createHash("sha256").update(source).digest("hex")).toBe(provenance.owned.runtimeSHA256);
  const original = mathJaxOriginalBody();
  const operations = JSON.stringify(
    parseSync(original, { syntax: "ecmascript" }),
    (key: string, value: unknown) =>
      ["span", "ctxt", "bodyCtxt"].includes(key) ? undefined : value,
  );
  expect(createHash("sha256").update(operations).digest("hex")).toBe(
    "86c65396e7cde72415d19305ec0a8e7d95cea46655a1c7f45a77b010f12dd2c4",
  );
  for (const mangle of [false, true]) {
    const options = {
      compress: { passes: 2, toplevel: true },
      format: { comments: false },
      keep_classnames: true,
      keep_fnames: true,
      mangle: mangle ? { reserved: terserReservedRuntimeEntryNames(), toplevel: true } : false,
      module: false,
      sourceMap: false,
      toplevel: true,
    };
    const restored = await minify(original, options);
    const after = await minify(source, options);
    expect(restored.code).toBeDefined();
    expect(after.code).toBe(restored.code);
    if (after.code === undefined) throw new Error("MathJax minifier output missing");
    expect(createHash("sha256").update(after.code).digest("hex")).toBe(
      mangle
        ? "736818570e10e57fc1956b7f272de8e1ac36c59799492a8b82f251b3ee6d1b78"
        : "4b31fe2ccef781731f6793111983141046894eb2a058e011523dbb008d7c59cd",
    );
  }
});

test("MathJax actual source rejects owned ABI, library, unknown and DOM misuse under strict compiler", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-mathjax-types-"));
  const before = fs.readFileSync(mathJaxRuntime);
  try {
    const input = path.join(directory, "input.ts");
    const project = path.join(directory, "tsconfig.json");
    fs.writeFileSync(
      project,
      JSON.stringify({
        extends: path.join(root, MATHJAX_TYPE_PROFILE),
        files: [mathJaxRuntime, mathJaxHost, input],
      }),
    );
    const lite = path
      .join(root, "node_modules/@mathjax/src/mjs/adaptors/liteAdaptor.js")
      .replaceAll(path.sep, "/");
    const cases = [
      [
        true,
        "",
        'globalThis.katanaRunMathJaxRuntime({source:"x", display:false, text:"#000", darkMode:false});',
      ],
      [
        false,
        "TS2322",
        'globalThis.katanaRunMathJaxRuntime({source:"x", display:"true", text:"#000", darkMode:false});',
      ],
      [
        false,
        "TS2741",
        'globalThis.katanaRunMathJaxRuntime({source:"x", display:false, text:"#000"});',
      ],
      [
        false,
        "TS2345",
        `import {liteAdaptor} from ${JSON.stringify(lite)}; liteAdaptor().outerHTML("svg");`,
      ],
      [false, "TS2304", "document.body; window.location;"],
      [false, "TS7006", "export function implicit(value) { return value; }"],
      [false, "TS6133", "export {}; const unused = 1;"],
      [
        false,
        "TS18046",
        "export function unknownCatch() { try { throw 1; } catch (error) { return error.stack; } }",
      ],
      [
        false,
        "TS18046",
        "export function unknownStack() { const error: unknown = null; const stack = (error as {readonly stack?:unknown}).stack; return stack.toUpperCase(); }",
      ],
    ] as const;
    for (const [valid, diagnostic, source] of cases) {
      fs.writeFileSync(input, source);
      const result = compileMathJaxProject(project);
      expect(result.error).toBeUndefined();
      if (valid) expect(result.status).toBe(0);
      else {
        expect(result.status).not.toBe(0);
        expect(result.stdout).toContain(diagnostic);
      }
    }
    const poison = path.join(directory, "poison.d.ts");
    fs.writeFileSync(poison, "interface OwnedPoison { value: MissingOwnedType; }\n");
    for (const [profile, expected] of [
      [MATHJAX_TYPE_PROFILE, 0],
      [MATHJAX_DECL_PROFILE, 1],
    ] as const) {
      fs.writeFileSync(
        project,
        JSON.stringify({
          extends: path.join(root, profile),
          files:
            profile === MATHJAX_TYPE_PROFILE
              ? [mathJaxRuntime, mathJaxHost, poison]
              : [mathJaxHost, poison],
        }),
      );
      const result = compileMathJaxProject(project);
      expect(result.error).toBeUndefined();
      if (expected === 0) expect(result.status).toBe(0);
      else {
        expect(result.status).not.toBe(0);
        expect(result.stdout).toContain("TS2304");
      }
    }
  } finally {
    fs.rmSync(directory, { recursive: true });
  }
  expect(fs.readFileSync(mathJaxRuntime)).toEqual(before);
});

type StackEvent = string | { read: number; receiver?: boolean; boxed?: string };
type ErrorFactory = (events: StackEvent[], sentinel: object) => unknown;
function stackObservation(operation: (error: unknown) => unknown, factory: ErrorFactory) {
  const events: StackEvent[] = [];
  const sentinel = { marker: "original-throw" };
  try {
    return { kind: "return", value: operation(factory(events, sentinel)), events };
  } catch (error) {
    return {
      kind: "throw",
      exact: error === sentinel,
      name: error instanceof Error ? error.name : null,
      message: error instanceof Error ? error.message : null,
      events,
    };
  }
}
test("MathJax property read annotations preserve real unknown/proxy/getter/primitive error behavior", () => {
  const before = errorResponseOperation(mathJaxOriginalBody());
  const after = errorResponseOperation(fs.readFileSync(mathJaxRuntime, "utf8"));
  const primitiveValues: [string, unknown][] = [
    ["undefined", undefined],
    ["null", null],
    ["false", false],
    ["zero", 0],
    ["NaN", NaN],
    ["empty", ""],
    ["true", true],
    ["number", 42],
    ["bigint", 2n],
    ["symbol", Symbol("error")],
    ["text", "plain"],
  ];
  const stackValues: [string, unknown][] = [
    ["stack undefined", undefined],
    ["stack null", null],
    ["stack zero", 0],
    ["stack false", false],
    ["stack empty", ""],
    ["stack string", "trace"],
    ["stack number", 42],
    ["stack object", { data: 1 }],
    ["stack symbol", Symbol("trace")],
    ["stack bigint", 2n],
  ];
  const cases: [string, ErrorFactory][] = [
    ...primitiveValues.map(([label, value]): [string, ErrorFactory] => [label, () => value]),
    ...stackValues.map(([label, value]): [string, ErrorFactory] => [
      label,
      () => ({ stack: value }),
    ]),
    [
      "getter two reads original receiver",
      (events) => {
        let reads = 0;
        const value = {
          get stack() {
            events.push({ read: ++reads, receiver: this === value });
            return reads === 1 ? "truthy" : 0;
          },
        };
        return value;
      },
    ],
    [
      "getter one read fallback",
      (events) => ({
        get stack() {
          events.push("stack");
          return "";
        },
        toString() {
          events.push("toString");
          return "fallback";
        },
      }),
    ],
    [
      "getter throws original value",
      (events, sentinel) => ({
        get stack() {
          events.push("stack");
          throw sentinel;
        },
      }),
    ],
    [
      "fallback coercion throws original value",
      (events, sentinel) => ({
        get stack() {
          events.push("stack");
          return undefined;
        },
        toString() {
          events.push("toString");
          throw sentinel;
        },
      }),
    ],
    [
      "proxy reads and fallback",
      (events) =>
        new Proxy(
          {},
          {
            get(target, key, receiver) {
              events.push(String(key));
              return Reflect.get(target, key, receiver);
            },
          },
        ),
    ],
    [
      "proxy stack throws",
      (events, sentinel) =>
        new Proxy(
          {},
          {
            get(_target, key) {
              events.push(String(key));
              throw sentinel;
            },
          },
        ),
    ],
  ];
  for (const [label, factory] of cases)
    expect(stackObservation(after, factory), label).toEqual(stackObservation(before, factory));
  const descriptor = Object.getOwnPropertyDescriptor(String.prototype, "stack");
  try {
    let events: StackEvent[] = [];
    let reads = 0;
    Object.defineProperty(String.prototype, "stack", {
      configurable: true,
      get(this: unknown) {
        events.push({ read: ++reads, boxed: Object.prototype.toString.call(this) });
        return reads === 1 ? "truthy" : "second";
      },
    });
    const observe = (operation: (error: unknown) => unknown) => {
      events = [];
      reads = 0;
      return { value: operation("primitive"), events };
    };
    expect(observe(after)).toEqual(observe(before));
  } finally {
    if (descriptor) Object.defineProperty(String.prototype, "stack", descriptor);
    else Reflect.deleteProperty(String.prototype, "stack");
  }
  const originalRead = new Function("error", "return error.stack;");
  const annotatedRead = new Function(
    "error",
    "return /** @type {{readonly stack?: unknown}} */ (error).stack;",
  );
  for (const value of [null, undefined]) {
    expect(() => originalRead(value)).toThrow(TypeError);
    expect(() => annotatedRead(value)).toThrow(TypeError);
  }
  expect(cases.length + 3).toBe(30);
});
