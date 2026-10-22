function parseInnerHtml(
  source,
  xmlMode = false,
  contextNamespace = null,
  contextLocalName = null,
) {
  const root = {
    localName: contextLocalName ?? "#root",
    namespaceURI: contextNamespace,
    children: [],
    appendChild(child) {
      this.children.push(child);
    },
  };
  const stack = [root];
  const tokenRegex = /<\/?([a-zA-Z0-9:_-]+)([^>]*)>|([^<]+)/g;
  const markup = katanaHtmlWithoutComments(source);
  Array.from(markup.matchAll(tokenRegex)).forEach((match) => {
    appendHtmlToken(stack, match, xmlMode);
  });
  return root.children;
}

function katanaHtmlWithoutComments(source) {
  return String(source)
    .replace(/<!-->/g, "")
    .replace(/<!--[\s\S]*?-->/g, "");
}

function appendHtmlToken(stack, match, xmlMode) {
  if (match[3] !== undefined) {
    appendHtmlText(stack[stack.length - 1], match[3]);
    return;
  }
  appendHtmlTag(stack, match, xmlMode);
}

function appendHtmlTag(stack, match, xmlMode) {
  if (match[0].startsWith("</")) {
    popHtmlStack(stack, match[1]);
    return;
  }
  appendHtmlStartTag(stack, match, xmlMode);
}

function appendHtmlStartTag(stack, match, xmlMode) {
  const parent = stack[stack.length - 1];
  const node = new KatanaNode(match[1], katanaHtmlNamespace(xmlMode, parent, match[1]));
  node.ownerDocument = document;
  parseAttributes(match[2]).forEach(([name, value]) => {
    node.setAttribute(name, value);
  });
  stack[stack.length - 1].appendChild(node);
  pushHtmlElementIfOpen(stack, node, match[0], match[1]);
}

function katanaHtmlNamespace(xmlMode, parent, tagName) {
  if (xmlMode) return "katana-xml";
  const normalizedTagName = String(tagName).toLowerCase();
  if (normalizedTagName === "svg") return KATANA_SVG_NAMESPACE;
  if (parent?.namespaceURI === KATANA_SVG_NAMESPACE) {
    return parent.localName === "foreignobject" ? KATANA_HTML_NAMESPACE : KATANA_SVG_NAMESPACE;
  }
  return KATANA_HTML_NAMESPACE;
}

const KATANA_SVG_NAMESPACE = "http://www.w3.org/2000/svg";

function pushHtmlElementIfOpen(stack, node, fullTag, tagName) {
  if (katanaIsOpenHtmlTag(fullTag, tagName)) {
    stack.push(node);
  }
}

function katanaIsOpenHtmlTag(fullTag, tagName) {
  return [!fullTag.endsWith("/>"), !isHtmlVoidTag(tagName)].every(Boolean);
}

function appendHtmlText(parent, value) {
  const text = decodeHtmlEntities(value);
  if (text.length > 0) {
    parent.appendChild(new KatanaTextNode(text));
  }
}

function popHtmlStack(stack, tagName) {
  const normalized = String(tagName).toLowerCase();
  const index = stack.findLastIndex((node) => node.localName === normalized);
  if (index > 0) {
    stack.splice(index);
  }
}

function isHtmlVoidTag(tagName) {
  return new Set(["br", "hr", "img", "input", "meta", "link"]).has(String(tagName).toLowerCase());
}

function parseAttributes(source) {
  const attrRegex = /([a-zA-Z0-9:_-]+)="([^"]*)"/g;
  return Array.from(source.matchAll(attrRegex)).map((match) => [
    match[1],
    decodeHtmlEntities(match[2]),
  ]);
}

function decodeHtmlEntities(value) {
  return katanaDecodeRemainingHtmlEntities(value);
}

function katanaDecodeRemainingHtmlEntities(value) {
  return String(value).replace(
    /&(?:#x([0-9a-f]+)|#([0-9]+)|([a-z][a-z0-9]+));/gi,
    (match, hexCodePoint, decimalCodePoint, name) => {
      if (hexCodePoint !== undefined) {
        return katanaHtmlCodePoint(hexCodePoint, 16, match);
      }
      if (decimalCodePoint !== undefined) {
        return katanaHtmlCodePoint(decimalCodePoint, 10, match);
      }
      return Object.hasOwn(KATANA_HTML_NAMED_ENTITIES, name)
        ? KATANA_HTML_NAMED_ENTITIES[name]
        : match;
    },
  );
}

function katanaHtmlCodePoint(value, radix, fallback) {
  const codePoint = Number.parseInt(value, radix);
  return Number.isInteger(codePoint) && codePoint >= 0 && codePoint <= 0x10ffff
    ? String.fromCodePoint(codePoint)
    : fallback;
}

const KATANA_HTML_NAMED_ENTITIES = {
  amp: "&",
  gt: ">",
  lt: "<",
  quot: '"',
  Igrave: "\u00CC",
  Ugrave: "\u00D9",
  Zeta: "\u0396",
  agrave: "\u00E0",
  alpha: "\u03B1",
  auml: "\u00E4",
  beta: "\u03B2",
  ccedil: "\u00E7",
  chi: "\u03C7",
  copy: "\u00A9",
  delta: "\u03B4",
  egrave: "\u00E8",
  epsilon: "\u03B5",
  gamma: "\u03B3",
  hellip: "\u2026",
  iota: "\u03B9",
  kappa: "\u03BA",
  lambda: "\u03BB",
  mdash: "\u2014",
  mu: "\u03BC",
  ndash: "\u2013",
  nbsp: "\u00A0",
  nu: "\u03BD",
  ograve: "\u00F2",
  omicron: "\u03BF",
  ouml: "\u00F6",
  phi: "\u03C6",
  pi: "\u03C0",
  psi: "\u03C8",
  reg: "\u00AE",
  rho: "\u03C1",
  rlm: "\u200F",
  sigma: "\u03C3",
  sigmaf: "\u03C2",
  szlig: "\u00DF",
  tau: "\u03C4",
  theta: "\u03B8",
  trade: "\u2122",
  upsilon: "\u03C5",
  uuml: "\u00FC",
  xi: "\u03BE",
  zwnj: "\u200C",
};

class KatanaTextNode extends KatanaNode {
  constructor(value) {
    super("#text");
    this.textContent = String(value);
  }
  get outerHTML() {
    return katanaEscapeSvgText(this.textContent);
  }
}

class KatanaCommentNode extends KatanaNode {
  constructor(value) {
    super("#comment");
    this.nodeType = 8;
    this.textContent = String(value);
  }
  get outerHTML() {
    return "";
  }
}
