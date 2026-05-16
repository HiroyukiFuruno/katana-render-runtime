import { expect, test } from "bun:test";
import { DrawioReferenceScorePolicy } from "./reference-score";

test("Draw.io baseline があっても全体下限を下回らない", () => {
  const policy = new DrawioReferenceScorePolicy(99, [
    {
      slug: "examples-UMLdiagram",
      score: 93.2,
    },
  ]);

  const threshold = policy.thresholdFor("examples-UMLdiagram");

  expect(threshold.minScore).toBe(99);
  expect(threshold.reason).toContain("既知差分");
});

test("Draw.io baseline がない場合は全体下限を使う", () => {
  const policy = new DrawioReferenceScorePolicy(99);

  const threshold = policy.thresholdFor("basic-03-basic-flow");

  expect(threshold.minScore).toBe(99);
  expect(threshold.reason).toBe("");
});
