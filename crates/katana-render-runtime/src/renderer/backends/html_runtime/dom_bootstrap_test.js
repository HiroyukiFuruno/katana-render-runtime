import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

/** @typedef {{getAttribute(name: string): string | null, loaded?: boolean}} LoadElement */
/** @typedef {{getAttribute(name: string): string | null, dispatchEvent(event: {type: string}): unknown}} ImageFixture */
/** @typedef {{contentDocument: object, dispatchEvent(event: {type: string}): unknown}} FrameFixture */
/** @typedef {[object, string, ((this: object, event: Event) => void) | null]} LoadHandler */
/** @typedef {{__krrNodeId: string}} MutationTargetFixture */
/** @typedef {{type: string, target: MutationTargetFixture, attributeName: string | null}} MutationRecordFixture */
/** @typedef {{observe(target: MutationTargetFixture, options: {attributes?: boolean, childList?: boolean, subtree?: boolean, attributeFilter?: string[]}): void, takeRecords(): MutationRecordFixture[]}} MutationObserverFixture */
/** @typedef {new (callback: () => void) => MutationObserverFixture} MutationObserverConstructor */
/** @typedef {{x: number, y: number, width: number, height: number}} IntersectionRectFixture */
/** @typedef {{owner: string, activeAxes?: [boolean, boolean] | undefined, x: number, y: number, width: number, height: number, corners: [number, number][], radiusX: number, radiusY: number, radii: [number, number][]}} ClipMetadataFixture */

