import generateCss from "css-tree/generator";
import { Lexer } from "css-tree/lexer";
import parseCss from "css-tree/parser";
import { clone, ident } from "css-tree/utils";
import walkCss from "css-tree/walker";

/** @type {Map<string, import("css-tree").Value | undefined>} */
const valueSyntaxCache = new Map();
const valueSyntaxCapacity = 256;
/** @type {WeakMap<import("css-tree").Value, boolean>} */
const fontFamilyValidity = new WeakMap();
/** @type {import("css-tree").Lexer | undefined} */
let fontFamilyLexer;

/** @param {import("css-tree").FunctionNode} node */
function variableArguments(node) {
  const args = node.children.toArray();
  if (args[0]?.type !== "Identifier") return undefined;
  const name = ident.decode(args[0].name);
  if (!name.startsWith("--") || name === "--") return undefined;
  if (args.length === 1) return { name, fallback: undefined };
  if (args[1]?.type !== "Operator" || args[1].value !== ",") return undefined;
  const fallback = clone(node);
  Reflect.set(fallback, "type", "Value");
  if (fallback.type !== "Value") throw new TypeError("CSS fallback must be a value");
  fallback.children.shift();
  fallback.children.shift();
  return { name, fallback: generateCss(fallback) };
}

/** @param {import("css-tree").Value} value */
function cloneValue(value) {
  const cloned = clone(value);
  if (cloned.type !== "Value") throw new TypeError("Cloned CSS must be a value");
  return cloned;
}

/** @param {string} value
 * @returns {import("css-tree").Value | undefined} */
function parseValue(value) {
  if (valueSyntaxCache.has(value)) {
    const cached = valueSyntaxCache.get(value);
    valueSyntaxCache.delete(value);
    valueSyntaxCache.set(value, cached);
    return cached;
  }
  let parsed;
  try {
    const syntax = parseCss(value, { context: "value" });
    if (syntax.type !== "Value") throw new TypeError("CSS parser must return a value");
    parsed = syntax;
  } catch (error) {
    if (
      typeof error !== "object" ||
      error === null ||
      !("name" in error) ||
      error.name !== "SyntaxError"
    )
      throw error;
  }
  valueSyntaxCache.set(value, parsed);
  if (valueSyntaxCache.size > valueSyntaxCapacity) {
    const oldest = valueSyntaxCache.keys().next();
    if (!oldest.done) valueSyntaxCache.delete(oldest.value);
  }
  return parsed;
}

/** @param {string} value
 * @returns {string[] | undefined} */
export function cssVariableReferences(value) {
  const ast = parseValue(value);
  if (!ast) return undefined;
  /** @type {Set<string>} */
  const names = new Set();
  let valid = true;
  walkCss(ast, {
    visit: "Function",
    enter(node) {
      if (ident.decode(node.name).toLowerCase() !== "var") return;
      const args = variableArguments(node);
      if (!args) {
        valid = false;
        return;
      }
      names.add(args.name);
      if (args.fallback !== undefined) {
        const fallbackNames = cssVariableReferences(args.fallback);
        if (!fallbackNames) valid = false;
        else for (const name of fallbackNames) names.add(name);
      }
      return walkCss.skip;
    },
  });
  return valid ? [...names] : undefined;
}

/** @param {string} value
 * @param {(name: string) => string | undefined} readVariable
 * @returns {string | undefined} */
export function substituteCssVariables(value, readVariable) {
  const syntax = parseValue(value);
  if (!syntax) return undefined;
  /* WHY: 字句解析だけを共有し、要素ごとの変数置換で共有ASTや他のscopeを変更しない。 */
  const ast = cloneValue(syntax);
  let valid = true;
  walkCss(ast, {
    visit: "Function",
    enter(node, item, list) {
      if (ident.decode(node.name).toLowerCase() !== "var") return;
      const args = variableArguments(node);
      if (!args || !item || !list) {
        valid = false;
        return walkCss.skip;
      }
      const computed = readVariable(args.name);
      const replacement =
        computed === undefined
          ? args.fallback === undefined
            ? undefined
            : substituteCssVariables(args.fallback, readVariable)
          : computed;
      if (replacement === undefined) {
        valid = false;
        return walkCss.skip;
      }
      const replacementAst = parseValue(replacement);
      if (!replacementAst) {
        valid = false;
        return walkCss.skip;
      }
      const clonedReplacement = cloneValue(replacementAst);
      list.insertList(clonedReplacement.children, item);
      list.remove(item);
      return walkCss.skip;
    },
  });
  return valid ? generateCss(ast) : undefined;
}

/** @param {string | undefined} value */
export function cssWideKeyword(value) {
  if (value === undefined) return undefined;
  const ast = parseValue(value);
  const first = ast?.children.first;
  if (!first || first !== ast.children.last || first.type !== "Identifier") return undefined;
  const name = ident.decode(first.name).toLowerCase();
  return ["initial", "inherit", "unset", "revert", "revert-layer"].includes(name)
    ? name
    : undefined;
}

/** @param {string} value */
export function validCssFontFamily(value) {
  const ast = parseValue(value);
  if (!ast) return false;
  if (!fontFamilyValidity.has(ast)) {
    if (!fontFamilyLexer) throw new TypeError("CSS variables are not installed");
    fontFamilyValidity.set(ast, Boolean(fontFamilyLexer.matchProperty("font-family", ast).matched));
  }
  return fontFamilyValidity.get(ast);
}

/** @param {import("css-tree").SyntaxConfig} fontFamilySyntax */
export function installCssVariables(fontFamilySyntax) {
  /* WHY: 全プロパティの辞書をruntimeへ持ち込まず、libraryのfont-family参照型だけを使う。 */
  fontFamilyLexer = new Lexer(fontFamilySyntax, { generate: generateCss });
  Object.assign(globalThis, {
    katanaCssVariableReferences: cssVariableReferences,
    katanaSubstituteCssVariables: substituteCssVariables,
    katanaValidCssFontFamily: validCssFontFamily,
    katanaCssWideKeyword: cssWideKeyword,
  });
}

export const decodeCssIdentifier = ident.decode;
