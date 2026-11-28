const __krrNativeDomRaw = globalThis.__krr_dom;
const __krrNativeBridgeErrorName = "KrrNativeDomError";
let __krrNativeDomBridgeFailed = false;
let __krrHasMutationObservers = false;
const __krrCallNativeDomRaw = (...args) => {
  __krrNativeDomBridgeFailed = false;
  try {
    return __krrNativeDomRaw(...args);
  } catch (error) {
    if (error?.name !== __krrNativeBridgeErrorName) throw error;
    __krrNativeDomBridgeFailed = true;
    return undefined;
  }
};
const __krrMutationTypeForOperation = (operation) => {
  switch (operation) {
    case "setAttribute":
    case "removeAttribute":
    case "styleSet":
      return "attributes";
    case "appendChild":
    case "appendText":
    case "insertAdjacentHTML":
    case "insertAdjacentText":
    case "remove":
    case "setTextContent":
    case "setInnerHTML":
      return "childList";
    default:
      return null;
  }
};
const __krrCaptureMutationPath = (nodeId) => {
  const path = __krrCallNativeDomRaw("eventPath", nodeId);
  if (__krrNativeDomBridgeFailed || !Array.isArray(path) || path.length === 0) return null;
  return path.map(String);
};
const __krrIsAdjacentSiblingInsertion = (operation, position) =>
  (operation === "insertAdjacentHTML" || operation === "insertAdjacentText") &&
  (position === "beforebegin" || position === "afterend");
