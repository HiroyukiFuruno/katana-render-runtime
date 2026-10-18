/** @param {string} text @param {RegExp} pattern @param {(group: string) => string} transform @returns {string} */
function katanaRewriteBalancedGroups(text, pattern, transform) {
  const state = { cursor: 0, result: "" };
  let match = pattern.exec(text);
  while (match !== null) {
    katanaAppendBalancedGroup(state, text, pattern, match, transform);
    match = pattern.exec(text);
  }
  return state.result + text.slice(state.cursor);
}

/** @param {KatanaBalancedRewriteState} state @param {string} text @param {RegExp} pattern @param {RegExpExecArray} match @param {(group: string) => string} transform @returns {void} */
function katanaAppendBalancedGroup(state, text, pattern, match, transform) {
  const start = match.index;
  const end = katanaFindBalancedGroupEnd(text, start);
  if (end < 0) {
    return;
  }
  state.result += text.slice(state.cursor, start);
  state.result += transform(text.slice(start, end));
  state.cursor = end;
  pattern.lastIndex = end;
}

/** @param {string} text @param {number} start @returns {number} */
function katanaFindBalancedGroupEnd(text, start) {
  const pattern = /<\/?g\b[^>]*>/g;
  const state = { depth: 0, end: -1 };
  pattern.lastIndex = start;
  let match = pattern.exec(text);
  while (match !== null) {
    katanaApplyGroupDepth(state, match, pattern);
    match = katanaNextGroupMatch(text, pattern, state);
  }
  return state.end;
}

/** @param {KatanaBalancedGroupDepthState} state @param {RegExpExecArray} match @param {RegExp} pattern @returns {void} */
function katanaApplyGroupDepth(state, match, pattern) {
  state.depth += katanaGroupDepthDelta(match[0]);
  katanaCaptureGroupEnd(state, pattern);
}

/** @param {string} tag @returns {number} */
function katanaGroupDepthDelta(tag) {
  if (tag.startsWith("</")) {
    return -1;
  }
  return 1;
}

/** @param {KatanaBalancedGroupDepthState} state @param {RegExp} pattern @returns {void} */
function katanaCaptureGroupEnd(state, pattern) {
  if (state.depth === 0) {
    state.end = pattern.lastIndex;
  }
}

/** @param {string} text @param {RegExp} pattern @param {KatanaBalancedGroupDepthState} state @returns {RegExpExecArray | null} */
function katanaNextGroupMatch(text, pattern, state) {
  if (state.end >= 0) {
    return null;
  }
  return pattern.exec(text);
}

/** @typedef {{cursor: number, result: string}} KatanaBalancedRewriteState */
/** @typedef {{depth: number, end: number}} KatanaBalancedGroupDepthState */
