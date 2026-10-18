import { expect, test } from "bun:test";
import { rasterCaptureFixture } from "./raster_binding_test_fixtures";
import type { JsonFields, JsonObject } from "./raster-json-input";
import {
  createRasterProductionReceipt,
  parseRasterProductionReceipt,
  type RasterEndpoint,
  type RasterPairObservation,
  type RasterProductionReceipt,
  type RasterRole,
  rasterSha256,
  type TrustedRasterInput,
  verifyRasterProductionBinding,
} from "./raster-production-binding";

const hash = (value: string): string => rasterSha256(Buffer.from(value));
const SVG = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>');
const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGk0AAAAASUVORK5CYII=",
  "base64",
);
const WHITE_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADUlEQVQIHWP4//8/AAX+Av4N70a4AAAAAElFTkSuQmCC",
  "base64",
);

function sample(role: RasterRole = "official") {
  const config = {
    viewport: { width: 1, height: 1, deviceScaleFactor: 1 },
    theme: "dark",
    referenceMode: "strictVendorOriginal",
    captureResize: "capture-box-only-raw-svg-v1",
    sourceFontUrls: [],
    externalImageWaitPolicy: "svg-image-load-decode-positive-dimensions-and-frame-v2",
  };
  const capture = rasterCaptureFixture(SVG, "capture DOM");
  const endpoints: RasterEndpoint[] = (
    ["producer", "browser", "playwright", "lock", "runtime", "font", "config"] as const
  ).map((kind, index) => {
    const identity = {
      path: `/pinned/${kind}`,
      canonical: `/pinned/${kind}`,
      sha256: hash(kind),
      size: 10,
      dev: 1,
      ino: index + 1,
      mode: 33188,
    };
    return {
      name: kind,
      kind,
      version: "synthetic-test-v1",
      before: identity,
      end: { ...identity },
    };
  });
  const manifest = {
    sourceSvgSha256: rasterSha256(SVG),
    renderedSvgSha256: rasterSha256(SVG),
    pngSha256: rasterSha256(PNG),
    configSha256: hash(JSON.stringify(config)),
  };
  const pair: RasterPairObservation = {
    slug: "class-ja-03",
    role,
    sourceSvg: SVG,
    renderedSvg: SVG,
    png: PNG,
    publishedPng: PNG,
    pngDimensions: { width: 1, height: 1 },
    config,
    manifest,
    rawSvgSha256: hash("raw SVG"),
    stageSvgSha256: hash("stage SVG"),
    canonicalSourceSha256: hash("canonical Mermaid"),
    provenanceSha256: hash("strict provenance"),
    loadedSvgSha256: rasterSha256(SVG),
    capture,
  };
  const receipt: RasterProductionReceipt = {
    schema: "katana-mermaid-raster-production-v1",
    diagnosticOnly: true,
    accepted: false,
    producerSucceeded: true,
    role,
    attempt: "/fresh/attempt-1",
    nonce: "external-nonce-1",
    inventory: [pair.slug],
    endpoints,
    rows: [
      {
        slug: pair.slug,
        ...manifest,
        loadedSvgSha256: pair.loadedSvgSha256,
        rawSvgSha256: pair.rawSvgSha256,
        stageSvgSha256: pair.stageSvgSha256,
        canonicalSourceSha256: pair.canonicalSourceSha256,
        provenanceSha256: pair.provenanceSha256,
        publishedPngSha256: manifest.pngSha256,
        png: pair.pngDimensions,
        capture,
      },
    ],
  };
  const bytes = createRasterProductionReceipt(receipt);
  const trusted: TrustedRasterInput = {
    receiptSha256: rasterSha256(bytes),
    role,
    attempt: receipt.attempt,
    nonce: receipt.nonce,
    inventory: receipt.inventory,
  };
  return { receipt, bytes, trusted, pair, endpoints };
}

