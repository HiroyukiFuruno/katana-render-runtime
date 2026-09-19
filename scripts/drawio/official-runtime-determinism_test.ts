import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { type Context, createContext, runInContext } from "node:vm";

import { installDrawioDeterminism } from "./official-runtime-determinism";

const FIXED_NOW = Date.parse("2026-01-01T00:00:00.000Z");
const SHARED_DOM_BROWSER_GLOBALS = readFileSync(
  new URL(
    "../../crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/browser_globals.js",
    import.meta.url,
  ),
  "utf8",
);

type DeterminismSample = {
  clock: number;
  randoms: number[];
};

type Runtime = {
  createContext: () => Context;
};

function createSharedDomContext(): Context {
  const context = createContext({});
  runInContext(SHARED_DOM_BROWSER_GLOBALS, context);
  return context;
}

function createOfficialRendererContext(): Context {
  const context = createContext({});
  const serializedInstall = `(${installDrawioDeterminism.toString()})();`;
  runInContext(serializedInstall, context);
  return context;
}

function sampleDeterminism(context: Context, randomCount: number): DeterminismSample {
  return runInContext(
    `({
      clock: Date.now(),
      randoms: Array.from({ length: ${randomCount} }, () => Math.random()),
    })`,
    context,
  ) as DeterminismSample;
}

const RUNTIMES: Runtime[] = [
  { createContext: createSharedDomContext },
  { createContext: createOfficialRendererContext },
];

test("公式 Draw.io renderer は SharedDom と同じ固定時計および1000回の乱数列を使う", () => {
  const sharedDom = sampleDeterminism(createSharedDomContext(), 1000);
  const officialRenderer = sampleDeterminism(createOfficialRendererContext(), 1000);

  expect(sharedDom.clock).toBe(FIXED_NOW);
  expect(officialRenderer).toEqual(sharedDom);
});

test("各 fresh runtime context は固定時計と100回の乱数列を先頭から再現する", () => {
  for (const runtime of RUNTIMES) {
    const first = sampleDeterminism(runtime.createContext(), 100);
    const second = sampleDeterminism(runtime.createContext(), 100);

    expect(first.clock).toBe(FIXED_NOW);
    expect(second).toEqual(first);
  }
});

test("runtime context ごとの乱数状態は他の context の消費から独立する", () => {
  for (const runtime of RUNTIMES) {
    const advanced = runtime.createContext();
    const untouched = runtime.createContext();

    sampleDeterminism(advanced, 100);
    const untouchedSample = sampleDeterminism(untouched, 100);
    const freshSample = sampleDeterminism(runtime.createContext(), 100);

    expect(untouchedSample).toEqual(freshSample);
    expect(sampleDeterminism(advanced, 100).randoms).not.toEqual(untouchedSample.randoms);
  }
});
