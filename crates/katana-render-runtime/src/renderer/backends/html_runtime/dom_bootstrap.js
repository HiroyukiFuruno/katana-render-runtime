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
const __krrElementInstances = new WeakSet();
const __krrElement = (nodeId) => {
  if (nodeId === null || nodeId === undefined || nodeId === "") return null;
  const normalizedId = String(nodeId);
  const cached = __krrElements.get(normalizedId);
  if (cached) return cached;
  const element = __krrInstallEventTarget(Object.create(__krrElementPrototype));
  Object.defineProperty(element, "__krrNodeId", { value: normalizedId });
  __krrElements.set(normalizedId, element);
  __krrElementInstances.add(element);
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
const __krrEmptyIntersectionRect = () => ({
  x: 0,
  y: 0,
  width: 0,
  height: 0,
  top: 0,
  right: 0,
  bottom: 0,
  left: 0,
});
const __krrBoundingIntersectionRect = (rects) => {
  if (rects.length === 0) return __krrEmptyIntersectionRect();
  const left = Math.min(...rects.map((rect) => rect.left));
  const top = Math.min(...rects.map((rect) => rect.top));
  const right = Math.max(...rects.map((rect) => rect.right));
  const bottom = Math.max(...rects.map((rect) => rect.bottom));
  return { x: left, y: top, width: right - left, height: bottom - top, top, right, bottom, left };
};
const __krrRectHasCrossedEdges = (rect) => rect.left > rect.right || rect.top > rect.bottom;
const __krrRectsIntersectOrAreEdgeAdjacent = (first, second) =>
  first.left <= second.right &&
  second.left <= first.right &&
  first.top <= second.bottom &&
  second.top <= first.bottom;
const __krrClippedRootBounds = (rootBounds, clips) => {
  if (__krrRectHasCrossedEdges(rootBounds)) {
    return { rect: rootBounds, separated: true };
  }
  return clips.reduce(
    (state, clip) => {
      const clipRect = __krrClipRect(clip);
      return {
        rect: __krrIntersectionRect(state.rect, clipRect),
        separated: state.separated || !__krrRectsIntersectOrAreEdgeAdjacent(state.rect, clipRect),
      };
    },
    { rect: rootBounds, separated: false },
  );
};
const __krrObservedElementBox = (element) => ({
  boundingClientRect: element.getBoundingClientRect(),
  isPresent: __krrNativeDom("layoutBoxPresent", element.__krrNodeId) === "1",
  metadata: JSON.parse(__krrNativeDom("intersectionMetadata", element.__krrNodeId)),
});
const __krrPathNodeIndex = (path, nodeId) =>
  Array.isArray(path) ? path.findIndex((value) => String(value) === String(nodeId)) : -1;
const __krrTargetIsDescendantOfRoot = (target, root) => {
  const path = __krrNativeDom("eventPath", target.__krrNodeId);
  return __krrPathNodeIndex(path.slice(1), root.__krrNodeId) >= 0;
};
const __krrMetadataBelongsToRoot = (metadata, target, root) => {
  if (metadata === null || typeof metadata !== "object") return false;
  const hasValidGeometry =
    Array.isArray(metadata.clips) &&
    metadata.clips.every(
      (clip) =>
        Number.isSafeInteger(clip.owner) &&
        [clip.x, clip.y, clip.width, clip.height].every(Number.isFinite) &&
        clip.width >= 0 &&
        clip.height >= 0,
    ) &&
    Array.isArray(metadata.fragments) &&
    metadata.fragments.every(
      (fragment) =>
        [fragment.x, fragment.y, fragment.width, fragment.height].every(Number.isFinite) &&
        fragment.width >= 0 &&
        fragment.height >= 0,
    );
  if (!hasValidGeometry || typeof metadata.viewportEscape !== "boolean") return false;
  if (String(metadata.positioningOrigin) === String(root.__krrNodeId)) return true;
  if (metadata.viewportEscape) return false;
  if (metadata.positioning === "in-flow") return true;
  const path = __krrNativeDom("eventPath", target.__krrNodeId);
  if (__krrPathNodeIndex(path, root.__krrNodeId) < 0) return false;
  if (metadata.positioning === "fixed") return false;
  if (
    (metadata.positioning !== "absolute" && metadata.positioning !== "fixed-containing-block") ||
    !Number.isSafeInteger(metadata.containingBlock)
  ) {
    return false;
  }
  const containingBlockIndex = __krrPathNodeIndex(path, metadata.containingBlock);
  const rootIndex = __krrPathNodeIndex(path, root.__krrNodeId);
  return containingBlockIndex >= 0 && rootIndex >= 0 && containingBlockIndex <= rootIndex;
};
const __krrClipsWithinRoot = (metadata, target, root) => {
  if (!metadata || !Array.isArray(metadata.clips)) return [];
  const path = __krrNativeDom("eventPath", target.__krrNodeId);
  const rootIndex = __krrPathNodeIndex(path, root.__krrNodeId);
  if (rootIndex < 0) return null;
  const clipIndexes = metadata.clips.map((clip) => __krrPathNodeIndex(path, clip.owner));
  if (clipIndexes.some((clipIndex) => clipIndex < 0)) return null;
  const rootClipIndex = clipIndexes.indexOf(rootIndex);
  return {
    rootClip: rootClipIndex >= 0 ? metadata.clips[rootClipIndex] : null,
    ancestorClips: metadata.clips.filter((_, index) => clipIndexes[index] < rootIndex),
  };
};
const __krrTargetWithinRoot = (target, root, metadata, clipsAreConsistent) =>
  __krrTargetIsDescendantOfRoot(target, root) &&
  clipsAreConsistent &&
  __krrMetadataBelongsToRoot(metadata, target, root);
const __krrClipRect = (clip) => {
  const scrollY = __krrLayoutMetrics().scrollY;
  const top = clip.y - scrollY;
  return {
    x: clip.x,
    y: top,
    width: clip.width,
    height: clip.height,
    top,
    right: clip.x + clip.width,
    bottom: top + clip.height,
    left: clip.x,
  };
};
const __krrRectPolygon = (rect) =>
  __krrRectHasCrossedEdges(rect)
    ? []
    : [
        { x: rect.left, y: rect.top },
        { x: rect.right, y: rect.top },
        { x: rect.right, y: rect.bottom },
        { x: rect.left, y: rect.bottom },
      ];
const __krrPolygonRect = (polygon) => {
  if (polygon.length === 0) return __krrEmptyIntersectionRect();
  const left = Math.min(...polygon.map((point) => point.x));
  const top = Math.min(...polygon.map((point) => point.y));
  const right = Math.max(...polygon.map((point) => point.x));
  const bottom = Math.max(...polygon.map((point) => point.y));
  return { x: left, y: top, width: right - left, height: bottom - top, top, right, bottom, left };
};
const __krrCross = (first, second, point) =>
  (second.x - first.x) * (point.y - first.y) - (second.y - first.y) * (point.x - first.x);
const __krrPolygonIsPoint = (polygon) =>
  polygon.length > 0 &&
  polygon.every((point) => point.x === polygon[0].x && point.y === polygon[0].y);
const __krrPointInPolygon = (point, polygon) => {
  if (polygon.length === 0) return false;
  let inside = false;
  for (let index = 0; index < polygon.length; index += 1) {
    const first = polygon[index];
    const second = polygon[(index + 1) % polygon.length];
    const cross = __krrCross(first, second, point);
    const withinSegment =
      cross === 0 &&
      point.x >= Math.min(first.x, second.x) &&
      point.x <= Math.max(first.x, second.x) &&
      point.y >= Math.min(first.y, second.y) &&
      point.y <= Math.max(first.y, second.y);
    if (withinSegment) return true;
    if (first.y > point.y !== second.y > point.y) {
      const intersectionX =
        first.x + ((second.x - first.x) * (point.y - first.y)) / (second.y - first.y);
      if (point.x < intersectionX) inside = !inside;
    }
  }
  return inside;
};
const __krrPolygonIntersection = (subject, clip) => {
  if (subject.length === 0 || clip.length === 0) return [];
  if (__krrPolygonIsPoint(clip)) {
    return __krrPointInPolygon(clip[0], subject) ? [clip[0]] : [];
  }
  let output = subject;
  for (let index = 0; index < clip.length && output.length > 0; index += 1) {
    const first = clip[index];
    const second = clip[(index + 1) % clip.length];
    const input = output;
    output = [];
    for (let pointIndex = 0; pointIndex < input.length; pointIndex += 1) {
      const previous = input[(pointIndex + input.length - 1) % input.length];
      const current = input[pointIndex];
      const previousInside = __krrCross(first, second, previous) >= 0;
      const currentInside = __krrCross(first, second, current) >= 0;
      if (currentInside !== previousInside) {
        const previousSide = __krrCross(first, second, previous);
        const currentSide = __krrCross(first, second, current);
        const ratio = previousSide / (previousSide - currentSide);
        output.push({
          x: previous.x + (current.x - previous.x) * ratio,
          y: previous.y + (current.y - previous.y) * ratio,
        });
      }
      if (currentInside) output.push(current);
    }
  }
  return output;
};
const __krrClipCorners = (clip) => {
  const scrollY = __krrLayoutMetrics().scrollY;
  if (
    Array.isArray(clip.corners) &&
    clip.corners.length === 4 &&
    clip.corners.every(
      (corner) => Array.isArray(corner) && corner.length === 2 && corner.every(Number.isFinite),
    )
  ) {
    return clip.corners.map(([x, y]) => ({ x, y: y - scrollY }));
  }
  return __krrRectPolygon(__krrClipRect(clip));
};
const __krrRoundedClipPolygon = (clip) => {
  const corners = __krrClipCorners(clip);
  const width = Math.hypot(corners[1].x - corners[0].x, corners[1].y - corners[0].y);
  const height = Math.hypot(corners[2].x - corners[1].x, corners[2].y - corners[1].y);
  const fallbackRadius = [
    [Number(clip.radiusX) || 0, Number(clip.radiusY) || 0],
    [Number(clip.radiusX) || 0, Number(clip.radiusY) || 0],
    [Number(clip.radiusX) || 0, Number(clip.radiusY) || 0],
    [Number(clip.radiusX) || 0, Number(clip.radiusY) || 0],
  ];
  const radii =
    Array.isArray(clip.radii) &&
    clip.radii.length === 4 &&
    clip.radii.every(
      (radius) =>
        Array.isArray(radius) &&
        radius.length === 2 &&
        radius.every((value) => Number.isFinite(value)),
    )
      ? clip.radii
      : fallbackRadius;
  if (width === 0 || height === 0) return corners;
  const normalizedRadii = radii.map(([radiusX, radiusY]) => [
    Math.max(0, radiusX),
    Math.max(0, radiusY),
  ]);
  const horizontalScale = Math.min(
    1,
    width /
      Math.max(
        normalizedRadii[0][0] + normalizedRadii[1][0],
        normalizedRadii[2][0] + normalizedRadii[3][0],
        width,
      ),
  );
  const verticalScale = Math.min(
    1,
    height /
      Math.max(
        normalizedRadii[0][1] + normalizedRadii[3][1],
        normalizedRadii[1][1] + normalizedRadii[2][1],
        height,
      ),
  );
  const scaledRadii = normalizedRadii.map(([radiusX, radiusY]) => [
    radiusX * horizontalScale,
    radiusY * verticalScale,
  ]);
  if (scaledRadii.every(([radiusX, radiusY]) => radiusX === 0 || radiusY === 0)) {
    return corners;
  }
  const origin = corners[0];
  const horizontal = {
    x: (corners[1].x - origin.x) / width,
    y: (corners[1].y - origin.y) / width,
  };
  const vertical = {
    x: (corners[3].x - origin.x) / height,
    y: (corners[3].y - origin.y) / height,
  };
  const pointAt = (x, y) => ({
    x: origin.x + horizontal.x * x + vertical.x * y,
    y: origin.y + horizontal.y * x + vertical.y * y,
  });
  const arcs = [
    [width - scaledRadii[1][0], scaledRadii[1][1], -Math.PI / 2, 0],
    [width - scaledRadii[2][0], height - scaledRadii[2][1], 0, Math.PI / 2],
    [scaledRadii[3][0], height - scaledRadii[3][1], Math.PI / 2, Math.PI],
    [scaledRadii[0][0], scaledRadii[0][1], Math.PI, Math.PI * 1.5],
  ];
  return arcs.flatMap(([centerX, centerY, start, end], arcIndex) =>
    Array.from({ length: 9 }, (_, index) => {
      const angle = start + ((end - start) * index) / 8;
      const [radiusX, radiusY] = scaledRadii[(arcIndex + 1) % 4];
      return pointAt(centerX + radiusX * Math.cos(angle), centerY + radiusY * Math.sin(angle));
    }),
  );
};
const __krrClipFragment = (fragment, boundary, clips) => {
  let polygon = __krrPolygonIntersection(__krrRectPolygon(fragment), __krrRectPolygon(boundary));
  for (const clip of clips)
    polygon = __krrPolygonIntersection(polygon, __krrRoundedClipPolygon(clip));
  return { polygon, rect: __krrPolygonRect(polygon) };
};
const __krrViewportRect = () => {
  const { width, height } = __krrLayoutMetrics();
  return { x: 0, y: 0, width, height, top: 0, right: width, bottom: height, left: 0 };
};
const __krrElementRootBaseBounds = (elementRoot, rootMetadata, rootHasOverflowClip) => {
  if (!elementRoot) return __krrViewportRect();
  if (rootHasOverflowClip && rootMetadata?.paddingEdge) {
    return __krrClipRect(rootMetadata.paddingEdge);
  }
  return elementRoot.getBoundingClientRect();
};
const __krrRootOverflowClip = (metadata) =>
  metadata?.clipsOverflow === true ? metadata.paddingEdge : null;
const __krrRootMarginIsZero = (margin) => margin.every((part) => part.amount === 0);
const __krrRootMarginOffsets = (root, margins) => {
  const [top, right, bottom, left] = margins;
  const resolve = (margin) =>
    margin.unit === "%" ? (root.width * margin.amount) / 100 : margin.amount;
  return [resolve(top), resolve(right), resolve(bottom), resolve(left)];
};
const __krrRootClipCorners = (clip) =>
  Array.isArray(clip.corners) &&
  clip.corners.length === 4 &&
  clip.corners.every(
    (corner) => Array.isArray(corner) && corner.length === 2 && corner.every(Number.isFinite),
  )
    ? clip.corners.map(([x, y]) => ({ x, y }))
    : [
        { x: clip.x, y: clip.y },
        { x: clip.x + clip.width, y: clip.y },
        { x: clip.x + clip.width, y: clip.y + clip.height },
        { x: clip.x, y: clip.y + clip.height },
      ];
const __krrExpandedRootClip = (clip, rootBounds, margins) => {
  if (!clip || __krrRootMarginIsZero(margins)) return clip;
  const rawCorners = __krrRootClipCorners(clip);
  const width = Math.hypot(rawCorners[1].x - rawCorners[0].x, rawCorners[1].y - rawCorners[0].y);
  const height = Math.hypot(rawCorners[3].x - rawCorners[0].x, rawCorners[3].y - rawCorners[0].y);
  const [top, right, bottom, left] = __krrRootMarginOffsets(rootBounds, margins);
  if (width === 0 || height === 0) {
    const origin = rawCorners[0];
    const horizontal =
      width > 0
        ? { x: (rawCorners[1].x - origin.x) / width, y: (rawCorners[1].y - origin.y) / width }
        : height > 0
          ? { x: (rawCorners[3].y - origin.y) / height, y: (origin.x - rawCorners[3].x) / height }
          : { x: 1, y: 0 };
    const vertical =
      height > 0
        ? { x: (rawCorners[3].x - origin.x) / height, y: (rawCorners[3].y - origin.y) / height }
        : { x: -horizontal.y, y: horizontal.x };
    const offset = (horizontalOffset, verticalOffset) => [
      origin.x + horizontal.x * horizontalOffset + vertical.x * verticalOffset,
      origin.y + horizontal.y * horizontalOffset + vertical.y * verticalOffset,
    ];
    return {
      ...clip,
      corners: [
        offset(-left, -top),
        offset(right, -top),
        offset(right, bottom),
        offset(-left, bottom),
      ],
      radii: [
        [0, 0],
        [0, 0],
        [0, 0],
        [0, 0],
      ],
      radiusX: 0,
      radiusY: 0,
    };
  }
  const horizontal = {
    x: (rawCorners[1].x - rawCorners[0].x) / width,
    y: (rawCorners[1].y - rawCorners[0].y) / width,
  };
  const vertical = {
    x: (rawCorners[3].x - rawCorners[0].x) / height,
    y: (rawCorners[3].y - rawCorners[0].y) / height,
  };
  const offset = (corner, horizontalOffset, verticalOffset) => ({
    x: corner.x + horizontal.x * horizontalOffset + vertical.x * verticalOffset,
    y: corner.y + horizontal.y * horizontalOffset + vertical.y * verticalOffset,
  });
  const radii =
    Array.isArray(clip.radii) &&
    clip.radii.length === 4 &&
    clip.radii.every(
      (radius) =>
        Array.isArray(radius) &&
        radius.length === 2 &&
        radius.every((value) => Number.isFinite(value)),
    )
      ? clip.radii
      : [[Number(clip.radiusX) || 0, Number(clip.radiusY) || 0]].concat(
          Array.from({ length: 3 }, () => [Number(clip.radiusX) || 0, Number(clip.radiusY) || 0]),
        );
  const expandedRadii = radii.map(([radiusX, radiusY], index) => {
    const horizontalOffset = index === 0 || index === 3 ? left : right;
    const verticalOffset = index === 0 || index === 1 ? top : bottom;
    return [Math.max(0, radiusX + horizontalOffset), Math.max(0, radiusY + verticalOffset)];
  });
  return {
    ...clip,
    corners: [
      offset(rawCorners[0], -left, -top),
      offset(rawCorners[1], right, -top),
      offset(rawCorners[2], right, bottom),
      offset(rawCorners[3], -left, bottom),
    ].map(({ x, y }) => [x, y]),
    radii: expandedRadii,
    radiusX: Math.max(...expandedRadii.map(([radiusX]) => radiusX)),
    radiusY: Math.max(...expandedRadii.map(([, radiusY]) => radiusY)),
  };
};
const __krrIntersectionEntry = (observer, target) => {
  const elementRoot = observer.root && observer.root !== document ? observer.root : null;
  const targetBox = __krrObservedElementBox(target);
  const boundingClientRect = targetBox.boundingClientRect;
  const requestedRootClipData = elementRoot
    ? __krrClipsWithinRoot(targetBox.metadata, target, elementRoot)
    : { rootClip: null, ancestorClips: [] };
  const rootClips = requestedRootClipData?.ancestorClips ?? [];
  const rootBox = elementRoot ? __krrObservedElementBox(elementRoot) : null;
  const rootOverflowClip = __krrRootOverflowClip(rootBox?.metadata);
  const targetWithinRoot =
    !elementRoot ||
    __krrTargetWithinRoot(target, elementRoot, targetBox.metadata, requestedRootClipData !== null);
  const rootBaseBounds = __krrElementRootBaseBounds(
    elementRoot,
    rootBox?.metadata,
    Boolean(rootOverflowClip),
  );
  const rootBounds = __krrExpandRootBounds(rootBaseBounds, observer.__krrRootMargin);
  const targetFragments = [boundingClientRect];
  const clippedRoot = __krrClippedRootBounds(rootBounds, rootClips);
  const clippedRootBounds = clippedRoot.rect;
  const rootShape = __krrExpandedRootClip(
    rootOverflowClip,
    rootBaseBounds,
    observer.__krrRootMargin,
  );
  const clipShapes = rootShape ? [...rootClips, rootShape] : rootClips;
  const clippedFragments = targetWithinRoot
    ? targetFragments.map((fragment) => __krrClipFragment(fragment, clippedRootBounds, clipShapes))
    : [];
  const intersectionFragments = clippedFragments.map((fragment) => fragment.rect);
  const positiveIntersectionFragments = intersectionFragments.filter(
    (rect) => rect.width > 0 && rect.height > 0,
  );
  const edgeAdjacentIntersectionFragments = intersectionFragments.filter(
    (_, index) => !clippedRoot.separated && clippedFragments[index].polygon.length > 0,
  );
  const intersectionRect = __krrBoundingIntersectionRect(
    positiveIntersectionFragments.length > 0
      ? positiveIntersectionFragments
      : edgeAdjacentIntersectionFragments,
  );
  const targetArea = boundingClientRect.width * boundingClientRect.height;
  const intersectionArea = intersectionRect.width * intersectionRect.height;
  const isIntersecting =
    targetWithinRoot &&
    targetBox.isPresent &&
    intersectionFragments.some(
      (rect, index) =>
        (rect.width > 0 && rect.height > 0) ||
        (!clippedRoot.separated && clippedFragments[index].polygon.length > 0),
    );
  const intersectionRatio =
    targetWithinRoot && targetBox.isPresent && targetArea > 0
      ? intersectionArea / targetArea
      : isIntersecting
        ? 1
        : 0;
  return {
    target,
    isIntersecting,
    intersectionRatio,
    boundingClientRect,
    intersectionRect,
    rootBounds,
    time: Date.now(),
  };
};
const __krrParseRootMargin = (value) => {
  const parts = String(value).trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) parts.push("0px");
  if (parts.length > 4) return null;
  const parsed = parts.map((part) => {
    const match = part.match(/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(px|%)$/);
    if (!match) return null;
    const amount = Number(part.slice(0, -match[1].length));
    return Number.isFinite(amount) ? { amount, unit: match[1] } : null;
  });
  if (parsed.some((part) => part === null)) return null;
  const [top, right = top, bottom = top, left = right] = parsed;
  return [top, right, bottom, left];
};
const __krrThresholdValues = (value) => {
  if (value === null || (typeof value !== "object" && typeof value !== "function")) {
    return [value];
  }
  const iterator = value[Symbol.iterator];
  if (iterator === null || iterator === undefined) return [value];
  if (typeof iterator !== "function") {
    throw new TypeError("IntersectionObserver threshold iterator must be callable");
  }
  return Array.from({ [Symbol.iterator]: () => iterator.call(value) });
};
const __krrNormalizeThresholds = (value) => {
  const values = __krrThresholdValues(value);
  const thresholds = values.map((threshold) => {
    const number = +threshold;
    if (!Number.isFinite(number)) {
      throw new TypeError("IntersectionObserver threshold must be a finite number");
    }
    if (number < 0 || number > 1) {
      throw new RangeError("IntersectionObserver threshold must be between 0 and 1");
    }
    return number;
  });
  thresholds.sort((first, second) => first - second);
  return thresholds.length === 0 ? [0] : thresholds;
};
const __krrSerializeRootMargin = (margins) =>
  margins.map((margin) => `${margin.amount}${margin.unit}`).join(" ");
