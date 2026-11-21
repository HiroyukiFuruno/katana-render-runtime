import { expect, test } from "bun:test";
import type { PageHandle } from "./official-renderer-types";
import {
  assertLoadedSvgExternalImages,
  strictSvgExternalImageUrls,
  waitForStrictSvgExternalImages,
} from "./strict-svg-external-images";

class PageHandleStub implements Pick<PageHandle, "evaluate"> {
  readonly evaluations: Array<{ argument: unknown; hasArgument: boolean }> = [];

  constructor(private readonly results: unknown[]) {}

  evaluate<Result>(callback: () => Result | Promise<Result>): Promise<Result>;
  evaluate<Result, Argument>(
    callback: (argument: Argument) => Result | Promise<Result>,
    argument: Argument,
  ): Promise<Result>;
  async evaluate<Result>(
    _callback: (() => Result | Promise<Result>) | ((argument: unknown) => Result | Promise<Result>),
    ...args: [] | [unknown]
  ): Promise<Result> {
    this.evaluations.push({ argument: args[0], hasArgument: args.length === 1 });
    if (this.results.length === 0) throw new Error("PageHandleStub has no evaluation result");
    return this.results.shift() as Result;
  }
}

test("strict SVG image references honor SVG2 href priority and deduplicate identical URLs", () => {
  expect(
    strictSvgExternalImageUrls([
      { href: "asset.png", xlinkHref: "legacy.png" },
      { href: null, xlinkHref: "asset.png" },
    ]),
  ).toEqual(["asset.png"]);
});

test("strict SVG rejects absent or empty href without falling through to xlink", () => {
  for (const reference of [
    { href: null, xlinkHref: null },
    { href: "", xlinkHref: "legacy.png" },
    { href: "   ", xlinkHref: "legacy.png" },
    { href: null, xlinkHref: "" },
  ]) {
    expect(() => strictSvgExternalImageUrls([reference])).toThrow("no usable href");
  }
});

test("strict SVG requires complete decoded images with positive natural dimensions", () => {
  expect(() =>
    assertLoadedSvgExternalImages([
      { url: "asset.png", complete: true, naturalWidth: 10, naturalHeight: 8, decoded: true },
    ]),
  ).not.toThrow();
  for (const invalid of [
    { url: "asset.png", complete: false, naturalWidth: 10, naturalHeight: 8, decoded: true },
    { url: "asset.png", complete: true, naturalWidth: 0, naturalHeight: 8, decoded: true },
    { url: "asset.png", complete: true, naturalWidth: 10, naturalHeight: 0, decoded: true },
    { url: "asset.png", complete: true, naturalWidth: 10, naturalHeight: 8, decoded: false },
  ]) {
    expect(() => assertLoadedSvgExternalImages([invalid])).toThrow("did not load and decode");
  }
});

const imageReadinessCases: Array<[string, boolean]> = [
  ["loaded", true],
  ["incomplete", false],
];

test.each(imageReadinessCases)(
  "shared image waiter preserves decode ordering: %s",
  async (_label, complete) => {
    const page = new PageHandleStub([
      [{ href: "asset.png", xlinkHref: null }],
      [{ url: "asset.png", complete, naturalWidth: 10, naturalHeight: 8, decoded: true }],
    ]);

    if (complete) await waitForStrictSvgExternalImages(page);
    else {
      await expect(waitForStrictSvgExternalImages(page)).rejects.toThrow(
        "Strict SVG image did not load and decode: asset.png",
      );
    }
    expect(page.evaluations).toEqual([
      { argument: undefined, hasArgument: false },
      { argument: ["asset.png"], hasArgument: true },
    ]);
  },
);
