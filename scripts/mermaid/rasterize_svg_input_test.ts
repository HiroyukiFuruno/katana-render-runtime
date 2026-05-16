import { expect, test } from "bun:test";
import { SvgRasterizeInput } from "./rasterize_svg_input";

test("SVG要素がない空fixtureは画像化対象から外す", () => {
  expect(new SvgRasterizeInput("").shouldRasterize()).toBe(false);
  expect(new SvgRasterizeInput("  \n").shouldRasterize()).toBe(false);
});

test("SVG要素を含むfixtureは画像化対象にする", () => {
  expect(new SvgRasterizeInput('<svg viewBox="0 0 1 1"></svg>').shouldRasterize()).toBe(true);
});

test("ブラウザー挿入時はXHTMLのvoid要素閉じタグを取り除く", () => {
  const input = new SvgRasterizeInput(
    '<svg><foreignObject><div xmlns="http://www.w3.org/1999/xhtml">a<br></br><hr></hr>b</div></foreignObject></svg>',
  );

  expect(input.browserInnerHtml()).toBe(
    '<svg><foreignObject><div xmlns="http://www.w3.org/1999/xhtml">a<br><hr>b</div></foreignObject></svg>',
  );
});
