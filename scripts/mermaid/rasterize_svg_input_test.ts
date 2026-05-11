import { expect, test } from "bun:test";
import { SvgRasterizeInput } from "./rasterize_svg_input";

test("SVG要素がない空fixtureは画像化対象から外す", () => {
  expect(new SvgRasterizeInput("").shouldRasterize()).toBe(false);
  expect(new SvgRasterizeInput("  \n").shouldRasterize()).toBe(false);
});

test("SVG要素を含むfixtureは画像化対象にする", () => {
  expect(new SvgRasterizeInput('<svg viewBox="0 0 1 1"></svg>').shouldRasterize()).toBe(true);
});
