const __krrNativeDom = globalThis.__krr_dom;
class __KrrEvent {
  constructor(...args) {
    if (args.length === 0) throw new TypeError("Event type must be provided");
    const [type, options = {}] = args;
    const normalizedOptions = options ?? {};
    this.type = String(type);
    this.bubbles = Boolean(normalizedOptions.bubbles);
    this.cancelable = Boolean(normalizedOptions.cancelable);
    this.composed = Boolean(normalizedOptions.composed);
    this.defaultPrevented = false;
    this.target = null;
    this.currentTarget = null;
    this.eventPhase = 0;
    this.isTrusted = false;
    this.timeStamp = Date.now();
    this.__krrDispatching = false;
    this.__krrPropagationStopped = false;
    this.__krrImmediatePropagationStopped = false;
    this.__krrPassiveListener = false;
  }
  preventDefault() {
    if (this.cancelable && !this.__krrPassiveListener) this.defaultPrevented = true;
  }
  stopPropagation() {
    this.__krrPropagationStopped = true;
  }
  stopImmediatePropagation() {
    this.__krrPropagationStopped = true;
    this.__krrImmediatePropagationStopped = true;
  }
}
globalThis.Event = __KrrEvent;
class __KrrURLSearchParams {
  constructor(init = "") {
    const source = String(init).replace(/^\?/, "");
    this.__krrEntries = source
      ? source.split("&").map((entry) => {
          const [name, ...rest] = entry.split("=");
          const decode = (value) => decodeURIComponent(String(value).replace(/\+/g, " "));
          return [decode(name), decode(rest.join("="))];
        })
      : [];
  }
  get(name) {
    name = String(name);
    return this.__krrEntries.find(([entry]) => entry === name)?.[1] ?? null;
  }
  getAll(name) {
    name = String(name);
    return this.__krrEntries.filter(([entry]) => entry === name).map(([, value]) => value);
  }
  has(name) {
    name = String(name);
    return this.__krrEntries.some(([entry]) => entry === name);
  }
}
globalThis.URLSearchParams = __KrrURLSearchParams;
globalThis.NodeList = Array;
class __KrrStorage {
  constructor() {
    this.__krrEntries = new Map();
  }
  get length() {
    return this.__krrEntries.size;
  }
  key(index) {
    return Array.from(this.__krrEntries.keys())[Number(index)] ?? null;
  }
  getItem(name) {
    name = String(name);
    return this.__krrEntries.has(name) ? this.__krrEntries.get(name) : null;
  }
  setItem(name, value) {
    this.__krrEntries.set(String(name), String(value));
  }
  removeItem(name) {
    this.__krrEntries.delete(String(name));
  }
  clear() {
    this.__krrEntries.clear();
  }
}
globalThis.Storage = __KrrStorage;
const __krrLocalStorage = new __KrrStorage();
globalThis.localStorage = new Proxy(__krrLocalStorage, {
  get(target, property) {
    if (Reflect.has(target, property)) {
      const value = Reflect.get(target, property, target);
      return typeof value === "function" ? value.bind(target) : value;
    }
    return target.getItem(property) ?? undefined;
  },
  set(target, property, value) {
    target.setItem(property, value);
    return true;
  },
  deleteProperty(target, property) {
    target.removeItem(property);
    return true;
  },
});
Object.assign(__KrrEvent, {
  NONE: 0,
  CAPTURING_PHASE: 1,
  AT_TARGET: 2,
  BUBBLING_PHASE: 3,
});
Object.assign(__KrrEvent.prototype, {
  NONE: 0,
  CAPTURING_PHASE: 1,
  AT_TARGET: 2,
  BUBBLING_PHASE: 3,
});
const __krrListenerOptions = (options) => ({
  capture: typeof options === "boolean" ? options : Boolean(options?.capture),
  once: Boolean(typeof options === "object" && options?.once),
  passive: Boolean(typeof options === "object" && options?.passive),
});
const __krrEventTargetListeners = new WeakMap();
const __krrEventHandlers = new WeakMap();
const __krrSyncEventTarget = (target, type, entries) => {
  if (!target.__krrNodeId) return;
  const handler = __krrEventHandlers.get(target)?.get(String(type));
  __krrNativeDom(
    "setEventTarget",
    target.__krrNodeId,
    String(type),
    String(entries.length > 0 || typeof handler === "function"),
  );
};
const __krrDispatchListeners = (listeners, target, event, capture) => {
  const entries = listeners.get(String(event.type)) || [];
  for (const entry of [...entries]) {
    if (!entries.includes(entry)) continue;
    if (entry.capture !== capture) continue;
    if (entry.once) entries.splice(entries.indexOf(entry), 1);
    event.__krrPassiveListener = entry.passive;
    try {
      if (typeof entry.callback === "function") entry.callback.call(target, event);
      else entry.callback.handleEvent.call(entry.callback, event);
    } finally {
      event.__krrPassiveListener = false;
    }
    if (event.__krrImmediatePropagationStopped) break;
  }
  __krrSyncEventTarget(target, event.type, entries);
};
const __krrDispatchHandler = (target, event) => {
  if (event.__krrImmediatePropagationStopped) return;
  const handler = target[`on${event.type}`];
  if (typeof handler !== "function") return;
  const result = handler.call(target, event);
  if (result === false && event.cancelable) event.preventDefault();
};
const __krrBeginDispatch = (target, event) => {
  if (!event || typeof event !== "object" || !event.type) throw new TypeError("Invalid event");
  if (event.__krrDispatching) throw new Error("Event is already being dispatched");
  event.__krrDispatching = true;
  event.__krrPropagationStopped = false;
  event.__krrImmediatePropagationStopped = false;
  event.target = target;
  event.currentTarget = target;
  event.eventPhase = 2;
};
const __krrEndDispatch = (event) => {
  event.currentTarget = null;
  event.eventPhase = 0;
  event.__krrDispatching = false;
};
const __krrDispatchTargetPhase = (target, event, capture, phase) => {
  event.currentTarget = target;
  event.eventPhase = phase;
  const listeners = __krrEventTargetListeners.get(target) || new Map();
  __krrDispatchListeners(listeners, target, event, capture);
  if (!capture) __krrDispatchHandler(target, event);
};
const __krrInstallEventTarget = (target) => {
  const listeners = new Map();
  __krrEventTargetListeners.set(target, listeners);
  Object.defineProperties(target, {
    addEventListener: {
      configurable: true,
      value(type, callback, options) {
        if (callback === null || callback === undefined) return;
        const callable =
          typeof callback === "function" ||
          (typeof callback === "object" && typeof callback.handleEvent === "function");
        if (!callable)
          throw new TypeError("Event listener must be a function or EventListener object");
        type = String(type);
        const normalized = __krrListenerOptions(options);
        const entries = listeners.get(type) || [];
        if (
          !entries.some(
            (entry) => entry.callback === callback && entry.capture === normalized.capture,
          )
        ) {
          entries.push({
            callback,
            capture: normalized.capture,
            once: normalized.once,
            passive: normalized.passive,
          });
          listeners.set(type, entries);
        }
        __krrSyncEventTarget(target, type, entries);
      },
    },
    removeEventListener: {
      configurable: true,
      value(type, callback, options) {
        const entries = listeners.get(String(type));
        if (!entries) return;
        const { capture } = __krrListenerOptions(options);
        const index = entries.findIndex(
          (entry) => entry.callback === callback && entry.capture === capture,
        );
        if (index >= 0) entries.splice(index, 1);
        __krrSyncEventTarget(target, type, entries);
      },
    },
    dispatchEvent: {
      configurable: true,
      value(event) {
        if (target.__krrNodeId) {
          const dispatched = __krrDispatchElementEvent(target, event);
          return !dispatched.defaultPrevented;
        }
        __krrBeginDispatch(target, event);
        try {
          __krrDispatchTargetPhase(target, event, true, Event.AT_TARGET);
          if (!event.__krrImmediatePropagationStopped)
            __krrDispatchTargetPhase(target, event, false, Event.AT_TARGET);
          return !event.defaultPrevented;
        } finally {
          __krrEndDispatch(event);
        }
      },
    },
  });
  return target;
};
class __KrrXMLHttpRequest {
  constructor() {
    __krrInstallEventTarget(this);
    this.readyState = 0;
    this.status = 0;
    this.statusText = "";
    this.response = "";
    this.responseText = "";
    this.responseURL = "";
    this.__krrMethod = "";
    this.__krrUrl = "";
  }
  open(method, url) {
    this.__krrMethod = String(method);
    this.__krrUrl = String(url);
    this.readyState = 1;
  }
  send() {
    if (this.readyState !== 1) throw new Error("XMLHttpRequest is not opened");
    const result = JSON.parse(__krrNativeDom("requestText", this.__krrMethod, this.__krrUrl));
    this.status = Number(result.status);
    this.statusText = String(result.statusText);
    this.response = String(result.responseText);
    this.responseText = this.response;
    this.responseURL = this.__krrUrl;
    this.readyState = 4;
    Promise.resolve().then(() => {
      this.dispatchEvent(new Event(result.ok ? "load" : "error"));
    });
  }
}
Object.assign(__KrrXMLHttpRequest, {
  UNSENT: 0,
  OPENED: 1,
  HEADERS_RECEIVED: 2,
  LOADING: 3,
  DONE: 4,
});
Object.assign(__KrrXMLHttpRequest.prototype, {
  UNSENT: 0,
  OPENED: 1,
  HEADERS_RECEIVED: 2,
  LOADING: 3,
  DONE: 4,
});
globalThis.XMLHttpRequest = __KrrXMLHttpRequest;
const __krrDatasetAttribute = (property) =>
  `data-${String(property).replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`;
