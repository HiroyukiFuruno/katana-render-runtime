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
const __krrIsBodyNode = () => false;
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