test("shared capture fixtures keep input-specific DOM hashes and independent nested state", () => {
  const first = rasterCaptureFixture(SVG, "capture DOM");
  const second = rasterCaptureFixture(Buffer.from("different SVG bytes"), "DOM");
  expect(first.domSha256Before).not.toBe(second.domSha256Before);
  expect(first.svg.inputSha256).not.toBe(second.svg.inputSha256);
  first.bounds.width = 2;
  expect(second.bounds.width).toBe(1);
});

for (const role of ["official", "candidate"] as const) {
  test(`accepts pinned ${role} synthetic receipt without promoting acceptance`, () => {
    const data = sample(role);
    const verified = verifyRasterProductionBinding(
      data.bytes,
      data.trusted,
      [data.pair],
      data.endpoints,
    );
    expect(verified.accepted).toBe(false);
    expect(verified.diagnosticOnly).toBe(true);
  });
}
test("rejects PNG and manifest SHA replacement against original trusted receipt", () => {
  const data = sample();
  const pair = {
    ...data.pair,
    png: WHITE_PNG,
    publishedPng: WHITE_PNG,
    manifest: { ...data.pair.manifest, pngSha256: rasterSha256(WHITE_PNG) },
  };
  expect(() =>
    verifyRasterProductionBinding(data.bytes, data.trusted, [pair], data.endpoints),
  ).toThrow("PNG differs from trusted raster output");
});
test("rejects PNG, manifest and sidecar together modified against external pin", () => {
  const data = sample();
  const changed = { ...structuredClone(data.receipt) };
  const row = changed.rows[0];
  if (!row) throw new Error("Missing test row");
  row.pngSha256 = rasterSha256(WHITE_PNG);
  row.publishedPngSha256 = row.pngSha256;
  const pair = {
    ...data.pair,
    png: WHITE_PNG,
    publishedPng: WHITE_PNG,
    manifest: { ...data.pair.manifest, pngSha256: row.pngSha256 },
  };
  expect(() =>
    verifyRasterProductionBinding(
      createRasterProductionReceipt(changed),
      data.trusted,
      [pair],
      data.endpoints,
    ),
  ).toThrow("Trusted raster receipt SHA mismatch");
});
test("rejects missing receipt and no externally supplied pin", () => {
  const data = sample();
  expect(() =>
    verifyRasterProductionBinding(Buffer.alloc(0), data.trusted, [data.pair], data.endpoints),
  ).toThrow("Trusted raster receipt SHA mismatch");
  expect(() =>
    verifyRasterProductionBinding(
      data.bytes,
      { ...data.trusted, receiptSha256: "" },
      [data.pair],
      data.endpoints,
    ),
  ).toThrow("Invalid digest");
});
for (const field of ["role", "attempt", "nonce"] as const) {
  test(`rejects different trusted ${field} and replay`, () => {
    const data = sample();
    const changed = {
      ...data.trusted,
      [field]: field === "role" ? "candidate" : "different",
    } as TrustedRasterInput;
    if (field === "attempt") changed.attempt = "/fresh/attempt-2";
    expect(() =>
      verifyRasterProductionBinding(data.bytes, changed, [data.pair], data.endpoints),
    ).toThrow("Raster role/attempt/nonce mismatch");
  });
}
for (const inventory of [
  [],
  ["class-ja-03", "class-ja-03"],
  ["other"],
  ["class-ja-03", "missing"],
]) {
  test(`rejects trusted inventory ${JSON.stringify(inventory)}`, () => {
    const data = sample();
    expect(() =>
      verifyRasterProductionBinding(
        data.bytes,
        { ...data.trusted, inventory },
        [data.pair],
        data.endpoints,
      ),
    ).toThrow();
  });
}
test("rejects missing or duplicate observed fixture inventory", () => {
  const data = sample();
  for (const pairs of [[], [data.pair, data.pair]]) {
    expect(() =>
      verifyRasterProductionBinding(data.bytes, data.trusted, pairs, data.endpoints),
    ).toThrow("Observed exact fixture inventory mismatch");
  }
});
for (const field of ["sourceSvg", "renderedSvg"] as const) {
  test(`rejects different physical ${field}`, () => {
    const data = sample();
    expect(() =>
      verifyRasterProductionBinding(
        data.bytes,
        data.trusted,
        [{ ...data.pair, [field]: Buffer.from("different SVG") }],
        data.endpoints,
      ),
    ).toThrow("SVG differs from trusted raster input");
  });
}
test("rejects published PNG differing from screenshot bytes", () => {
  const data = sample();
  expect(() =>
    verifyRasterProductionBinding(
      data.bytes,
      data.trusted,
      [{ ...data.pair, publishedPng: WHITE_PNG }],
      data.endpoints,
    ),
  ).toThrow("PNG differs from trusted raster output");
});
test("rejects config, dimensions, source provenance and capture state drift", () => {
  const data = sample();
  const changes: Partial<RasterPairObservation>[] = [
    { config: { theme: "default" } },
    { pngDimensions: { width: 2, height: 1 } },
    { canonicalSourceSha256: hash("other source") },
    { capture: { ...data.pair.capture, theme: "default" } },
  ];
  for (const change of changes)
    expect(() =>
      verifyRasterProductionBinding(
        data.bytes,
        data.trusted,
        [{ ...data.pair, ...change }],
        data.endpoints,
      ),
    ).toThrow();
});
for (const name of ["producer", "browser", "runtime"] as const) {
  test(`rejects ${name} physical tool identity drift`, () => {
    const data = sample();
    const endpoints = structuredClone(data.endpoints);
    const endpoint = endpoints.find((row) => row.name === name);
    if (!endpoint) throw new Error("Missing test endpoint");
    endpoint.end.sha256 = hash("different physical file");
    expect(() =>
      verifyRasterProductionBinding(data.bytes, data.trusted, [data.pair], endpoints),
    ).toThrow("Observed producer/tool endpoints mismatch");
  });
}
test("rejects incomplete browser identity, DOM endpoint drift and malformed schema even with supplied hash", () => {
  const data = sample();
  for (const mutation of [
    (receipt: JsonObject) => {
      receipt.schema = "legacy";
    },
    (receipt: JsonObject) => {
      receipt.accepted = true;
    },
    (receipt: JsonObject) => {
      receipt.sidecarAutoAccept = true;
    },
  ]) {
    const receipt: JsonObject = JSON.parse(data.bytes.toString());
    mutation(receipt);
    const bytes = Buffer.from(JSON.stringify(receipt));
    expect(() =>
      verifyRasterProductionBinding(
        bytes,
        { ...data.trusted, receiptSha256: rasterSha256(bytes) },
        [data.pair],
        data.endpoints,
      ),
    ).toThrow();
  }
  const incomplete = { ...structuredClone(data.receipt) };
  incomplete.endpoints = incomplete.endpoints.filter((row) => row.kind !== "browser");
  expect(() => createRasterProductionReceipt(incomplete)).toThrow(
    "Incomplete or duplicate producer/tool endpoints",
  );
  const changed = { ...structuredClone(data.receipt) };
  const row = changed.rows[0];
  if (!row) throw new Error("Missing test row");
  row.capture.domSha256After = hash("different DOM");
  expect(() => createRasterProductionReceipt(changed)).toThrow("Capture DOM endpoint drift");
});

