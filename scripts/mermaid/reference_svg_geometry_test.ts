import { expect, test } from "bun:test";
import {
  assertCompletePaintProjectedTargetSlugs,
  compareSvgGeometry,
  compareSvgGeometryForNeutralRaster,
  isNeutralGeometryTargetSlug,
  NEUTRAL_GEOMETRY_TARGET_SLUGS,
  PAINT_PROJECTED_REFERENCE_TARGET_SLUGS,
  PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA,
  prepareNeutralSvg,
} from "./reference_svg_geometry";

const sourceSvg = (rootId: string, fill: string, shift: string, label = "Box") =>
  `<svg xmlns="http://www.w3.org/2000/svg" id="${rootId}" width="120px" height="80px" viewBox="0 0 120 80"><style>#${rootId} .box{fill:${fill};stroke:#222;stroke-width:2px} #${rootId} .label{fill:${fill};font-size:16px}</style><g transform="translate(0,${shift})"><rect class="box" x="10" y="10" width="50" height="30"/><line x1="10" y1="40" x2="60" y2="40" stroke="#444"/><text class="label" x="12" y="25">${label}<tspan> text</tspan></text></g></svg>`;

test("neutral geometry scope is the fixed current C4 and TreeView fixture family", () => {
  expect(NEUTRAL_GEOMETRY_TARGET_SLUGS).toHaveLength(8);
  expect(NEUTRAL_GEOMETRY_TARGET_SLUGS.every(isNeutralGeometryTargetSlug)).toBe(true);
  expect(PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA).toHaveLength(8);
  expect(PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA.every(isNeutralGeometryTargetSlug)).toBe(true);
  expect(isNeutralGeometryTargetSlug("11-c4")).toBe(false);
  expect(isNeutralGeometryTargetSlug("08-04-c4x")).toBe(false);
  expect(isNeutralGeometryTargetSlug("custom-tree-view-prototype")).toBe(false);
});

test.each([
  ...PAINT_PROJECTED_REFERENCE_TARGET_SLUGS,
  ...PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA,
])("paint projection recognizes the exact language target slug: %s", (slug) => {
  expect(isNeutralGeometryTargetSlug(slug)).toBe(true);
});

test.each([
  { pairCount: 59, slugs: PAINT_PROJECTED_REFERENCE_TARGET_SLUGS },
  { pairCount: 58, slugs: PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA },
])(
  "accepts the complete target set for the language pair count: $pairCount",
  ({ pairCount, slugs }) => {
    expect(() => assertCompletePaintProjectedTargetSlugs(pairCount, slugs)).not.toThrow();
  },
);

test.each([
  { name: "missing target", slugs: PAINT_PROJECTED_REFERENCE_TARGET_SLUGS.slice(0, 7) },
  {
    name: "duplicate target",
    slugs: [
      ...PAINT_PROJECTED_REFERENCE_TARGET_SLUGS.slice(0, 7),
      PAINT_PROJECTED_REFERENCE_TARGET_SLUGS[0],
    ],
  },
  { name: "wrong language", slugs: PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA },
])("rejects an incomplete or wrong target set: $name", ({ slugs }) => {
  expect(() => assertCompletePaintProjectedTargetSlugs(59, slugs)).toThrow();
});

test("rejects duplicate targets even for a scoped pair count", () => {
  expect(() =>
    assertCompletePaintProjectedTargetSlugs(2, ["scoped-target", "scoped-target"]),
  ).toThrow("Paint projection target slugs are duplicated");
});

test("neutral SVG geometry accepts palette-only changes and equivalent root IDs", () => {
  const left = prepareNeutralSvg(
      sourceSvg("a", "#444444", "0").replace("</svg>", '<metadata data-color="#abcdef"/></svg>'),
    ),
    right = prepareNeutralSvg(
      sourceSvg("b", "#e0e0e0", "0").replace("</svg>", '<metadata data-color="#abcdef"/></svg>'),
    );
  expect(compareSvgGeometry(left, right)).toMatchObject({ accepted: true, maxTransformDelta: 0 });
});

test("neutral SVG geometry preserves paint alpha instead of hiding transparency", () => {
  const opaque = prepareNeutralSvg(sourceSvg("official", "rgba(20, 30, 40, 1)", "0"));
  const transparent = prepareNeutralSvg(sourceSvg("candidate", "rgba(200, 210, 220, 0)", "0"));
  const partial = prepareNeutralSvg(sourceSvg("candidate", "rgba(200, 210, 220, 0.5)", "0"));
  const sameTransparency = prepareNeutralSvg(sourceSvg("candidate", "rgba(200, 210, 220, 0)", "0"));
  expect(compareSvgGeometry(opaque, transparent).accepted).toBe(false);
  expect(compareSvgGeometry(opaque, partial).accepted).toBe(false);
  expect(compareSvgGeometry(transparent, sameTransparency).accepted).toBe(true);
});

