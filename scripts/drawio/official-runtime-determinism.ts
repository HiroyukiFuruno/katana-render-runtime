export function installDrawioDeterminism() {
  const deterministicNow = Date.parse("2026-01-01T00:00:00.000Z");
  Date.now = () => deterministicNow;

  let randomState = 0x12345678;
  Math.random = () => {
    randomState = (1664525 * randomState + 1013904223) >>> 0;
    return randomState / 0x100000000;
  };
}