test("mandatory expected-input to actual SVG DOM chain rejects each independent mutation", () => {
  for (const field of [
    "inputSha256",
    "expectedDomSha256",
    "assignedDomSha256",
    "beforeDomSha256",
    "endDomSha256",
  ] as const) {
    const data = sample(),
      changed = { ...structuredClone(data.receipt) };
    const changedRow = changed.rows[0];
    if (changedRow === undefined) throw new Error("Missing test row");
    changedRow.capture.svg[field] = hash("wrong DOM or raw input");
    expect(() => createRasterProductionReceipt(changed)).toThrow();
  }
  const data = sample();
  const firstRow = data.receipt.rows[0];
  if (firstRow === undefined) throw new Error("Missing test row");
  const oldSchema: JsonObject = { ...structuredClone(firstRow.capture) };
  delete oldSchema.svg;
  const changed: JsonFields<RasterProductionReceipt> = { ...structuredClone(data.receipt) };
  changed.rows = [{ ...firstRow, capture: oldSchema }];
  expect(() =>
    verifyRasterProductionBinding(
      Buffer.from(JSON.stringify(changed)),
      { ...data.trusted, receiptSha256: hash(JSON.stringify(changed)) },
      [data.pair],
      data.endpoints,
    ),
  ).toThrow();
});