test("neutral SVG geometry preserves gradient references and stop alpha", () => {
  const source = (gradientId: string, color: string) =>
    `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><linearGradient id="${gradientId}"><stop offset="0" stop-color="${color}"/></linearGradient></defs><rect x="1" y="1" width="18" height="18" fill="url(#${gradientId})"/></svg>`;
  const official = prepareNeutralSvg(source("official-gradient", "rgba(10,20,30,0.5)"));
  const recolored = prepareNeutralSvg(source("candidate-gradient", "rgba(220,210,200,0.5)"));
  const transparent = prepareNeutralSvg(source("candidate-gradient", "rgba(220,210,200,0)"));
  expect(compareSvgGeometry(official, recolored).accepted).toBe(true);
  expect(compareSvgGeometry(official, transparent).accepted).toBe(false);
});

test("neutral raster structure ignores paint declarations but distinguishes reference resolution", () => {
  const withoutDirectPaint = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18"/></svg>',
    ),
    withDirectPaint = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18" fill="#222" stroke="none"/></svg>',
    );
  expect(compareSvgGeometry(withoutDirectPaint, withDirectPaint).accepted).toBe(false);
  expect(compareSvgGeometryForNeutralRaster(withoutDirectPaint, withDirectPaint).accepted).toBe(
    true,
  );
  const unresolved = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18" fill="url(#missing)"/></svg>',
    ),
    unresolvedWithDifferentId = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18" fill="url(#also-missing)"/></svg>',
    ),
    resolved = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><linearGradient id="present"><stop offset="0" stop-color="#000"/></linearGradient></defs><rect x="1" y="1" width="18" height="18" fill="url(#present)"/></svg>',
    );
  expect(compareSvgGeometry(unresolved, unresolvedWithDifferentId).accepted).toBe(true);
  expect(compareSvgGeometry(unresolved, resolved).accepted).toBe(false);
});

test("neutral SVG geometry rejects missing boxes, connectors, labels, and text changes", () => {
  const original = prepareNeutralSvg(sourceSvg("official", "#444444", "0"));
  const removedBox = prepareNeutralSvg(
    sourceSvg("official", "#444444", "0").replace(/<rect[^>]*\/>/, ""),
  );
  const removedConnector = prepareNeutralSvg(
    sourceSvg("official", "#444444", "0").replace(/<line[^>]*\/>/, ""),
  );
  const removedLabel = prepareNeutralSvg(
    sourceSvg("official", "#444444", "0").replace(/<text[\s\S]*?<\/text>/, ""),
  );
  const changedText = prepareNeutralSvg(sourceSvg("official", "#444444", "0", "Box shifted"));
  for (const changed of [removedBox, removedConnector, removedLabel, changedText]) {
    expect(compareSvgGeometry(original, changed).accepted).toBe(false);
  }
});

test("neutral SVG geometry bounds transform tolerance and preserves viewport dimensions", () => {
  const original = prepareNeutralSvg(sourceSvg("official", "#444444", "0"));
  const tinyDelta = prepareNeutralSvg(sourceSvg("candidate", "#444444", "0.000009"));
  const shifted = prepareNeutralSvg(sourceSvg("candidate", "#444444", "0.00002"));
  const scaled = prepareNeutralSvg(
    sourceSvg("candidate", "#444444", "0").replace('viewBox="0 0 120 80"', 'viewBox="0 0 121 80"'),
  );
  const shiftedComparison = compareSvgGeometry(original, shifted);
  expect(compareSvgGeometry(original, tinyDelta).accepted).toBe(true);
  expect(shiftedComparison.accepted).toBe(false);
  expect(shiftedComparison.maxTransformDelta).toBe(0.00002);
  expect(compareSvgGeometry(original, scaled).accepted).toBe(false);
});

test("neutral SVG geometry reports the measured delta for a rejected transform", () => {
  const original = prepareNeutralSvg(sourceSvg("official", "#444444", "0"));
  const shifted = prepareNeutralSvg(sourceSvg("candidate", "#444444", "0.3203125"));
  const comparison = compareSvgGeometry(original, shifted);

  expect(comparison.accepted).toBe(false);
  expect(comparison.reasons).toContain("$svg/1:transform");
  expect(comparison.maxTransformDelta).toBe(0.3203125);
});

test("neutral SVG geometry leaves unmeasurable transform deltas at zero", () => {
  const original = prepareNeutralSvg(sourceSvg("official", "#444444", "0"));
  const malformed = prepareNeutralSvg(
    sourceSvg("candidate", "#444444", "0").replace(
      'transform="translate(0,0)"',
      'transform="translate(1e999,0)"',
    ),
  );
  const comparison = compareSvgGeometry(original, malformed);

  expect(comparison.accepted).toBe(false);
  expect(comparison.reasons).toContain("$svg/1:transform");
  expect(comparison.maxTransformDelta).toBe(0);
});

test("neutral SVG geometry fails closed for malformed and unsupported SVG content", () => {
  expect(() => prepareNeutralSvg("<svg><g></svg>")).toThrow();
  expect(() =>
    prepareNeutralSvg('<!DOCTYPE svg [<!ENTITY x SYSTEM "file:///etc/passwd">]><svg>&x;</svg>'),
  ).toThrow();
  expect(() =>
    prepareNeutralSvg('<svg xmlns="http://www.w3.org/2000/svg"><image href="x.png"/></svg>'),
  ).toThrow();
});
