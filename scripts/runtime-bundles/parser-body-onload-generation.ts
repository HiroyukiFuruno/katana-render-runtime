import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { transformSync } from "@swc/core";

export const parserBodyOnloadDirectory =
  "crates/katana-render-runtime/src/renderer/backends/html_runtime";

export function generateParserBodyOnload(canonical: string, root: string): string {
  const emitted = transformSync(canonical, {
    jsc: { parser: { syntax: "typescript" }, target: "es2024" },
    isModule: false,
    sourceMaps: false,
    minify: false,
  }).code;
  // Rust は Script の評価結果を Function として捕捉するため、bundle wrapper を加えない。
  return execFileSync(
    "bunx",
    [
      "--no-install",
      "biome",
      "format",
      "--stdin-file-path",
      `${parserBodyOnloadDirectory}/parser_body_onload.js`,
    ],
    { cwd: root, input: emitted, encoding: "utf8" },
  );
}

export function checkParserBodyOnload(generated: string, expected: string): void {
  if (generated !== expected) throw new Error("Parser body-onload generated runtime is stale");
}

export function checkParserBodyOnloadFiles(root: string): void {
  const directory = path.join(root, parserBodyOnloadDirectory);
  const canonical = fs.readFileSync(path.join(directory, "parser_body_onload.ts"), "utf8");
  const expected = fs.readFileSync(path.join(directory, "parser_body_onload.js"), "utf8");
  checkParserBodyOnload(generateParserBodyOnload(canonical, root), expected);
}

if (import.meta.main) {
  if (process.argv.length !== 3 || process.argv[2] !== "--check") {
    throw new Error("Usage: parser-body-onload-generation.ts --check");
  }
  checkParserBodyOnloadFiles(path.resolve(import.meta.dir, "../.."));
}
