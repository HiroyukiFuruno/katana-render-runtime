import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const source = readFileSync(new URL("./dom_bootstrap.js", import.meta.url), "utf8");
const nativeDomCapture = source.match(/^const __krrNativeDom = globalThis\.__krr_dom;$/m)?.[0];
const imageLoadEventType = source.match(
  /const __krrImageLoadEventType = \(source\) => \{[\s\S]*?\n\};/,
)?.[0];
const imageSrcProperties = source.match(
  /^ {2}get src\(\) \{[\s\S]*?^ {2}set src\(value\) \{[\s\S]*?^ {2}\},$/m,
)?.[0];
const getAttributeMethod = source.match(/^ {2}getAttribute\(name\) \{[\s\S]*?^ {2}\},$/m)?.[0];
const setAttributeMethod = source.match(
  /^ {2}setAttribute\(name, value\) \{[\s\S]*?^ {2}\},$/m,
)?.[0];
const bodyLoadHandler = source.match(
  /^const __krrInstallBodyLoadHandler = \(body, source\) => \{[\s\S]*?^\};/m,
)?.[0];
const staticBodyLoadHandler = source.match(
  /^globalThis\.__krrInstallStaticBodyLoadHandler = \(\) => \{[\s\S]*?^\};/m,
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
const dispatchTargetPhase = source.match(
  /^const __krrDispatchTargetPhase = \(target, event, capture, phase(?:, deferListenerError = false)?\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchImageLoad = source.match(
  /^const __krrDispatchImageLoad = \(image\) => \{[\s\S]*?^\};/m,
)?.[0];
const dispatchWindowLoad = source.match(
  /^globalThis\.__krrDispatchWindowLoad = async \(\) => \{[\s\S]*?^\};/m,
)?.[0];

const installStaticLoadHandler = (body, frameset) => {
  const pageGlobal = {};
  const window = {};
  const document = {
    body,
    querySelector(selector) {
      expect(selector).toBe("frameset");
      return frameset;
    },
  };
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
    (...arguments_) => eventHandlers.push(arguments_),
  );

  install();
  return { eventHandlers, window };
};

test("画像検証はページコードがglobalThis.__krr_domを置換してもnative bridgeを使う", () => {
  expect(nativeDomCapture).toBeDefined();
  expect(imageLoadEventType).toBeDefined();

  const calls = [];
  const nativeBridge = (...arguments_) => {
    calls.push(arguments_);
    return "load";
  };
  const pageGlobal = { __krr_dom: nativeBridge };
  const getImageLoadEventType = new Function(
    "globalThis",
    `${nativeDomCapture}\n${imageLoadEventType}\nreturn __krrImageLoadEventType;`,
  )(pageGlobal);

  pageGlobal.__krr_dom = () => "error";

  expect(getImageLoadEventType("data:image/png;base64,AA==")).toBe("load");
  expect(calls).toEqual([["validateImageDataUrl", "data:image/png;base64,AA=="]]);
});

test("image.src propertyとsetAttributeがnative src属性へ反映されload/error判定される", () => {
  expect(imageSrcProperties).toBeDefined();
  expect(setAttributeMethod).toBeDefined();
  expect(imageLoadEventType).toBeDefined();

  const attributes = new Map();
  const bridge = (operation, _nodeId, name, value) => {
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
    `const __krrNativeDom = globalThis.__krr_dom;
const __krrNormalizeLifecycleEventType = () => null;
const __krrIsWindowLoadNode = () => false;
const __krrInstallBodyLoadHandler = () => {};
const __krrInstallInlineHandler = () => {};
return ({${imageSrcProperties}\n${getAttributeMethod}\n${setAttributeMethod}});`,
  )({ __krr_dom: bridge });
  const image = Object.assign(Object.create(elementPrototype), { __krrNodeId: "image-1" });
  const getImageLoadEventType = new Function(
    "globalThis",
    `${nativeDomCapture}\n${imageLoadEventType}\nreturn __krrImageLoadEventType;`,
  )({ __krr_dom: bridge });

  image.src = "data:image/png;base64,AA==";
  expect(image.src).toBe("data:image/png;base64,AA==");
  expect(getImageLoadEventType(image.getAttribute("src"))).toBe("load");

  image.setAttribute("src", "data:image/png;base64,invalid");
  expect(image.src).toBe("data:image/png;base64,invalid");
  expect(getImageLoadEventType(image.getAttribute("src"))).toBe("error");
});

test("document.bodyがないframesetのonloadをWindow load handlerとして登録する", () => {
  expect(bodyLoadHandler).toBeDefined();
  expect(staticBodyLoadHandler).toBeDefined();

  const frameset = { getAttribute: (name) => (name === "onload" ? "this.loaded = true" : null) };
  const { eventHandlers, window } = installStaticLoadHandler(null, frameset);

  expect(eventHandlers).toHaveLength(1);
  expect(eventHandlers[0][0]).toBe(window);
  expect(eventHandlers[0][1]).toBe("load");
  eventHandlers[0][2].call(window, new Event("load"));
  expect(frameset.loaded).toBe(true);
  expect(window.loaded).toBeUndefined();
});

test("document.bodyがある場合は従来どおりbodyのonloadをWindow load handlerとして登録する", () => {
  const body = { getAttribute: (name) => (name === "onload" ? "this.loaded = true" : null) };
  const frameset = { getAttribute: () => "this.loaded = false" };
  const { eventHandlers, window } = installStaticLoadHandler(body, frameset);

  expect(eventHandlers).toHaveLength(1);
  eventHandlers[0][2].call(window, new Event("load"));
  expect(body.loaded).toBe(true);
  expect(frameset.loaded).toBeUndefined();
});

test("後からcurrent bodyまたはframesetになった要素のload handlerをWindowへ移す", () => {
  expect(windowLoadNodeCheck).toBeDefined();
  expect(routeWindowLoadElement).toBeDefined();

  for (const nodeName of ["body", "frameset"]) {
    const current = { body: null, frameset: null };
    const pageGlobal = { __krr_dom: (_operation, selector) => current[selector] };
    const isWindowLoadNode = new Function(
      "globalThis",
      `${nativeDomCapture}\n${windowLoadNodeCheck}\nreturn __krrIsWindowLoadNode;`,
    )(pageGlobal);
    const handlers = new WeakMap();
    const store = (target, type, handler) => {
      const targetHandlers = handlers.get(target) ?? new Map();
      if (handler === null) targetHandlers.delete(type);
      else targetHandlers.set(type, handler);
      handlers.set(target, targetHandlers);
    };
    const window = {};
    const route = new Function(
      "window",
      "__krrIsWindowLoadNode",
      "__krrEventHandlers",
      "__krrStoreEventHandler",
      `${routeWindowLoadElement}\nreturn __krrRouteWindowLoadElement;`,
    )(window, isWindowLoadNode, handlers, store);
    const element = { __krrNodeId: nodeName };
    let calledWith;
    store(element, "load", function handler() {
      calledWith = this;
    });

    expect(route(element)).toBe(false);
    current[nodeName] = nodeName;
    expect(route(element)).toBe(true);
    handlers.get(window).get("load")();
    expect(calledWith).toBe(element);
    expect(handlers.get(element).has("load")).toBe(false);
  }
});

test("readystatechange listener例外の後も後続listenerとproperty handlerを呼ぶ", () => {
  expect(dispatchListeners).toBeDefined();
  expect(dispatchListenerEntry).toBeDefined();
  expect(dispatchHandler).toBeDefined();
  expect(dispatchTargetPhase).toBeDefined();

  const calls = [];
  const target = {
    onreadystatechange() {
      calls.push("property");
    },
  };
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
  const dispatch = new Function(
    "target",
    "__krrEventTargetListeners",
    "__krrSyncEventTarget",
    `${dispatchListenerEntry}\n${dispatchListeners}\n${dispatchHandler}\n${dispatchTargetPhase}\nreturn __krrDispatchTargetPhase;`,
  )(target, new WeakMap([[target, new Map([["readystatechange", entries]])]]), () => {});

  expect(() => dispatch(target, { type: "readystatechange" }, false, 2)).toThrow(
    "listener failure",
  );

  expect(calls).toEqual(["throwing listener", "following listener", "property"]);
});

test("通常イベントのlistener例外はdispatchを中断して呼び出し元へ伝播する", () => {
  expect(dispatchListenerEntry).toBeDefined();
  const calls = [];
  const target = {};
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
  const dispatch = new Function(
    "target",
    "__krrEventTargetListeners",
    "__krrSyncEventTarget",
    `${dispatchListenerEntry}\n${dispatchListeners}\n${dispatchHandler}\n${dispatchTargetPhase}\nreturn __krrDispatchTargetPhase;`,
  )(target, new WeakMap([[target, new Map([["custom", entries]])]]), () => {});

  expect(() => dispatch(target, { type: "custom" }, false, 2)).toThrow("listener failure");
  expect(calls).toEqual(["throwing listener"]);
});

test("onerrorのsrc差替えはload/errorを再評価し最大8回で終了する", () => {
  expect(dispatchImageLoad).toBeDefined();

  const eventTypes = [];
  const image = {
    source: "broken",
    getAttribute(name) {
      expect(name).toBe("src");
      return this.source;
    },
    dispatchEvent(event) {
      eventTypes.push(event.type);
      if (event.type === "error" && this.source === "broken") this.source = "valid";
    },
  };
  const dispatch = new Function(
    "image",
    "Event",
    "__krrImageLoadEventType",
    `${dispatchImageLoad}\nreturn __krrDispatchImageLoad;`,
  )(
    image,
    class Event {
      constructor(type) {
        this.type = type;
      }
    },
    (source) => (source === "valid" ? "load" : "error"),
  );

  dispatch(image);
  expect(eventTypes).toEqual(["error", "load"]);

  image.source = "a";
  image.dispatchEvent = (event) => {
    eventTypes.push(event.type);
    image.source = image.source === "a" ? "b" : "a";
  };
  eventTypes.length = 0;
  dispatch(image);
  expect(eventTypes).toHaveLength(8);
  expect(eventTypes.every((eventType) => eventType === "error")).toBe(true);
});

test("resource eventがqueueMicrotaskを登録した後にWindow loadをdispatchする", async () => {
  expect(dispatchWindowLoad).toBeDefined();

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
    querySelectorAll(selector) {
      return selector === "img" ? [image] : [];
    },
  };
  const pageGlobal = {};
  const dispatch = new Function(
    "globalThis",
    "document",
    "window",
    "Event",
    "__krrRouteWindowLoadElement",
    "__krrElement",
    "__krrNativeDom",
    "__krrImageLoadEventType",
    "__krrDispatchImageLoad",
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
      constructor(type) {
        this.type = type;
      }
    },
    () => false,
    () => null,
    () => null,
    () => "load",
    (target) => target.dispatchEvent(new Event("load")),
    "interactive",
    () => {},
    () => {},
    () => {},
  );

  await dispatch();
  expect(calls).toEqual(["resource", "microtask", "window"]);
});
