(() => {
  const nativeDom = __krrNativeDom;
  const target = window;
  const pageGlobal = globalThis;
  const element = __krrElement;
  const compile = Function;
  const normalize = String;
  const apply = Reflect.apply;
  const uncurry = <Receiver, Args extends unknown[], Result>(
    method: (this: Receiver, ...args: Args) => Result,
  ): ((receiver: Receiver, ...args: Args) => Result) => Function.prototype.call.bind(method);
  const weakGet: KrrWeakGet = uncurry(WeakMap.prototype.get);
  const weakSet: KrrWeakSet = uncurry(WeakMap.prototype.set);
  const mapGet: KrrMapGet = uncurry(Map.prototype.get);
  const mapSet: KrrMapSet = uncurry(Map.prototype.set);
  const mapDelete: KrrMapDelete = uncurry(Map.prototype.delete);
  const setDelete: KrrSetDelete = uncurry(Set.prototype.delete);
  const MapConstructor = Map;
  const initialId =
    nativeDom("querySelector", "body") ?? nativeDom("querySelector", "html > frameset");
  if (initialId !== null) element(initialId);
  const store = (handler: KrrLoadHandler | null) => {
    let handlers = weakGet(__krrEventHandlers, target);
    if (!handlers) {
      handlers = new MapConstructor<string, KrrLoadHandler>();
      weakSet(__krrEventHandlers, target, handlers);
    }
    if (handler === null) mapDelete(handlers, "load");
    else mapSet(handlers, "load", handler);
  };
  const install = (body: KrrElement | null, source: string | null | undefined) => {
    const overrides = weakGet(__krrLifecyclePropertyOverrides, pageGlobal);
    if (overrides) setDelete(overrides, "load");
    if (source === null || source === undefined) {
      store(null);
      return;
    }
    try {
      const handler = compile("event", normalize(source));
      store((event) => apply(handler, body, [event]));
    } catch (_error) {
      store(null);
    }
  };
  return (parserSource: string | undefined, later: boolean) => {
    const nodeId =
      nativeDom("querySelector", "body") ?? nativeDom("querySelector", "html > frameset");
    if (nodeId === null) return;
    const body = mapGet(__krrElements, normalize(nodeId)) ?? element(nodeId);
    const currentSource = nativeDom("getAttribute", nodeId, "onload");
    if (parserSource === undefined) install(body, currentSource);
    else if (currentSource === null) {
      nativeDom("setAttribute", nodeId, "onload", parserSource);
      install(body, parserSource);
    } else if (!later && currentSource === parserSource) install(body, currentSource);
  };
})();