const source = readFileSync(new URL("./dom_bootstrap.js", import.meta.url), "utf8");
const elementChildrenImplementation = source.match(
  /^const __krrElementChildren = \(element\) => \{[\s\S]*?^\};/m,
)?.[0];
const elementChildrenIndexedItemImplementation = source.match(
  /^const __krrElementChildrenIndexedItem = \(property, currentNodeIds\) => \{[\s\S]*?^\};/m,
)?.[0];
const elementChildrenGetter = source.match(
  /^ {2}get children\(\) \{\n {4}return __krrElementChildren\(this\);\n {2}\},$/m,
)?.[0];
const intersectionImplementation = source.match(
  /^const __krrIntersectionRect = \(first, second\) => \{[\s\S]*?(?=^globalThis\.IntersectionObserver)/m,
)?.[0];
const nativeDomCapture =
  source.match(/^const __krrNativeDomRaw = globalThis\.__krr_dom;$/m)?.[0] +
  "\nconst __krrNativeDom = __krrNativeDomRaw;";
const timerScheduler = source.match(
  /^const __krrTimerQueue = \[\];[\s\S]*?(?=^class __KrrEvent)/m,
)?.[0];
const mutationWrapper = source.match(
  /^const __krrNativeDomRaw = globalThis\.__krr_dom;[\s\S]*?^};(?=\nconst __krrLifecycleCheckpoint)/m,
)?.[0];
const mutationObserverImplementation = source.match(
  /^const __krrMutationObservers = new Set\(\);[\s\S]*?^};(?=\nlet __krrDocumentReadyState)/m,
)?.[0];
const lifecycleCheckpoint = source.match(
  /^const __krrLifecycleCheckpoint = Promise\.resolve\.bind\(Promise\);$/m,
)?.[0];
const listenerOptions = source.match(
  /^const __krrListenerOptions = \(options\) => \(\{[\s\S]*?^\}\);/m,
)?.[0];
const imageLoadEventType = source.match(
  /const __krrImageLoadEventType = \(_image, source\) => \{[\s\S]*?\n\};/,
)?.[0];
const imageSrcProperties = source.match(
  /^ {2}get src\(\) \{[\s\S]*?^ {2}set src\(value\) \{[\s\S]*?^ {2}\},$/m,
)?.[0];
const getAttributeMethod = source.match(/^ {2}getAttribute\(name\) \{[\s\S]*?^ {2}\},$/m)?.[0];
const focusedNodeImplementation = source.match(/^let __krrFocusedNodeId = null;[\s\S]*?^};$/m)?.[0];
const activeElementGetter = source.match(/^ {2}get activeElement\(\) \{[\s\S]*?^ {2}\},$/m)?.[0];
const tagNameGetter = source.match(/^ {2}get tagName\(\) \{[\s\S]*?^ {2}\},$/m)?.[0];
const anchorHashGetter = source.match(/^ {2}get hash\(\) \{[\s\S]*?^ {2}\},$/m)?.[0];
const valueGetter = source.match(/^ {2}get value\(\) \{[\s\S]*?^ {2}\},$/m)?.[0];
const valueSetter = source.match(/^ {2}set value\(value\) \{[\s\S]*?^ {2}\},$/m)?.[0];
const hiddenGetter = source.match(/^ {2}get hidden\(\) \{[\s\S]*?^ {2}\},$/m)?.[0];
const setAttributeMethod = source.match(
  /^ {2}setAttribute\(name, value\) \{[\s\S]*?^ {2}\},$/m,
)?.[0];
const bodyLoadHandler = source.match(
  /^const __krrInstallBodyLoadHandler = \(body, source\) => \{[\s\S]*?^\};/m,
)?.[0];
const staticBodyLoadHandler = source.match(
  /^globalThis\.__krrInstallStaticBodyLoadHandler = \(parserSource, laterToken = false\) => \{[\s\S]*?^\};/m,
)?.[0];
const windowLoadNodeCheck = source.match(
  /^const __krrIsWindowLoadNode = \(nodeId\) => \{[\s\S]*?^\};/m,
)?.[0];
const routeWindowLoadElement = source.match(
  /^const __krrRouteWindowLoadElement = \(element\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchListeners = source.match(
  /^const __krrDispatchListeners = \(listeners, target, event, capture\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchListenerEntry = source.match(
  /^const __krrDispatchListenerEntry = \(entries, entry, target, event, capture\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchHandler = source.match(
  /^const __krrDispatchHandler = \(target, event\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchDefaultAction = source.match(
  /^const __krrDispatchDefaultAction = \(target, event\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchTargetPhase = source.match(
  /^const __krrDispatchTargetPhase = \(target, event, capture, phase(?:, deferListenerError = false)?\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchElementPhase = source.match(
  /^const __krrDispatchElementPhase = \(target, event, capture, phase\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchElementEvent = source.match(
  /^const __krrDispatchElementEvent = \(target, event\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchCapture = source.match(
  /^const __krrDispatchCapture = \(event, ancestors\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchTarget = source.match(
  /^const __krrDispatchTarget = \(target, event\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchBubble = source.match(
  /^const __krrDispatchBubble = \(event, ancestors\) => \{[\s\S]*?^\};/m,
)?.[0];
const beginDispatch = source.match(
  /^const __krrBeginDispatch = \(target, event\) => \{[\s\S]*?^\};/m,
)?.[0];
const endDispatch = source.match(/^const __krrEndDispatch = \(event\) => \{[\s\S]*?^\};/m)?.[0];
const throwDeferredListenerError = source.match(
  /^const __krrThrowDeferredListenerError = \(event\) => \{[\s\S]*?^\};/m,
)?.[0];
const installEventTarget = source.match(
  /^const __krrInstallEventTarget = \(target\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchImageLoad = source.match(
  /^const __krrDispatchImageLoad = async \(image\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchPendingImages = source.match(
  /^const __krrDispatchPendingImages = async \(document, state = \{\}\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchLocalFrame = source.match(
  /^const __krrDispatchLocalFrame = \(frame\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchPendingLocalFrames = source.match(
  /^const __krrDispatchPendingLocalFrames = async \(document, state = \{\}\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchWindowLoad = source.match(
  /^globalThis\.__krrDispatchWindowLoad = async \(\) => \{[\s\S]*?^\};/m,
)?.[0];
const seedPrevalidatedImageEvents = source.match(
  /^const __krrSeedPrevalidatedImageEvents = \(\) => \{[\s\S]*?^\};/m,
)?.[0];

/** @param {ClipMetadataFixture} clip @param {IntersectionRectFixture} targetRect */
const installIntersectionHarness = (clip, targetRect = { x: 0, y: 0, width: 100, height: 100 }) => {
  /** @param {string} operation @param {string} nodeId */
  const nativeDom = (operation, nodeId) => {
    if (operation === "layoutMetrics")
      return JSON.stringify({ width: 100, height: 100, scrollX: 0, scrollY: 0 });
    if (operation === "layoutBoxPresent") return "1";
    if (operation === "intersectionMetadata")
      return JSON.stringify({
        clips: [clip],
        fragments: [],
        positioningOrigin: String(nodeId),
        positioning: "in-flow",
        viewportEscape: false,
      });
    throw new Error(`unexpected native operation: ${operation}`);
  };
  const document = {};
  const target = {
    __krrNodeId: "target",
    getBoundingClientRect: () => ({
      ...targetRect,
      top: targetRect.y,
      right: targetRect.x + targetRect.width,
      bottom: targetRect.y + targetRect.height,
      left: targetRect.x,
    }),
  };
  const observer = {
    root: null,
    __krrRootMargin: [
      { amount: 0, unit: "px" },
      { amount: 0, unit: "px" },
      { amount: 0, unit: "px" },
      { amount: 0, unit: "px" },
    ],
  };
  const install = new Function(
    "document",
    "__krrNativeDom",
    `const __krrLayoutMetrics = () => ({ width: 100, height: 100, scrollX: 0, scrollY: 0 });\n${intersectionImplementation}\nreturn { entry: (observer, target) => __krrIntersectionEntry(observer, target), active: (subject, clip) => __krrActivePolygonIntersection(subject, clip) };`,
  );
  const installed = install(document, nativeDom);
  return { entry: installed.entry(observer, target), active: installed.active, target };
};

const installStaticLoadHandler = (
  /** @type {LoadElement | null} */ body,
  /** @type {LoadElement | null} */ frameset,
) => {
  const pageGlobal = {};
  /** @type {{loaded?: boolean}} */
  const window = {};
  const document = {
    body,
    querySelector(/** @type {string} */ selector) {
      expect(selector).toBe("html > frameset");
      return frameset;
    },
  };
  /** @type {LoadHandler[]} */
  const eventHandlers = [];
  const install = new Function(
    "globalThis",
    "window",
    "document",
    "__krrLifecyclePropertyOverrides",
    "__krrStoreEventHandler",
    `${bodyLoadHandler}\n${staticBodyLoadHandler}\nreturn globalThis.__krrInstallStaticBodyLoadHandler;`,
  )(
    pageGlobal,
    window,
    document,
    new Map([[pageGlobal, new Map([["load", true]])]]),
    (
      /** @type {[object, string, ((this: object, event: Event) => void) | null]} */ ...arguments_
    ) => eventHandlers.push(arguments_),
  );

  install();
  return { eventHandlers, window };
};

test("画像検証はページコードがglobalThis.__krr_domを置換してもnative bridgeを使う", () => {
  expect(nativeDomCapture).toBeDefined();
  expect(imageLoadEventType).toBeDefined();

  /** @type {unknown[][]} */
  const calls = [];
  const nativeBridge = (/** @type {unknown[]} */ ...arguments_) => {
    calls.push(arguments_);
    return "load";
  };
  const pageGlobal = { __krr_dom: nativeBridge };
  const getImageLoadEventType = new Function(
    "globalThis",
    "__krrPrevalidatedImageEvents",
    `${nativeDomCapture}\n${imageLoadEventType}\nreturn __krrImageLoadEventType;`,
  )(pageGlobal, new WeakMap());

  pageGlobal.__krr_dom = () => "error";

  expect(getImageLoadEventType(null, "data:image/png;base64,AA==")).toBe("load");
  expect(calls).toEqual([["validateImageDataUrl", "data:image/png;base64,AA=="]]);
});

test("DOM property getters preserve native HTML values and activeElement wrapper identity", () => {
  expect(focusedNodeImplementation).toBeDefined();
  expect(activeElementGetter).toBeDefined();
  expect(tagNameGetter).toBeDefined();
  expect(valueGetter).toBeDefined();
  expect(hiddenGetter).toBeDefined();
  const wrappers = new Map([
    ["42", { __krrNodeId: "42" }],
    ["43", { __krrNodeId: "43" }],
  ]);
  const body = { __krrNodeId: "body" };
  /** @type {(nodeId: unknown) => unknown} */
  const elementForNode = (nodeId) => {
    if (!wrappers.has(String(nodeId))) throw new Error("unknown wrapper id");
    return wrappers.get(String(nodeId));
  };
  /** @type {(operation: string, nodeId: unknown) => string} */
  const nativeDomForActiveElement = (operation, nodeId) =>
    operation === "isConnected" && nodeId === "42" ? "1" : "0";
  const activeElement = new Function(
    "globalThis",
    "__krrElement",
    "__krrNativeDom",
    "body",
    `${focusedNodeImplementation}\nconst document = {get body() { return body; }, ${activeElementGetter}}; return {document, set: globalThis.__krrSetFocusedNodeId};`,
  )({ __krrElement: undefined }, elementForNode, nativeDomForActiveElement, body);
  expect(activeElement.document.activeElement).toBe(body);
  activeElement.set("42");
  expect(activeElement.document.activeElement).toBe(wrappers.get("42"));
  activeElement.set("43");
  expect(activeElement.document.activeElement).toBe(body);
  activeElement.set("999");
  expect(activeElement.document.activeElement).toBe(body);
  activeElement.set(null);
  expect(activeElement.document.activeElement).toBe(body);
  activeElement.set("999");
  expect(activeElement.document.activeElement).toBe(body);
  expect(() => activeElement.set(1)).toThrow(TypeError);
  expect(() => activeElement.set("1e2")).toThrow(TypeError);

  /** @type {unknown[][]} */
  const calls = [];
  /** @type {string | undefined} */
  let nativeValue = "DIV";
  /** @type {(operation: string, nodeId: string) => unknown} */
  const nativeDom = (operation, nodeId) => {
    calls.push([operation, nodeId]);
    if (operation === "tagName") return "DIV";
    if (operation === "value") return nativeValue;
    if (operation === "hidden") return "1";
    return undefined;
  };
  const element = new Function(
    "__krrNativeDom",
    `return ({__krrNodeId: "node", ${tagNameGetter}\n${valueGetter}\n${hiddenGetter}});`,
  )(nativeDom);
  expect(element.tagName).toBe("DIV");
  nativeValue = undefined;
  expect(element.value).toBeUndefined();
  expect(element.hidden).toBe(true);
  expect(calls).toEqual([
    ["tagName", "node"],
    ["value", "node"],
    ["hidden", "node"],
  ]);
});

test("anchor hash getter resolves against the live location and excludes other elements", () => {
  expect(anchorHashGetter).toBeDefined();
  /** @type {Array<[string, string, string | undefined]>} */
  const calls = [];
  /** @type {(operation: string, nodeId: string, baseHref?: string) => unknown} */
  const nativeDom = (operation, nodeId, baseHref) => {
    calls.push([operation, nodeId, baseHref]);
    if (operation === "tagName") return nodeId === "anchor" ? "A" : "DIV";
    if (operation === "anchorHash") {
      return baseHref?.endsWith("#current") ? "#target" : "";
    }
    return undefined;
  };
  const makeElement = new Function(
    "globalThis",
    "__krrNativeDom",
    `return ({__krrNodeId: "anchor", ${tagNameGetter}\n${anchorHashGetter}});`,
  );
  const global = { location: { href: "https://example.test/page#current" } };
  const anchor = makeElement(global, nativeDom);
  expect(anchor.hash).toBe("#target");
  expect(calls).toEqual([
    ["tagName", "anchor", undefined],
    ["anchorHash", "anchor", "https://example.test/page#current"],
  ]);

  calls.length = 0;
  /** @type {(operation: string, nodeId: string, baseHref?: string) => unknown} */
  const otherNativeDom = (operation, nodeId, baseHref) => {
    calls.push([operation, nodeId, baseHref]);
    return "DIV";
  };
  const other = makeElement(global, otherNativeDom);
  expect(other.hash).toBeUndefined();
  expect(calls).toEqual([["tagName", "anchor", undefined]]);
});

test("select value setter uses selection semantics while other form setters retain attributes", () => {
  expect(valueSetter).toBeDefined();
  /** @type {unknown[][]} */
  const calls = [];
  /** @type {(operation: string, ...arguments_: unknown[]) => unknown} */
  const nativeDom = (operation, ...arguments_) => {
    calls.push([operation, ...arguments_]);
    return operation === "tagName" ? "SELECT" : undefined;
  };
  const setter = new Function("__krrNativeDom", `return ({__krrNodeId: "node", ${valueSetter}});`)(
    nativeDom,
  );
  setter.value = "second";
  expect(calls).toEqual([
    ["tagName", "node"],
    ["setSelectValue", "node", "second"],
  ]);

  calls.length = 0;
  /** @type {(operation: string, ...arguments_: unknown[]) => unknown} */
  const inputNativeDom = (operation, ...arguments_) => {
    calls.push([operation, ...arguments_]);
    return operation === "tagName" ? "INPUT" : undefined;
  };
  const inputSetter = new Function(
    "__krrNativeDom",
    `return ({__krrNodeId: "input", ${valueSetter}});`,
  )(inputNativeDom);
  inputSetter.value = "typed";
  expect(calls).toEqual([
    ["tagName", "input"],
    ["setAttribute", "input", "value", "typed"],
  ]);
});

test("clickのform既定動作はsubmit型だけを発火し、disabledと取消しを尊重する", () => {
  expect(dispatchDefaultAction).toBeDefined();
  class FakeEvent {
    static CAPTURING_PHASE = 1;
    static AT_TARGET = 2;
    static BUBBLING_PHASE = 3;
    constructor(
      /** @type {string} */ type,
      /** @type {{bubbles: boolean, cancelable: boolean}} */ options,
    ) {
      this.type = type;
      this.defaultPrevented = false;
      this.bubbles = options.bubbles;
      this.cancelable = options.cancelable;
    }
  }
  const invoke = new Function(
    "Event",
    `${dispatchDefaultAction}\nreturn __krrDispatchDefaultAction;`,
  )(FakeEvent);
  const submit = (
    /** @type {string} */ type,
    /** @type {Record<string, string>} */ attributes = {},
  ) => {
    /** @type {FakeEvent[]} */
    const calls = [];
    const form = { dispatchEvent: (/** @type {FakeEvent} */ event) => calls.push(event) };
    const button = {
      tagName: "BUTTON",
      getAttribute: (/** @type {string} */ name) =>
        attributes[name] ?? (name === "type" ? type : null),
      closest: (/** @type {string} */ selector) => (selector === "form" ? form : null),
    };
    const target = {
      closest: (/** @type {string} */ selector) => (selector === "button" ? button : null),
    };
    invoke(target, { type: "click", defaultPrevented: false });
    return calls;
  };
  expect(submit("submit")).toHaveLength(1);
  expect(submit("button")).toHaveLength(0);
  expect(submit("reset")).toHaveLength(0);
  expect(submit("submit", { disabled: "" })).toHaveLength(0);
  const cancelled = submit("submit");
  expect(cancelled).toHaveLength(1);
  invoke(
    {
      closest: () => ({
        tagName: "BUTTON",
        getAttribute: () => "submit",
        closest: () => ({ dispatchEvent: () => {} }),
      }),
    },
    { type: "click", defaultPrevented: true },
  );
});

test("timerはclearTimeoutで除外され、明示的なpumpでFIFO実行される", () => {
  expect(timerScheduler).toBeDefined();
  const pageGlobal = {};
  const install = new Function(
    "globalThis",
    `${timerScheduler}\nreturn {setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout, run: globalThis.__krrRunTimers};`,
  )(pageGlobal);
  /** @type {string[]} */
  const order = [];
  install.setTimeout(() => order.push("first"), 0);
  const cancelled = install.setTimeout(() => order.push("cancelled"), 0);
  install.setTimeout(() => order.push("last"), 10);
  expect(order).toEqual([]);
  install.clearTimeout(cancelled);
  install.run();
  expect(order).toEqual(["first"]);
  install.run(10);
  expect(order).toEqual(["first", "last"]);
});

test("timerは登録順に関係なく期限順、同一期限では登録順に実行される", () => {
  const install = new Function(
    "globalThis",
    `${timerScheduler}\nreturn {setTimeout: globalThis.setTimeout, run: globalThis.__krrRunTimers};`,
  )({});
  /** @type {string[]} */
  const order = [];
  install.setTimeout(() => order.push("later"), 25);
  install.setTimeout(() => order.push("earlier"), 5);
  install.setTimeout(() => order.push("same-deadline-first"), 5);
  install.run(25);
  expect(order).toEqual(["earlier", "same-deadline-first", "later"]);
});

test("timer callback can cancel a due sibling and enqueue immediate work", () => {
  const install = new Function(
    "globalThis",
    `${timerScheduler}\nreturn {setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout, run: globalThis.__krrRunTimers};`,
  )({});
  /** @type {string[]} */
  const order = [];
  let cancelled = 0;
  install.setTimeout(() => {
    order.push("first");
    install.clearTimeout(cancelled);
    install.setTimeout(() => order.push("reentrant"), 0);
  }, 0);
  cancelled = install.setTimeout(() => order.push("cancelled"), 0);
  install.run();
  expect(order).toEqual(["first", "reentrant"]);
});

/** @param {[number, number][]} corners @param {number} width @param {number} height @param {[boolean, boolean] | undefined} activeAxes */
const clipMetadata = (corners, width, height, activeAxes) => ({
  owner: "ancestor",
  activeAxes,
  x: Math.min(...corners.map(([x]) => x)),
  y: Math.min(...corners.map(([, y]) => y)),
  width,
  height,
  corners: /** @type {[number, number][]} */ (corners),
  radiusX: 0,
  radiusY: 0,
  radii: /** @type {[number, number][]} */ ([
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
  ]),
});

test("IntersectionObserverは高さ0のx軸clipでもx半平面を保持する", () => {
  /** @type {[number, number][]} */
  const corners = [
    [20, 20],
    [60, 20],
    [60, 20],
    [20, 20],
  ];
  const { entry } = installIntersectionHarness(clipMetadata(corners, 40, 0, [true, false]));
  expect(entry.intersectionRect).toEqual({
    x: 20,
    y: 0,
    width: 40,
    height: 100,
    top: 0,
    right: 60,
    bottom: 100,
    left: 20,
  });
  expect(entry.intersectionRatio).toBeCloseTo(0.4);
  expect(entry.isIntersecting).toBe(true);
});

test("IntersectionObserverは幅0のy軸clipでもy半平面を保持する", () => {
  /** @type {[number, number][]} */
  const corners = [
    [20, 20],
    [20, 20],
    [20, 60],
    [20, 60],
  ];
  const { entry } = installIntersectionHarness(clipMetadata(corners, 0, 40, [false, true]));
  expect(entry.intersectionRatio).toBeCloseTo(0.4);
  expect(entry.isIntersecting).toBe(true);
});

test("IntersectionObserverは回転した高さ0のclipでも選択軸を維持する", () => {
  /** @type {[number, number][]} */
  const corners = [
    [29.2893218813, 29.2893218813],
    [70.7106781187, 70.7106781187],
    [70.7106781187, 70.7106781187],
    [29.2893218813, 29.2893218813],
  ];
  const { entry } = installIntersectionHarness(
    clipMetadata(corners, 41.4213562374, 0, [true, false]),
  );
  expect(entry.intersectionRatio).toBeGreaterThan(0);
  expect(entry.isIntersecting).toBe(true);
});

test("IntersectionObserverは選択軸の幅0をedge adjacencyとして扱う", () => {
  /** @type {[number, number][]} */
  const corners = [
    [40, 20],
    [40, 20],
    [40, 60],
    [40, 60],
  ];
  const { entry } = installIntersectionHarness(clipMetadata(corners, 0, 40, [true, false]));
  expect(entry.intersectionRatio).toBe(0);
  expect(entry.isIntersecting).toBe(true);
});

test("IntersectionObserverはancestor片軸clipをroot境界へ二重適用しない", () => {
  /** @type {[number, number][]} */
  const corners = [
    [0, 0],
    [100, 0],
    [100, 40],
    [0, 40],
  ];
  const { entry } = installIntersectionHarness(clipMetadata(corners, 100, 40, [true, false]), {
    x: 0,
    y: 0,
    width: 500,
    height: 400,
  });
  expect(entry.intersectionRect).toEqual({
    x: 0,
    y: 0,
    width: 100,
    height: 100,
    top: 0,
    right: 100,
    bottom: 100,
    left: 0,
  });
  expect(entry.intersectionRatio).toBeCloseTo(0.05);
  expect(entry.isIntersecting).toBe(true);
});

test("IntersectionObserverはactiveAxes欠落を両軸有効として解釈しない", () => {
  /** @type {[number, number][]} */
  const corners = [
    [20, 20],
    [60, 20],
    [60, 60],
    [20, 60],
  ];
  const { entry } = installIntersectionHarness(clipMetadata(corners, 40, 40, undefined));
  expect(entry.intersectionRatio).toBe(0);
  expect(entry.isIntersecting).toBe(false);
});

const installMutationObserverHarness = () => {
  const parents = new Map([["child", "root"]]);
  /** @type {string[]} */
  const operations = [];
  /** @type {unknown[][]} */
  const nativeArguments = [];
  /** @param {string} operation @param {unknown} nodeId */
  const nativeMutationFailure = (operation, nodeId) => {
    if (nodeId === "native-error" || (nodeId === "mutation-error" && operation !== "eventPath"))
      return Object.assign(new Error("native failure"), { name: "KrrNativeDomError" });
    return nodeId === "unknown-error" ? new Error("unexpected failure") : null;
  };
  /** @param {unknown} nodeId */
  const nativeEventPath = (nodeId) => {
    const path = [String(nodeId)];
    while (true) {
      const current = path.at(-1);
      if (current === undefined) break;
      const parent = parents.get(current);
      if (parent === undefined) break;
      path.push(parent);
    }
    return path;
  };
  /** @type {(operation: string, nodeId: unknown, ...args: unknown[]) => unknown} */
  const nativeMutation = (operation, nodeId, ...args) => {
    operations.push(operation);
    nativeArguments.push([operation, nodeId, ...args]);
    const failure = nativeMutationFailure(operation, nodeId);
    if (failure) throw failure;
    if (operation === "eventPath") return nativeEventPath(nodeId);
    if (operation === "remove") parents.delete(String(nodeId));
    return "native-result";
  };
  const pageGlobal = {
    __krr_dom: nativeMutation,
  };
  /** @param {string} nodeId */
  const element = (nodeId) => ({ __krrNodeId: nodeId });
  const install = new Function(
    "globalThis",
    "__krrElement",
    "operations",
    "nativeArguments",
    `${mutationWrapper}\nconst earlyResult = __krrNativeDom("setAttribute", "root", "title", "early");\n${mutationObserverImplementation}\nreturn {nativeDom: __krrNativeDom, MutationObserver: globalThis.MutationObserver, earlyResult, operations, nativeArguments};`,
  );
  return /** @type {{nativeDom: (...args: unknown[]) => unknown, MutationObserver: MutationObserverConstructor, earlyResult: unknown, operations: string[], nativeArguments: unknown[][]}} */ (
    install(pageGlobal, element, operations, nativeArguments)
  );
};

const installElementChildrenHarness = () => {
  let revision = 0;
  const childIds = new Map([["parent", ["first", "second"]]]);
  const attributes = new Map([
    [
      "first",
      new Map([
        ["id", "duplicate"],
        ["name", "first-name"],
      ]),
    ],
    ["second", new Map([["name", "duplicate"]])],
  ]);
  const wrappers = new Map();
  const nativeDom = (
    /** @type {string} */ operation,
    /** @type {string} */ nodeId,
    /** @type {string} */ name,
  ) => {
    if (operation === "treeRevision") return String(revision);
    if (operation === "elementChildren") return [...(childIds.get(nodeId) ?? [])];
    if (operation === "getAttribute") return attributes.get(nodeId)?.get(name) ?? null;
    throw new Error(`unexpected native operation: ${operation}`);
  };
  const element = (/** @type {string} */ nodeId) => {
    const existing = wrappers.get(nodeId);
    if (existing) return existing;
    const wrapper = {
      __krrNodeId: nodeId,
      get id() {
        return nativeDom("getAttribute", nodeId, "id") ?? "";
      },
      getAttribute(/** @type {string} */ name) {
        return nativeDom("getAttribute", nodeId, name);
      },
    };
    wrappers.set(nodeId, wrapper);
    return wrapper;
  };
  const childrenFor = new Function(
    "__krrNativeDom",
    "__krrElement",
    `const __krrElementChildrenCollections = new WeakMap();\n${elementChildrenIndexedItemImplementation}\n${elementChildrenImplementation}\nreturn __krrElementChildren;`,
  )(nativeDom, element);
  const elementPrototype = new Function(
    "__krrElementChildren",
    `return ({${elementChildrenGetter}});`,
  )(childrenFor);
  const parent = Object.assign(Object.create(elementPrototype), { __krrNodeId: "parent" });
  return {
    parent,
    element,
    childIds,
    attributes,
    bumpRevision: () => {
      revision += 1;
    },
  };
};

test("MutationObserverは実操作種別と属性名で通知し、属性filterを適用する", () => {
  expect(mutationWrapper).toBeDefined();
  expect(mutationObserverImplementation).toBeDefined();
  const { nativeDom, MutationObserver, earlyResult, operations, nativeArguments } =
    installMutationObserverHarness();
  expect(earlyResult).toBe("native-result");
  expect(operations).toEqual(["setAttribute"]);
  const target = { __krrNodeId: "root" };
  const attributes = new MutationObserver(() => {});
  const childList = new MutationObserver(() => {});
  const combined = new MutationObserver(() => {});
  const filtered = new MutationObserver(() => {});
  const caseMismatch = new MutationObserver(() => {});
  const whitespaceMismatch = new MutationObserver(() => {});
  const unicodeFilter = new MutationObserver(() => {});
  const unicodeMismatch = new MutationObserver(() => {});
  attributes.observe(target, { attributes: true });
  childList.observe(target, { childList: true });
  combined.observe(target, { attributes: true, childList: true });
  filtered.observe(target, { attributeFilter: ["class"] });
  caseMismatch.observe(target, { attributeFilter: ["CLASS"] });
  whitespaceMismatch.observe(target, { attributeFilter: [" class "] });
  unicodeFilter.observe(target, { attributeFilter: ["İd"] });
  unicodeMismatch.observe(target, { attributeFilter: ["İD"] });
  expect(() =>
    new MutationObserver(() => {}).observe(target, {
      attributes: false,
      attributeFilter: ["class"],
      childList: true,
    }),
  ).toThrow(TypeError);

  expect(nativeDom("setAttribute", "root", "title", "value")).toBe("native-result");
  nativeDom("setAttribute", "root", " CLASS ", "active");
  nativeDom("setAttribute", "root", "İD", "active");
  nativeDom("styleSet", "root", "fontSize", "12px");
  for (const mutation of [
    ["appendChild", "root", "child"],
    ["appendText", "root", "text"],
    ["insertAdjacentHTML", "root", "beforeend", "<i></i>"],
    ["insertAdjacentText", "root", "beforeend", "text"],
    ["remove", "root"],
    ["setTextContent", "root", "text"],
    ["setInnerHTML", "root", "<b></b>"],
  ]) {
    nativeDom(...mutation);
  }
  expect(attributes.takeRecords().map((record) => [record.type, record.attributeName])).toEqual([
    ["attributes", "title"],
    ["attributes", "class"],
    ["attributes", "İd"],
    ["attributes", "style"],
  ]);
  expect(childList.takeRecords().map((record) => record.type)).toEqual(
    Array.from({ length: 7 }, () => "childList"),
  );
  expect(combined.takeRecords().map((record) => record.type)).toEqual([
    "attributes",
    "attributes",
    "attributes",
    "attributes",
    ...Array.from({ length: 7 }, () => "childList"),
  ]);
  expect(filtered.takeRecords().map((record) => record.attributeName)).toEqual(["class"]);
  expect(caseMismatch.takeRecords()).toEqual([]);
  expect(whitespaceMismatch.takeRecords()).toEqual([]);
  expect(unicodeFilter.takeRecords().map((record) => record.attributeName)).toEqual(["İd"]);
  expect(unicodeMismatch.takeRecords()).toEqual([]);
  expect(nativeArguments).toContainEqual(["setAttribute", "root", " CLASS ", "active"]);
  expect(nativeArguments).toContainEqual(["setAttribute", "root", "İD", "active"]);
});

test("MutationObserverは変更前の祖先でremoveを配送し、同一targetのobserveを置換する", () => {
  const { nativeDom, MutationObserver } = installMutationObserverHarness();
  const root = { __krrNodeId: "root" };
  const child = { __krrNodeId: "child" };
  const subtree = new MutationObserver(() => {});
  const childSubtree = new MutationObserver(() => {});
  const replaced = new MutationObserver(() => {});
  subtree.observe(root, { childList: true, subtree: true });
  childSubtree.observe(child, { childList: true, subtree: true });
  replaced.observe(root, { childList: true });
  replaced.observe(root, { attributes: true, attributeFilter: ["title"] });

  nativeDom("insertAdjacentHTML", "child", "beforebegin", "<i></i>");
  expect(childSubtree.takeRecords()).toEqual([]);
  nativeDom("remove", "child");
  expect(subtree.takeRecords().map((record) => [record.type, record.target.__krrNodeId])).toEqual([
    ["childList", "root"],
    ["childList", "root"],
  ]);
  expect(subtree.takeRecords()).toEqual([]);
  nativeDom("setTextContent", "root", "text");
  nativeDom("setAttribute", "root", "title", "value");
  expect(replaced.takeRecords().map((record) => [record.type, record.attributeName])).toEqual([
    ["attributes", "title"],
  ]);
});

test("childrenは同一collectionを保ち、namedItemとlive iterationへ構造変更を反映する", () => {
  expect(elementChildrenImplementation).toBeDefined();
  expect(elementChildrenIndexedItemImplementation).toBeDefined();
  expect(elementChildrenGetter).toBeDefined();
  const { parent, element, childIds, attributes, bumpRevision } = installElementChildrenHarness();
  const children = parent.children;
  expect(parent.children).toBe(children);
  expect(children.length).toBe(2);
  expect(children[0]).toBe(element("first"));
  expect(children.item(1)).toBe(element("second"));
  expect(children.item(2)).toBeNull();
  expect(children.namedItem("duplicate")).toBe(element("first"));
  expect(children.namedItem("")).toBeNull();
  expect(Array.from(children)).toEqual([element("first"), element("second")]);

  attributes.get("first")?.set("id", "first-id");
  attributes.get("first")?.set("name", "duplicate");
  attributes.get("second")?.set("id", "duplicate");
  expect(children.namedItem("duplicate")).toBe(element("second"));
  attributes.get("first")?.set("name", "first-name");
  expect(children.duplicate).toBe(element("second"));
  expect("duplicate" in children).toBe(true);
  expect(Object.hasOwn(children, "duplicate")).toBe(true);
  expect(Reflect.ownKeys(children)).toContain("duplicate");
  expect(Object.keys(children)).toEqual(["0", "1"]);
  expect(Object.getOwnPropertyDescriptor(children, "duplicate")).toEqual({
    value: element("second"),
    writable: false,
    enumerable: false,
    configurable: true,
  });
  expect(Reflect.set(children, "length", 3)).toBe(false);
  expect(Reflect.set(children, "0", element("third"))).toBe(false);
  expect(Reflect.set(children, "duplicate", element("third"))).toBe(false);
  expect(Reflect.defineProperty(children, "0", { value: element("third") })).toBe(false);
  expect(Reflect.defineProperty(children, "duplicate", { value: element("third") })).toBe(false);
  expect(Reflect.deleteProperty(children, "0")).toBe(false);
  expect(Reflect.deleteProperty(children, "duplicate")).toBe(false);
  expect(Reflect.set(children, "customExpando", 1)).toBe(true);
  expect(children.customExpando).toBe(1);

  const iterator = children[Symbol.iterator]();
  expect(iterator.next()).toEqual({ done: false, value: element("first") });
  childIds.set("parent", ["first", "second", "third"]);
  bumpRevision();
  expect(iterator.next()).toEqual({ done: false, value: element("second") });
  expect(iterator.next()).toEqual({ done: false, value: element("third") });
  expect(iterator.next().done).toBe(true);

  attributes.get("first")?.set("id", "renamed");
  expect(children.namedItem("duplicate")).toBe(element("second"));
  childIds.set("parent", ["second"]);
  bumpRevision();
  expect(children.length).toBe(1);
  expect(children[0]).toBe(element("second"));
  expect(children[1]).toBeUndefined();
});

test("native bridge例外では公開戻り値を維持し、MutationObserverへ成功記録を送らない", () => {
  const { nativeDom, MutationObserver } = installMutationObserverHarness();
  const observer = new MutationObserver(() => {});
  observer.observe({ __krrNodeId: "root" }, { attributes: true });
  expect(nativeDom("setAttribute", "native-error", "title", "value")).toBeUndefined();
  expect(nativeDom("setAttribute", "mutation-error", "title", "value")).toBeUndefined();
  expect(observer.takeRecords()).toEqual([]);
  expect(() => nativeDom("setAttribute", "unknown-error", "title", "value")).toThrow(
    "unexpected failure",
  );
});

test("image.src propertyとsetAttributeがnative src属性へ反映されload/error判定される", () => {
  expect(imageSrcProperties).toBeDefined();
  expect(setAttributeMethod).toBeDefined();
  expect(imageLoadEventType).toBeDefined();

  const attributes = new Map();
  const bridge = (
    /** @type {string} */ operation,
    /** @type {string} */ _nodeId,
    /** @type {string} */ name,
    /** @type {string} */ value,
  ) => {
    if (operation === "setAttribute") {
      attributes.set(name, value);
      return null;
    }
    if (operation === "getAttribute") return attributes.get(name) ?? null;
    if (operation === "validateImageDataUrl")
      return _nodeId.includes(";base64,AA==") ? "load" : "error";
    throw new Error(`unexpected native operation: ${operation}`);
  };
  const elementPrototype = new Function(
    "globalThis",
    "__krrPrevalidatedImageEvents",
    `const __krrNativeDom = globalThis.__krr_dom;
const __krrNormalizeLifecycleEventType = () => null;
const __krrIsWindowLoadNode = () => false;
const __krrInstallBodyLoadHandler = () => {};
const __krrInstallInlineHandler = () => {};
return ({${imageSrcProperties}\n${getAttributeMethod}\n${setAttributeMethod}});`,
  )({ __krr_dom: bridge }, new WeakMap());
  const image = Object.assign(Object.create(elementPrototype), { __krrNodeId: "image-1" });
  const getImageLoadEventType = new Function(
    "globalThis",
    "__krrPrevalidatedImageEvents",
    `${nativeDomCapture}\n${imageLoadEventType}\nreturn __krrImageLoadEventType;`,
  )({ __krr_dom: bridge }, new WeakMap());

  image.src = "data:image/png;base64,AA==";
  expect(image.src).toBe("data:image/png;base64,AA==");
  expect(getImageLoadEventType(image, image.getAttribute("src"))).toBe("load");

  image.setAttribute("src", "data:image/png;base64,invalid");
  expect(image.src).toBe("data:image/png;base64,invalid");
  expect(getImageLoadEventType(image, image.getAttribute("src"))).toBe("error");
});

test("document.bodyがないframesetのonloadをWindow load handlerとして登録する", () => {
  expect(bodyLoadHandler).toBeDefined();
  expect(staticBodyLoadHandler).toBeDefined();

  /** @type {LoadElement} */
  const frameset = {
    getAttribute: (/** @type {string} */ name) => (name === "onload" ? "this.loaded = true" : null),
  };
  const { eventHandlers, window } = installStaticLoadHandler(null, frameset);

  expect(eventHandlers).toHaveLength(1);
  const loadHandler = eventHandlers[0];
  if (!loadHandler) throw new Error("load handler missing");
  expect(loadHandler[0]).toBe(window);
  expect(loadHandler[1]).toBe("load");
  if (!loadHandler[2]) throw new Error("load callback missing");
  loadHandler[2].call(window, new Event("load"));
  expect(frameset.loaded).toBe(true);
  expect(window.loaded).toBeUndefined();
});

test("document.bodyがある場合は従来どおりbodyのonloadをWindow load handlerとして登録する", () => {
  /** @type {LoadElement} */
  const body = {
    getAttribute: (/** @type {string} */ name) => (name === "onload" ? "this.loaded = true" : null),
  };
  /** @type {LoadElement} */
  const frameset = { getAttribute: () => "this.loaded = false" };
  const { eventHandlers, window } = installStaticLoadHandler(body, frameset);

  expect(eventHandlers).toHaveLength(1);
  const loadHandler = eventHandlers[0];
  if (!loadHandler) throw new Error("load handler missing");
  if (!loadHandler[2]) throw new Error("load callback missing");
  loadHandler[2].call(window, new Event("load"));
  expect(body.loaded).toBe(true);
  expect(frameset.loaded).toBeUndefined();
});

test("bodyがある文書内のforeign framesetはWindow load handlerにならない", () => {
  expect(bodyLoadHandler).toBeDefined();
  expect(staticBodyLoadHandler).toBeDefined();

  const foreignFrameset = { getAttribute: (/** @type {string} */ _name) => "this.loaded = true" };
  const pageGlobal = {};
  /** @type {{loaded?: boolean}} */
  const window = {};
  const document = {
    body: { getAttribute: () => null },
    querySelector(/** @type {string} */ selector) {
      throw new Error(`unexpected selector: ${selector}`);
    },
  };
  /** @type {LoadHandler[]} */
  const eventHandlers = [];
  new Function(
    "globalThis",
    "window",
    "document",
    "__krrLifecyclePropertyOverrides",
    "__krrStoreEventHandler",
    `${bodyLoadHandler}\n${staticBodyLoadHandler}\nreturn globalThis.__krrInstallStaticBodyLoadHandler;`,
  )(
    pageGlobal,
    window,
    document,
    new Map(),
    (
      /** @type {[object, string, ((this: object, event: Event) => void) | null]} */ ...arguments_
    ) => eventHandlers.push(arguments_),
  )();

  expect(foreignFrameset.getAttribute("onload")).toBe("this.loaded = true");
  expect(eventHandlers).toHaveLength(1);
  const loadHandler = eventHandlers[0];
  if (!loadHandler) throw new Error("load handler missing");
  expect(loadHandler[0]).toBe(window);
  expect(loadHandler[2]).toBeNull();
});

test("後からcurrent bodyまたはframesetになった要素のload handlerをWindowへ移す", () => {
  expect(windowLoadNodeCheck).toBeDefined();
  expect(routeWindowLoadElement).toBeDefined();

  for (const nodeName of ["body", "frameset"]) {
    /** @type {Record<string, string | null>} */
    const current = { body: null, frameset: null };
    const pageGlobal = {
      __krr_dom: (/** @type {string} */ _operation, /** @type {string} */ selector) =>
        current[selector === "html > frameset" ? "frameset" : selector],
    };
    const isWindowLoadNode = new Function(
      "globalThis",
      `${nativeDomCapture}\n${windowLoadNodeCheck}\nreturn __krrIsWindowLoadNode;`,
    )(pageGlobal);
    /** @type {WeakMap<object, Map<string, (this: object) => void>>} */
    const handlers = new WeakMap();
    const store = (
      /** @type {object} */ target,
      /** @type {string} */ type,
      /** @type {((this: object) => void) | null} */ handler,
    ) => {
      const targetHandlers = handlers.get(target) ?? new Map();
      if (handler === null) targetHandlers.delete(type);
      else targetHandlers.set(type, handler);
      handlers.set(target, targetHandlers);
    };
    /** @type {{loaded?: boolean}} */
    const window = {};
    const route = new Function(
      "window",
      "__krrIsWindowLoadNode",
      "__krrEventHandlers",
      "__krrStoreEventHandler",
      `${routeWindowLoadElement}\nreturn __krrRouteWindowLoadElement;`,
    )(window, isWindowLoadNode, handlers, store);
    const element = { __krrNodeId: nodeName };
    /** @type {object | undefined} */
    let calledWith;
    store(
      element,
      "load",
      /** @this {object} */ function handler() {
        calledWith = this;
      },
    );

    expect(route(element)).toBe(false);
    current[nodeName] = nodeName;
    expect(route(element)).toBe(true);
    const windowHandlers = handlers.get(window);
    const routedHandler = windowHandlers?.get("load");
    if (!routedHandler) throw new Error("load handler missing");
    routedHandler.call(window);
    expect(calledWith).toBe(element);
    const elementHandlers = handlers.get(element);
    if (!elementHandlers) throw new Error("element handlers missing");
    expect(elementHandlers.has("load")).toBe(false);
  }
});

test("bodyがない文書でもforeign framesetはWindow loadへroutingしない", () => {
  expect(nativeDomCapture).toBeDefined();
  expect(windowLoadNodeCheck).toBeDefined();

  /** @type {string[]} */
  const selectors = [];
  const nativeBridge = (/** @type {string} */ _operation, /** @type {string} */ selector) => {
    selectors.push(selector);
    if (selector === "frameset") return "foreign-frameset";
    return null;
  };
  const isWindowLoadNode = new Function(
    "globalThis",
    `${nativeDomCapture}\n${windowLoadNodeCheck}\nreturn __krrIsWindowLoadNode;`,
  )({ __krr_dom: nativeBridge });

  expect(isWindowLoadNode("foreign-frameset")).toBe(false);
  expect(selectors).toEqual(["body", "html > frameset"]);
});

/** @param {object} target @param {string} type @param {string[]} calls */
const createListenerFailureDispatcher = (target, type, calls) => {
  const entries = [
    {
      callback() {
        calls.push("throwing listener");
        throw new Error("listener failure");
      },
      capture: false,
      once: false,
      passive: false,
    },
    {
      callback() {
        calls.push("following listener");
      },
      capture: false,
      once: false,
      passive: false,
    },
  ];
  return new Function(
    "target",
    "__krrEventTargetListeners",
    "__krrEventHandlers",
    "__krrLifecyclePropertyOverrides",
    "__krrLifecycleEventTypes",
    "__krrElementInstances",
    "__krrSyncEventTarget",
    `${dispatchListenerEntry}\n${dispatchListeners}\n${dispatchHandler}\n${dispatchTargetPhase}\nreturn __krrDispatchTargetPhase;`,
  )(
    target,
    new WeakMap([[target, new Map([[type, entries]])]]),
    new WeakMap(),
    new WeakMap(),
    new Set(["load", "error", "readystatechange", "DOMContentLoaded"]),
    new WeakSet(),
    () => {},
  );
};

test("readystatechange listener例外の後も後続listenerとproperty handlerを呼ぶ", () => {
  expect(dispatchListeners).toBeDefined();
  expect(dispatchListenerEntry).toBeDefined();
  expect(dispatchHandler).toBeDefined();
  expect(dispatchTargetPhase).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const target = {
    onreadystatechange() {
      calls.push("property");
    },
  };
  const dispatch = createListenerFailureDispatcher(target, "readystatechange", calls);

  expect(() => dispatch(target, { type: "readystatechange" }, false, 2)).toThrow(
    "listener failure",
  );

  expect(calls).toEqual(["throwing listener", "following listener", "property"]);
});

test("resource listener例外の後も後続listenerとproperty handlerを呼ぶ", () => {
  /** @type {string[]} */
  const calls = [];
  /** @type {{onload?: () => void, oncustom?: () => void}} */
  const target = {
    onload() {
      calls.push("property");
    },
  };
  const dispatch = createListenerFailureDispatcher(target, "load", calls);

  expect(() => dispatch(target, { type: "load" }, false, 2)).toThrow("listener failure");
  expect(calls).toEqual(["throwing listener", "following listener", "property"]);
});

test("通常イベントのlistener例外はdispatchを中断して呼び出し元へ伝播する", () => {
  expect(dispatchListenerEntry).toBeDefined();
  /** @type {string[]} */
  const calls = [];
  const target = {};
  const dispatch = createListenerFailureDispatcher(target, "custom", calls);

  expect(() => dispatch(target, { type: "custom" }, false, 2)).toThrow("listener failure");
  expect(calls).toEqual(["throwing listener"]);
});

/** @param {object} target
 * @param {WeakMap<object, Map<string, () => unknown>>} handlers
 * @param {WeakMap<object, Set<string>>} overrides */
const createStoredHandlerDispatcher = (target, handlers, overrides) => {
  return new Function(
    "target",
    "__krrEventHandlers",
    "__krrLifecyclePropertyOverrides",
    "__krrLifecycleEventTypes",
    "__krrElementInstances",
    `${dispatchHandler}\nreturn __krrDispatchHandler;`,
  )(
    target,
    handlers,
    overrides,
    new Set(["load", "error", "readystatechange", "DOMContentLoaded"]),
    new WeakSet([target]),
  );
};

test("visible propertyをdeleteしてもlifecycle Mapのhandlerをdispatchする", () => {
  expect(dispatchHandler).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  /** @type {{onload?: () => void, oncustom?: () => void}} */
  const target = {
    onload() {
      calls.push("visible shadow");
    },
  };
  const handler = () => calls.push("stored handler");
  const handlers = new WeakMap([[target, new Map([["load", handler]])]]);
  const overrides = new WeakMap([[target, new Set(["load"])]]);
  delete target.onload;

  const dispatch = createStoredHandlerDispatcher(target, handlers, overrides);

  dispatch(target, { type: "load", cancelable: false });
  expect(calls).toEqual(["stored handler"]);
});

test("null-cleared lifecycle handler does not fall through while custom expando still dispatches", () => {
  expect(dispatchHandler).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  /** @type {{onload?: () => void, oncustom?: () => void}} */
  const target = {
    onload() {
      calls.push("shadow");
    },
    oncustom() {
      calls.push("custom expando");
    },
  };
  const handlers = new WeakMap([[target, new Map()]]);
  const overrides = new WeakMap([[target, new Set(["load"])]]);
  delete target.onload;

  const dispatch = createStoredHandlerDispatcher(target, handlers, overrides);

  dispatch(target, { type: "load", cancelable: false });
  dispatch(target, { type: "custom", cancelable: false });
  expect(calls).toEqual(["custom expando"]);
});

test("generic dispatchEventでもcaptureのstopImmediatePropagation後にdeferred lifecycle例外を再送出する", () => {
  expect(listenerOptions).toBeDefined();
  expect(installEventTarget).toBeDefined();
  expect(throwDeferredListenerError).toBeDefined();

  class LifecycleEvent {
    constructor(/** @type {string} */ type) {
      this.type = type;
      this.cancelable = false;
      this.defaultPrevented = false;
      this.__krrDispatching = false;
      this.__krrPropagationStopped = false;
      this.__krrImmediatePropagationStopped = false;
    }
    stopImmediatePropagation() {
      this.__krrPropagationStopped = true;
      this.__krrImmediatePropagationStopped = true;
    }
  }
  LifecycleEvent.AT_TARGET = 2;
  const listeners = new WeakMap();
  const target = new Function(
    "Event",
    "__krrEventTargetListeners",
    "__krrEventHandlers",
    "__krrLifecyclePropertyOverrides",
    "__krrLifecycleEventTypes",
    "__krrElementInstances",
    "__krrSyncEventTarget",
    `${listenerOptions}\n${dispatchListenerEntry}\n${dispatchListeners}\n${dispatchHandler}\n${dispatchTargetPhase}\n${beginDispatch}\n${endDispatch}\n${throwDeferredListenerError}\n${installEventTarget}\nreturn __krrInstallEventTarget({});`,
  )(
    LifecycleEvent,
    listeners,
    new WeakMap(),
    new WeakMap(),
    new Set(["load", "error", "readystatechange", "DOMContentLoaded"]),
    new WeakSet(),
    () => {},
  );
  target.addEventListener(
    "load",
    (/** @type {{stopImmediatePropagation(): void}} */ event) => {
      event.stopImmediatePropagation();
      throw new Error("capture failure");
    },
    { capture: true },
  );

  const event = new LifecycleEvent("load");
  expect(() => target.dispatchEvent(event)).toThrow("capture failure");
  expect(event.__krrDispatching).toBe(false);
  if (!("currentTarget" in event) || !("eventPhase" in event))
    throw new Error("dispatched event state missing");
  expect(event.currentTarget).toBeNull();
  expect(event.eventPhase).toBe(0);
});

/** @param {ImageFixture} image
 * @param {(image: object, source: string) => string} validate */
const createImageLoadDispatcher = (image, validate) => {
  return new Function(
    "image",
    "Event",
    "__krrImageLoadEventType",
    "__krrLifecycleCheckpoint",
    `${dispatchImageLoad}\nreturn __krrDispatchImageLoad;`,
  )(
    image,
    class Event {
      constructor(/** @type {string} */ type) {
        this.type = type;
      }
    },
    validate,
    Promise.resolve.bind(Promise),
  );
};

test("onerrorのsrc差替えはload/errorを再評価し最大8回で終了する", async () => {
  expect(dispatchImageLoad).toBeDefined();

  /** @type {string[]} */
  const eventTypes = [];
  const image = {
    source: "broken",
    getAttribute(/** @type {string} */ name) {
      expect(name).toBe("src");
      return this.source;
    },
    dispatchEvent(/** @type {{type: string}} */ event) {
      eventTypes.push(event.type);
      if (event.type === "error" && this.source === "broken") this.source = "valid";
    },
  };
  const dispatch = createImageLoadDispatcher(image, (_image, source) =>
    source === "valid" ? "load" : "error",
  );

  await dispatch(image);
  expect(eventTypes).toEqual(["error", "load"]);

  image.source = "a";
  image.dispatchEvent = (event) => {
    eventTypes.push(event.type);
    image.source = image.source === "a" ? "b" : "a";
  };
  eventTypes.length = 0;
  await dispatch(image);
  expect(eventTypes).toHaveLength(8);
  expect(eventTypes.every((eventType) => eventType === "error")).toBe(true);
});

test("onloadのsrc差替え後も新しいsrcを検証してload/errorを再dispatchする", async () => {
  expect(dispatchImageLoad).toBeDefined();

  /** @type {string[]} */
  const eventTypes = [];
  const image = {
    source: "valid",
    getAttribute(/** @type {string} */ name) {
      expect(name).toBe("src");
      return this.source;
    },
    dispatchEvent(/** @type {{type: string}} */ event) {
      eventTypes.push(event.type);
      if (event.type === "load" && this.source === "valid") this.source = "broken";
    },
  };
  const dispatch = createImageLoadDispatcher(image, (_image, source) =>
    source === "valid" ? "load" : "error",
  );

  await dispatch(image);
  expect(eventTypes).toEqual(["load", "error"]);
});

test("同一画像の各source event前にmicrotaskを処理し中間srcをキャンセルする", async () => {
  expect(dispatchImageLoad).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const image = {
    source: "A",
    getAttribute(/** @type {string} */ name) {
      expect(name).toBe("src");
      return this.source;
    },
    dispatchEvent(/** @type {{type: string}} */ event) {
      calls.push(`${this.source}:${event.type}`);
      if (this.source === "A") {
        this.source = "B";
        queueMicrotask(() => {
          this.source = "C";
          calls.push("microtask");
        });
      }
    },
  };
  const dispatch = createImageLoadDispatcher(image, (_image, source) =>
    source === "A" || source === "C" ? "load" : "error",
  );

  await dispatch(image);

  expect(calls).toEqual(["A:load", "microtask", "C:load"]);
  expect(image.source).toBe("C");
});

test("8回目の同期src差替えは未dispatchとして次のscanで評価する", async () => {
  expect(dispatchImageLoad).toBeDefined();

  /** @type {string[]} */
  const dispatchedSources = [];
  const image = {
    source: "source0",
    getAttribute(/** @type {string} */ name) {
      expect(name).toBe("src");
      return this.source;
    },
    dispatchEvent() {
      dispatchedSources.push(this.source);
      const index = Number(this.source.slice("source".length));
      if (index < 8) this.source = `source${index + 1}`;
    },
  };
  const dispatchImage = new Function(
    "Event",
    "__krrImageLoadEventType",
    "__krrLifecycleCheckpoint",
    `${dispatchImageLoad}\nreturn __krrDispatchImageLoad;`,
  )(class Event {}, () => "load", Promise.resolve.bind(Promise));
  const document = { querySelectorAll: () => [image] };
  const dispatchPending = createPendingImageDispatcher(document, dispatchImage);

  await dispatchPending(document);

  expect(dispatchedSources).toEqual(Array.from({ length: 9 }, (_, index) => `source${index}`));
  expect(image.source).toBe("source8");
});

test("Window load schedulerはページ側Promise変更後もcompleteまで進む", async () => {
  /** @type {string[]} */
  const noResourceCalls = [];
  const pagePromise = {
    resolve: () => {
      throw new Error("page Promise.resolve patched");
    },
  };
  await createWindowLoadDispatcher([], noResourceCalls, [], pagePromise)();
  expect(noResourceCalls).toEqual(["window"]);

  /** @type {string[]} */
  const calls = [];
  const replacementPromise = {
    resolve: () => {
      throw new Error("replacement Promise.resolve patched");
    },
  };
  const images = [
    {
      getAttribute: () => "valid",
      dispatchEvent: (/** @type {{type: string}} */ event) => calls.push(`image:${event.type}`),
    },
  ];
  const frames = [
    {
      contentDocument: {},
      dispatchEvent: (/** @type {{type: string}} */ event) => calls.push(`frame:${event.type}`),
    },
  ];
  await createWindowLoadDispatcher(images, calls, frames, replacementPromise)();
  expect(calls).toEqual(["frame:load", "image:load", "window"]);
});

test("別画像のload handlerによるsrc差替え後に既読画像を再評価する", async () => {
  expect(dispatchPendingImages).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  /** @type {ImageFixture[]} */
  const images = [];
  const firstImage = {
    source: "valid",
    getAttribute(/** @type {string} */ name) {
      expect(name).toBe("src");
      return this.source;
    },
    dispatchEvent(/** @type {{type: string}} */ event) {
      calls.push(`first:${event.type}`);
    },
  };
  const secondImage = {
    source: "valid",
    getAttribute(/** @type {string} */ name) {
      expect(name).toBe("src");
      return this.source;
    },
    dispatchEvent(/** @type {{type: string}} */ event) {
      calls.push(`second:${event.type}`);
      if (event.type === "load") firstImage.source = "broken";
    },
  };
  images.push(firstImage, secondImage);
  const document = {
    querySelectorAll(/** @type {string} */ selector) {
      expect(selector).toBe("img");
      return images;
    },
  };
  const dispatch = createPendingImageDispatcher(document, (/** @type {ImageFixture} */ image) => {
    const source = image.getAttribute("src");
    image.dispatchEvent(new Event(source === "valid" ? "load" : "error"));
    return source;
  });

  await dispatch(document);
  expect(calls).toEqual(["first:load", "second:load", "first:error"]);
});

test("画像load handlerのmicrotaskによるsrc差替えを保留して再評価する", async () => {
  expect(dispatchPendingImages).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const image = {
    source: "valid",
    getAttribute(/** @type {string} */ name) {
      expect(name).toBe("src");
      return this.source;
    },
    dispatchEvent(/** @type {{type: string}} */ event) {
      calls.push(event.type);
      if (event.type === "load") queueMicrotask(() => (this.source = "broken"));
    },
  };
  const document = {
    querySelectorAll(/** @type {string} */ selector) {
      expect(selector).toBe("img");
      return [image];
    },
  };
  const dispatch = createPendingImageDispatcher(document, (/** @type {ImageFixture} */ target) => {
    const source = target.getAttribute("src");
    target.dispatchEvent(new Event(source === "valid" ? "load" : "error"));
    return source;
  });

  await dispatch(document);
  expect(calls).toEqual(["load", "error"]);
});

/** @param {object} document @param {string[]} calls
 * @param {(document: object) => Promise<unknown>} dispatchPending
 * @param {(document: object) => Promise<unknown>} dispatchFrames
 * @param {{resolve: () => unknown}} pagePromise
 * @param {() => Promise<unknown>} checkpoint
 * @param {(image: ImageFixture) => unknown} dispatchImage */
const createDocumentWindowLoadDispatcher = (
  document,
  calls,
  dispatchPending,
  dispatchFrames,
  pagePromise,
  checkpoint,
  dispatchImage,
) => {
  const pageGlobal = {};
  return new Function(
    "globalThis",
    "document",
    "window",
    "Event",
    "Promise",
    "__krrLifecycleCheckpoint",
    "__krrRouteWindowLoadElement",
    "__krrElement",
    "__krrNativeDom",
    "__krrImageLoadEventType",
    "__krrDispatchImageLoad",
    "__krrDispatchPendingImages",
    "__krrDispatchPendingLocalFrames",
    "__krrDocumentReadyState",
    "__krrDispatchDocumentReadyStateChange",
    "__krrDispatchElementReadyStateChange",
    "__krrThrowReadyStateListenerError",
    `${dispatchWindowLoad}\nreturn globalThis.__krrDispatchWindowLoad;`,
  )(
    pageGlobal,
    document,
    { dispatchEvent: () => calls.push("window") },
    class Event {
      constructor(/** @type {string} */ type) {
        this.type = type;
      }
    },
    pagePromise,
    checkpoint,
    () => false,
    () => null,
    () => null,
    () => "load",
    dispatchImage,
    dispatchPending,
    dispatchFrames,
    "interactive",
    () => {},
    () => {},
    () => {},
  );
};

test("resource eventがqueueMicrotaskを登録した後にWindow loadをdispatchする", async () => {
  expect(dispatchWindowLoad).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const image = {
    getAttribute: () => "valid",
    dispatchEvent() {
      calls.push("resource");
      queueMicrotask(() => calls.push("microtask"));
    },
  };
  const body = null;
  const document = {
    body,
    querySelectorAll(/** @type {string} */ selector) {
      return selector === "img" ? [image] : [];
    },
  };
  const dispatchPending = createPendingImageDispatcher(
    document,
    (/** @type {ImageFixture} */ target) => {
      const source = target.getAttribute("src");
      target.dispatchEvent(new Event("load"));
      return source;
    },
  );
  const dispatchFrames = createPendingLocalFrameDispatcher(document);
  const dispatch = createDocumentWindowLoadDispatcher(
    document,
    calls,
    dispatchPending,
    dispatchFrames,
    Promise,
    Promise.resolve.bind(Promise),
    (target) => target.dispatchEvent(new Event("load")),
  );

  await dispatch();
  expect(calls).toEqual(["resource", "microtask", "window"]);
});

/** @param {string[]} calls @param {string} eventType
 * @param {(images: ImageFixture[], added: ImageFixture) => unknown} append */
const createAppendingImageDispatcher = (calls, eventType, append) => {
  /** @type {ImageFixture[]} */
  const images = [];
  const addedImage = {
    getAttribute: () => "valid",
    dispatchEvent: (/** @type {{type: string}} */ event) => calls.push(`added:${event.type}`),
  };
  images.push({
    getAttribute: () => (eventType === "error" ? "broken" : "valid"),
    dispatchEvent(/** @type {{type: string}} */ event) {
      calls.push(`first:${event.type}`);
      if (event.type === eventType) append(images, addedImage);
    },
  });
  return createWindowLoadDispatcher(images, calls);
};

test("image load handlerが同期的に追加したimgもWindow load前にdispatchする", async () => {
  expect(dispatchWindowLoad).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const dispatch = createAppendingImageDispatcher(calls, "load", (images, addedImage) =>
    images.push(addedImage),
  );

  await dispatch();
  expect(calls).toEqual(["first:load", "added:load", "window"]);
});

test("image error handlerが同期的に追加したimgもWindow load前にdispatchする", async () => {
  expect(dispatchWindowLoad).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const dispatch = createAppendingImageDispatcher(calls, "error", (images, addedImage) =>
    images.push(addedImage),
  );

  await dispatch();
  expect(calls).toEqual(["first:error", "added:load", "window"]);
});

test("image load handlerのPromise microtaskで追加したimgもWindow load前にdispatchする", async () => {
  expect(dispatchWindowLoad).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const dispatch = createAppendingImageDispatcher(calls, "load", (images, addedImage) =>
    Promise.resolve().then(() => images.push(addedImage)),
  );

  await dispatch();
  expect(calls).toEqual(["first:load", "added:load", "window"]);
});

test("画像dispatch間でmicrotaskを実行してから次の画像をdispatchする", async () => {
  expect(dispatchWindowLoad).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const images = [
    {
      getAttribute: () => "valid",
      dispatchEvent(/** @type {{type: string}} */ event) {
        calls.push(`first:${event.type}`);
        queueMicrotask(() => calls.push("microtask"));
      },
    },
    {
      getAttribute: () => "valid",
      dispatchEvent: (/** @type {{type: string}} */ event) => calls.push(`second:${event.type}`),
    },
  ];
  const dispatch = createWindowLoadDispatcher(images, calls);

  await dispatch();
  expect(calls).toEqual(["first:load", "microtask", "second:load", "window"]);
});

test("image handlerが無限にimgを追加してもdispatch数を1024件に制限する", async () => {
  expect(dispatchWindowLoad).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  /** @type {ImageFixture[]} */
  const images = [];
  const createImage = () => ({
    getAttribute: () => "valid",
    dispatchEvent(/** @type {{type: string}} */ event) {
      if (event.type === "load") {
        calls.push("image");
        images.push(createImage());
      }
    },
  });
  images.push(createImage());
  const dispatch = createWindowLoadDispatcher(images, calls);

  await dispatch();
  expect(calls.filter((call) => call === "image")).toHaveLength(1024);
  expect(calls.at(-1)).toBe("window");
});

test("image handlerが追加したlocal iframeをbounded fixed-pointでWindow load前にdispatchする", async () => {
  expect(dispatchPendingLocalFrames).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  /** @type {ImageFixture[]} */
  const images = [];
  /** @type {FrameFixture[]} */
  const frames = [];
  images.push({
    getAttribute: () => "valid",
    dispatchEvent(/** @type {{type: string}} */ event) {
      calls.push(`image:${event.type}`);
      if (event.type === "load") {
        frames.push({
          contentDocument: {},
          dispatchEvent(/** @type {{type: string}} */ frameEvent) {
            calls.push(`frame:${frameEvent.type}`);
          },
        });
      }
    },
  });
  const dispatch = createWindowLoadDispatcher(images, calls, frames);

  await dispatch();
  expect(calls).toEqual(["image:load", "frame:load", "window"]);
});

test("local iframe dispatch間でmicrotaskを実行してから次のiframeをdispatchする", async () => {
  expect(dispatchWindowLoad).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const frames = [
    {
      contentDocument: {},
      dispatchEvent(/** @type {{type: string}} */ event) {
        calls.push(`first:${event.type}`);
        queueMicrotask(() => calls.push("microtask"));
      },
    },
    {
      contentDocument: {},
      dispatchEvent: (/** @type {{type: string}} */ event) => calls.push(`second:${event.type}`),
    },
  ];
  const dispatch = createWindowLoadDispatcher([], calls, frames);

  await dispatch();
  expect(calls).toEqual(["first:load", "microtask", "second:load", "window"]);
});

const createWindowLoadDispatcher = (
  /** @type {ImageFixture[]} */ images,
  /** @type {string[]} */ calls,
  /** @type {FrameFixture[]} */ frames = [],
  /** @type {{resolve: () => unknown}} */ pagePromise = Promise,
  checkpoint = Promise.resolve.bind(Promise),
) => {
  const document = {
    body: null,
    querySelectorAll(/** @type {string} */ selector) {
      if (selector === "img") return images;
      if (selector === "iframe") return frames;
      return [];
    },
  };
  const dispatchPending = createPendingImageDispatcher(
    document,
    (/** @type {ImageFixture} */ image) => {
      const source = image.getAttribute("src");
      const eventType = source === "broken" ? "error" : "load";
      image.dispatchEvent(new Event(eventType));
      return source;
    },
    pagePromise,
    checkpoint,
  );
  const dispatchFrames = createPendingLocalFrameDispatcher(document, pagePromise, checkpoint);
  const dispatch = createDocumentWindowLoadDispatcher(
    document,
    calls,
    dispatchPending,
    dispatchFrames,
    pagePromise,
    checkpoint,
    (image) => {
      const source = image.getAttribute("src");
      image.dispatchEvent(new Event("load"));
      return source;
    },
  );
  return dispatch;
};

const createPendingImageDispatcher = (
  /** @type {object} */ document,
  /** @type {(image: ImageFixture) => string | null} */ dispatchImage,
  /** @type {{resolve: () => unknown}} */ pagePromise = Promise,
  checkpoint = Promise.resolve.bind(Promise),
) =>
  new Function(
    "document",
    "Promise",
    "__krrLifecycleCheckpoint",
    "__krrDispatchImageLoad",
    `${dispatchPendingImages}\nreturn __krrDispatchPendingImages;`,
  )(document, pagePromise, checkpoint, dispatchImage);

const createPendingLocalFrameDispatcher = (
  /** @type {object} */ document,
  /** @type {{resolve: () => unknown}} */ pagePromise = Promise,
  checkpoint = Promise.resolve.bind(Promise),
) =>
  new Function(
    "document",
    "Promise",
    "__krrLifecycleCheckpoint",
    `${dispatchLocalFrame}\n${dispatchPendingLocalFrames}\nreturn __krrDispatchPendingLocalFrames;`,
  )(document, pagePromise, checkpoint);

test("空iframe再走査はページ側Promise置換に依存しない", async () => {
  expect(lifecycleCheckpoint).toBeDefined();

  /** @type {string[]} */
  const calls = [];
  const pagePromise = {
    resolve() {
      calls.push("page-promise");
      return Promise.resolve();
    },
  };
  const document = { querySelectorAll: () => [] };
  const dispatch = createPendingLocalFrameDispatcher(document, pagePromise, () => {
    calls.push("lifecycle-checkpoint");
    return Promise.resolve();
  });

  await dispatch(document);
  expect(calls).toEqual(["lifecycle-checkpoint", "lifecycle-checkpoint"]);
});

test("author指定の画像loadマーカーを除去しnative検証結果を使う", () => {
  expect(seedPrevalidatedImageEvents).toBeDefined();
  expect(imageLoadEventType).toBeDefined();

  const sourceUrl = "data:image/png;base64,AA==";
  const attributes = new Map([
    ["src", sourceUrl],
    ["data-krr-prevalidated-image-event", "load"],
  ]);
  const image = {
    getAttribute(/** @type {string} */ name) {
      return attributes.get(name) ?? null;
    },
    removeAttribute(/** @type {string} */ name) {
      attributes.delete(name);
    },
  };
  const seed = new Function(
    "document",
    "__krrPrevalidatedImageEventAttribute",
    `${seedPrevalidatedImageEvents}\nreturn __krrSeedPrevalidatedImageEvents;`,
  )({ querySelectorAll: () => [image] }, "data-krr-prevalidated-image-event");
  seed();

  expect(attributes.has("data-krr-prevalidated-image-event")).toBe(false);
  /** @type {unknown[][]} */
  const calls = [];
  const imageEventType = new Function(
    "globalThis",
    `${nativeDomCapture}\n${imageLoadEventType}\nreturn __krrImageLoadEventType;`,
  )({
    __krr_dom(/** @type {unknown[]} */ ...arguments_) {
      calls.push(arguments_);
      return "error";
    },
  });

  expect(imageEventType(image, sourceUrl)).toBe("error");
  expect(calls).toEqual([["validateImageDataUrl", sourceUrl]]);
});

test("author指定の画像errorマーカーもnativeのload結果を上書きしない", () => {
  expect(imageLoadEventType).toBeDefined();

  const sourceUrl = "data:image/png;base64,AA==";
  /** @type {unknown[][]} */
  const calls = [];
  const imageEventType = new Function(
    "globalThis",
    `${nativeDomCapture}\n${imageLoadEventType}\nreturn __krrImageLoadEventType;`,
  )({
    __krr_dom(/** @type {unknown[]} */ ...arguments_) {
      calls.push(arguments_);
      return "load";
    },
  });

  expect(imageEventType({ getAttribute: () => sourceUrl }, sourceUrl)).toBe("load");
  expect(calls).toEqual([["validateImageDataUrl", sourceUrl]]);
});

test("初期DOM列挙を提供しないminimal bridgeではマーカー除去をskipする", () => {
  expect(seedPrevalidatedImageEvents).toBeDefined();

  const seed = new Function(
    "document",
    "__krrPrevalidatedImageEventAttribute",
    `${seedPrevalidatedImageEvents}\nreturn __krrSeedPrevalidatedImageEvents;`,
  )(
    {
      querySelectorAll() {
        throw new Error("DOM state is unavailable");
      },
    },
    "data-krr-prevalidated-image-event",
  );

  expect(() => seed()).not.toThrow();
});

test("DOMContentLoaded listenerの例外後も後続listenerとproperty handlerを実行してから再送出する", () => {
  expect(dispatchListenerEntry).toBeDefined();
  expect(dispatchListeners).toBeDefined();
  expect(dispatchHandler).toBeDefined();
  expect(dispatchTargetPhase).toBeDefined();

  const dispatchTargetPhaseForTest = new Function(
    "__krrEventTargetListeners",
    "__krrEventHandlers",
    "__krrLifecyclePropertyOverrides",
    "__krrLifecycleEventTypes",
    "__krrElementInstances",
    "__krrSyncEventTarget",
    `${dispatchListenerEntry}\n${dispatchListeners}\n${dispatchHandler}\n${dispatchTargetPhase}\nreturn __krrDispatchTargetPhase;`,
  );
  /** @type {string[]} */
  const order = [];
  const target = {
    onDOMContentLoaded() {
      order.push("property");
    },
  };
  const listeners = new Map([
    [
      "DOMContentLoaded",
      [
        {
          callback() {
            order.push("first");
            throw new Error("listener failure");
          },
          capture: false,
          once: false,
          passive: false,
        },
        {
          callback() {
            order.push("second");
          },
          capture: false,
          once: false,
          passive: false,
        },
      ],
    ],
  ]);
  const eventTargetListeners = new WeakMap([[target, listeners]]);
  const dispatch = dispatchTargetPhaseForTest(
    eventTargetListeners,
    new WeakMap(),
    new WeakMap(),
    new Set(["load", "error", "readystatechange", "DOMContentLoaded"]),
    new WeakSet(),
    () => {},
  );
  const event = { type: "DOMContentLoaded", __krrImmediatePropagationStopped: false };

  expect(() => dispatch(target, event, false, 2)).toThrow("listener failure");
  expect(order).toEqual(["first", "second", "property"]);
});

test.each(["DOMContentLoaded", "load"])(
  "%sのcapture listenerがstopImmediatePropagationしてもdeferred exceptionを再送出する",
  (type) => {
    expect(dispatchElementPhase).toBeDefined();
    expect(dispatchElementEvent).toBeDefined();
    expect(beginDispatch).toBeDefined();
    expect(endDispatch).toBeDefined();

    const pageGlobal = { __krrNodeId: "window" };
    const pageDocument = { __krrNodeId: "document" };
    const target = type === "load" ? pageGlobal : pageDocument;
    const listeners = new WeakMap([
      [
        pageGlobal,
        new Map([
          [
            type,
            [
              {
                callback(/** @type {{stopImmediatePropagation(): void}} */ event) {
                  event.stopImmediatePropagation();
                  throw new Error(`${type} capture failure`);
                },
                capture: true,
                once: false,
                passive: false,
              },
            ],
          ],
        ]),
      ],
    ]);
    class LifecycleEvent {
      constructor(/** @type {string} */ eventType) {
        this.type = eventType;
        this.bubbles = false;
        this.cancelable = false;
        this.__krrImmediatePropagationStopped = false;
        this.__krrPropagationStopped = false;
      }
      stopImmediatePropagation() {
        this.__krrImmediatePropagationStopped = true;
        this.__krrPropagationStopped = true;
      }
    }
    Object.assign(LifecycleEvent, { CAPTURING_PHASE: 1, AT_TARGET: 2, BUBBLING_PHASE: 3 });
    const dispatch = new Function(
      "window",
      "document",
      "Event",
      "__krrNativeDom",
      "__krrElement",
      "__krrEventTargetListeners",
      "__krrEventHandlers",
      "__krrLifecyclePropertyOverrides",
      "__krrLifecycleEventTypes",
      "__krrElementInstances",
      "__krrSyncEventTarget",
      `${dispatchListenerEntry}\n${dispatchListeners}\n${dispatchHandler}\n${dispatchTargetPhase}\n${dispatchElementPhase}\n${beginDispatch}\n${endDispatch}\n${throwDeferredListenerError}\n${dispatchCapture}\n${dispatchTarget}\n${dispatchBubble}\n${dispatchElementEvent}\nreturn __krrDispatchElementEvent;`,
    )(
      pageGlobal,
      pageDocument,
      LifecycleEvent,
      (/** @type {string} */ operation, /** @type {string} */ nodeId) =>
        operation === "eventPath" ? [nodeId] : null,
      (/** @type {string} */ nodeId) =>
        nodeId === "window" ? pageGlobal : nodeId === "document" ? pageDocument : undefined,
      listeners,
      new WeakMap(),
      new WeakMap(),
      new Set(["load", "error", "readystatechange", "DOMContentLoaded"]),
      new WeakSet(),
      () => {},
    );

    expect(() => dispatch(target, new LifecycleEvent(type))).toThrow(`${type} capture failure`);
  },
);
