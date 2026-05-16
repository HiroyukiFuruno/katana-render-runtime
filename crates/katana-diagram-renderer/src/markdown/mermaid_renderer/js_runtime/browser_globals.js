globalThis.window = globalThis;
globalThis.self = globalThis;
globalThis.navigator = { userAgent: "KatanA Rust-managed Mermaid runtime" };
globalThis.location = {
  href: "https://katana.local/",
  origin: "https://katana.local",
  protocol: "https:",
  host: "katana.local",
  hostname: "katana.local",
  pathname: "/",
  search: "",
  hash: "",
};
const KATANA_DETERMINISTIC_NOW = Date.parse("2026-01-01T00:00:00.000Z");
globalThis.Date.now = () => KATANA_DETERMINISTIC_NOW;
globalThis.performance = { now: () => Date.now() };
globalThis.devicePixelRatio = 1;
globalThis.innerWidth = 1520;
globalThis.innerHeight = 845;
globalThis.screen = {
  width: globalThis.innerWidth,
  height: globalThis.innerHeight,
  availWidth: globalThis.innerWidth,
  availHeight: globalThis.innerHeight,
};
globalThis.localStorage = katanaStorage();
globalThis.sessionStorage = katanaStorage();
class KatanaMessageChannel {
  constructor() {
    this.port1 = { onmessage: null };
    this.port2 = {
      postMessage: (data) => {
        queueMicrotask(() => this.port1.onmessage?.({ data }));
      },
    };
  }
}

class KatanaSegmenter {
  segment(value) {
    const input = String(value);
    let index = 0;
    return Array.from(input).map((segment) => {
      const entry = { segment, index, input };
      index += segment.length;
      return entry;
    });
  }
}
const katanaIntl = globalThis.Intl ?? {};
// WHY: Some Mermaid diagrams call Intl.Segmenter for wrapping, but embedded V8 native dispatch can terminate the process.
katanaIntl.Segmenter = KatanaSegmenter;
globalThis.Intl = katanaIntl;
globalThis.crypto = {
  getRandomValues(array) {
    for (let index = 0; index < array.length; index += 1) {
      array[index] = katanaDeterministicByte(index);
    }
    return array;
  },
  randomUUID() {
    return "00000000-0000-4000-8000-000000000000";
  },
};
Math.random = katanaDeterministicRandom;
let katanaRandomState = 0x12345678;
globalThis.queueMicrotask = (callback) => Promise.resolve().then(callback);

function katanaDeterministicRandom() {
  katanaRandomState = (1664525 * katanaRandomState + 1013904223) >>> 0;
  return katanaRandomState / 0x100000000;
}

function katanaDeterministicByte(index) {
  return (index * 73 + 41) & 0xff;
}

function katanaStorage() {
  const values = {};
  return {
    get length() {
      return Object.keys(values).length;
    },
    clear() {
      Object.keys(values).forEach((key) => delete values[key]);
    },
    getItem(key) {
      return values[String(key)] ?? null;
    },
    key(index) {
      return Object.keys(values)[index] ?? null;
    },
    removeItem(key) {
      delete values[String(key)];
    },
    setItem(key, value) {
      values[String(key)] = String(value);
    },
  };
}

globalThis.setTimeout = (callback, _delay, ...args) => callback(...args);
globalThis.clearTimeout = () => {};
globalThis.setInterval = (callback, _delay, ...args) => callback(...args);
globalThis.clearInterval = () => {};
globalThis.MessageChannel = KatanaMessageChannel;
let katanaAnimationFrameDepth = 0;
globalThis.requestAnimationFrame = (callback) => katanaRunAnimationFrame(callback);
function katanaRunAnimationFrame(callback) {
  if (katanaAnimationFrameDepth > 4) {
    return 0;
  }
  return katanaInvokeAnimationFrame(callback);
}
function katanaInvokeAnimationFrame(callback) {
  katanaAnimationFrameDepth += 1;
  try {
    return callback(Date.now());
  } finally {
    katanaAnimationFrameDepth -= 1;
  }
}
globalThis.cancelAnimationFrame = () => {};
globalThis.addEventListener = () => {};
globalThis.removeEventListener = () => {};
globalThis.__katanaMissingSelectors = [];

const KATANA_BASE64_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";

globalThis.btoa = (value) => {
  const utf8 = unescape(encodeURIComponent(String(value)));
  return katanaBase64Triplets(utf8).map(katanaBase64Chunk).join("");
};

function katanaBase64Triplets(value) {
  return value.match(/[\s\S]{1,3}/g) ?? [];
}

function katanaBase64Chunk(chunk) {
  const first = chunk.charCodeAt(0);
  const second = chunk.charCodeAt(1);
  const third = chunk.charCodeAt(2);
  return [
    first >> 2,
    ((first & 3) << 4) | (second >> 4),
    katanaThirdBase64Index(second, third),
    katanaFourthBase64Index(third),
  ]
    .map((index) => KATANA_BASE64_CHARS.charAt(index))
    .join("");
}

function katanaThirdBase64Index(second, third) {
  if (Number.isNaN(second)) {
    return 64;
  }
  return ((second & 15) << 2) | (third >> 6);
}

function katanaFourthBase64Index(third) {
  if (Number.isNaN(third)) {
    return 64;
  }
  return third & 63;
}

globalThis.DOMPurify = {
  sanitize(value) {
    return String(value ?? "");
  },
  addHook() {},
  removeHook() {},
};

class KatanaStyle {
  constructor() {
    this.values = {};
  }
  setProperty(name, value) {
    this.values[String(name)] = String(value);
  }
  getPropertyValue(name) {
    return this.values[String(name)] ?? "";
  }
  removeProperty(name) {
    const value = this.getPropertyValue(name);
    delete this.values[String(name)];
    return value;
  }
  set mixBlendMode(value) {
    this.setProperty("mix-blend-mode", value);
  }
  get mixBlendMode() {
    return this.getPropertyValue("mix-blend-mode");
  }
  get cssText() {
    return Object.entries(this.values)
      .map(([key, value]) => `${key}: ${value};`)
      .join(" ");
  }
  clone() {
    const cloned = new KatanaStyle();
    cloned.values = { ...this.values };
    return cloned;
  }
}

class KatanaCSSRule {
  constructor(cssText) {
    this.cssText = String(cssText);
  }
}

class KatanaCSSStyleSheet {
  constructor() {
    this.cssRules = [];
  }
  insertRule(rule, index = this.cssRules.length) {
    const insertIndex = katanaCssRuleInsertIndex(index, this.cssRules.length);
    this.cssRules.splice(insertIndex, 0, new KatanaCSSRule(rule));
    return insertIndex;
  }
  replaceSync(cssText) {
    this.cssRules = katanaStyleSheetRuleTexts(cssText).map((it) => new KatanaCSSRule(it));
  }
}

function katanaCssRuleInsertIndex(index, length) {
  const parsed = Number(index);
  if (!Number.isFinite(parsed)) {
    return length;
  }
  return Math.max(0, Math.min(Math.trunc(parsed), length));
}

function katanaStyleSheetRuleTexts(cssText) {
  const source = String(cssText ?? "").trim();
  if (source.length === 0) {
    return [];
  }
  return source.match(/[^{}]+\{[^{}]*\}/g) ?? [source];
}

globalThis.CSSStyleSheet = KatanaCSSStyleSheet;

globalThis.getComputedStyle = (node) => ({
  getPropertyValue(name) {
    return node?.style?.getPropertyValue?.(name) ?? "";
  },
});
