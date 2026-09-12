import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";

const SEQUENCE_FIXES = readFileSync(
  new URL(
    "../../crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/svg_sequence_activation_fixes.js",
    import.meta.url,
  ),
  "utf8",
);

type SvgNormalizer = (svg: string) => string;

function normalize(svg: string): string {
  const context = createContext({});
  runInContext(SEQUENCE_FIXES, context);
  return (runInContext("katanaNormalizeSequenceActivationSvg", context) as SvgNormalizer)(svg);
}

test("review 正規化後の Mermaid 12 simple sequence を公式 geometry へ補正する", () => {
  const normalized = normalize(`
    <svg width="450" height="309" viewBox="-50 -10 450 309" aria-roledescription="sequence">
      <rect height="75" y="183"/><text y="220.5"/><line y1="75" y2="183"/><text y="37.5"/>
      <text y="90"/><line y1="119" y2="119" data-from="User" data-to="App"/>
      <text y="134"/><line y1="163" y2="163" data-from="App" data-to="User"/>
    </svg>`);

  expect(normalized).toContain('height="281"');
  expect(normalized).toContain('viewBox="-50 -10 450 311"');
  expect(normalized).toContain('height="74" y="186"');
  expect(normalized).toContain('y1="120" y2="120"');
  expect(normalized).toContain('y1="166" y2="166"');
});

test("review 正規化後の Mermaid 12 EN activation sequence を公式 geometry へ補正する", () => {
  const normalized = normalize(`
    <svg width="500" height="397" viewBox="-50 -10 500 397" aria-roledescription="sequence">
      <rect class="activation1" height="75" y="271"/><text y="308.5"/><line y1="75" y2="271"/><text y="37.5"/>
      <rect y="123"/><rect y="173"/><text x="195" y="90"/><line y1="119" y2="119" x2="314" data-from="Alice" data-to="John"/>
      <text x="201" y="134"/><line y1="167" y2="167"/><text y="178"/><line y1="211" y2="211"/>
      <text y="222"/><line y1="259" y2="259" data-from="John" data-to="Alice"/>
    </svg>`);

  expect(normalized).toContain('height="373"');
  expect(normalized).toContain('width="501"');
  expect(normalized).toContain('viewBox="-50 -10 501 403"');
  expect(normalized).toContain('x="196" y="89"');
  expect(normalized).toContain('y1="166" y2="166"');
  expect(normalized).toContain('y1="258" y2="258"');
});

test("review 正規化後の Mermaid 12 JA activation sequence を公式 geometry へ補正する", () => {
  const normalized = normalize(`
    <svg width="595" height="397" viewBox="-50 -10 595 397" aria-roledescription="sequence">
      <rect class="activation1" height="75" y="271"/><text y="308.5"/><line y1="75" y2="271"/><text y="37.5"/>
      <rect y="123"/><rect y="173"/><text x="243" y="90"/><line y1="119" y2="119" x2="409" data-from="田中" data-to="鈴木"/>
      <text x="249" y="134"/><line y1="167" y2="167"/><text y="178"/><line y1="211" y2="211"/>
      <text y="222"/><line y1="259" y2="259" data-from="鈴木" data-to="田中"/>
    </svg>`);

  expect(normalized).toContain('viewBox="-50 -10 571 403"');
  expect(normalized).toContain('width="571"');
  expect(normalized).toContain('x="231" y="89"');
  expect(normalized).toContain('x="237" y="135"');
  expect(normalized).toContain('x2="385"');
});

test("Mermaid 12 EN activation の review 前の width=500 を先に補正する", () => {
  const normalized = normalize(`
    <svg width="500" height="405" viewBox="-50 -10 500 405" aria-roledescription="sequence">
      <rect class="activation1" height="46"/><rect height="144" x="320"/><text x="325"/>
      <line x1="320" x2="317" data-from="Alice" data-to="John"/>
      <line data-from="John" data-to="Alice"/>
    </svg>`);

  expect(normalized).toContain('width="501"');
  expect(normalized).toContain('height="373"');
  expect(normalized).toContain('x="321"');
  expect(normalized).toContain('height="138"');
  expect(normalized).toContain('height="44"');
});

test("Mermaid 12 JA activation の review 前の width=595 を先に補正する", () => {
  const normalized = normalize(`
    <svg width="595" height="405" viewBox="-50 -10 595 405" aria-roledescription="sequence">
      <rect class="activation1" height="46"/><rect height="144" x="345"/><text x="420"/>
      <line x1="420" x2="412" data-from="田中" data-to="鈴木"/>
      <line data-from="鈴木" data-to="田中"/><text x="244"/><text x="247"/>
    </svg>`);

  expect(normalized).toContain('width="571"');
  expect(normalized).toContain('height="373"');
  expect(normalized).toContain('x="321"');
  expect(normalized).toContain('x="396"');
  expect(normalized).toContain('x2="388"');
});

test("旧 Mermaid 11 activation 補正を維持する", () => {
  const normalized = normalize(`
    <svg width="500" viewBox="-50 -10 500 355" aria-roledescription="sequence">
      <rect class="activation1" y="113" height="144"/><rect height="46"/><line y1="159" y2="159"/>
      <line y1="205" y2="205"/><line y1="253" y2="253"/>Hello John, how are you?
    </svg>`);

  expect(normalized).toContain('width="501"');
  expect(normalized).toContain('y="111" height="138"');
  expect(normalized).toContain('y1="157" y2="157"');
  expect(normalized).toContain('y1="249" y2="249"');
});

test("対象外の sequence は変えない", () => {
  const svg = '<svg aria-roledescription="sequence" width="999"></svg>';
  expect(normalize(svg)).toBe(svg);
});
