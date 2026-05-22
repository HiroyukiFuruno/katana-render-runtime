function katanaCssComputedStyleValue(node, name) {
  if (!node) {
    return "";
  }
  return katanaCssRuleValue(katanaCssText(node), node, name);
}

function katanaCssText(node) {
  const root = katanaCssRoot(node);
  return katanaCssStyleNodes(root)
    .map((styleNode) => styleNode.textContent)
    .join("\n");
}

function katanaCssRoot(node) {
  return katanaCssRootNode(node) || document.documentElement;
}

function katanaCssRootNode(node) {
  return KATANA_CSS_ROOT_READERS[Number(Boolean(katanaCssParentNode(node)))](node);
}

function katanaCssParentNode(node) {
  return node?.parentNode || null;
}

function katanaCssStyleNodes(root) {
  return katanaCssDescendantNodes(root).filter(katanaIsCssStyleNode);
}

function katanaCssDescendantNodes(node) {
  return [node].filter(Boolean).flatMap(katanaCssNodeWithDescendants);
}

function katanaCssNodeWithDescendants(node) {
  return [node].concat(Array.from(node.children || []).flatMap(katanaCssNodeWithDescendants));
}

function katanaIsCssStyleNode(node) {
  return node.localName === "style";
}

function katanaCssRuleValue(cssText, node, name) {
  return (
    Array.from(cssText.matchAll(/([^{}]+)\{([^{}]+)\}/g))
      .map((rule) => katanaCssRuleDeclarationValue(rule, node, name))
      .filter(Boolean)
      .at(-1) ?? ""
  );
}

function katanaCssRuleDeclarationValue(rule, node, name) {
  return (
    [rule]
      .filter((it) => katanaCssRuleMatchesNode(it[1], node))
      .map((it) => katanaCssDeclarationValue(it[2], name))[0] ?? ""
  );
}

const KATANA_CSS_ROOT_READERS = [
  (node) => node,
  (node) => katanaCssRootNode(katanaCssParentNode(node)),
];

function katanaCssRuleMatchesNode(selectorText, node) {
  return selectorText
    .split(",")
    .some((selector) => katanaCssSelectorMatchesNode(node, selector));
}

function katanaCssSelectorMatchesNode(node, selector) {
  const parts = String(selector).trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) {
    return false;
  }
  return katanaCssSelectorTailMatches(node, parts, parts.length - 1);
}

function katanaCssSelectorTailMatches(node, parts, index) {
  if (!node || !katanaCssSimpleSelectorMatches(node, parts[index])) {
    return false;
  }
  if (index === 0) {
    return true;
  }
  return katanaCssAncestorMatches(node.parentNode, parts, index - 1);
}

function katanaCssAncestorMatches(node, parts, index) {
  if (!node) {
    return false;
  }
  if (katanaCssSelectorTailMatches(node, parts, index)) {
    return true;
  }
  return katanaCssAncestorMatches(node.parentNode, parts, index);
}

function katanaCssSimpleSelectorMatches(node, selector) {
  const normalized = katanaCssSimpleSelector(selector);
  if (normalized === "*") {
    return true;
  }
  if (normalized === ":root") {
    return node === document.documentElement;
  }
  return [
    katanaCssSelectorTagMatches(node, normalized),
    katanaCssSelectorIdMatches(node, normalized),
    katanaCssSelectorClassesMatch(node, normalized),
    katanaCssSelectorAttributesMatch(node, normalized),
  ].every(Boolean);
}

function katanaCssSimpleSelector(selector) {
  return String(selector).trim().replace(/:{1,2}(?!root\b)[a-zA-Z-]+(?:\([^)]*\))?/g, "");
}

function katanaCssSelectorTagMatches(node, selector) {
  const tag = selector.match(/^[a-zA-Z][a-zA-Z0-9:_-]*/)?.[0];
  return tag ? node.localName === tag.toLowerCase() : true;
}

function katanaCssSelectorIdMatches(node, selector) {
  const id = selector.match(/#([a-zA-Z0-9_-]+)/)?.[1];
  return id ? node.id === id : true;
}

function katanaCssSelectorClassesMatch(node, selector) {
  const classNames = String(node.className ?? "").split(/\s+/).filter(Boolean);
  return Array.from(selector.matchAll(/\.([a-zA-Z0-9_-]+)/g))
    .map((match) => match[1])
    .every((className) => classNames.includes(className));
}

function katanaCssSelectorAttributesMatch(node, selector) {
  return Array.from(selector.matchAll(/\[([a-zA-Z0-9:_-]+)(?:="([^"]*)")?\]/g))
    .map((match) => katanaCssSelectorAttributeMatches(node, match[1], match[2]))
    .every(Boolean);
}

function katanaCssSelectorAttributeMatches(node, name, value) {
  const actual = node.getAttribute?.(name);
  return value === undefined ? actual !== null : actual === value;
}

function katanaCssDeclarationValue(declarations, name) {
  return (
    String(declarations)
      .split(";")
      .map((declaration) => katanaCssDeclarationEntry(declaration))
      .find((entry) => entry.name === name)?.value ?? ""
  );
}

function katanaCssDeclarationEntry(declaration) {
  const parts = String(declaration).split(":");
  return {
    name: String(parts.shift() ?? "").trim(),
    value: parts.join(":").trim(),
  };
}
