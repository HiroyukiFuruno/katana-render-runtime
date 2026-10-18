import { expect, test } from "bun:test";
import { runtimeBundleDefinitions } from "./runtime-bundle-definitions";

test("runtime registry preserves all production routes and fresh mutable definitions", () => {
  const first = runtimeBundleDefinitions();
  const second = runtimeBundleDefinitions();
  expect(first.map((definition) => definition.name)).toEqual([
    "mermaid",
    "drawio",
    "zenuml",
    "mathjax",
  ]);
  expect(second).toEqual(first);
  first.forEach((definition, index) => {
    expect(second[index]).not.toBe(definition);
  });
});
