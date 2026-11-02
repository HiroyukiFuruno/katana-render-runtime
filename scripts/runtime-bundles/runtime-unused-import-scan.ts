import fs from "node:fs";
import { isBuiltin } from "node:module";
import path from "node:path";

interface ImportState {
  pending: string[];
  sources: Set<string>;
  tools: Set<string>;
  dedicatedTools: Set<string>;
}

function sourceLoader(file: string): "tsx" | "jsx" | "ts" | "js" {
  if (file.endsWith(".tsx")) return "tsx";
  if (file.endsWith(".jsx")) return "jsx";
  return /\.[cm]?ts$/.test(file) ? "ts" : "js";
}

function registerImport(
  root: string,
  checked: string,
  specifier: string,
  resolved: string,
  state: ImportState,
): void {
  if (resolved.startsWith(path.join(root, "node_modules") + path.sep)) {
    state.tools.add(resolved);
    if (
      checked === path.join(root, "scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts") &&
      ["@moonrepo/cli/utils", "@moonrepo/cli/package.json"].includes(specifier)
    )
      state.dedicatedTools.add(resolved);
  } else if (!resolved.startsWith(root + path.sep)) {
    throw new Error("Runtime unused import is outside the pinned repository");
  } else if (/\.[cm]?[jt]sx?$/.test(resolved)) state.pending.push(resolved);
  else state.sources.add(resolved);
}

function scanImports(
  root: string,
  checked: string,
  state: ImportState,
  canonical: (file: string, label: string) => string,
): void {
  const scanner = new Bun.Transpiler({ loader: sourceLoader(checked) });
  for (const imported of scanner.scanImports(fs.readFileSync(checked, "utf8"))) {
    if (isBuiltin(imported.path) || imported.path.startsWith("bun:")) continue;
    const resolved = canonical(
      Bun.resolveSync(imported.path, path.dirname(checked)),
      "runtime unused import",
    );
    registerImport(root, checked, imported.path, resolved, state);
  }
}

export function runtimeUnusedImportScan(
  root: string,
  entries: readonly string[],
  canonical: (file: string, label: string) => string,
) {
  const state: ImportState = {
    pending: entries.map((name) => path.join(root, name)),
    sources: new Set(),
    tools: new Set(),
    dedicatedTools: new Set(),
  };
  while (state.pending.length > 0) {
    const file = state.pending.pop();
    if (!file || state.sources.has(file)) continue;
    const checked = canonical(file, "runtime unused source");
    state.sources.add(checked);
    scanImports(root, checked, state, canonical);
  }
  return {
    sources: [...state.sources].sort(),
    tools: [...state.tools].sort(),
    dedicatedTools: [...state.dedicatedTools].sort(),
  };
}
