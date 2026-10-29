import { expect, test } from "bun:test";
import { extractPictures } from "./ja04-picture-diagnostic";

test("extracts serialized picture and keeps its trace event identity", () => {
  const rows = extractPictures({
    traceEvents: [{ name: "Picture", pid: 7, args: { snapshot: { skp64: "AQID" } } }],
  });
  expect(rows).toHaveLength(1);
  expect([...(rows[0]?.bytes ?? [])]).toEqual([1, 2, 3]);
  expect(rows[0]?.events).toMatchObject([{ name: "Picture", pid: 7 }]);
  expect(rows[0]?.sha256).toHaveLength(64);
});

test("missing pictures remain absent and malformed trace data is rejected", () => {
  expect(extractPictures({ traceEvents: [{ name: "Paint", args: {} }] })).toEqual([]);
  expect(() => extractPictures({})).toThrow("traceEvents missing");
  expect(() => extractPictures({ traceEvents: [null] })).toThrow("invalid trace object");
  for (const value of ["", "AB==", "not base64", 42]) {
    expect(() => extractPictures({ traceEvents: [{ args: { skp64: value } }] })).toThrow();
  }
});

test("duplicate picture payload retains every event", () => {
  const rows = extractPictures({
    traceEvents: [
      { name: "Picture", ts: 10, args: { layerId: 1, skp64: "AQID" } },
      { name: "Picture", ts: 20, args: { layerId: 2, skp64: "AQID" } },
    ],
  });
  expect(rows).toHaveLength(1);
  expect(rows[0]?.events).toMatchObject([
    { ts: 10, args: { layerId: 1 } },
    { ts: 20, args: { layerId: 2 } },
  ]);
});
