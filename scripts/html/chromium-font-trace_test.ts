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