const __krrExpandRootBounds = (root, margins) => {
  const [topOffset, rightOffset, bottomOffset, leftOffset] = __krrRootMarginOffsets(root, margins);
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
    const suppliedRoot = options.root;
    const root = suppliedRoot === undefined ? null : suppliedRoot;
    if (root !== null && root !== document && !__krrElementInstances.has(root)) {
      throw new TypeError("IntersectionObserver root must be null, document, or an element");
    }
    this.root = root;
    const rootMargin = String(options.rootMargin === undefined ? "0px" : options.rootMargin);
    this.__krrRootMargin = __krrParseRootMargin(rootMargin);
    if (this.__krrRootMargin === null) {
      throw new SyntaxError(
        "IntersectionObserver rootMargin must use 1 to 4 px or percentage values",
      );
    }
    this.rootMargin = __krrSerializeRootMargin(this.__krrRootMargin);
    this.thresholds = __krrNormalizeThresholds(
      options.threshold === undefined ? 0 : options.threshold,
    );
    this.targets = new Set();
    this.intersections = new Map();
  }
  observe(target) {
    if (!target || target.__krrNodeId === undefined) {
      throw new TypeError("IntersectionObserver target must be an element");
    }
    const wasEmpty = this.targets.size === 0;
    this.targets.add(target);
    if (wasEmpty) __krrIntersectionObservers.add(this);
    if (__krrIntersectionLayoutMetricsReady) this.__krrNotify([target]);
  }
  unobserve(target) {
    this.targets.delete(target);
    this.intersections.delete(target);
    if (this.targets.size === 0) __krrIntersectionObservers.delete(this);
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
    const entries = targets.map((target) => __krrIntersectionEntry(this, target));
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
  globalThis.__krrPrepareIntersectionObservers();
  for (const observer of [...__krrIntersectionObservers]) {
    observer.__krrNotify([...observer.targets]);
  }
};
globalThis.__krrPrepareIntersectionObservers = () => {
  __krrIntersectionLayoutMetricsReady = true;
  return [...__krrIntersectionObservers].some((observer) => observer.targets.size > 0);
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