const __krrMutationAttributeName = (args, operation, mutationType) => {
  if (mutationType !== "attributes") return null;
  if (operation === "styleSet") return "style";
  return typeof args[2] === "string"
    ? args[2].trim().replace(/[A-Z]/g, (letter) => letter.toLowerCase())
    : null;
};
const __krrMutationRecord = (mutationType, operation, nodeId, path, position, attributeName) => {
  const useParent = operation === "remove" || __krrIsAdjacentSiblingInsertion(operation, position);
  return {
    type: mutationType,
    target: useParent ? (path[1] ?? nodeId) : nodeId,
    path: useParent ? path.slice(1) : path,
    attributeName,
  };
};
let __krrQueueMutation = () => {};
const __krrDispatchNativeMutation = (args, operation, mutationType) => {
  if (!__krrHasMutationObservers) return __krrCallNativeDomRaw(...args);

  const nodeId = args[1];
  if (typeof nodeId !== "string") return __krrCallNativeDomRaw(...args);
  const attributeName = __krrMutationAttributeName(args, operation, mutationType);
  const adjacentPosition =
    operation === "insertAdjacentHTML" || operation === "insertAdjacentText" ? args[2] : "";
  const previousPath = __krrCaptureMutationPath(nodeId);
  if (!previousPath) return undefined;
  const result = __krrCallNativeDomRaw(...args);
  if (__krrNativeDomBridgeFailed) return undefined;
  if (mutationType === "attributes" && attributeName === null) return result;
  __krrQueueMutation(
    __krrMutationRecord(
      mutationType,
      operation,
      nodeId,
      previousPath,
      adjacentPosition,
      attributeName,
    ),
  );
  return result;
};
const __krrNativeDom = (...args) => {
  const operation = typeof args[0] === "string" ? args[0] : String(args[0]);
  const mutationType = __krrMutationTypeForOperation(operation);
  if (!mutationType) return __krrCallNativeDomRaw(...args);
  return __krrDispatchNativeMutation(args, operation, mutationType);
};
const __krrLifecycleCheckpoint = Promise.resolve.bind(Promise);
const __krrTimerQueue = [];
let __krrTimerId = 0;
let __krrTimerClock = 0;
globalThis.setTimeout = (callback, delay = 0, ...args) => {
  if (typeof callback !== "function") throw new TypeError("setTimeout callback must be callable");
  const id = ++__krrTimerId;
  const normalizedDelay = Number.isFinite(Number(delay)) ? Math.max(0, Number(delay)) : 0;
  __krrTimerQueue.push({ id, callback, args, due: __krrTimerClock + normalizedDelay });
  return id;
};
globalThis.clearTimeout = (id) => {
  const index = __krrTimerQueue.findIndex((timer) => timer.id === Number(id));
  if (index >= 0) __krrTimerQueue.splice(index, 1);
};
globalThis.__krrRunTimers = (elapsed = 0) => {
  __krrTimerClock += Math.max(0, Number(elapsed) || 0);
  let index = -1;
  for (let candidate = 0; candidate < __krrTimerQueue.length; candidate += 1) {
    const timer = __krrTimerQueue[candidate];
    if (timer.due > __krrTimerClock) continue;
    const selected = __krrTimerQueue[index];
    if (
      index < 0 ||
      timer.due < selected.due ||
      (timer.due === selected.due && timer.id < selected.id)
    ) {
      index = candidate;
    }
  }
  if (index < 0) return false;
  const [timer] = __krrTimerQueue.splice(index, 1);
  timer.callback(...timer.args);
  return true;
};
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
const __krrReadyStateTargets = new Set();
let __krrReadyStateAttributeTargetsInitialized = false;
const __krrLifecyclePropertyOverrides = new WeakMap();
const __krrPrevalidatedImageEventAttribute = "data-krr-prevalidated-image-event";
const __krrLifecycleEventTypes = new Set(["load", "error", "readystatechange", "DOMContentLoaded"]);
const __krrNormalizeLifecycleEventType = (type) => {
  const normalized = String(type).toLowerCase();
  if (normalized === "load" || normalized === "error" || normalized === "readystatechange")
    return normalized;
  if (normalized === "domcontentloaded") return "DOMContentLoaded";
  return null;
};
const __krrSyncEventTarget = (target, type, entries) => {
  if (!target.__krrNodeId) return;
  const handler = __krrEventHandlers.get(target)?.get(String(type));
  if (String(type) === "readystatechange") {
    if (entries.length > 0 || typeof handler === "function") __krrReadyStateTargets.add(target);
    else __krrReadyStateTargets.delete(target);
  }
  __krrNativeDom(
    "setEventTarget",
    target.__krrNodeId,
    String(type),
    String(entries.length > 0 || typeof handler === "function"),
  );
};
const __krrStoreEventHandler = (target, type, handler) => {
  let handlers = __krrEventHandlers.get(target);
  if (!handlers) {
    handlers = new Map();
    __krrEventHandlers.set(target, handlers);
  }
  const eventType = String(type);
  if (handler === null || handler === undefined) handlers.delete(eventType);
  else handlers.set(eventType, handler);
  const listeners = __krrEventTargetListeners.get(target) || new Map();
  __krrSyncEventTarget(target, eventType, listeners.get(eventType) || []);
};
const __krrInstallInlineHandler = (target, type, source) => {
  const eventType = __krrNormalizeLifecycleEventType(type);
  if (eventType === null) return;
  __krrLifecyclePropertyOverrides.get(target)?.delete(eventType);
  if (source === null || source === undefined) {
    __krrStoreEventHandler(target, eventType, null);
    return;
  }
  try {
    __krrStoreEventHandler(target, eventType, Function("event", String(source)));
  } catch (_error) {
    __krrStoreEventHandler(target, eventType, null);
  }
};
const __krrInstallLifecycleProperty = (target, type, value) => {
  const eventType = __krrNormalizeLifecycleEventType(type);
  if (eventType === null) return;
  let overrides = __krrLifecyclePropertyOverrides.get(target);
  if (!overrides) {
    overrides = new Set();
    __krrLifecyclePropertyOverrides.set(target, overrides);
  }
  overrides.add(eventType);
  __krrStoreEventHandler(target, eventType, typeof value === "function" ? value : null);
};
const __krrDispatchListenerEntry = (entries, entry, target, event, capture) => {
  if (!entries.includes(entry) || entry.capture !== capture) return true;
  if (entry.once) entries.splice(entries.indexOf(entry), 1);
  event.__krrPassiveListener = entry.passive;
  try {
    if (typeof entry.callback === "function") entry.callback.call(target, event);
    else entry.callback.handleEvent.call(entry.callback, event);
  } catch (error) {
    if (!["readystatechange", "load", "error", "DOMContentLoaded"].includes(String(event.type)))
      throw error;
    if (!event.__krrHasListenerError) {
      event.__krrListenerError = error;
      event.__krrHasListenerError = true;
    }
  } finally {
    event.__krrPassiveListener = false;
  }
  return !event.__krrImmediatePropagationStopped;
};
const __krrDispatchListeners = (listeners, target, event, capture) => {
  const entries = listeners.get(String(event.type)) || [];
  for (const entry of [...entries]) {
    if (!__krrDispatchListenerEntry(entries, entry, target, event, capture)) break;
  }
  __krrSyncEventTarget(target, event.type, entries);
};
const __krrDispatchHandler = (target, event) => {
  if (event.__krrImmediatePropagationStopped) return;
  const eventType = String(event.type);
  const handlers = __krrEventHandlers.get(target);
  const isManagedLifecycleHandler =
    (target === globalThis && eventType === "load") ||
    (target === globalThis.document && eventType === "readystatechange") ||
    (__krrElementInstances.has(target) && __krrLifecycleEventTypes.has(eventType));
  const handler = isManagedLifecycleHandler ? handlers?.get(eventType) : target[`on${eventType}`];
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
const __krrDispatchTargetPhase = (target, event, capture, phase, deferListenerError = false) => {
  event.currentTarget = target;
  event.eventPhase = phase;
  const listeners = __krrEventTargetListeners.get(target) || new Map();
  __krrDispatchListeners(listeners, target, event, capture);
  if (!capture) {
    try {
      __krrDispatchHandler(target, event);
    } catch (error) {
      if (!event.__krrHasListenerError) throw error;
    }
    if (event.__krrHasListenerError && !deferListenerError) {
      const error = event.__krrListenerError;
      delete event.__krrListenerError;
      delete event.__krrHasListenerError;
      throw error;
    }
  }
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
          __krrThrowDeferredListenerError(event);
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
const __krrElementChildrenCollections = new WeakMap();
const __krrIsWindowLoadNode = (nodeId) => {
  const normalizedId = String(nodeId);
  const bodyId = String(__krrNativeDom("querySelector", "body") ?? "");
  return (
    bodyId === normalizedId ||
    (bodyId === "" &&
      String(__krrNativeDom("querySelector", "html > frameset") ?? "") === normalizedId)
  );
};
const __krrInstallBodyLoadHandler = (body, source) => {
  __krrLifecyclePropertyOverrides.get(globalThis)?.delete("load");
  if (source === null || source === undefined) {
    __krrStoreEventHandler(window, "load", null);
    return;
  }
  try {
    const handler = Function("event", String(source));
    __krrStoreEventHandler(window, "load", (event) => handler.call(body, event));
  } catch (_error) {
    __krrStoreEventHandler(window, "load", null);
  }
};
const __krrRouteWindowLoadElement = (element) => {
  if (!element || !__krrIsWindowLoadNode(element.__krrNodeId)) return false;
  const handler = __krrEventHandlers.get(element)?.get("load");
  if (typeof handler === "function") {
    __krrStoreEventHandler(window, "load", (event) => handler.call(element, event));
    __krrStoreEventHandler(element, "load", null);
  }
  return true;
};
const __krrElement = (nodeId) => {
  if (nodeId === null || nodeId === undefined || nodeId === "") return null;
  const normalizedId = String(nodeId);
  const cached = __krrElements.get(normalizedId);
  if (cached) return cached;
  const element = __krrInstallEventTarget(Object.create(__krrElementPrototype));
  Object.defineProperty(element, "__krrNodeId", { value: normalizedId });
  for (const eventType of __krrLifecycleEventTypes) {
    const source = __krrNativeDom("getAttribute", normalizedId, `on${eventType}`);
    if (source !== null) {
      // body.onload は Window の load プロパティと共有する。静的文書では Rust 側が
      // body 開始タグのパーサ位置で導入するため、bootstrap 時には先取りしない。
      if (__krrIsWindowLoadNode(normalizedId) && eventType === "load") continue;
      else __krrInstallInlineHandler(element, eventType, source);
    }
  }
  __krrElements.set(normalizedId, element);
  __krrElementInstances.add(element);
  return element;
};
const __krrElementChildrenIndexedItem = (property, currentNodeIds) => {
  if (typeof property !== "string" || !/^(0|[1-9][0-9]*)$/.test(property)) return undefined;
  const index = Number(property);
  if (!Number.isSafeInteger(index) || index > 0xfffffffe) return undefined;
  const nodeId = currentNodeIds()[index];
  return nodeId === undefined ? undefined : __krrElement(nodeId);
};
const __krrElementChildren = (element) => {
  const existing = __krrElementChildrenCollections.get(element);
  if (existing) return existing.collection;

  const state = { revision: null, nodeIds: [], collection: null };
  const currentNodeIds = () => {
    const revision = __krrNativeDom("treeRevision");
    if (revision !== state.revision) {
      state.nodeIds = __krrNativeDom("elementChildren", element.__krrNodeId);
      state.revision = revision;
    }
    return state.nodeIds;
  };
  const item = (index) => {
    const numericIndex = Number(index);
    const normalizedIndex = Number.isFinite(numericIndex) ? Math.trunc(numericIndex) >>> 0 : 0;
    const nodeId = currentNodeIds()[normalizedIndex];
    return nodeId === undefined ? null : __krrElement(nodeId);
  };
  const namedItem = (name) => {
    const key = String(name);
    if (key === "") return null;
    const nodeId = findNamedNodeId(key);
    return nodeId === undefined ? null : __krrElement(nodeId);
  };
  const findNamedNodeId = (key) => {
    if (key === "") return undefined;
    const nodeIds = currentNodeIds();
    // WHY: Chrome の HTMLCollection は name 一致より全体の id 一致を優先する。
    for (const nodeId of nodeIds) {
      const child = __krrElement(nodeId);
      if (child.id === key) return nodeId;
    }
    for (const nodeId of nodeIds) {
      const child = __krrElement(nodeId);
      if (child.getAttribute("name") === key) return nodeId;
    }
    return undefined;
  };
  const namedProperties = () => {
    const properties = new Set();
    for (const nodeId of currentNodeIds()) {
      const child = __krrElement(nodeId);
      if (child.id !== "") properties.add(child.id);
      const name = child.getAttribute("name");
      if (name !== null && name !== "") properties.add(name);
    }
    return properties;
  };
  const indexedNodeId = (property) => {
    if (typeof property !== "string" || !/^(0|[1-9][0-9]*)$/.test(property)) return undefined;
    const index = Number(property);
    if (!Number.isSafeInteger(index) || index > 0xfffffffe) return undefined;
    return currentNodeIds()[index];
  };
  const iterator = () => {
    let index = 0;
    let finished = false;
    const iteratorState = {
      next() {
        if (finished) return { done: true, value: undefined };
        const nodeIds = currentNodeIds();
        if (index >= nodeIds.length) {
          finished = true;
          return { done: true, value: undefined };
        }
        return { done: false, value: __krrElement(nodeIds[index++]) };
      },
      [Symbol.iterator]() {
        return this;
      },
    };
    return iteratorState;
  };
  const target = Object.create(null);
  const collectionMethods = new Set(["length", "item", "namedItem"]);
  const builtinProperty = (property) => {
    if (property === "length") return currentNodeIds().length;
    if (property === "item") return item;
    if (property === "namedItem") return namedItem;
    if (property === Symbol.iterator) return iterator;
    if (property === Symbol.toStringTag) return "HTMLCollection";
    return undefined;
  };
  const readProperty = (property) => {
    const builtin = builtinProperty(property);
    if (builtin !== undefined) return builtin;
    if (Reflect.has(target, property)) return Reflect.get(target, property);
    const nodeId = indexedNodeId(property);
    if (nodeId !== undefined) return __krrElement(nodeId);
    if (typeof property !== "string") return undefined;
    const namedNodeId = findNamedNodeId(property);
    return namedNodeId === undefined ? undefined : __krrElement(namedNodeId);
  };
  const collection = new Proxy(target, {
    get(_target, property) {
      return readProperty(property);
    },
    has(_target, property) {
      if (
        collectionMethods.has(property) ||
        property === Symbol.iterator ||
        property === Symbol.toStringTag
      )
        return true;
      if (Reflect.has(target, property)) return true;
      if (indexedNodeId(property) !== undefined) return true;
      return typeof property === "string" && findNamedNodeId(property) !== undefined;
    },
    ownKeys() {
      const nodeIds = currentNodeIds();
      const keys = nodeIds.map((_nodeId, index) => String(index));
      for (const property of namedProperties()) {
        if (!keys.includes(property)) keys.push(property);
      }
      for (const property of Reflect.ownKeys(target)) {
        if (!keys.includes(property)) keys.push(property);
      }
      return keys;
    },
    getOwnPropertyDescriptor(_target, property) {
      if (Reflect.has(target, property)) return Reflect.getOwnPropertyDescriptor(target, property);
      const nodeId = indexedNodeId(property);
      if (nodeId !== undefined) {
        return {
          value: __krrElement(nodeId),
          writable: false,
          enumerable: true,
          configurable: true,
        };
      }
      if (typeof property === "string") {
        const nodeId = findNamedNodeId(property);
        if (nodeId !== undefined) {
          return {
            value: __krrElement(nodeId),
            writable: false,
            enumerable: false,
            configurable: true,
          };
        }
      }
      return undefined;
    },
    set(_target, property, value) {
      if (collectionMethods.has(property) || indexedNodeId(property) !== undefined) return false;
      if (typeof property === "string" && findNamedNodeId(property) !== undefined) return false;
      return Reflect.set(target, property, value);
    },
    defineProperty(_target, property, descriptor) {
      if (collectionMethods.has(property) || indexedNodeId(property) !== undefined) return false;
      if (typeof property === "string" && findNamedNodeId(property) !== undefined) return false;
      return Reflect.defineProperty(target, property, descriptor);
    },
    deleteProperty(_target, property) {
      if (indexedNodeId(property) !== undefined) return false;
      if (typeof property === "string" && findNamedNodeId(property) !== undefined) return false;
      return Reflect.deleteProperty(target, property);
    },
  });
  state.collection = collection;
  __krrElementChildrenCollections.set(element, state);
  return state.collection;
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
  get children() {
    return __krrElementChildren(this);
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
  get tagName() {
    return __krrNativeDom("tagName", this.__krrNodeId) || "";
  },
  set id(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "id", String(value));
  },
  get href() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "href") || "";
  },
  get hash() {
    const tagName = this.tagName;
    if (tagName !== "A" && tagName !== "AREA") return undefined;
    return __krrNativeDom("anchorHash", this.__krrNodeId, globalThis.location.href);
  },
  set href(value) {
    __krrNativeDom("setAttribute", this.__krrNodeId, "href", String(value));
  },
  get src() {
    return __krrNativeDom("getAttribute", this.__krrNodeId, "src") || "";
  },
  set src(value) {
    this.setAttribute("src", String(value));
  },
  get value() {
    return __krrNativeDom("value", this.__krrNodeId);
  },
  set value(value) {
    const nodeId = this.__krrNodeId;
    const tagName = __krrNativeDom("tagName", nodeId);
    if (tagName === "SELECT") {
      __krrNativeDom("setSelectValue", nodeId, String(value));
    } else if (tagName === "TEXTAREA") {
      __krrNativeDom("setTextControlValue", nodeId, String(value));
    } else {
      __krrNativeDom("setAttribute", nodeId, "value", String(value));
    }
  },
  get hidden() {
    const value = __krrNativeDom("hidden", this.__krrNodeId);
    return value === undefined ? undefined : value === "1";
  },
  set hidden(value) {
    if (value) __krrNativeDom("setAttribute", this.__krrNodeId, "hidden", "");
    else __krrNativeDom("removeAttribute", this.__krrNodeId, "hidden");
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
    const attributeName = String(name);
    const attributeValue = String(value);
    __krrNativeDom("setAttribute", this.__krrNodeId, attributeName, attributeValue);
    const lifecycleEventType = attributeName.toLowerCase().startsWith("on")
      ? __krrNormalizeLifecycleEventType(attributeName.slice(2))
      : null;
    if (__krrIsWindowLoadNode(this.__krrNodeId) && lifecycleEventType === "load")
      __krrInstallBodyLoadHandler(this, attributeValue);
    else if (lifecycleEventType !== null)
      __krrInstallInlineHandler(this, lifecycleEventType, attributeValue);
  },
  removeAttribute(name) {
    const attributeName = String(name);
    const lifecycleEventType = attributeName.toLowerCase().startsWith("on")
      ? __krrNormalizeLifecycleEventType(attributeName.slice(2))
      : null;
    const hadAttribute = __krrNativeDom("getAttribute", this.__krrNodeId, attributeName) !== null;
    __krrNativeDom("removeAttribute", this.__krrNodeId, attributeName);
    if (hadAttribute && lifecycleEventType !== null) {
      if (__krrIsWindowLoadNode(this.__krrNodeId) && lifecycleEventType === "load")
        __krrInstallBodyLoadHandler(this, null);
      else __krrInstallInlineHandler(this, lifecycleEventType, null);
    }
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
  append(...children) {
    for (const child of children) {
      if (child && typeof child === "object" && child.__krrNodeId !== undefined) {
        this.appendChild(child);
      } else {
        __krrNativeDom("appendText", this.__krrNodeId, String(child));
      }
    }
  },
  insertAdjacentHTML(position, value) {
    __krrNativeDom("insertAdjacentHTML", this.__krrNodeId, String(position), String(value));
  },
  insertAdjacentText(position, value) {
    __krrNativeDom("insertAdjacentText", this.__krrNodeId, String(position), String(value));
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
  if (
    __krrLifecycleEventTypes.has(String(event.type)) &&
    (typeof target[`on${event.type}`] === "function" ||
      __krrLifecyclePropertyOverrides.get(target)?.has(String(event.type)))
  ) {
    return;
  }
  const source = target.getAttribute?.(`on${event.type}`);
  if (!source) return;
  let handler;
  try {
    handler = Function("event", source);
  } catch (_error) {
    return;
  }
  const result = handler.call(target, event);
  if (result === false && event.cancelable) event.preventDefault();
};
const __krrDispatchElementPhase = (target, event, capture, phase) => {
  __krrDispatchTargetPhase(target, event, capture, phase, true);
  if (!capture && !event.__krrImmediatePropagationStopped) {
    try {
      __krrInlineHandler(target, event);
    } catch (error) {
      if (!event.__krrHasListenerError) throw error;
    }
    if (event.__krrHasListenerError) {
      const error = event.__krrListenerError;
      delete event.__krrListenerError;
      delete event.__krrHasListenerError;
      throw error;
    }
  }
};
const __krrThrowDeferredListenerError = (event) => {
  if (!event.__krrHasListenerError) return;
  const error = event.__krrListenerError;
  delete event.__krrListenerError;
  delete event.__krrHasListenerError;
  throw error;
};
const __krrDispatchCapture = (event, ancestors) => {
  const captures = [window, document, ...ancestors.slice().reverse()];
  for (const currentTarget of captures) {
    __krrDispatchElementPhase(currentTarget, event, true, Event.CAPTURING_PHASE);
    if (event.__krrPropagationStopped) break;
  }
};
const __krrDispatchTarget = (target, event) => {
  if (event.__krrPropagationStopped) return;
  __krrDispatchElementPhase(target, event, true, Event.AT_TARGET);
  if (!event.__krrImmediatePropagationStopped)
    __krrDispatchElementPhase(target, event, false, Event.AT_TARGET);
};
const __krrDispatchBubble = (event, ancestors) => {
  if (!event.bubbles || event.__krrPropagationStopped) return;
  for (const currentTarget of [...ancestors, document, window]) {
    __krrDispatchElementPhase(currentTarget, event, false, Event.BUBBLING_PHASE);
    if (event.__krrPropagationStopped) break;
  }
};
const __krrDispatchDefaultAction = (target, event) => {
  if (String(event.type) !== "click" || event.defaultPrevented) return;
  const submitter = target.closest?.("button") || target.closest?.("input");
  if (!submitter || submitter.getAttribute?.("disabled") !== null) return;
  const tagName = String(submitter.tagName || "").toLowerCase();
  const type = String(
    submitter.getAttribute?.("type") || (tagName === "button" ? "submit" : "text"),
  ).toLowerCase();
  if ((tagName !== "button" && tagName !== "input") || type !== "submit") return;
  const form = submitter.closest("form");
  if (!form) return;
  const submitEvent = new Event("submit", { bubbles: true, cancelable: true });
  submitEvent.submitter = submitter;
  form.dispatchEvent(submitEvent);
};
const __krrDispatchElementEvent = (target, event) => {
  __krrBeginDispatch(target, event);
  try {
    const nodeIds = __krrNativeDom("eventPath", target.__krrNodeId);
    if (!Array.isArray(nodeIds)) return event;
    const path = nodeIds.map(__krrElement);
    const ancestors = path.slice(1);
    __krrDispatchCapture(event, ancestors);
    __krrDispatchTarget(target, event);
    __krrDispatchBubble(event, ancestors);
    __krrThrowDeferredListenerError(event);
    __krrDispatchDefaultAction(target, event);
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
const __krrMutationObservers = new Set();
let __krrMutationDeliveryScheduled = false;
const __krrDeliverMutations = () => {
  __krrMutationDeliveryScheduled = false;
  for (const observer of [...__krrMutationObservers]) {
    if (observer.__krrRecords.length === 0) continue;
    const records = observer.__krrRecords.splice(0);
    observer.__krrCallback(records, observer);
  }
};
const __krrRecordMutation = (observer, mutation) => {
  const normalizedId = String(mutation.target);
  const path = mutation.path.map(String);
  const registration = observer.__krrRegistrations.find((candidate) => {
    const observed = String(candidate.target.__krrNodeId);
    const inScope =
      normalizedId === observed || (candidate.options.subtree && path.includes(observed));
    if (!inScope || !candidate.options[mutation.type]) return false;
    return (
      mutation.type !== "attributes" ||
      candidate.options.attributeFilter === null ||
      candidate.options.attributeFilter.includes(mutation.attributeName)
    );
  });
  if (!registration) return;
  const record = { type: mutation.type, target: __krrElement(normalizedId) };
  if (mutation.type === "attributes") record.attributeName = mutation.attributeName;
  observer.__krrRecords.push(record);
};
__krrQueueMutation = (mutation) => {
  for (const observer of __krrMutationObservers) __krrRecordMutation(observer, mutation);
  if (__krrMutationObservers.size > 0 && !__krrMutationDeliveryScheduled) {
    __krrMutationDeliveryScheduled = true;
    Promise.resolve().then(__krrDeliverMutations);
  }
};
globalThis.MutationObserver = class MutationObserver {
  constructor(callback) {
    if (typeof callback !== "function")
      throw new TypeError("MutationObserver callback must be a function");
    this.__krrCallback = callback;
    this.__krrRecords = [];
    this.__krrRegistrations = [];
  }
  observe(target, options = {}) {
    if (!target || target.__krrNodeId === undefined)
      throw new TypeError("MutationObserver target must be an element");
    const attributeFilter = Array.isArray(options.attributeFilter)
      ? options.attributeFilter.map(String)
      : null;
    if (options.attributes === false && attributeFilter !== null)
      throw new TypeError("attributeFilter requires attributes to be enabled");
    const normalizedOptions = {
      childList: Boolean(options.childList),
      attributes: Boolean(options.attributes || attributeFilter !== null),
      subtree: Boolean(options.subtree),
      attributeFilter,
    };
    if (!normalizedOptions.childList && !normalizedOptions.attributes)
      throw new TypeError("MutationObserver options need childList or attributes");
    const registration = {
      target,
      options: normalizedOptions,
    };
    const existingIndex = this.__krrRegistrations.findIndex(
      (candidate) => String(candidate.target.__krrNodeId) === String(target.__krrNodeId),
    );
    if (existingIndex < 0) this.__krrRegistrations.push(registration);
    else this.__krrRegistrations[existingIndex] = registration;
    __krrMutationObservers.add(this);
    __krrHasMutationObservers = true;
  }
  disconnect() {
    this.__krrRegistrations = [];
    this.__krrRecords = [];
    __krrMutationObservers.delete(this);
    __krrHasMutationObservers = __krrMutationObservers.size > 0;
  }
  takeRecords() {
    return this.__krrRecords.splice(0);
  }
};
let __krrDocumentReadyState = "loading";
let __krrFocusedNodeId = null;
globalThis.__krrSetFocusedNodeId = (nodeId) => {
  if (nodeId === null) {
    __krrFocusedNodeId = null;
    return;
  }
  if (typeof nodeId !== "string" || !/^(0|[1-9][0-9]*)$/.test(nodeId))
    throw new TypeError("focused DOM node id must be a decimal string or null");
  __krrFocusedNodeId = nodeId;
};
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
  get documentElement() {
    return __krrElement(__krrNativeDom("querySelector", "html")) || this.body;
  },
  get activeElement() {
    if (__krrFocusedNodeId === null || __krrNativeDom("isConnected", __krrFocusedNodeId) !== "1") {
      return this.body;
    }
    return __krrElement(__krrFocusedNodeId) || this.body;
  },
  fonts: { ready: Promise.resolve() },
  get location() {
    return globalThis.location;
  },
  get readyState() {
    return __krrDocumentReadyState;
  },
});
const __krrDocumentPrototype = Object.create(Object.getPrototypeOf(globalThis.document));
Object.defineProperty(__krrDocumentPrototype, "onreadystatechange", {
  configurable: true,
  get() {
    return __krrEventHandlers.get(this)?.get("readystatechange") ?? null;
  },
  set(value) {
    __krrInstallLifecycleProperty(this, "readystatechange", value);
  },
});
Object.setPrototypeOf(globalThis.document, __krrDocumentPrototype);
for (const eventType of __krrLifecycleEventTypes) {
  Object.defineProperty(__krrElementPrototype, `on${eventType}`, {
    configurable: true,
    get() {
      if (eventType === "load" && __krrRouteWindowLoadElement(this))
        return __krrEventHandlers.get(window)?.get("load") ?? null;
      return __krrEventHandlers.get(this)?.get(eventType) ?? null;
    },
    set(value) {
      if (eventType === "load" && __krrRouteWindowLoadElement(this)) {
        __krrInstallLifecycleProperty(window, eventType, value);
        return;
      }
      __krrInstallLifecycleProperty(this, eventType, value);
    },
  });
}
globalThis.window = globalThis;
__krrInstallEventTarget(globalThis);
Object.defineProperty(globalThis, "onload", {
  configurable: true,
  get() {
    return __krrEventHandlers.get(globalThis)?.get("load") ?? null;
  },
  set(value) {
    __krrInstallLifecycleProperty(globalThis, "load", value);
  },
});
globalThis.__krrInstallStaticBodyLoadHandler = (parserSource, laterToken = false) => {
  if (laterToken) {
    const nodeId =
      __krrNativeDom("querySelector", "body") ?? __krrNativeDom("querySelector", "html > frameset");
    if (nodeId !== null && __krrNativeDom("getAttribute", nodeId, "onload") === null) {
      __krrNativeDom("setAttribute", nodeId, "onload", parserSource);
      __krrInstallBodyLoadHandler(__krrElement(nodeId), parserSource);
    }
    return;
  }
  const install = (element) => {
    if (parserSource === undefined) {
      __krrInstallBodyLoadHandler(element, element.getAttribute("onload"));
      return;
    }
    const currentSource = __krrNativeDom("getAttribute", element.__krrNodeId, "onload");
    if (currentSource === null) {
      __krrNativeDom("setAttribute", element.__krrNodeId, "onload", parserSource);
      __krrInstallBodyLoadHandler(element, parserSource);
    } else if (currentSource === parserSource) {
      __krrInstallBodyLoadHandler(element, currentSource);
    }
  };
  const body = document.body;
  if (body !== null) {
    install(body);
    return;
  }
  const frameset = document.querySelector("html > frameset");
  if (frameset !== null) install(frameset);
};
const __krrSeedPrevalidatedImageEvents = () => {
  let images;
  try {
    images = document.querySelectorAll("img");
  } catch (_error) {
    return;
  }
  for (const image of images) {
    image.removeAttribute(__krrPrevalidatedImageEventAttribute);
    // HTML属性は文書作成者が指定できるため、検証済み結果の証拠としては使わない。
  }
};
__krrSeedPrevalidatedImageEvents();
const __krrLayoutMetrics = () => JSON.parse(__krrNativeDom("layoutMetrics"));
Object.defineProperties(globalThis, {
  innerWidth: { get: () => __krrLayoutMetrics().width },
  innerHeight: { get: () => __krrLayoutMetrics().height },
  pageXOffset: { get: () => __krrLayoutMetrics().scrollX },
  scrollX: { get: () => __krrLayoutMetrics().scrollX },
  pageYOffset: { get: () => __krrLayoutMetrics().scrollY },
  scrollY: { get: () => __krrLayoutMetrics().scrollY },
});
const __krrIntersectionObservers = new Set();
let __krrIntersectionLayoutMetricsReady = false;
const __krrResizeObservers = new Set();
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
      const clipped = __krrClipFragment(state.rect, state.rect, [clip]);
      return {
        rect: clipped.rect,
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
        Array.isArray(clip.activeAxes) &&
        clip.activeAxes.length === 2 &&
        clip.activeAxes.every((axis) => typeof axis === "boolean") &&
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
  const { scrollX, scrollY } = __krrLayoutMetrics();
  const left = clip.x - scrollX;
  const top = clip.y - scrollY;
  return {
    x: left,
    y: top,
    width: clip.width,
    height: clip.height,
    top,
    right: left + clip.width,
    bottom: top + clip.height,
    left,
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
const __krrClipPolygonEdge = (input, first, second) => {
  const output = [];
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
  return output;
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
    output = __krrClipPolygonEdge(output, first, second);
  }
  return output;
};
const __krrPolygonIntersectionActiveEdges = (subject, clip, activeEdge) => {
  let output = subject;
  for (let index = 0; index < clip.length && output.length > 0; index += 1) {
    if (!activeEdge(index)) continue;
    const edge = clip[index];
    const first = edge.first ?? edge;
    const second = edge.second ?? clip[(index + 1) % clip.length];
    if (first.x === second.x && first.y === second.y) return [];
    output = __krrClipPolygonEdge(output, first, second);
  }
  return output;
};
const __krrActivePolygonIntersection = (subject, clip) => {
  const activeAxes = clip.activeAxes;
  if (
    !Array.isArray(activeAxes) ||
    activeAxes.length !== 2 ||
    activeAxes.some((axis) => typeof axis !== "boolean")
  ) {
    return [];
  }
  if (!activeAxes[0] && !activeAxes[1]) return subject;
  if (activeAxes[0] && activeAxes[1])
    return __krrPolygonIntersection(subject, __krrRoundedClipPolygon(clip));
  const corners = __krrClipCorners(clip);
  const edgeDirection = (index) => {
    const first = corners[index];
    const second = corners[(index + 1) % corners.length];
    if (first.x !== second.x || first.y !== second.y) return { first, second };
    const referenceIndexes = index % 2 === 0 ? [1, 3] : [0, 2];
    const referenceIndex = referenceIndexes.find((candidate) => {
      const candidateFirst = corners[candidate];
      const candidateSecond = corners[(candidate + 1) % corners.length];
      return candidateFirst.x !== candidateSecond.x || candidateFirst.y !== candidateSecond.y;
    });
    if (referenceIndex === undefined) return null;
    const referenceFirst = corners[referenceIndex];
    const referenceSecond = corners[(referenceIndex + 1) % corners.length];
    const referenceX = referenceSecond.x - referenceFirst.x;
    const referenceY = referenceSecond.y - referenceFirst.y;
    const sign = index === 1 || index === 2 ? 1 : -1;
    return {
      first,
      second: {
        x: first.x - sign * referenceY,
        y: first.y + sign * referenceX,
      },
    };
  };
  const activeEdges = corners.map((_, index) => edgeDirection(index));
  if (
    activeEdges.some(
      (edge, index) => (index % 2 === 0 ? activeAxes[1] : activeAxes[0]) && edge === null,
    )
  ) {
    return [];
  }
  return __krrPolygonIntersectionActiveEdges(subject, activeEdges, (index) =>
    index % 2 === 0 ? activeAxes[1] : activeAxes[0],
  );
};
const __krrClipCorners = (clip) => {
  const { scrollX, scrollY } = __krrLayoutMetrics();
  if (
    Array.isArray(clip.corners) &&
    clip.corners.length === 4 &&
    clip.corners.every(
      (corner) => Array.isArray(corner) && corner.length === 2 && corner.every(Number.isFinite),
    )
  ) {
    return clip.corners.map(([x, y]) => ({ x: x - scrollX, y: y - scrollY }));
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
  for (const clip of clips) polygon = __krrActivePolygonIntersection(polygon, clip);
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
  const resolveHorizontal = (margin) =>
    margin.unit === "%" ? (root.width * margin.amount) / 100 : margin.amount;
  const resolveVertical = (margin) =>
    margin.unit === "%" ? (root.height * margin.amount) / 100 : margin.amount;
  return [
    resolveVertical(top),
    resolveHorizontal(right),
    resolveVertical(bottom),
    resolveHorizontal(left),
  ];
};
const __krrRootClipCorners = (clip) =>
  Array.isArray(clip.corners) &&
  clip.corners.length === 4 &&
  clip.corners.every(
    (corner) => Array.isArray(corner) && corner.length === 2 && corner.every(Number.isFinite),
  )
    ? clip.corners.map(([x, y]) => ({ x: x - __krrLayoutMetrics().scrollX, y }))
    : [
        { x: clip.x - __krrLayoutMetrics().scrollX, y: clip.y },
        { x: clip.x + clip.width - __krrLayoutMetrics().scrollX, y: clip.y },
        { x: clip.x + clip.width - __krrLayoutMetrics().scrollX, y: clip.y + clip.height },
        { x: clip.x - __krrLayoutMetrics().scrollX, y: clip.y + clip.height },
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
    : { rootClip: null, ancestorClips: targetBox.metadata?.clips ?? [] };
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
  const rootShape = __krrExpandedRootClip(
    rootOverflowClip,
    rootBaseBounds,
    observer.__krrRootMargin,
  );
  const clipShapes = rootShape ? [...rootClips, rootShape] : rootClips;
  const clippedFragments = targetWithinRoot
    ? targetFragments.map((fragment) => __krrClipFragment(fragment, rootBounds, clipShapes))
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
globalThis.ResizeObserver = class ResizeObserver {
  constructor(callback) {
    if (typeof callback !== "function")
      throw new TypeError("ResizeObserver callback must be a function");
    this.callback = callback;
    this.targets = new Set();
  }
  observe(target) {
    if (!target || typeof target.getBoundingClientRect !== "function") {
      throw new TypeError("ResizeObserver target must be an element");
    }
    this.targets.add(target);
    __krrResizeObservers.add(this);
  }
  unobserve(target) {
    this.targets.delete(target);
    if (this.targets.size === 0) __krrResizeObservers.delete(this);
  }
  disconnect() {
    this.targets.clear();
    __krrResizeObservers.delete(this);
  }
};
globalThis.__krrPrepareResizeObservers = () =>
  [...__krrResizeObservers].some((observer) => observer.targets.size > 0);
globalThis.__krrRefreshResizeObservers = () => {
  for (const observer of [...__krrResizeObservers]) {
    const entries = [...observer.targets].map((target) => ({
      target,
      contentRect: target.getBoundingClientRect(),
    }));
    if (entries.length > 0) observer.callback(entries, observer);
  }
};
const __krrDispatchDocumentReadyStateChange = () => {
  // document handler の例外は、同じ state transition の要素通知を中断させない。
  try {
    document.dispatchEvent(new Event("readystatechange"));
  } catch (_error) {}
};
const __krrInitializeReadyStateAttributeTargets = () => {
  if (__krrReadyStateAttributeTargetsInitialized) return;
  __krrReadyStateAttributeTargetsInitialized = true;
  for (const element of document.querySelectorAll("[onreadystatechange]")) {
    __krrElement(element.__krrNodeId);
  }
};
const __krrDispatchElementReadyStateChange = () => {
  __krrInitializeReadyStateAttributeTargets();
  const orderedTargets = document.querySelectorAll("[onreadystatechange]");
  const dispatchedTargets = new Set();
  for (const element of [...orderedTargets, ...__krrReadyStateTargets]) {
    if (dispatchedTargets.has(element)) continue;
    dispatchedTargets.add(element);
    const handler = __krrEventHandlers.get(element)?.get("readystatechange");
    const listeners = __krrEventTargetListeners.get(element)?.get("readystatechange");
    if (typeof handler === "function" || (listeners?.length ?? 0) > 0) {
      // 個々の要素 handler の例外は、後続要素と window.load を中断させない。
      try {
        element.dispatchEvent(new Event("readystatechange"));
      } catch (_error) {}
    }
  }
};
globalThis.__krrDispatchDocumentContentLoaded = () => {
  __krrDocumentReadyState = "interactive";
  __krrDispatchDocumentReadyStateChange();
  __krrDispatchElementReadyStateChange();
  document.dispatchEvent(new Event("DOMContentLoaded"));
};
const __krrBase64Alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const __krrBase64Index = (value) => __krrBase64Alphabet.indexOf(String.fromCharCode(value));
const __krrBase64PayloadIsValid = (encoded, padding) => {
  for (let index = 0; index < encoded.length - padding; index += 1)
    if (__krrBase64Index(encoded[index]) < 0) return false;
  return true;
};
const __krrBase64QuartetBytes = (encoded, offset) => {
  const first = __krrBase64Index(encoded[offset]);
  const second = __krrBase64Index(encoded[offset + 1]);
  const third = encoded[offset + 2] === 61 ? 0 : __krrBase64Index(encoded[offset + 2]);
  const fourth = encoded[offset + 3] === 61 ? 0 : __krrBase64Index(encoded[offset + 3]);
  if (first < 0 || second < 0 || third < 0 || fourth < 0) return null;
  const bytes = [(first << 2) | (second >> 4)];
  if (encoded[offset + 2] !== 61) bytes.push(((second & 15) << 4) | (third >> 2));
  if (encoded[offset + 3] !== 61) bytes.push(((third & 3) << 6) | fourth);
  return bytes;
};
const __krrBase64ImageBytes = (payload) => {
  const encoded = __krrPercentEncodedImageBytes(payload);
  if (encoded === null || encoded.length % 4 !== 0) return null;
  const padding = encoded.at(-1) === 61 ? (encoded.at(-2) === 61 ? 2 : 1) : 0;
  if (!__krrBase64PayloadIsValid(encoded, padding)) return null;
  const bytes = [];
  for (let offset = 0; offset < encoded.length; offset += 4) {
    const quartet = __krrBase64QuartetBytes(encoded, offset);
    if (quartet === null) return null;
    bytes.push(...quartet);
  }
  return bytes;
};

const __krrDecodeJpegDcCoefficient = (reader, tables, huffmanTables) => {
  const length = reader.decodeSymbol(huffmanTables.get(`0:${tables >> 4}`));
  return length !== null && length <= 11 && reader.readBits(length) !== null;
};
const __krrDecodeJpegAcCoefficients = (reader, tables, huffmanTables) => {
  for (let coefficient = 1; coefficient < 64; ) {
    const symbol = reader.decodeSymbol(huffmanTables.get(`1:${tables & 15}`));
    if (symbol === null) return false;
    const run = symbol >> 4;
    const size = symbol & 15;
    if (size === 0) {
      if (run === 0) return true;
      if (run !== 15 || coefficient + 16 > 64) return false;
      coefficient += 16;
      continue;
    }
    if (size > 10 || coefficient + run >= 64 || reader.readBits(size) === null) return false;
    coefficient += run + 1;
  }
  return true;
};
const __krrDecodeBaselineJpegBlock = (reader, tables, huffmanTables) =>
  __krrDecodeJpegDcCoefficient(reader, tables, huffmanTables) &&
  __krrDecodeJpegAcCoefficients(reader, tables, huffmanTables);
const __krrJpegEntropyReader = (bytes, offset) => {
  let byteOffset = offset;
  let currentByte = 0;
  let remainingBits = 0;
  const readByte = () => {
    if (byteOffset >= bytes.length) return null;
    const value = bytes[byteOffset];
    byteOffset += 1;
    if (value !== 255) return value;
    if (byteOffset >= bytes.length || bytes[byteOffset] !== 0) {
      byteOffset -= 1;
      return null;
    }
    byteOffset += 1;
    return 255;
  };
  const readBits = (count) => {
    let value = 0;
    for (let index = 0; index < count; index += 1) {
      if (remainingBits === 0) {
        const next = readByte();
        if (next === null) return null;
        currentByte = next;
        remainingBits = 8;
      }
      value = (value << 1) | ((currentByte >> (remainingBits - 1)) & 1);
      remainingBits -= 1;
    }
    return value;
  };
  const decodeSymbol = (table) => {
    let code = 0;
    for (let length = 1; length <= 16; length += 1) {
      const bit = readBits(1);
      if (bit === null) return null;
      code = (code << 1) | bit;
      const symbol = table[length].get(code);
      if (symbol !== undefined) return symbol;
    }
    return null;
  };
  const consumeRestartMarker = (expected) => {
    if (
      remainingBits > 0 &&
      (currentByte & ((1 << remainingBits) - 1)) !== (1 << remainingBits) - 1
    )
      return false;
    remainingBits = 0;
    if (bytes[byteOffset] !== 255) return false;
    while (bytes[byteOffset] === 255) byteOffset += 1;
    if (byteOffset >= bytes.length || bytes[byteOffset] !== 208 + expected) return false;
    byteOffset += 1;
    return true;
  };
  return { readBits, decodeSymbol, consumeRestartMarker, offset: () => byteOffset };
};
const __krrBaselineJpegMcuCount = (frame) => {
  const horizontal = Math.max(...frame.components.map((component) => component.horizontal));
  const vertical = Math.max(...frame.components.map((component) => component.vertical));
  return Math.ceil(frame.width / (8 * horizontal)) * Math.ceil(frame.height / (8 * vertical));
};
const __krrDecodeBaselineJpegEntropy = (
  bytes,
  offset,
  frame,
  scanComponents,
  huffmanTables,
  restartInterval,
) => {
  const reader = __krrJpegEntropyReader(bytes, offset);
  const mcuCount = __krrBaselineJpegMcuCount(frame);
  for (let mcu = 0; mcu < mcuCount; mcu += 1) {
    for (const component of scanComponents) {
      for (let block = 0; block < component.horizontal * component.vertical; block += 1) {
        if (!__krrDecodeBaselineJpegBlock(reader, component.tables, huffmanTables)) return null;
      }
    }
    if (
      restartInterval > 0 &&
      mcu + 1 < mcuCount &&
      (mcu + 1) % restartInterval === 0 &&
      !reader.consumeRestartMarker((Math.floor((mcu + 1) / restartInterval) - 1) % 8)
    )
      return null;
  }
  return reader.offset();
};
const __krrJpegStartOfFrame = (marker) =>
  (marker >= 192 && marker <= 195) ||
  (marker >= 197 && marker <= 199) ||
  (marker >= 201 && marker <= 203) ||
  (marker >= 205 && marker <= 207);
const __krrReadJpegMarker = (bytes, offset) => {
  if (bytes[offset] !== 255) return null;
  let markerOffset = offset;
  while (bytes[markerOffset] === 255) markerOffset += 1;
  if (markerOffset >= bytes.length || bytes[markerOffset] === 0) return null;
  return { marker: bytes[markerOffset], offset: markerOffset + 1 };
};
const __krrReadJpegSegment = (bytes, offset) => {
  if (offset + 2 > bytes.length) return null;
  const length = (bytes[offset] << 8) | bytes[offset + 1];
  if (length < 2 || offset + length > bytes.length) return null;
  return { start: offset + 2, end: offset + length };
};
const __krrReadJpegQuantizationTables = (bytes, segment, tables) => {
  let offset = segment.start;
  while (offset < segment.end) {
    const descriptor = bytes[offset];
    const precision = descriptor >> 4;
    const tableId = descriptor & 15;
    const length = precision === 0 ? 65 : precision === 1 ? 129 : 0;
    if (tableId > 3 || length === 0 || offset + length > segment.end) return false;
    tables.add(tableId);
    offset += length;
  }
  return offset === segment.end;
};
const __krrReadJpegHuffmanTable = (bytes, offset, end) => {
  if (offset + 17 > end) return null;
  const descriptor = bytes[offset];
  const tableClass = descriptor >> 4;
  const tableId = descriptor & 15;
  if (tableClass > 1 || tableId > 3) return null;
  let values = 0;
  let code = 0;
  const table = Array.from({ length: 17 }, () => new Map());
  for (let length = 1; length <= 16; length += 1) {
    const count = bytes[offset + length];
    values += count;
    code <<= 1;
    if (code + count > 1 << length) return null;
    for (let index = 0; index < count; index += 1)
      table[length].set(code++, bytes[offset + 17 + values - count + index]);
  }
  const nextOffset = offset + 17 + values;
  if (nextOffset > end) return null;
  return { key: `${tableClass}:${tableId}`, table, nextOffset };
};
const __krrReadJpegHuffmanTables = (bytes, segment, tables) => {
  let offset = segment.start;
  while (offset < segment.end) {
    const result = __krrReadJpegHuffmanTable(bytes, offset, segment.end);
    if (result === null) return false;
    tables.set(result.key, result.table);
    offset = result.nextOffset;
  }
  return offset === segment.end;
};
const __krrReadJpegFrame = (bytes, marker, segment) => {
  const length = segment.end - segment.start;
  if (length < 6) return null;
  const height = (bytes[segment.start + 1] << 8) | bytes[segment.start + 2];
  const width = (bytes[segment.start + 3] << 8) | bytes[segment.start + 4];
  const componentCount = bytes[segment.start + 5];
  if (height === 0 || width === 0 || componentCount === 0 || length !== 6 + componentCount * 3)
    return null;
  const componentsById = new Map();
  const components = [];
  for (let offset = segment.start + 6; offset < segment.end; offset += 3) {
    const id = bytes[offset];
    const sampling = bytes[offset + 1];
    const quantizationTable = bytes[offset + 2];
    const component = { id, horizontal: sampling >> 4, vertical: sampling & 15, quantizationTable };
    if (
      id === 0 ||
      component.horizontal === 0 ||
      component.vertical === 0 ||
      quantizationTable > 3 ||
      componentsById.has(id)
    )
      return null;
    componentsById.set(id, component);
    components.push(component);
  }
  return { marker, width, height, components, componentsById };
};
const __krrReadJpegScanComponents = (bytes, segment, state) => {
  const length = segment.end - segment.start;
  const count = bytes[segment.start];
  if (count === 0 || length !== 4 + count * 2) return null;
  const components = [];
  for (let offset = segment.start + 1; offset < segment.start + 1 + count * 2; offset += 2) {
    const component = state.frame.componentsById.get(bytes[offset]);
    const tables = bytes[offset + 1];
    if (
      component === undefined ||
      !state.quantizationTables.has(component.quantizationTable) ||
      !state.huffmanTables.has(`0:${tables >> 4}`) ||
      !state.huffmanTables.has(`1:${tables & 15}`)
    )
      return null;
    components.push({ ...component, tables });
  }
  return components;
};
const __krrFindJpegScanMarker = (bytes, offset) => {
  let scanOffset = offset;
  while (scanOffset < bytes.length) {
    if (bytes[scanOffset] !== 255) {
      scanOffset += 1;
      continue;
    }
    const markerOffset = scanOffset;
    scanOffset += 1;
    while (bytes[scanOffset] === 255) scanOffset += 1;
    if (scanOffset >= bytes.length) return null;
    const marker = bytes[scanOffset];
    scanOffset += 1;
    if (marker === 0 || (marker >= 208 && marker <= 215)) continue;
    return markerOffset;
  }
  return null;
};
const __krrReadJpegRestartInterval = (bytes, segment, state) => {
  if (segment.end - segment.start !== 2) return false;
  state.restartInterval = (bytes[segment.start] << 8) | bytes[segment.start + 1];
  return true;
};
const __krrReadJpegScan = (bytes, segment, state) => {
  const components = __krrReadJpegScanComponents(bytes, segment, state);
  if (components === null) return null;
  state.hasScan = true;
  if (state.frame.marker === 192 && components.length === state.frame.components.length)
    return __krrDecodeBaselineJpegEntropy(
      bytes,
      segment.end,
      state.frame,
      components,
      state.huffmanTables,
      state.restartInterval,
    );
  return __krrFindJpegScanMarker(bytes, segment.end);
};
const __krrJpegMarkerHasNoSegment = (marker) => marker === 1 || (marker >= 208 && marker <= 215);
const __krrAdvanceJpegSegment = (bytes, marker, offset, state) => {
  const segment = __krrReadJpegSegment(bytes, offset);
  if (segment === null) return null;
  if (marker === 219)
    return __krrReadJpegQuantizationTables(bytes, segment, state.quantizationTables)
      ? segment.end
      : null;
  if (marker === 196)
    return __krrReadJpegHuffmanTables(bytes, segment, state.huffmanTables) ? segment.end : null;
  if (marker === 221)
    return __krrReadJpegRestartInterval(bytes, segment, state) ? segment.end : null;
  if (__krrJpegStartOfFrame(marker)) {
    state.frame = __krrReadJpegFrame(bytes, marker, segment);
    state.hasFrame = state.frame !== null;
    return state.hasFrame ? segment.end : null;
  }
  return marker === 218 && state.hasFrame ? __krrReadJpegScan(bytes, segment, state) : segment.end;
};
const __krrAdvanceJpeg = (bytes, marker, offset, state) => {
  if (marker === 217)
    return state.hasFrame && state.hasScan && offset === bytes.length ? true : null;
  if (__krrJpegMarkerHasNoSegment(marker)) return offset;
  return __krrAdvanceJpegSegment(bytes, marker, offset, state);
};
const __krrJpegBytesAreDecodable = (bytes) => {
  if (bytes.length < 4 || bytes[0] !== 255 || bytes[1] !== 216) return false;
  const state = {
    hasFrame: false,
    hasScan: false,
    quantizationTables: new Set(),
    huffmanTables: new Map(),
    frame: null,
    restartInterval: 0,
  };
  for (let offset = 2; offset < bytes.length; ) {
    const marker = __krrReadJpegMarker(bytes, offset);
    if (marker === null) return false;
    const nextOffset = __krrAdvanceJpeg(bytes, marker.marker, marker.offset, state);
    if (nextOffset === true) return true;
    if (nextOffset === null) return false;
    offset = nextOffset;
  }
  return false;
};
const __krrGifUint16 = (bytes, offset) => bytes[offset] | (bytes[offset + 1] << 8);
const __krrGifColorTableEnd = (bytes, offset, packed) => {
  if ((packed & 0x80) === 0) return offset;
  const end = offset + 3 * (1 << ((packed & 0x07) + 1));
  return end <= bytes.length ? end : null;
};
const __krrGifSkipSubBlocks = (bytes, offset, mustContainData) => {
  let sawData = false;
  while (offset < bytes.length) {
    const length = bytes[offset];
    offset += 1;
    if (length === 0) return !mustContainData || sawData ? offset : null;
    const end = offset + length;
    if (end > bytes.length) return null;
    sawData = true;
    offset = end;
  }
  return null;
};
const __krrGifScreen = (bytes) => {
  const startsWith = (prefix) => prefix.every((value, index) => bytes[index] === value);
  const supportedHeader =
    startsWith([71, 73, 70, 56, 55, 97]) || startsWith([71, 73, 70, 56, 57, 97]);
  if (!supportedHeader || bytes.length < 13) return null;
  const width = __krrGifUint16(bytes, 6);
  const height = __krrGifUint16(bytes, 8);
  if (width === 0 || height === 0) return null;
  const globalColorTable = (bytes[10] & 0x80) !== 0;
  const offset = __krrGifColorTableEnd(bytes, 13, bytes[10]);
  return offset === null ? null : { width, height, globalColorTable, offset };
};
const __krrGifImageEnd = (bytes, offset, screen) => {
  if (bytes.length - offset < 9) return null;
  const left = __krrGifUint16(bytes, offset);
  const top = __krrGifUint16(bytes, offset + 2);
  const width = __krrGifUint16(bytes, offset + 4);
  const height = __krrGifUint16(bytes, offset + 6);
  const packed = bytes[offset + 8];
  const fitsScreen = left + width <= screen.width && top + height <= screen.height;
  if (width === 0 || height === 0 || !fitsScreen || (!screen.globalColorTable && !(packed & 0x80)))
    return null;
  const dataStart = __krrGifColorTableEnd(bytes, offset + 9, packed);
  if (dataStart === null || dataStart >= bytes.length) return null;
  const lzwCodeSize = bytes[dataStart];
  if (lzwCodeSize < 2 || lzwCodeSize > 8) return null;
  return __krrGifSkipSubBlocks(bytes, dataStart + 1, true);
};
const __krrGifNextBlock = (bytes, offset, screen) => {
  const marker = bytes[offset];
  if (marker === 0x3b) return { complete: true, image: false, offset: offset + 1 };
  if (marker === 0x21) {
    if (offset + 1 >= bytes.length) return null;
    const nextOffset = __krrGifSkipSubBlocks(bytes, offset + 2, false);
    return nextOffset === null ? null : { complete: false, image: false, offset: nextOffset };
  }
  if (marker !== 0x2c) return null;
  const nextOffset = __krrGifImageEnd(bytes, offset + 1, screen);
  return nextOffset === null ? null : { complete: false, image: true, offset: nextOffset };
};
// GIF は logical-screen descriptor、色テーブル、画像descriptorとdata sub-blockを検証する。
const __krrGifBytesAreDecodable = (bytes) => {
  const screen = __krrGifScreen(bytes);
  if (screen === null) return false;
  let offset = screen.offset;
  let sawImage = false;
  while (offset < bytes.length) {
    const block = __krrGifNextBlock(bytes, offset, screen);
    if (block === null) return false;
    if (block.complete) return sawImage && block.offset === bytes.length;
    sawImage ||= block.image;
    offset = block.offset;
  }
  return false;
};
const __krrSvgLatin1Text = (bytes) => {
  const chunkSize = 8192;
  const chunks = [];
  for (let offset = 0; offset < bytes.length; offset += chunkSize)
    chunks.push(String.fromCharCode(...bytes.slice(offset, offset + chunkSize)));
  return chunks.join("");
};
const __krrSkipXmlWhitespace = (text, offset) => {
  while (offset < text.length && /[\t\n\r ]/.test(text[offset])) offset += 1;
  return offset;
};
const __krrXmlTerminatorEnd = (text, offset, terminator) => {
  const end = text.indexOf(terminator, offset);
  return end < 0 ? -1 : end + terminator.length;
};
const __krrSvgDoctypeEnd = (text, start) => {
  const matched = /^<!DOCTYPE[\t\n\r ]+(?:"[^"]*"|'[^']*'|\[[\s\S]*?\]|[^>])*>/i.exec(
    text.slice(start),
  );
  return matched === null ? -1 : start + matched[0].length;
};
const __krrSvgPreambleItem = (text, offset, sawDoctype) => {
  if (text.startsWith("<!--", offset))
    return { end: __krrXmlTerminatorEnd(text, offset, "-->"), doctype: false };
  if (text.startsWith("<?", offset))
    return { end: __krrXmlTerminatorEnd(text, offset, "?>"), doctype: false };
  if (!sawDoctype && text.slice(offset, offset + 9).toUpperCase() === "<!DOCTYPE") {
    if (!/[\t\n\r ]/.test(text[offset + 9])) return { end: -1, doctype: true };
    return { end: __krrSvgDoctypeEnd(text, offset), doctype: true };
  }
  return null;
};
const __krrSvgBytesAreDecodable = (bytes) => {
  const text = __krrSvgLatin1Text(bytes);
  let offset = bytes[0] === 239 && bytes[1] === 187 && bytes[2] === 191 ? 3 : 0;
  offset = __krrSkipXmlWhitespace(text, offset);
  if (text.slice(offset, offset + 5) === "<?xml") {
    if (!/[\t\n\r ]/.test(text[offset + 5])) return false;
    offset = __krrXmlTerminatorEnd(text, offset, "?>");
    if (offset < 0) return false;
    offset = __krrSkipXmlWhitespace(text, offset);
  }
  let sawDoctype = false;
  for (;;) {
    const item = __krrSvgPreambleItem(text, offset, sawDoctype);
    if (item === null) break;
    if (item.end < 0) return false;
    sawDoctype ||= item.doctype;
    offset = __krrSkipXmlWhitespace(text, item.end);
  }
  return /^<svg(?:[\t\n\r />])/.test(text.slice(offset));
};
const __krrImageBytesAreDecodable = (mediaType, bytes) => {
  if (mediaType === "image/gif") return __krrGifBytesAreDecodable(bytes);
  // JPEG は SOI/EOI だけでは破損データを見逃すため、フレームと走査を構造まで検証する。
  if (mediaType === "image/jpeg") return __krrJpegBytesAreDecodable(bytes);
  if (mediaType === "image/svg+xml") return __krrSvgBytesAreDecodable(bytes);
  return false;
};
const __krrPercentEncodedImageBytes = (payload) => {
  const bytes = [];
  for (let offset = 0; offset < payload.length; ) {
    const character = payload[offset];
    if (character === "%") {
      const encoded = payload.slice(offset + 1, offset + 3);
      if (!/^[0-9A-Fa-f]{2}$/.test(encoded)) return null;
      bytes.push(Number.parseInt(encoded, 16));
      offset += 3;
      continue;
    }
    const codePoint = payload.codePointAt(offset);
    if (codePoint === undefined || codePoint > 127) return null;
    bytes.push(codePoint);
    offset += 1;
  }
  return bytes;
};
const __krrImageLoadEventType = (_image, source) => {
  if (typeof source !== "string" || source.slice(0, "data:".length).toLowerCase() !== "data:")
    return "error";
  const separator = source.indexOf(",");
  if (separator < 0) return "error";
  const metadata = source.slice("data:".length, separator);
  const mediaType = metadata.split(";", 1)[0].trim().toLowerCase();
  if (!mediaType.startsWith("image/")) return "error";
  // GIF/JPEG/PNG/WebP/SVGは、static/interactiveで共通のRust完全デコーダへ委譲する。
  if (["image/gif", "image/jpeg", "image/png", "image/svg+xml", "image/webp"].includes(mediaType))
    return __krrNativeDom("validateImageDataUrl", source) === "load" ? "load" : "error";
  // 未対応の画像形式は、payloadをJSのnumber配列へ変換せず即時に拒否する。
  return "error";
};

const __krrDispatchImageLoad = async (image) => {
  let source = image.getAttribute("src");
  let lastDispatchedSource = null;
  // load/error handlerによるsrc差替えを再試行し、相互差替えでも有限回で打ち切る。
  for (let attempt = 0; source !== null && attempt < 8; attempt += 1) {
    const eventType = __krrImageLoadEventType(image, source);
    lastDispatchedSource = source;
    try {
      image.dispatchEvent(new Event(eventType));
    } catch (_error) {}
    await __krrLifecycleCheckpoint();
    const replacement = image.getAttribute("src");
    if (replacement === source) return lastDispatchedSource;
    source = replacement;
  }
  return lastDispatchedSource;
};

const __krrDispatchPendingImages = async (document, state = {}) => {
  const dispatchedImageSources = state.dispatchedImageSources || new Map();
  state.dispatchedImageSources = dispatchedImageSources;
  const maxImageDispatches = 1024;
  const maxImageRescans = 8;
  let emptyRescanCount = 0;
  let imageDispatches = state.imageDispatches || 0;
  let dispatched = false;
  for (
    let rescan = 0;
    rescan < maxImageRescans && emptyRescanCount < 2 && imageDispatches < maxImageDispatches;
    rescan += 1
  ) {
    let foundImage = false;
    for (const image of document.querySelectorAll("img")) {
      const source = image.getAttribute("src");
      if (dispatchedImageSources.get(image) === source) continue;
      foundImage = true;
      dispatched = true;
      const dispatchedSource = await __krrDispatchImageLoad(image);
      // 実dispatch済みsourceだけを記録し、上限時の差替え先は次のscanで評価する。
      dispatchedImageSources.set(image, dispatchedSource);
      await __krrLifecycleCheckpoint();
      // microtaskでsrcが差し替えられた場合は、次のscanで新しい値を再評価する。
      imageDispatches += 1;
      state.imageDispatches = imageDispatches;
      if (imageDispatches >= maxImageDispatches) {
        await __krrLifecycleCheckpoint();
        return dispatched;
      }
    }
    await __krrLifecycleCheckpoint();
    emptyRescanCount = foundImage ? 0 : emptyRescanCount + 1;
  }
  return dispatched;
};

const __krrDispatchLocalFrame = (frame) => {
  try {
    frame.dispatchEvent(new Event("load"));
  } catch (_error) {}
};

const __krrDispatchPendingLocalFrames = async (document, state = {}) => {
  const dispatchedFrames = state.dispatchedFrames || new Set();
  state.dispatchedFrames = dispatchedFrames;
  const maxFrameDispatches = 1024;
  const maxFrameRescans = 8;
  let emptyFrameRescanCount = 0;
  let frameDispatches = state.frameDispatches || 0;
  let dispatched = false;
  for (
    let rescan = 0;
    rescan < maxFrameRescans && emptyFrameRescanCount < 2 && frameDispatches < maxFrameDispatches;
    rescan += 1
  ) {
    let foundFrame = false;
    for (const frame of document.querySelectorAll("iframe")) {
      if (!frame.contentDocument || dispatchedFrames.has(frame)) continue;
      foundFrame = true;
      dispatched = true;
      dispatchedFrames.add(frame);
      __krrDispatchLocalFrame(frame);
      await __krrLifecycleCheckpoint();
      frameDispatches += 1;
      state.frameDispatches = frameDispatches;
      if (frameDispatches >= maxFrameDispatches) break;
    }
    await __krrLifecycleCheckpoint();
    emptyFrameRescanCount = foundFrame ? 0 : emptyFrameRescanCount + 1;
  }
  return dispatched;
};

globalThis.__krrDispatchWindowLoad = async () => {
  const body = document.body;
  if (body !== null) __krrRouteWindowLoadElement(body);
  else
    __krrRouteWindowLoadElement(__krrElement(__krrNativeDom("querySelector", "html > frameset")));
  const resourceDispatchState = {};
  const resourcePresence = String(__krrNativeDom("resourcePresence"));
  let [hasImages, hasFrames] = resourcePresence.includes(":")
    ? resourcePresence.split(":").map((value) => value === "1")
    : [document.querySelectorAll("img").length > 0, document.querySelectorAll("iframe").length > 0];
  for (let pass = 0; pass < 8; pass += 1) {
    const framesDispatched = hasFrames
      ? await __krrDispatchPendingLocalFrames(document, resourceDispatchState)
      : false;
    const imagesDispatched = hasImages
      ? await __krrDispatchPendingImages(document, resourceDispatchState)
      : false;
    if (!framesDispatched && !imagesDispatched) break;
    const updatedPresence = String(__krrNativeDom("resourcePresence"));
    [hasImages, hasFrames] = updatedPresence.includes(":")
      ? updatedPresence.split(":").map((value) => value === "1")
      : [
          document.querySelectorAll("img").length > 0,
          document.querySelectorAll("iframe").length > 0,
        ];
    await __krrLifecycleCheckpoint();
  }
  __krrDocumentReadyState = "complete";
  __krrDispatchDocumentReadyStateChange();
  __krrDispatchElementReadyStateChange();
  window.dispatchEvent(new Event("load"));
};
