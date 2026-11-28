import { parseMarkerInventory } from "./svg-marker-browser-capture";

export function markerInventoryFixture() {
  const cases = [
    ...Array.from({ length: 3 }, (_, index) => ({
      input: `tests/painting/context/context-${index}.svg`,
      inputSha256: "a".repeat(64),
      upstreamPngSha256: "b".repeat(64),
      requestedWidth: 300,
      requestedHeight: 300,
    })),
    ...Array.from({ length: 44 }, (_, index) => ({
      input:
        index === 0
          ? "tests/painting/marker/percent-values.svg"
          : `tests/painting/marker/marker-${index}.svg`,
      inputSha256: "c".repeat(64),
      upstreamPngSha256: "d".repeat(64),
      requestedWidth: 300,
      requestedHeight: index === 0 ? 150 : 300,
    })),
    {
      input: "tests/painting/overflow/overflow.svg",
      inputSha256: "e".repeat(64),
      upstreamPngSha256: "0".repeat(64),
      requestedWidth: 300,
      requestedHeight: 300,
    },
  ];
  return parseMarkerInventory({
    schema: "krr-svg-marker-capture-cases-v1",
    requestedWidth: 300,
    cases,
  });
}
