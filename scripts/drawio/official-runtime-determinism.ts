export function installDrawioDeterminism() {
  const deterministicNow = Date.parse("2026-01-01T00:00:00.000Z");
  const originalDate = globalThis.Date;
  globalThis.Date = new Proxy(originalDate, {
    apply(target) {
      return Reflect.construct(target, [deterministicNow]).toUTCString();
    },
    construct(target, args, newTarget) {
      return Reflect.construct(target, args.length === 0 ? [deterministicNow] : args, newTarget);
    },
  });
  Date.now = () => deterministicNow;
  const datePrototype = originalDate.prototype;
  const utcGetters = [
    ["getDate", "getUTCDate"],
    ["getDay", "getUTCDay"],
    ["getFullYear", "getUTCFullYear"],
    ["getHours", "getUTCHours"],
    ["getMilliseconds", "getUTCMilliseconds"],
    ["getMinutes", "getUTCMinutes"],
    ["getMonth", "getUTCMonth"],
    ["getSeconds", "getUTCSeconds"],
  ] as const;
  for (const [localGetter, utcGetter] of utcGetters) {
    const utcMethod = datePrototype[utcGetter];
    Object.defineProperty(datePrototype, localGetter, {
      configurable: true,
      writable: true,
      value: function katanaDrawioUtcGetter(this: Date) {
        return utcMethod.call(this);
      },
    });
  }
  Object.defineProperty(datePrototype, "getTimezoneOffset", {
    configurable: true,
    writable: true,
    value: function katanaDrawioUtcTimezoneOffset() {
      return 0;
    },
  });
  datePrototype.toLocaleDateString = function katanaDrawioLocaleDate() {
    return this.toISOString().slice(0, 10);
  };
  datePrototype.toLocaleString = function katanaDrawioLocaleDate() {
    return this.toISOString().slice(0, 10);
  };
  datePrototype.toLocaleTimeString = function katanaDrawioLocaleTime() {
    return this.toISOString().slice(11, 19);
  };

  let randomState = 0x12345678;
  Math.random = () => {
    randomState = (1664525 * randomState + 1013904223) >>> 0;
    return randomState / 0x100000000;
  };
}
