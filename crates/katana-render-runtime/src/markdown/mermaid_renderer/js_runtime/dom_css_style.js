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
  return katanaCssRuleDeclaration(cssText, node, name)?.value ?? "";
}

function katanaCssRuleDeclaration(cssText, node, name) {
  let selected;
  for (const rule of cssText.matchAll(/([^{}]+)\{([^{}]+)\}/g)) {
    if (!katanaCssRuleMatchesNode(rule[1], node)) continue;
    const declaration = katanaCssDeclaration(rule[2], name);
    if (declaration && (declaration.priority || !selected?.priority)) selected = declaration;
  }
  return selected;
}

function katanaCascadedStyleValue(node, name, cssText) {
  const inline = node?.style?.getPropertyValue?.(name) ?? "";
  if (inline && node?.style?.getPropertyPriority?.(name) === "important") return inline;
  const declaration = katanaCssRuleDeclaration(cssText, node, name);
  if (declaration?.priority && node?.style?.getPropertyPriority?.(name) !== "important") {
    return declaration.value;
  }
  return inline || declaration?.value || "";
}

const KATANA_CSS_ROOT_READERS = [
  (node) => node,
  (node) => katanaCssRootNode(katanaCssParentNode(node)),
];

function katanaCssRuleMatchesNode(selectorText, node) {
  return selectorText.split(",").some((selector) => katanaCssSelectorMatchesNode(node, selector));
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
  return String(selector)
    .trim()
    .replace(/:{1,2}(?!root\b)[a-zA-Z-]+(?:\([^)]*\))?/g, "");
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
  const classNames = String(node.className ?? "")
    .split(/\s+/)
    .filter(Boolean);
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

function katanaCssDeclaration(declarations, name) {
  let selected;
  for (const entry of katanaCssEntries(declarations)) {
    if (entry.name !== name) continue;
    if (entry.priority || !selected?.priority) selected = entry;
  }
  return selected;
}

function katanaOwnCustomPropertyValue(node, name, css) {
  const inline = node?.style?.getPropertyValue?.(name) ?? "";
  const priority = node?.style?.getPropertyPriority?.(name);
  if (inline && priority === "important") return inline;
  const declaration = katanaCssRuleDeclaration(css, node, name);
  if (declaration?.priority && priority !== "important") return declaration.value;
  return inline || declaration?.value;
}

function katanaCloseCssVariableComponent(state, property, references, stack, active) {
  const component = [];
  let member;
  do {
    member = stack.pop();
    active.delete(member);
    state.checked.add(member);
    component.push(member);
  } while (member !== property);
  if (component.length > 1 || references?.includes(property)) {
    for (const cyclic of component) state.cyclic.add(cyclic);
  }
}

function katanaVisitCssVariableGraph(
  state,
  element,
  css,
  property,
  indices,
  lowLinks,
  stack,
  active,
) {
  const index = indices.size;
  indices.set(property, index);
  lowLinks.set(property, index);
  stack.push(property);
  active.add(property);
  const specified = katanaOwnCustomPropertyValue(element, property, css);
  state.declarations.set(property, specified);
  const keyword = globalThis.katanaCssWideKeyword(specified);
  const references =
    specified === undefined ||
    ["initial", "inherit", "unset", "revert", "revert-layer"].includes(keyword)
      ? []
      : globalThis.katanaCssVariableReferences(specified);
  state.dependencies.set(property, references);
  for (const dependency of references ?? []) {
    if (state.checked.has(dependency)) continue;
    if (!indices.has(dependency)) {
      katanaVisitCssVariableGraph(
        state,
        element,
        css,
        dependency,
        indices,
        lowLinks,
        stack,
        active,
      );
      lowLinks.set(property, Math.min(lowLinks.get(property), lowLinks.get(dependency)));
    } else if (active.has(dependency)) {
      lowLinks.set(property, Math.min(lowLinks.get(property), indices.get(dependency)));
    }
  }
  if (lowLinks.get(property) !== index) return;
  katanaCloseCssVariableComponent(state, property, references, stack, active);
}

function katanaResolvedCssVariableValue(node, value, css) {
  const states = new Map();
  const read = (element, name) => {
    if (!element) return undefined;
    let state = states.get(element);
    if (!state) {
      state = {
        values: new Map(),
        declarations: new Map(),
        dependencies: new Map(),
        checked: new Set(),
        cyclic: new Set(),
      };
      states.set(element, state);
    }
    if (state.values.has(name)) return state.values.get(name);
    if (!state.checked.has(name)) {
      /* WHY: 値を確定する前に強連結成分全体を無効化し、重なる循環を途中のcacheで切らない。 */
      const indices = new Map();
      const lowLinks = new Map();
      const stack = [];
      const active = new Set();
      katanaVisitCssVariableGraph(state, element, css, name, indices, lowLinks, stack, active);
    }
    const specified = state.declarations.get(name);
    const keyword = globalThis.katanaCssWideKeyword(specified);
    if (
      specified === undefined ||
      ["inherit", "unset", "revert", "revert-layer"].includes(keyword)
    ) {
      const inherited = read(element.parentNode, name);
      state.values.set(name, inherited);
      return inherited;
    }
    const computed =
      keyword === "initial" || state.cyclic.has(name) || !state.dependencies.get(name)
        ? undefined
        : globalThis.katanaSubstituteCssVariables(specified, (dependency) =>
            read(element, dependency),
          );
    state.values.set(name, computed);
    return computed;
  };
  return globalThis.katanaSubstituteCssVariables(value, (name) => read(node, name));
}

function katanaResolvedTextFamilyValue(node, value, css) {
  if (!/var\s*\(/i.test(value) && !value.includes("\\")) return value;
  /* WHY: custom property は宣言された要素で計算してから継承し、子の変数で親の値を再解釈しない。 */
  const resolved = katanaResolvedCssVariableValue(node, value, css);
  if (resolved === undefined || !globalThis.katanaValidCssFontFamily(resolved)) return "";
  return globalThis.katanaCssWideKeyword(resolved) ?? resolved;
}

function katanaResolvedNodeTextFamily(node) {
  const chain = [];
  for (let current = node; current; current = current.parentNode) chain.push(current);
  const css = katanaCssText(node);
  return chain
    .reverse()
    .reduce(
      (family, element) =>
        katanaInheritedTextValue(
          katanaResolvedTextFamilyValue(
            element,
            katanaOwnTextFontValue(element, "font-family", css),
            css,
          ),
          family,
          KATANA_DEFAULT_TEXT_FONT.family,
        ),
      KATANA_DEFAULT_TEXT_FONT.family,
    );
}
