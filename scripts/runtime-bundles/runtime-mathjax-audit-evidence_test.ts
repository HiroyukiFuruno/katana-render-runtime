import { afterAll, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { z } from "zod";
import {
  mathJaxEvidenceFingerprint,
  readMathJaxAuditEvidence,
  validateMathJaxAuditEvidence,
} from "./runtime-mathjax-audit-evidence";
import {
  inputs,
  record,
  runtimeUnusedImportInputs,
  verifyMathJaxToolDependencies,
  verifyRuntimeUnusedImportPins,
} from "./runtime-mathjax-audit-evidence-inputs";
import { inspectUnusedBiomeReport } from "./runtime-unused-biome";
import { readMathJaxOwnedAuditEvidence } from "./runtime-unused-mathjax";

const root = fs.realpathSync(path.resolve(import.meta.dir, "../.."));
const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mathjax-evidence-contract-"));
const objectSchema = z.record(z.string(), z.unknown());
const objectsSchema = z.array(objectSchema);
afterAll(() => fs.rmSync(directory, { recursive: true, force: true }));

function currentReceipt() {
  const pointer = objectSchema.parse(
    JSON.parse(
      fs.readFileSync(path.join(root, "tmp/quality/mathjax-audit-evidence/current.json"), "utf8"),
    ),
  );
  const receiptPath = z.string().parse(pointer.path);
  return objectSchema.parse(JSON.parse(fs.readFileSync(receiptPath, "utf8")));
}

function changePin(value: Record<string, unknown>, field: string, index = 0) {
  const records = objectsSchema.parse(value[field]);
  const pin = records[index];
  if (!pin) throw new Error("missing actual evidence pin");
  pin.sha256 = "0".repeat(64);
  value[field] = records;
}

test("actual current MathJax evidence revalidates physical provenance and both diagnostic outcomes", () => {
  const evidence = readMathJaxAuditEvidence(root);
  expect(evidence.report.canonical.physicalSourceCount).toBe(219);
  expect(evidence.report.canonical.physicalInputPinCount).toBe(3);
  expect(evidence.report.diagnostics).toHaveLength(0);
  expect(evidence.unicode.diagnostics).toHaveLength(1);
  expect(evidence.unicode.exit).toBe(1);
});

test("MathJax evidence fingerprint is stable after complete validation", () => {
  const first = mathJaxEvidenceFingerprint(root);
  expect(first).toMatch(/^[a-f0-9]{64}$/);
  expect(mathJaxEvidenceFingerprint(root)).toBe(first);
});

test("MathJax evidence fingerprint CLI emits the stable schema", () => {
  const child = Bun.spawnSync(
    [process.execPath, "scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts", "fingerprint"],
    {
      cwd: root,
      stdout: "pipe",
      stderr: "pipe",
    },
  );
  expect(child.exitCode).toBe(0);
  expect(JSON.parse(Buffer.from(child.stdout).toString("utf8"))).toMatchObject({
    schema: "krr-mathjax-evidence-fingerprint-v1",
    fingerprint: expect.stringMatching(/^[a-f0-9]{64}$/),
  });
});

test("MathJax import pin preflight CLI runs the existing input validation", () => {
  const child = Bun.spawnSync(
    [process.execPath, "scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts", "preflight"],
    {
      cwd: root,
      env: { ...process.env, BIOME_BINARY: "untrusted-preflight-path" },
      stdout: "pipe",
      stderr: "pipe",
    },
  );
  expect(child.exitCode).toBe(1);
  expect(Buffer.from(child.stderr).toString("utf8")).toContain(
    "MathJax evidence rejects unpinned tool override",
  );
  expect(fs.readdirSync(directory)).toHaveLength(0);
});

test("fixed runtime regressions require every imported tool's physical bytes", () => {
  const imported = runtimeUnusedImportInputs(root);
  const pins = inputs(root);
  expect(() => verifyRuntimeUnusedImportPins(imported, pins)).not.toThrow();
  for (const name of ["p-map", "source-map-js", "acorn-loose", "acorn-walk"]) {
    const prefix = path.join(root, "node_modules", name) + path.sep;
    expect(imported.tools.some((file) => file.startsWith(prefix))).toBe(true);
    const remaining = pins.filter((pin) => !pin.path.startsWith(prefix));
    expect(() => verifyRuntimeUnusedImportPins(imported, remaining)).toThrow(
      "not physically pinned",
    );
  }
});

test("invalid envelope and absent prepared evidence fail without acquisition", () => {
  for (const value of [
    null,
    {},
    [],
    { schema: "krr-mathjax-owned-evidence-v1", status: "completed" },
  ])
    expect(() => validateMathJaxAuditEvidence(root, value)).toThrow();
  expect(() => readMathJaxAuditEvidence(directory)).toThrow();
  expect(fs.readdirSync(directory)).toHaveLength(0);
});

test.each(["fdir", "picomatch", "@types/resolve"])(
  "actual tool dependency closure rejects omitted %s package pins",
  (name) => {
    const records = inputs(root);
    const ending = path.join("node_modules", name, "package.json");
    const remaining = records.filter((pin) => !pin.path.endsWith(ending));
    expect(remaining.length).toBeLessThan(records.length);
    expect(() => verifyMathJaxToolDependencies(remaining)).toThrow("absent from physical pins");
  },
);

const mutations: readonly [string, (value: Record<string, unknown>) => void][] = [
  [
    "schema",
    (value) => {
      value.schema = "forged";
    },
  ],
  [
    "unfinished acquisition",
    (value) => {
      value.status = "running";
    },
  ],
  [
    "unknown envelope field",
    (value) => {
      value.cachedSuccess = true;
    },
  ],
  [
    "producer argv",
    (value) => {
      value.argv = [];
    },
  ],
  [
    "foreign root",
    (value) => {
      value.root = directory;
    },
  ],
  [
    "foreign producer directory",
    (value) => {
      value.directory = directory;
    },
  ],
  [
    "tool version",
    (value) => {
      value.bunVersion = "forged";
    },
  ],
  [
    "environment",
    (value) => {
      value.environment = { BIOME_BINARY: "forged" };
    },
  ],
  ["before source pin", (value) => changePin(value, "inputsBefore")],
  ["after source pin", (value) => changePin(value, "inputsAfter")],
  [
    "current source pin",
    (value) => {
      changePin(value, "inputsBefore");
      changePin(value, "inputsAfter");
    },
  ],
  [
    "input inventory omission",
    (value) => {
      value.inputsBefore = objectsSchema.parse(value.inputsBefore).slice(1);
      value.inputsAfter = objectsSchema.parse(value.inputsAfter).slice(1);
    },
  ],
  [
    "duplicate input inventory",
    (value) => {
      const records = objectsSchema.parse(value.inputsAfter);
      value.inputsBefore = [...records, records[0]];
      value.inputsAfter = [...records, records[0]];
    },
  ],
  [
    "missing artifact",
    (value) => {
      value.artifacts = objectsSchema.parse(value.artifacts).slice(1);
    },
  ],
  [
    "artifact order",
    (value) => {
      value.artifacts = objectsSchema.parse(value.artifacts).reverse();
    },
  ],
  ["physical report/map/config pin", (value) => changePin(value, "artifacts")],
  [
    "owned diagnostic argv",
    (value) => {
      const invocation = objectSchema.parse(value.owned);
      invocation.argv = [];
      value.owned = invocation;
    },
  ],
  [
    "Unicode diagnostic argv",
    (value) => {
      const invocation = objectSchema.parse(value.unicode);
      invocation.argv = [];
      value.unicode = invocation;
    },
  ],
  [
    "failed owned diagnostic exit",
    (value) => {
      const invocation = objectSchema.parse(value.owned);
      invocation.exit = 17;
      value.owned = invocation;
    },
  ],
  [
    "fabricated Unicode empty result",
    (value) => {
      const invocation = objectSchema.parse(value.unicode);
      invocation.exit = 0;
      value.unicode = invocation;
    },
  ],
  [
    "owned diagnostic input",
    (value) => {
      const invocation = objectSchema.parse(value.owned);
      invocation.codeSha256 = "0".repeat(64);
      value.owned = invocation;
    },
  ],
  [
    "Unicode diagnostic input",
    (value) => {
      const invocation = objectSchema.parse(value.unicode);
      invocation.codeSha256 = "0".repeat(64);
      value.unicode = invocation;
    },
  ],
  [
    "unknown diagnostic invocation field",
    (value) => {
      const invocation = objectSchema.parse(value.owned);
      invocation.timeout = 10000;
      value.owned = invocation;
    },
  ],
];

test.each(mutations)("actual evidence rejects mutated %s", (_name, mutate) => {
  const receipt = currentReceipt();
  mutate(receipt);
  expect(() => validateMathJaxAuditEvidence(root, receipt)).toThrow();
});

test.each(["map", "sourceMapAsset", "modules"])(
  "physical inspection consumer rejects actual %s transport drift",
  (field) => {
    const evidence = readMathJaxAuditEvidence(root);
    const inspection = objectSchema.parse(
      JSON.parse(fs.readFileSync(path.join(evidence.directory, "inspection.json"), "utf8")),
    );
    if (field === "map") {
      const output = objectSchema.parse(inspection.output);
      output.map = {};
      inspection.output = output;
    } else inspection[field] = [];
    expect(() =>
      readMathJaxOwnedAuditEvidence(root, evidence.directory, inspection, {
        diagnostics: inspectUnusedBiomeReport(
          JSON.parse(
            fs.readFileSync(path.join(evidence.directory, "mathjax-owned-raw.json"), "utf8"),
          ),
          evidence.report.diagnosticExit,
        ),
        exit: evidence.report.diagnosticExit,
        codeSha256: evidence.report.sourceSha256,
      }),
    ).toThrow();
  },
);

test.each(["package.json", "mjs/svg.js"])(
  "actual NewCM font %s belongs to acquisition pins",
  (relative) => {
    const file = path.join(root, "node_modules/@mathjax/mathjax-newcm-font", relative);
    expect(inputs(root)).toContainEqual(record(file));
  },
);

test.each(["package.json", "mjs/svg.js"])(
  "current evidence rejects omitted NewCM font %s acquisition pins",
  (relative) => {
    const value = currentReceipt();
    const file = path.join(root, "node_modules/@mathjax/mathjax-newcm-font", relative);
    for (const field of ["inputsBefore", "inputsAfter"]) {
      const pins = objectsSchema.parse(value[field]);
      const remaining = pins.filter((pin) => pin.path !== file);
      expect(remaining).toHaveLength(pins.length - 1);
      value[field] = remaining;
    }
    expect(() => validateMathJaxAuditEvidence(root, value)).toThrow("input drift");
  },
);
