import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { readChromiumFontBinding } from "./chromium-font-binding";
import { observerLaunchArguments } from "./chromium-font-capture";
import { buildJoinedStaticFontTrace } from "./chromium-font-test-support";
import { readChromiumFontTrace } from "./chromium-font-trace";

const sessionId = "fixture-session";
const processId = 42;

function event(
  name: string,
  args: Record<string, unknown>,
  pid = processId,
): Record<string, unknown> {
  return {
    cat: "katana.font-observer",
    name,
    pid,
    args: { observerSessionId: sessionId, processId: pid, processRole: "renderer", ...args },
  };
}

test("Chromium font trace join is strict and table-driven", () => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-font-trace-")));
  try {
    const fontPath = path.join(root, "font.bin");
    const fontBytes = Buffer.from("fixture-font-bytes");
    fs.writeFileSync(fontPath, fontBytes);
    const physicalSha = createHash("sha256").update(fontBytes).digest("hex");
    const descriptorSha = "a".repeat(64);
    const base = {
      actualFont: JSON.stringify({
        postScriptName: "Fixture-Regular",
        physicalFileSHA256: physicalSha,
        physicalFileAvailable: true,
        physicalFaceBindingStatus: "observed:unique-table-match",
        physicalFaceTtcIndex: 0,
        physicalFaceFvarTablePresent: false,
        axes: [],
        descriptorURL: fontPath,
      }),
      skiaVariationPosition: JSON.stringify({ available: false, coordinates: [] }),
      descriptorSHA256: descriptorSha,
      descriptorLength: fontBytes.length,
    };
    const variableBase = {
      ...base,
      actualFont: JSON.stringify({
        ...JSON.parse(base.actualFont as string),
        postScriptName: "Fixture-Variable",
        physicalFaceFvarTablePresent: true,
        axisInventoryAvailable: true,
        axes: [{ identifier: 1196445523, minimum: 100, maximum: 900 }],
      }),
      skiaVariationPosition: JSON.stringify({
        available: true,
        axisCount: 1,
        coordinates: [{ identifier: 1196445523, value: 700 }],
      }),
    };
    const valid = buildJoinedStaticFontTrace({
      sessionId,
      processId,
      physicalFilePath: fontPath,
      physicalFileSha256: physicalSha,
      descriptorSha256: descriptorSha,
      descriptorLength: fontBytes.length,
    }).events;
    const variableValid = valid.map((item) =>
      item.name === "scaler-font"
        ? { ...item, args: { ...(item.args as Record<string, unknown>), ...variableBase } }
        : item,
    );
    const variableDuplicateAxis = variableValid.map((item) =>
      item.name === "scaler-font"
        ? {
            ...item,
            args: {
              ...(item.args as Record<string, unknown>),
              actualFont: JSON.stringify({
                ...JSON.parse(variableBase.actualFont),
                axes: [
                  { identifier: 1196445523, minimum: 100, maximum: 900 },
                  { identifier: 1196445523, minimum: 100, maximum: 900 },
                ],
              }),
            },
          }
        : item,
    );
    const staticMissingCoordinates = valid.map((item) =>
      item.name === "scaler-font"
        ? {
            ...item,
            args: {
              ...(item.args as Record<string, unknown>),
              skiaVariationPosition: JSON.stringify({ available: false }),
            },
          }
        : item,
    );
    const relativePath = path.relative(process.cwd(), fontPath);
    const relativePathValid = valid.map((item) =>
      item.name === "scaler-font"
        ? {
            ...item,
            args: {
              ...(item.args as Record<string, unknown>),
              actualFont: JSON.stringify({
                ...(JSON.parse(base.actualFont as string) as Record<string, unknown>),
                descriptorURL: relativePath,
              }),
            },
          }
        : item,
    );
    const unknownFvar = valid.map((item) =>
      item.name === "scaler-font"
        ? {
            ...item,
            args: {
              ...(item.args as Record<string, unknown>),
              actualFont: JSON.stringify(
                Object.fromEntries(
                  Object.entries(JSON.parse(base.actualFont)).filter(
                    ([key]) => key !== "physicalFaceFvarTablePresent",
                  ),
                ),
              ),
            },
          }
        : item,
    );
    const partialJoin = [
      ...valid,
      event("strike-cache", {
        paintSerial: 1,
        cacheHit: false,
        descriptorSHA256: "c".repeat(64),
        descriptorLength: fontBytes.length,
        descriptorChecksum: 2,
      }),
    ];
    const malformedTarget = [
      ...valid,
      {
        cat: "katana.font-observer",
        name: "scaler-font",
        pid: processId,
        args: { observerSessionId: sessionId },
      },
    ];
    const malformedOtherSession = [
      ...valid,
      {
        cat: "katana.font-observer",
        name: "scaler-font",
        pid: processId,
        args: { observerSessionId: "other-session" },
      },
    ];
    const cases = [
      { name: "joined static instance", events: valid, ok: true, expectedFvar: false },
      {
        name: "static missing coordinates",
        events: staticMissingCoordinates,
        ok: true,
        expectedFvar: false,
      },
      { name: "relative existing path", events: relativePathValid, ok: true, expectedFvar: false },
      { name: "joined variable instance", events: variableValid, ok: true, expectedFvar: true },
      { name: "duplicate variable axis is rejected", events: variableDuplicateAxis, ok: false },
      { name: "unknown fvar is rejected", events: unknownFvar, ok: false },
      { name: "partial descriptor join is rejected", events: partialJoin, ok: false },
      { name: "malformed target event is rejected", events: malformedTarget, ok: false },
      {
        name: "malformed other session is ignored",
        events: malformedOtherSession,
        ok: true,
        expectedFvar: false,
      },
      { name: "standalone scaler is rejected", events: [valid[2]], ok: false },
      { name: "missing retained end is rejected", events: valid.slice(0, 3), ok: false },
      {
        name: "physical bytes mismatch is rejected",
        events: valid.map((item) =>
          item.name === "scaler-font"
            ? {
                ...item,
                args: {
                  ...(item.args as Record<string, unknown>),
                  actualFont: JSON.stringify({
                    ...(JSON.parse(base.actualFont as string) as Record<string, unknown>),
                    physicalFileSHA256: "b".repeat(64),
                  }),
                },
              }
            : item,
        ),
        ok: false,
      },
    ];
    for (const fixture of cases) {
      const tracePath = path.join(root, `${fixture.name.replaceAll(" ", "-")}.json`);
      const trace = JSON.stringify({ traceEvents: fixture.events });
      fs.writeFileSync(tracePath, trace);
      const traceSha256 = createHash("sha256").update(trace).digest("hex");
      if (fixture.ok) {
        const result = readChromiumFontTrace({ tracePath, traceSha256, sessionId });
        expect(result.instances).toHaveLength(1);
        expect(result.instances[0]?.paintSerials).toEqual([1]);
        expect(result.instances[0]?.fvarTablePresent).toBe(fixture.expectedFvar);
        if (fixture.name === "joined static instance")
          assertCaptureBinding(fontPath, physicalSha, tracePath, traceSha256);
      } else {
        expect(() => readChromiumFontTrace({ tracePath, traceSha256, sessionId })).toThrow();
      }
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("used scaler rejections are granular and redact trace values", () => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-font-trace-")));
  try {
    const fontPath = path.join(root, "font.bin");
    const fontBytes = Buffer.from("fixture-font-bytes");
    fs.writeFileSync(fontPath, fontBytes);
    const physicalSha = createHash("sha256").update(fontBytes).digest("hex");
    const descriptorSha = "d".repeat(64);
    const valid = buildJoinedStaticFontTrace({
      sessionId,
      processId,
      physicalFilePath: fontPath,
      physicalFileSha256: physicalSha,
      descriptorSha256: descriptorSha,
      descriptorLength: fontBytes.length,
    }).events;
    const mutateScaler = (
      events: Record<string, unknown>[],
      mutation: (args: Record<string, unknown>) => Record<string, unknown>,
    ): Record<string, unknown>[] =>
      events.map((item) =>
        item.name === "scaler-font"
          ? { ...item, args: mutation(item.args as Record<string, unknown>) }
          : item,
      );
    const mutateActual = (
      events: Record<string, unknown>[],
      mutation: (actual: Record<string, unknown>) => Record<string, unknown>,
    ): Record<string, unknown>[] =>
      mutateScaler(events, (args) => ({
        ...args,
        actualFont: JSON.stringify(mutation(JSON.parse(args.actualFont as string))),
      }));
    const axis = { identifier: 1196445523, minimum: 100, maximum: 900 };
    const coordinate = { identifier: axis.identifier, value: 700 };
    const variable = mutateScaler(
      mutateActual(valid, (actual) => ({
        ...actual,
        physicalFaceFvarTablePresent: true,
        axisInventoryAvailable: true,
        axes: [axis],
      })),
      (args) => ({
        ...args,
        skiaVariationPosition: JSON.stringify({
          available: true,
          axisCount: 1,
          coordinates: [coordinate],
        }),
      }),
    );
    const mutateVariable = (
      actualMutation: (actual: Record<string, unknown>) => Record<string, unknown>,
      positionMutation: (position: Record<string, unknown>) => Record<string, unknown>,
    ): Record<string, unknown>[] =>
      mutateScaler(mutateActual(variable, actualMutation), (args) => ({
        ...args,
        skiaVariationPosition: JSON.stringify(
          positionMutation(JSON.parse(args.skiaVariationPosition as string)),
        ),
      }));
    const conflicting = [
      ...valid,
      ...mutateActual([valid[2] as Record<string, unknown>], (actual) => ({
        ...actual,
        physicalFaceTtcIndex: 1,
      })),
    ];
    const cases = [
      {
        name: "no-event",
        events: valid.filter((item) => item.name !== "scaler-font"),
        code: "observer-scaler-no-event",
      },
      { name: "conflict", events: conflicting, code: "observer-scaler-conflicting-descriptor" },
      {
        name: "invalid-font",
        events: mutateActual(valid, (actual) => ({ ...actual, postScriptName: "" })),
        code: "observer-scaler-invalid-font-schema",
      },
      {
        name: "invalid-variation",
        events: mutateScaler(variable, (args) => ({ ...args, skiaVariationPosition: "not-json" })),
        code: "observer-scaler-invalid-variation-schema",
      },
      {
        name: "missing-descriptor-path",
        events: mutateActual(valid, (actual) => {
          const { descriptorURL: _descriptorURL, ...withoutPath } = actual;
          return withoutPath;
        }),
        code: "observer-scaler-missing-descriptor-path",
      },
      {
        name: "file-uri",
        events: mutateActual(valid, (actual) => ({
          ...actual,
          descriptorURL: `file://${fontPath}`,
        })),
        code: "observer-scaler-file-uri-needs-normalization",
      },
      {
        name: "unavailable-file",
        events: mutateActual(valid, (actual) => ({
          ...actual,
          descriptorURL: "/unavailable/fixture-font.bin",
        })),
        code: "observer-scaler-unavailable-absolute-file",
      },
      {
        name: "digest-mismatch",
        events: mutateActual(valid, (actual) => ({
          ...actual,
          physicalFileSHA256: "e".repeat(64),
        })),
        code: "observer-scaler-file-digest-mismatch",
      },
      {
        name: "static-fvar-inconsistent",
        events: mutateActual(valid, (actual) => ({ ...actual, axes: [axis] })),
        code: "observer-scaler-static-fvar-inconsistent",
      },
      {
        name: "inventory-unavailable",
        events: mutateVariable(
          (actual) => ({ ...actual, axisInventoryAvailable: false }),
          (position) => position,
        ),
        code: "observer-scaler-fvar-inventory-unavailable",
      },
      {
        name: "position-unavailable",
        events: mutateVariable(
          (actual) => actual,
          (position) => ({ ...position, available: false }),
        ),
        code: "observer-scaler-skia-position-unavailable",
      },
      {
        name: "axis-count-mismatch",
        events: mutateVariable(
          (actual) => actual,
          (position) => ({ ...position, axisCount: 0 }),
        ),
        code: "observer-scaler-axis-count-mismatch",
      },
      {
        name: "coordinate-length-mismatch",
        events: mutateVariable(
          (actual) => actual,
          (position) => {
            const { coordinates: _coordinates, ...withoutCoordinates } = position;
            return withoutCoordinates;
          },
        ),
        code: "observer-scaler-coordinate-length-mismatch",
      },
      {
        name: "invalid-axis",
        events: mutateVariable(
          (actual) => ({ ...actual, axes: [{ ...axis, minimum: 901 }] }),
          (position) => position,
        ),
        code: "observer-scaler-invalid-axis",
      },
      {
        name: "duplicate-axis",
        events: mutateVariable(
          (actual) => ({ ...actual, axes: [axis, axis] }),
          (position) => ({ ...position, axisCount: 2, coordinates: [coordinate, coordinate] }),
        ),
        code: "observer-scaler-duplicate-axis",
      },
      {
        name: "missing-coordinate",
        events: mutateVariable(
          (actual) => ({ ...actual, axes: [axis, { ...axis, identifier: 1196445524 }] }),
          (position) => ({ ...position, axisCount: 2, coordinates: [coordinate] }),
        ),
        code: "observer-scaler-missing-coordinate",
      },
      {
        name: "excess-coordinate",
        events: mutateVariable(
          (actual) => actual,
          (position) => ({
            ...position,
            coordinates: [coordinate, { identifier: 1196445524, value: 700 }],
          }),
        ),
        code: "observer-scaler-excess-coordinate",
      },
      {
        name: "duplicate-coordinate",
        events: mutateVariable(
          (actual) => ({ ...actual, axes: [axis, { ...axis, identifier: 1196445524 }] }),
          (position) => ({ ...position, axisCount: 2, coordinates: [coordinate, coordinate] }),
        ),
        code: "observer-scaler-duplicate-coordinate",
      },
      {
        name: "out-of-range-coordinate",
        events: mutateVariable(
          (actual) => actual,
          (position) => ({ ...position, coordinates: [{ ...coordinate, value: 901 }] }),
        ),
        code: "observer-scaler-out-of-range-coordinate",
      },
    ];
    for (const fixture of cases) {
      const message = traceRejection(root, fixture.name, fixture.events);
      expect(message).toBe(`used strike has no joined scaler: ${fixture.code}`);
      expect(message).not.toContain(root);
      expect(message).not.toContain(fontPath);
      expect(message).not.toContain("Fixture-Regular");
      expect(message).not.toContain("1196445523");
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("existing unreadable descriptor rejects without exposing its path", () => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-font-trace-")));
  try {
    const fontPath = path.join(root, "font.bin");
    const fontBytes = Buffer.from("fixture-font-bytes");
    fs.writeFileSync(fontPath, fontBytes);
    const physicalSha = createHash("sha256").update(fontBytes).digest("hex");
    const events = buildJoinedStaticFontTrace({
      sessionId,
      processId,
      physicalFilePath: fontPath,
      physicalFileSha256: physicalSha,
      descriptorSha256: "f".repeat(64),
      descriptorLength: fontBytes.length,
    }).events.map((item) =>
      item.name === "scaler-font"
        ? {
            ...item,
            args: {
              ...(item.args as Record<string, unknown>),
              actualFont: JSON.stringify({
                ...(JSON.parse(
                  (item.args as Record<string, unknown>).actualFont as string,
                ) as Record<string, unknown>),
                descriptorURL: root,
              }),
            },
          }
        : item,
    );
    const message = traceRejection(root, "unreadable", events);
    expect(message).toBe("observer-scaler-file-read-failed");
    expect(message).not.toContain(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

function traceRejection(
  directory: string,
  name: string,
  events: readonly Record<string, unknown>[],
): string {
  const tracePath = path.join(directory, `${name}.json`);
  const trace = JSON.stringify({ traceEvents: events });
  fs.writeFileSync(tracePath, trace);
  const traceSha256 = createHash("sha256").update(trace).digest("hex");
  try {
    readChromiumFontTrace({ tracePath, traceSha256, sessionId });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  throw new Error("expected Chromium font trace rejection");
}

function assertCaptureBinding(
  filePath: string,
  fileSha: string,
  tracePath: string,
  traceSha256: string,
): void {
  const expected = {
    conditionId: "fixture-condition",
    sourceSha256: fileSha,
    viewport: { width: 320, height: 240, scale: 1 },
    browserExecutablePath: filePath,
    browserExecutableSha256: fileSha,
    capturePath: filePath,
    captureSha256: fileSha,
  };
  const binding = {
    ...expected,
    schema: "krr-chromium-font-capture-v1",
    tracePath,
    traceSha256,
    sessionId,
    argv: [filePath, ...observerLaunchArguments(sessionId, tracePath)],
    processId,
    actualExitCode: 0,
  };
  expect(readChromiumFontBinding(binding, expected).instances).toHaveLength(1);
  for (const mutation of [
    { conditionId: "other-condition" },
    { sourceSha256: "b".repeat(64) },
    { viewport: { ...expected.viewport, scale: 2 } },
    { actualExitCode: 1 },
    { processId: 0 },
    { traceSha256: "b".repeat(64) },
    { argv: binding.argv.slice(0, -1) },
    { unexpected: true },
  ])
    expect(() => readChromiumFontBinding({ ...binding, ...mutation }, expected)).toThrow();
}
