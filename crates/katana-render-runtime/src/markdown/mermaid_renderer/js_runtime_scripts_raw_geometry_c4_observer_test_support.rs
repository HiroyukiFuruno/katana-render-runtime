pub(super) const SOURCE: &str = r#"const c4GeometryObservations = [];
const c4GeometryTraceLimit = 16;
let c4GeometryObserverEnabled = false;
let c4GeometryHookCalls = {getBBox: 0, getBoundingClientRect: 0};
const c4GeometryHookNodes = [];
let c4GeometryCallDepth = 0;
let c4GeometryPrototype;
let c4GetBBoxHook;
let c4GetBoundingClientRectHook;
if (/^C4(?:Context|Container|Component|Dynamic|Deployment)\b/.test(String(request.source ?? ''))) {
  c4GeometryObserverEnabled = true;
  const svgProbe = globalThis.document?.createElementNS?.(
    'http://www.w3.org/2000/svg',
    'svg',
  );
  const elementPrototype = svgProbe && Object.getPrototypeOf(svgProbe);
  const originalGetBBox = elementPrototype?.getBBox;
  const originalGetBoundingClientRect = elementPrototype?.getBoundingClientRect;
  if (typeof originalGetBBox !== 'function' || typeof originalGetBoundingClientRect !== 'function')
    throw new Error('C4 geometry observers absent');
  const isObserved = (node) =>
    (node.localName === 'svg' && node.getAttribute('id') === svgId) ||
    (node.localName === 'line' && node.hasAttribute('x1') && node.hasAttribute('y1')) ||
    ['text', 'g', 'rect', 'circle'].includes(node.localName);
  const nodeDescriptor = (node) => {
    const text = String(node.textContent ?? '');
    const computed = globalThis.getComputedStyle?.(node);
    return {
      tagName: node.tagName,
      localName: node.localName,
      id: node.getAttribute('id'),
      text: text.slice(0, 120),
      textTruncated: text.length > 120,
      x1: node.getAttribute('x1'),
      y1: node.getAttribute('y1'),
      x2: node.getAttribute('x2'),
      y2: node.getAttribute('y2'),
      fontAttributes: Object.fromEntries(
        ['font-family', 'font-size', 'font-weight', 'font-style', 'style'].map(name => [
          name,
          node.getAttribute(name),
        ]),
      ),
      computedFont: computed
        ? {
            family: computed.fontFamily ?? computed.getPropertyValue('font-family'),
            size: computed.fontSize ?? computed.getPropertyValue('font-size'),
            weight: computed.fontWeight ?? computed.getPropertyValue('font-weight'),
            style: computed.getPropertyValue('font-style'),
          }
        : null,
    };
  };
  const boxDescriptor = (box) => ({
    x: box.x,
    y: box.y,
    width: box.width,
    height: box.height,
  });
  c4GeometryPrototype = elementPrototype;
  const observeGeometryCall = (kind, original) => function(...args) {
    const depth = c4GeometryCallDepth;
    c4GeometryHookCalls[kind] += 1;
    c4GeometryCallDepth += 1;
    try {
      const box = original.apply(this, args);
      if (c4GeometryHookNodes.length < c4GeometryTraceLimit)
        c4GeometryHookNodes.push({kind, node: {...nodeDescriptor(this), depth}});
      if (depth === 0 && isObserved(this) && c4GeometryObservations.length < c4GeometryTraceLimit)
        c4GeometryObservations.push({kind, node: nodeDescriptor(this), box: boxDescriptor(box)});
      return box;
    } finally {
      c4GeometryCallDepth -= 1;
    }
  };
  c4GetBBoxHook = observeGeometryCall('getBBox', originalGetBBox);
  c4GetBoundingClientRectHook = observeGeometryCall(
    'getBoundingClientRect',
    originalGetBoundingClientRect,
  );
  elementPrototype.getBBox = c4GetBBoxHook;
  elementPrototype.getBoundingClientRect = c4GetBoundingClientRectHook;
}"#;
