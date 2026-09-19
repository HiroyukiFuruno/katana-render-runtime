export function installDrawioDeterminism() {
  const deterministicNow = Date.parse("2026-01-01T00:00:00.000Z");
  const originalDate = globalThis.Date;
  globalThis.Date = new Proxy(originalDate, {
    apply(target) {
      return Reflect.construct(target, [deterministicNow]).toString();
    },
    construct(target, args, newTarget) {
      return Reflect.construct(target, args.length === 0 ? [deterministicNow] : args, newTarget);
    },
  });
  Date.now = () => deterministicNow;

  let randomState = 0x12345678;
  Math.random = () => {
    randomState = (1664525 * randomState + 1013904223) >>> 0;
    return randomState / 0x100000000;
  };
}
