import { describe, expect, test } from "bun:test";
import { fidelityThresholdForPhase } from "./fidelity-phase";
import { INPUT_STATE_OPERATIONS, runInputStateFidelity } from "./visual-state-fidelity";

describe("input-state visual operation coverage", () => {
  test("covers every catalog boundary state with an executable operation", () => {
    expect(INPUT_STATE_OPERATIONS).toEqual([
      "empty",
      "filled",
      "selected",
      "expanded",
      "disabled",
      "focus-visible",
    ]);
  });
});

describe("input-state visual fidelity phases", () => {
  test("keeps final at 99 and accepts only the explicit interim threshold of 90", () => {
    expect(fidelityThresholdForPhase("final", 99)).toBe(99);
    expect(fidelityThresholdForPhase("interim", 90)).toBe(90);
    expect(() => fidelityThresholdForPhase("final", 90)).toThrow(/final.*99/);
    expect(() => fidelityThresholdForPhase("interim", 99)).toThrow(/interim.*90/);
    expect(() => fidelityThresholdForPhase("interim", 95)).toThrow(/interim.*90/);
    expect(() => fidelityThresholdForPhase("unrecognized" as never, 90)).toThrow(
      /unsupported.*phase/,
    );
  });

  test("rejects an unsupported input-state phase before starting captures", async () => {
    await expect(
      runInputStateFidelity("", "", 90, { phase: "unrecognized" } as never),
    ).rejects.toThrow(/unsupported.*phase/);
  });
});