test("preserves exact large device and inode identities through receipt parsing", () => {
  const data = sample();
  const changed = structuredClone(data.receipt);
  const endpoint = changed.endpoints[0];
  if (endpoint === undefined) throw new Error("Missing test endpoint");
  endpoint.before.dev = "9007199254740992";
  endpoint.before.ino = "9007199254740993";
  endpoint.end = { ...endpoint.before };
  const parsed = parseRasterProductionReceipt(createRasterProductionReceipt(changed));
  expect(parsed.endpoints[0]?.before.dev).toBe("9007199254740992");
  expect(parsed.endpoints[0]?.before.ino).toBe("9007199254740993");
});

test("rejects adjacent large inode drift instead of rounding it away", () => {
  const data = sample();
  const changed = structuredClone(data.receipt);
  const endpoint = changed.endpoints[0];
  if (endpoint === undefined) throw new Error("Missing test endpoint");
  endpoint.before.dev = "9007199254740992";
  endpoint.before.ino = "9007199254740993";
  endpoint.end = { ...endpoint.before, ino: "9007199254740994" };
  expect(() => createRasterProductionReceipt(changed)).toThrow("Producer/tool endpoint drift");
});

test("rejects unsafe numeric and noncanonical identity encodings", () => {
  for (const value of [Number.MAX_SAFE_INTEGER + 1, "01", "1e3", "1.0", "0"]) {
    const data = sample();
    const changed = structuredClone(data.receipt);
    const endpoint = changed.endpoints[0];
    if (endpoint === undefined) throw new Error("Missing test endpoint");
    endpoint.before.ino = value;
    endpoint.end = { ...endpoint.before };
    expect(() => createRasterProductionReceipt(changed)).toThrow();
  }
});

test("rejects unsafe observed endpoint identities even when their rounded text matches", () => {
  const data = sample();
  const changed = structuredClone(data.receipt);
  const receiptEndpoint = changed.endpoints[0];
  if (receiptEndpoint === undefined) throw new Error("Missing test endpoint");
  receiptEndpoint.before.dev = "9007199254740992";
  receiptEndpoint.before.ino = "9007199254740993";
  receiptEndpoint.end = { ...receiptEndpoint.before };
  const observed = structuredClone(data.endpoints);
  const observedEndpoint = observed[0];
  if (observedEndpoint === undefined) throw new Error("Missing observed endpoint");
  observedEndpoint.before.dev = 9007199254740992;
  // legacy numeric identityの丸めを再現し、隣接する正確なinodeとの差を拒否する。
  observedEndpoint.before.ino = Number(9007199254740993n);
  observedEndpoint.end = { ...observedEndpoint.before };
  const bytes = createRasterProductionReceipt(changed);
  expect(() =>
    verifyRasterProductionBinding(
      bytes,
      { ...data.trusted, receiptSha256: rasterSha256(bytes) },
      [data.pair],
      observed,
    ),
  ).toThrow("Invalid identity integer");
});
