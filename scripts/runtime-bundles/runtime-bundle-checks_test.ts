import { expect, test } from "bun:test";
import { containsRuntimeModuleSyntax } from "./runtime-bundle-checks";

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
