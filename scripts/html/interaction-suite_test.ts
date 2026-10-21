import { describe, expect, test } from "bun:test";
import {
  compareInteractionStates,
  evaluateInteractionEvidence,
  type InteractionState,
  parseInteractionOptions,
} from "./interaction-suite";

const states: InteractionState = {
  link: { url: "file:///linked-page.html" },
  scroll: { scrollY: 600, contentHeight: 1800, viewportHeight: 900 },
  form: { value: "ok", result: "submitted" },
  js: { expanded: true, result: "opened" },
};

describe("interaction suite contract", () => {
  test("requires an independent KRR report and validates numeric options", () => {
    expect(() => parseInteractionOptions(["--html", "fixture.html", "--width", "12.5"])).toThrow(
      "Invalid integer option: --width",
    );
    expect(
      parseInteractionOptions(["--html", "fixture.html", "--krr-report", "report.json"]).krrReport,
    ).toContain("report.json");
  });

  test("scores all four interaction states independently", () => {
    const comparisons = compareInteractionStates(states, {
      ...states,
      scroll: { ...states.scroll, scrollY: 599.99 },
    });
    expect(Object.values(comparisons).filter((value) => value.passed)).toHaveLength(3);
    expect(comparisons.scroll.mismatches).toHaveLength(1);
  });

  test("does not treat missing semantic state as a pass", () => {
    const comparisons = compareInteractionStates(states, {});
    expect(Object.values(comparisons).every((value) => !value.passed)).toBe(true);
  });

  test("requires all independent operations and measured performance without inventing a semantic score", () => {
    const comparisons = compareInteractionStates(states, states);
    const metrics = { firstFrameMs: 30, closeMs: 10, contractMaxMs: 60 };
    const evidence = evaluateInteractionEvidence(comparisons, metrics, 60);
    expect(evidence.passed).toBe(true);
    expect(evidence.matchedInteractions).toBe(4);
    expect(evidence).not.toHaveProperty("semanticScore");
    expect(evidence).not.toHaveProperty("interactionScore");
    expect(evaluateInteractionEvidence(comparisons, undefined, 60).passed).toBe(false);
    expect(evaluateInteractionEvidence(comparisons, metrics, 39).passed).toBe(false);
    expect(
      evaluateInteractionEvidence(comparisons, { ...metrics, closeMs: 30 }, 60).performancePassed,
    ).toBe(false);
    expect(
      evaluateInteractionEvidence(comparisons, { ...metrics, closeMs: 29.999 }, 60).passed,
    ).toBe(true);
    expect(
      evaluateInteractionEvidence(compareInteractionStates(states, {}), metrics, 60).passed,
    ).toBe(false);
  });
});