const __krrClassToken = (token) => {
  token = String(token);
  if (!token || /\s/.test(token)) throw new TypeError("Invalid class token");
  return token;
};
const __krrElements = new Map();
const __krrElement = (nodeId) => {
  if (nodeId === null || nodeId === undefined || nodeId === "") return null;
  const normalizedId = String(nodeId);
  const cached = __krrElements.get(normalizedId);
  if (cached) return cached;
  const element = __krrInstallEventTarget(Object.create(__krrElementPrototype));
  Object.defineProperty(element, "__krrNodeId", { value: normalizedId });
  __krrElements.set(normalizedId, element);
  return element;
};
const __krrElementPrototype = {
  getBoundingClientRect() {
    return JSON.parse(__krrNativeDom("boundingClientRect", this.__krrNodeId));
  },
  get contentDocument() {
    return this.getAttribute("data-krr-local-frame") !== null ? document : null;
  },
  get onclick() {
    return __krrEventHandlers.get(this)?.get("click") ?? null;
  },
  set onclick(value) {
    let handlers = __krrEventHandlers.get(this);
    if (!handlers) {
      handlers = new Map();
      __krrEventHandlers.set(this, handlers);
    }
    if (value === null || value === undefined) handlers.delete("click");
    else handlers.set("click", value);
    const listeners = __krrEventTargetListeners.get(this) || new Map();
    __krrSyncEventTarget(this, "click", listeners.get("click") || []);
  },
  get textContent() {
    return __krrNativeDom("textContent", this.__krrNodeId);
  },
  set textContent(value) {
    __krrNativeDom("setTextContent", this.__krrNodeId, String(value));
  },
  get innerHTML() {
    return __krrNativeDom("innerHTML", this.__krrNodeId);
  },
  set innerHTML(value) {
    __krrNativeDom("setInnerHTML", this.__krrNodeId, String(value));
  },
  get outerHTML() {
    return __krrNativeDom("outerHTML", this.__krrNodeId);
  },
  get firstElementChild() {
    return __krrElement(__krrNativeDom("firstElementChild", this.__krrNodeId));
  },
  get lastElementChild() {
    return __krrElement(__krrNativeDom("lastElementChild", this.__krrNodeId));
  },
  get className() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "class") || "";
  },
  set className(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "class", String(value));
  },
  get classList() {
    const read = () => new Set(this.className.split(/\s+/).filter(Boolean));
    const write = (tokens) => {
      const value = Array.from(tokens).join(" ");
      if (value) this.className = value;
      else this.removeAttribute("class");
    };
    return {
      add(...values) {
        const tokens = read();
        for (const value of values) tokens.add(__krrClassToken(value));
        write(tokens);
      },
      remove(...values) {
        const tokens = read();
        for (const value of values) tokens.delete(__krrClassToken(value));
        write(tokens);
      },
      contains(value) {
        return read().has(__krrClassToken(value));
      },
      toggle(value, force) {
        const token = __krrClassToken(value);
        const tokens = read();
        const enabled = force === undefined ? !tokens.has(token) : Boolean(force);
        if (enabled) tokens.add(token);
        else tokens.delete(token);
        write(tokens);
        return enabled;
      },
    };
  },
  get id() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "id") || "";
  },
  set id(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "id", String(value));
  },
  get href() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "href") || "";
  },
  set href(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "href", String(value));
  },
  get value() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "value") || "";
  },
  set value(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "value", String(value));
  },
  get checked() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "checked") !== null;
  },
  set checked(value) {
    if (value) __krrNativeDom("setAttribute", this.__krrNodeId, "checked", "");
    else __krrNativeDom("removeAttribute", this.__krrNodeId, "checked");
  },
  get open() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "open") !== null;
  },
  set open(value) {
    if (value) __krrNativeDom("setAttribute", this.__krrNodeId, "open", "");
    else __krrNativeDom("removeAttribute", this.__krrNodeId, "open");
  },
  get dataset() {
    const nodeId = this.__krrNodeId;
    return new Proxy(
      {},
      {
        get(_target, property) {
          return (
            __krrNativeDom("getAttribute", nodeId, __krrDatasetAttribute(property)) || undefined
          );
        },
        set(_target, property, value) {
          __krrNativeDom("setAttribute", nodeId, __krrDatasetAttribute(property), String(value));
          return true;
        },
      },
    );
  },
  get style() {
    const nodeId = this.__krrNodeId;
    return new Proxy(
      {},
      {
        get(_target, property) {
          return __krrNativeDom("styleGet", nodeId, String(property)) || "";
        },
        set(_target, property, value) {
          __krrNativeDom("styleSet", nodeId, String(property), String(value));
          return true;
        },
      },
    );
  },
  getAttribute(name) {
    return __krrNativeDom("getAttribute", this.__krrNodeId, String(name));
  },
  setAttribute(name, value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, String(name), String(value));
  },
  removeAttribute(name) {
    __krrNativeDom("removeAttribute", this.__krrNodeId, String(name));
  },
  querySelector(selector) {
    return __krrElement(__krrNativeDom("elementQuerySelector", this.__krrNodeId, String(selector)));
  },
  querySelectorAll(selector) {
    return __krrNativeDom("elementQuerySelectorAll", this.__krrNodeId, String(selector)).map(
      __krrElement,
    );
  },
  appendChild(child) {
    __krrNativeDom("appendChild", this.__krrNodeId, child.__krrNodeId);
    return child;
  },
  insertAdjacentHTML(position, value) {
    __krrNativeDom("insertAdjacentHTML", this.__krrNodeId, String(position), String(value));
  },
  click() {
    this.dispatchEvent(new Event("click", { bubbles: true, cancelable: true }));
  },
  remove() {
    __krrNativeDom("remove", this.__krrNodeId);
  },
  matches(selector) {
    return __krrNativeDom("closest", this.__krrNodeId, String(selector)) === this.__krrNodeId;
  },
  closest(selector) {
    return __krrElement(__krrNativeDom("closest", this.__krrNodeId, String(selector)));
  },
};
const __krrInlineHandler = (target, event) => {
  const source = target.getAttribute?.(`on${event.type}`);
  if (!source) return;
  const result = Function("event", source).call(target, event);
  if (result === false && event.cancelable) event.preventDefault();
};
const __krrDispatchElementPhase = (target, event, capture, phase) => {
  __krrDispatchTargetPhase(target, event, capture, phase);
  if (!capture && !event.__krrImmediatePropagationStopped) __krrInlineHandler(target, event);
};
const __krrDispatchElementEvent = (target, event) => {
  __krrBeginDispatch(target, event);
  try {
    const nodeIds = __krrNativeDom("eventPath", target.__krrNodeId);
    if (!Array.isArray(nodeIds)) return event;
    const path = nodeIds.map(__krrElement);
    const ancestors = path.slice(1);
    const captures = [window, document, ...ancestors.slice().reverse()];
    for (const currentTarget of captures) {
      __krrDispatchElementPhase(currentTarget, event, true, Event.CAPTURING_PHASE);
      if (event.__krrPropagationStopped) break;
    }
    if (!event.__krrPropagationStopped) {
      __krrDispatchElementPhase(target, event, true, Event.AT_TARGET);
      if (!event.__krrImmediatePropagationStopped)
        __krrDispatchElementPhase(target, event, false, Event.AT_TARGET);
    }
    if (event.bubbles && !event.__krrPropagationStopped) {
      for (const currentTarget of [...ancestors, document, window]) {
        __krrDispatchElementPhase(currentTarget, event, false, Event.BUBBLING_PHASE);
        if (event.__krrPropagationStopped) break;
      }
    }
    return event;
  } finally {
    __krrEndDispatch(event);
  }
};
globalThis.__krrDispatchHostEvent = (nodeId, type, key, bubbles, cancelable) => {
  const event = new Event(type, { bubbles, cancelable });
  if (key !== null && key !== undefined) event.key = String(key);
  return __krrDispatchElementEvent(__krrElement(nodeId), event);
};
let __krrDocumentReadyState = "loading";
globalThis.document = __krrInstallEventTarget({
  getElementById(id) {
    return __krrElement(__krrNativeDom("getElementById", String(id)));
  },
  querySelector(selector) {
    return __krrElement(__krrNativeDom("querySelector", String(selector)));
  },
  querySelectorAll(selector) {
    return __krrNativeDom("querySelectorAll", String(selector)).map(__krrElement);
  },
  createElement(tag) {
    return __krrElement(__krrNativeDom("createElement", String(tag)));
  },
  get body() {
    return __krrElement(__krrNativeDom("querySelector", "body"));
  },
  get location() {
    return globalThis.location;
  },
  get readyState() {
    return __krrDocumentReadyState;
  },
});
globalThis.window = globalThis;
__krrInstallEventTarget(globalThis);
const __krrLayoutMetrics = () => JSON.parse(__krrNativeDom("layoutMetrics"));
Object.defineProperties(globalThis, {
  innerWidth: { get: () => __krrLayoutMetrics().width },
  innerHeight: { get: () => __krrLayoutMetrics().height },
  pageYOffset: { get: () => __krrLayoutMetrics().scrollY },
  scrollY: { get: () => __krrLayoutMetrics().scrollY },
});
const __krrIntersectionObservers = new Set();
let __krrIntersectionLayoutMetricsReady = false;
const __krrIntersectionRect = (first, second) => {
  const left = Math.max(first.left, second.left);
  const top = Math.max(first.top, second.top);
  const right = Math.min(first.right, second.right);
  const bottom = Math.min(first.bottom, second.bottom);
  const width = Math.max(0, right - left);
  const height = Math.max(0, bottom - top);
  return { x: left, y: top, width, height, top, right: left + width, bottom: top + height, left };
};
const __krrViewportRect = () => {
  const { width, height } = __krrLayoutMetrics();
  return { x: 0, y: 0, width, height, top: 0, right: width, bottom: height, left: 0 };
};
const __krrParseRootMargin = (value) => {
  const parts = String(value ?? "0px")
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (parts.length < 1 || parts.length > 4) return null;
  const parsed = parts.map((part) => {
    const match = part.match(/^(-?(?:\d+(?:\.\d*)?|\.\d+))(px|%)$/);
    if (!match) return null;
    const amount = Number(match[1]);
    return Number.isFinite(amount) ? { amount, unit: match[2] } : null;
  });
  if (parsed.some((part) => part === null)) return null;
  const [top, right = top, bottom = top, left = right] = parsed;
  return [top, right, bottom, left];
};
const __krrExpandRootBounds = (root, margins) => {
  const [top, right, bottom, left] = margins;
  const resolve = (margin) =>
    margin.unit === "%" ? (root.width * margin.amount) / 100 : margin.amount;
  const topOffset = resolve(top);
  const rightOffset = resolve(right);
  const bottomOffset = resolve(bottom);
  const leftOffset = resolve(left);
  return {
    x: root.left - leftOffset,
    y: root.top - topOffset,
    width: root.width + leftOffset + rightOffset,
    height: root.height + topOffset + bottomOffset,
    top: root.top - topOffset,
    right: root.right + rightOffset,
    bottom: root.bottom + bottomOffset,
    left: root.left - leftOffset,
  };
};
globalThis.IntersectionObserver = class IntersectionObserver {
  constructor(callback, options = {}) {
    if (typeof callback !== "function") {
      throw new TypeError("IntersectionObserver callback must be a function");
    }
    this.callback = callback;
    this.root = options.root || null;
    this.rootMargin = options.rootMargin || "0px";
    this.__krrRootMargin = __krrParseRootMargin(this.rootMargin) || __krrParseRootMargin("0px");
    this.thresholds = Array.isArray(options.threshold)
      ? options.threshold
      : [options.threshold || 0];
    this.targets = new Set();
    this.intersections = new Map();
    __krrIntersectionObservers.add(this);
  }
  observe(target) {
    if (!target || target.__krrNodeId === undefined) {
      throw new TypeError("IntersectionObserver target must be an element");
    }
    this.targets.add(target);
    __krrIntersectionObservers.add(this);
    if (__krrIntersectionLayoutMetricsReady) this.__krrNotify([target]);
  }
  unobserve(target) {
    this.targets.delete(target);
    this.intersections.delete(target);
  }
  disconnect() {
    this.targets.clear();
    this.intersections.clear();
    __krrIntersectionObservers.delete(this);
  }
  takeRecords() {
    return [];
  }
  __krrNotify(targets) {
    const entries = targets.map((target) => {
      const rootBounds = __krrExpandRootBounds(
        this.root ? this.root.getBoundingClientRect() : __krrViewportRect(),
        this.__krrRootMargin,
      );
      const boundingClientRect = target.getBoundingClientRect();
      const intersectionRect = __krrIntersectionRect(rootBounds, boundingClientRect);
      const targetArea = boundingClientRect.width * boundingClientRect.height;
      const intersectionArea = intersectionRect.width * intersectionRect.height;
      const isIntersecting = targetArea > 0 && intersectionArea > 0;
      const intersectionRatio = targetArea > 0 ? intersectionArea / targetArea : 0;
      return {
        target,
        isIntersecting,
        intersectionRatio,
        boundingClientRect,
        intersectionRect,
        rootBounds,
        time: Date.now(),
      };
    });
    const changed = entries.filter((entry) => {
      const previous = this.intersections.get(entry.target);
      this.intersections.set(entry.target, entry);
      return (
        previous === undefined ||
        previous.isIntersecting !== entry.isIntersecting ||
        this.thresholds.some(
          (threshold) =>
            entry.intersectionRatio >= threshold !== previous.intersectionRatio >= threshold,
        )
      );
    });
    if (changed.length > 0) this.callback(changed, this);
  }
};
globalThis.__krrRefreshIntersectionObservers = () => {
  __krrIntersectionLayoutMetricsReady = true;
  for (const observer of [...__krrIntersectionObservers]) {
    observer.__krrNotify([...observer.targets]);
  }
};
window.addEventListener("scroll", globalThis.__krrRefreshIntersectionObservers);
globalThis.__krrDispatchDocumentContentLoaded = () => {
  __krrDocumentReadyState = "interactive";
  document.dispatchEvent(new Event("readystatechange"));
  document.dispatchEvent(new Event("DOMContentLoaded"));
};
globalThis.__krrDispatchWindowLoad = () => {
  __krrDocumentReadyState = "complete";
  document.dispatchEvent(new Event("readystatechange"));
  for (const frame of document.querySelectorAll("iframe")) {
    if (frame.contentDocument) frame.dispatchEvent(new Event("load"));
  }
  window.dispatchEvent(new Event("load"));
};
