function matchesSelector(node, selector) {
  const matcher = KATANA_SELECTOR_MATCHERS.find((entry) => entry.applies(selector));
  if (matcher) {
    return matcher.matches(node, selector);
  }
  return katanaMatchesTag(node, selector);
}

const KATANA_SELECTOR_MATCHERS = [
  { applies: katanaIsBareFirstChildSelector, matches: katanaMatchesBareFirstChild },
  { applies: katanaIsNotFirstChildSelector, matches: katanaMatchesNotFirstChild },
  { applies: katanaIsNestedFirstChildSelector, matches: katanaMatchesNestedFirstChild },
  { applies: katanaIsWildcardSelector, matches: katanaMatchesWildcard },
  { applies: katanaIsTagIdSelector, matches: katanaMatchesTagId },
  { applies: katanaIsTagClassSelector, matches: katanaMatchesTagClass },
  { applies: katanaIsBodySelector, matches: katanaMatchesBody },
  { applies: katanaIsIdSelector, matches: katanaMatchesId },
  { applies: katanaIsIdAttributeSelector, matches: katanaMatchesIdAttribute },
  { applies: katanaIsClassSelector, matches: katanaMatchesClass },
];

function katanaIsBareFirstChildSelector(selector) {
  return [":first-child", "::first-child"].includes(selector);
}

function katanaMatchesBareFirstChild(node) {
  return node.parentNode?.firstElementChild === node;
}

function katanaIsNotFirstChildSelector(selector) {
  return /^[a-zA-Z0-9:_-]+:not\(:first-child\)$/.test(selector);
}

function katanaMatchesNotFirstChild(node, selector) {
  const base = selector.slice(0, -":not(:first-child)".length);
  return [matchesSelector(node, base), !katanaMatchesBareFirstChild(node)].every(Boolean);
}

function katanaIsNestedFirstChildSelector(selector) {
  return selector.endsWith(":first-child");
}

function katanaMatchesNestedFirstChild(node, selector) {
  const base = selector.replace(/:{1,2}first-child$/, "");
  return [matchesSelector(node, base), katanaMatchesBareFirstChild(node)].every(Boolean);
}

function katanaIsWildcardSelector(selector) {
  return selector === "*";
}

function katanaMatchesWildcard(node) {
  return node.nodeType === Node.ELEMENT_NODE;
}

function katanaIsTagIdSelector(selector) {
  return /^([a-zA-Z0-9:_-]+)#(.+)$/.test(selector);
}

function katanaMatchesTagId(node, selector) {
  const match = selector.match(/^([a-zA-Z0-9:_-]+)#(.+)$/);
  return [node.localName === match[1].toLowerCase(), node.id === match[2]].every(Boolean);
}

function katanaIsTagClassSelector(selector) {
  return /^[a-zA-Z0-9:_-]+(?:\.[a-zA-Z0-9_-]+)+$/.test(selector);
}

function katanaMatchesTagClass(node, selector) {
  const [tagName, ...classNames] = selector.split(".");
  const actualClasses = String(node.className).split(/\s+/).filter(Boolean);
  return [
    node.localName === tagName.toLowerCase(),
    classNames.every((className) => actualClasses.includes(className)),
  ].every(Boolean);
}

function katanaIsBodySelector(selector) {
  return selector === "body";
}

function katanaMatchesBody(node) {
  return node.localName === "body";
}

function katanaIsIdSelector(selector) {
  return selector.startsWith("#");
}

function katanaMatchesId(node, selector) {
  return node.id === selector.slice(1);
}

function katanaIsIdAttributeSelector(selector) {
  return /^\[id="([^"]+)"\]$/.test(selector);
}

function katanaMatchesIdAttribute(node, selector) {
  return node.id === selector.match(/^\[id="([^"]+)"\]$/)[1];
}

function katanaIsClassSelector(selector) {
  return selector.startsWith(".");
}

function katanaMatchesClass(node, selector) {
  return String(node.className).split(/\s+/).includes(selector.slice(1));
}

function katanaMatchesTag(node, selector) {
  return node.localName === selector.toLowerCase();
}

function queryCompoundSelector(root, selector) {
  const alternatives = splitSelectorList(selector);
  if (alternatives.length > 1) {
    return querySelectorAlternatives(root, alternatives);
  }
  const parts = (alternatives[0] ?? selector).split(/\s+/).filter(Boolean);
  if (parts.length <= 1) {
    return null;
  }
  return katanaQuerySelectorParts([root], parts);
}

function querySelectorAlternatives(root, alternatives) {
  const selected = new Set(
    alternatives.flatMap((alternative) => querySelectorAlternative(root, alternative)),
  );
  const ordered = [];
  const visit = (node) => {
    node.children.forEach((child) => {
      if (selected.has(child)) ordered.push(child);
      visit(child);
    });
  };
  visit(root);
  return ordered;
}

function splitSelectorList(selector) {
  const alternatives = [];
  const state = { start: 0, quote: "", brackets: 0, parentheses: 0, escaped: false };
  for (let index = 0; index < selector.length; index += 1) {
    if (advanceSelectorScan(state, selector[index])) {
      const alternative = selector.slice(state.start, index).trim();
      if (alternative) alternatives.push(alternative);
      state.start = index + 1;
    }
  }
  const finalAlternative = selector.slice(state.start).trim();
  if (finalAlternative) alternatives.push(finalAlternative);
  return alternatives;
}

function advanceSelectorScan(state, character) {
  if (state.escaped) {
    state.escaped = false;
    return false;
  }
  if (character === "\\") {
    state.escaped = true;
    return false;
  }
  if (state.quote) {
    if (character === state.quote) state.quote = "";
    return false;
  }
  if (character === '"' || character === "'") {
    state.quote = character;
    return false;
  }
  if (character === "[") state.brackets += 1;
  if (character === "]") state.brackets = Math.max(0, state.brackets - 1);
  if (character === "(") state.parentheses += 1;
  if (character === ")") state.parentheses = Math.max(0, state.parentheses - 1);
  return character === "," && state.brackets === 0 && state.parentheses === 0;
}

function querySelectorAlternative(root, selector) {
  const parts = selector.split(/\s+/).filter(Boolean);
  if (parts.length <= 1) {
    return querySimpleSelector(root, selector);
  }
  return katanaQuerySelectorParts([root], parts);
}

function katanaQuerySelectorParts(candidates, parts) {
  return parts.reduce(
    (current, part) => current.flatMap((candidate) => querySimpleSelector(candidate, part)),
    candidates,
  );
}

function querySimpleSelector(root, selector) {
  const results = [];
  root.children.forEach((node) => {
    katanaVisitSelector(node, selector, results);
  });
  return results;
}

function katanaVisitSelector(node, selector, results) {
  katanaAddMatchingSelector(node, selector, results);
  node.children.forEach((child) => {
    katanaVisitSelector(child, selector, results);
  });
}

function katanaAddMatchingSelector(node, selector, results) {
  if (matchesSelector(node, selector)) {
    results.push(node);
  }
}
