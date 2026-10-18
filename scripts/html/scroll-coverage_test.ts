import { describe, expect, test } from "bun:test";
import { assertScrollCoverage, BOTTOM_SCROLL_REQUEST, buildScrollPlan } from "./scroll-coverage";

describe("scroll coverage planning", () => {
  test("covers the whole extent with overlap and preserves legacy samples plus bottom sentinel", () => {
    const plan = buildScrollPlan(2_000, 500, [420, 1_234]);
    expect(plan.some((item) => item.requested === 600)).toBe(true);
    expect(plan.some((item) => item.requested === 1_800)).toBe(true);
    expect(plan.some((item) => item.requested === 420)).toBe(true);
    expect(plan.at(-1)).toEqual({ name: "bottom", requested: BOTTOM_SCROLL_REQUEST });
    expect(() =>
      assertScrollCoverage(
        plan.map((item) => Math.min(item.requested, 2_000)),
        2_000,
        500,
      ),
    ).not.toThrow();
  });

  test("rejects a gap even when top and bottom samples exist", () => {
    expect(() => assertScrollCoverage([0, 1_000], 1_000, 500)).toThrow("coverage gap");
  });

  test("rejects an extent that has no bottom sample", () => {
    expect(() => assertScrollCoverage([0, 400], 1_000, 500)).toThrow(
      "does not reach document bottom",
    );
  });

  test("rejects invalid extents", () => {
    expect(() => buildScrollPlan(-1, 500)).toThrow("maxScrollY");
    expect(() => assertScrollCoverage([0], 1_000, 0)).toThrow("viewportHeight");
    expect(() => assertScrollCoverage([0, Number.NaN, 1_000], 1_000, 500)).toThrow(
      "invalid position",
    );
    expect(() => assertScrollCoverage([-1, 1_000], 1_000, 500)).toThrow("invalid position");
    expect(() => assertScrollCoverage([0, 1_001], 1_000, 500)).toThrow("invalid position");
  });

  test("rounds target positions to Chrome's scroll pixel grid", () => {
    const plan = buildScrollPlan(1_000, 500, [123.49, 456.51]);
    expect(plan.some((item) => item.requested === 123)).toBe(true);
    expect(plan.some((item) => item.requested === 457)).toBe(true);
  });
});
