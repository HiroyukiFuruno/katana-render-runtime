// 公式CommonJSの関数exportを記述し、ESMのdefault importにも同じ公式型を使う。
declare module "css-tree/parser" {
  import { parse } from "css-tree";
  export = parse;
}
declare module "css-tree/generator" {
  import { generate } from "css-tree";
  export = generate;
}
declare module "css-tree/walker" {
  import { walk } from "css-tree";
  export = walk;
}
declare module "css-tree/utils" {
  export { clone, ident } from "css-tree";
}
declare module "css-tree/lexer" {
  import { generate, Lexer as RootLexer, type SyntaxConfig } from "css-tree";
  export class Lexer extends RootLexer {
    constructor(config: SyntaxConfig, syntax: { generate: typeof generate });
  }
}
