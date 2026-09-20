import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
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

function sampleDateCallInTimezone(timezone: string): string {
  const script = `
    const installDrawioDeterminism = ${installDrawioDeterminism.toString()};
    installDrawioDeterminism();
    process.stdout.write(Date());
  `;
  const result = spawnSync(process.execPath, ["-e", script], {
    env: { ...process.env, TZ: timezone },
    encoding: "utf8",
  });
  expect(result.status).toBe(0);
  expect(result.stderr).toBe("");
  return result.stdout;
}

function sampleDateToStringInTimezone(timezone: string): string {
  const script = `
    const installDrawioDeterminism = ${installDrawioDeterminism.toString()};
    installDrawioDeterminism();
    process.stdout.write(new Date("2024-02-03T04:05:06.000Z").toString());
  `;
  const result = spawnSync(process.execPath, ["-e", script], {
    env: { ...process.env, TZ: timezone },
    encoding: "utf8",
  });
  expect(result.status).toBe(0);
  expect(result.stderr).toBe("");
  return result.stdout;
}

function sampleLocaleCalls(
  timezone: string,
  locale: string,
): {
  date: string;
  string: string;
  time: string;
} {
  const script = `
    const installDrawioDeterminism = ${installDrawioDeterminism.toString()};
    installDrawioDeterminism();
    process.stdout.write(JSON.stringify({
      date: new Date().toLocaleDateString(),
      string: new Date().toLocaleString(),
      time: new Date().toLocaleTimeString(),
    }));
  `;
  const result = spawnSync(process.execPath, ["-e", script], {
    env: { ...process.env, TZ: timezone, LANG: locale, LC_ALL: locale },
    encoding: "utf8",
  });
  expect(result.status).toBe(0);
  expect(result.stderr).toBe("");
  return JSON.parse(result.stdout) as { date: string; string: string; time: string };
}

function sampleFormattedPlaceholderInTimezone(timezone: string): string {
  const script = `
    const installDrawioDeterminism = ${installDrawioDeterminism.toString()};
    installDrawioDeterminism();
    const date = new Date();
    const pad = (value) => String(value).padStart(2, "0");
    process.stdout.write(
      [date.getFullYear(), pad(date.getMonth() + 1), pad(date.getDate())].join("-") +
        "T" +
        [pad(date.getHours()), pad(date.getMinutes()), pad(date.getSeconds())].join(":") +
        "Z",
    );
  `;
  const result = spawnSync(process.execPath, ["-e", script], {
    env: { ...process.env, TZ: timezone },
    encoding: "utf8",
  });
  expect(result.status).toBe(0);
  expect(result.stderr).toBe("");
  return result.stdout;
}

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

test("公式 Draw.io renderer は Date constructor を固定し静的APIとprototypeを維持する", () => {
  const context = createOfficialRendererContext();
  const sample = runInContext(
    `(() => {
      const explicit = new Date("2024-02-03T04:05:06.000Z");
      const explicitUndefined = new Date(undefined);
      return {
        implicit: new Date().getTime(),
        called: Date(),
        explicit: explicit.getTime(),
        explicitUndefinedIsInvalid: Number.isNaN(explicitUndefined.getTime()),
        parsed: Date.parse("2024-02-03T04:05:06.000Z"),
        utc: Date.UTC(2024, 1, 3, 4, 5, 6),
        prototypeCompatible: Object.getPrototypeOf(explicit) === Date.prototype,
        datePrototypeCompatible: Date.prototype.toISOString.call(explicit),
      };
    })()`,
    context,
  ) as {
    implicit: number;
    called: string;
    explicit: number;
    explicitUndefinedIsInvalid: boolean;
    parsed: number;
    utc: number;
    prototypeCompatible: boolean;
    datePrototypeCompatible: string;
  };

  expect(sample).toEqual({
    implicit: FIXED_NOW,
    called: new Date(FIXED_NOW).toUTCString(),
    explicit: Date.parse("2024-02-03T04:05:06.000Z"),
    explicitUndefinedIsInvalid: true,
    parsed: Date.parse("2024-02-03T04:05:06.000Z"),
    utc: Date.UTC(2024, 1, 3, 4, 5, 6),
    prototypeCompatible: true,
    datePrototypeCompatible: "2024-02-03T04:05:06.000Z",
  });
});

test("公式 Draw.io renderer の Date() はホストのタイムゾーンに依存しない", () => {
  const utc = sampleDateCallInTimezone("UTC");
  const pacific = sampleDateCallInTimezone("America/Los_Angeles");

  expect(utc).toBe("Thu, 01 Jan 2026 00:00:00 GMT");
  expect(pacific).toBe(utc);
});

test("公式 Draw.io renderer の Date.prototype.toString はホストのタイムゾーン名に依存しない", () => {
  const utc = sampleDateToStringInTimezone("UTC");
  const pacific = sampleDateToStringInTimezone("America/Los_Angeles");

  expect(utc).toBe("Sat Feb 03 2024 04:05:06 GMT+0000 (UTC)");
  expect(pacific).toBe(utc);
});

test("公式 Draw.io renderer の locale Date API はホストのタイムゾーンとロケールに依存しない", () => {
  const utc = sampleLocaleCalls("UTC", "en_US.UTF-8");
  const pacific = sampleLocaleCalls("America/Los_Angeles", "de_DE.UTF-8");

  expect(utc).toEqual({
    date: "2026-01-01",
    string: "2026-01-01T00:00:00.000Z",
    time: "00:00:00",
  });
  expect(pacific).toEqual(utc);
});

test("公式 Draw.io renderer の formatted date placeholder はホストのタイムゾーンに依存しない", () => {
  const utc = sampleFormattedPlaceholderInTimezone("UTC");
  const pacific = sampleFormattedPlaceholderInTimezone("America/Los_Angeles");

  expect(utc).toBe("2026-01-01T00:00:00Z");
  expect(pacific).toBe(utc);
});
