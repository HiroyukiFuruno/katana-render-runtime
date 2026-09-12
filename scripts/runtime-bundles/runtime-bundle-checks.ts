import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { AnyNode, CallExpression } from "acorn";
import { parse as parseLoose } from "acorn-loose";
import { full } from "acorn-walk";
import type { RuntimeBundlePaths } from "./runtime-bundle-paths";
import type { GeneratedBundle } from "./runtime-bundle-types";
import { runtimeEntryName } from "./runtime-entry-names";

function isIdentifier(node: AnyNode, name: string): boolean {
  return node.type === "Identifier" && node.name === name;
}

function isEvalCall(node: AnyNode): node is CallExpression {
  if (node.type !== "CallExpression") {
    return false;
  }
  const callee = node.callee;
  return (
    isIdentifier(callee, "eval") ||
    (callee.type === "MemberExpression" && isIdentifier(callee.property, "eval"))
  );
}

function isModuleSyntax(node: AnyNode): boolean {
  if (
    node.type === "ImportDeclaration" ||
    node.type === "ImportExpression" ||
    node.type === "ExportNamedDeclaration" ||
    node.type === "ExportDefaultDeclaration" ||
    node.type === "ExportAllDeclaration"
  ) {
    return true;
  }
  return (
    node.type === "MetaProperty" &&
    isIdentifier(node.meta, "import") &&
    isIdentifier(node.property, "meta")
  );
}

function evalStringSource(node: AnyNode): string | undefined {
  if (!isEvalCall(node)) {
    return undefined;
  }
  const [argument] = node.arguments;
  return argument?.type === "Literal" && typeof argument.value === "string"
    ? argument.value
    : undefined;
}

function inspectModuleSyntax(source: string): {
  evaluatedSources: string[];
  found: boolean;
} {
  const file = parseLoose(source, { ecmaVersion: "latest", sourceType: "module" });
  const evaluatedSources: string[] = [];
  let found = false;

  full(file, (node) => {
    if (isModuleSyntax(node)) {
      found = true;
    }
    const evaluatedSource = evalStringSource(node);
    if (evaluatedSource !== undefined) {
      evaluatedSources.push(evaluatedSource);
    }
  });
  return { evaluatedSources, found };
}

export function containsRuntimeModuleSyntax(source: string): boolean {
  const pending = [source];
  const inspected = new Set<string>();
  while (pending.length > 0) {
    const current = pending.pop();
    if (current === undefined || inspected.has(current)) {
      continue;
    }
    inspected.add(current);
    const result = inspectModuleSyntax(current);
    if (result.found) {
      return true;
    }
    pending.push(...result.evaluatedSources);
  }
  return false;
}

export class RuntimeBundleChecks {
  constructor(private readonly paths: RuntimeBundlePaths) {}

  checkGeneratedBundles(bundles: GeneratedBundle[]): void {
    const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "krr-runtime-bundles-"));
    try {
      for (const bundle of bundles) {
        this.checkSyncedBundle(bundle, scratch);
        this.checkBundleSemantics(bundle);
      }
      this.checkChecksumManifest(bundles);
      this.checkRustEntrypointUsage();
    } finally {
      fs.rmSync(scratch, { recursive: true, force: true });
    }
  }

  private checkSyncedBundle(bundle: GeneratedBundle, scratch: string): void {
    const expected = fs.readFileSync(bundle.outputPath, "utf8");
    const scratchPath = path.join(scratch, bundle.definition.outputFile);
    fs.writeFileSync(scratchPath, bundle.content, "utf8");
    if (expected !== bundle.content) {
      throw new Error(
        `Runtime bundle is stale: ${this.paths.relative(bundle.outputPath)} differs from ${scratchPath}`,
      );
    }
  }

  private checkChecksumManifest(bundles: GeneratedBundle[]): void {
    const expected = renderRuntimeBundleManifest(bundles);
    const manifestPath = this.paths.checksumManifestPath();
    const actual = fs.readFileSync(manifestPath, "utf8");
    if (actual !== expected) {
      throw new Error(`Runtime bundle checksum manifest is stale: ${manifestPath}`);
    }
  }

  private checkBundleSemantics(bundle: GeneratedBundle): void {
    const body = this.bundleBody(bundle);
    this.checkNoModuleSyntax(bundle, body);
    this.checkEntrypoint(bundle, body);
    this.checkMinifiedShape(bundle, body);
  }

  private bundleBody(bundle: GeneratedBundle): string {
    const marker = "\n\n";
    const bodyStart = bundle.content.indexOf(marker);
    if (bodyStart === -1) {
      throw new Error(`Runtime bundle generated header is missing: ${bundle.definition.name}`);
    }
    return bundle.content.slice(bodyStart + marker.length).trim();
  }

  private checkNoModuleSyntax(bundle: GeneratedBundle, body: string): void {
    if (containsRuntimeModuleSyntax(body)) {
      throw new Error(`Runtime bundle must not contain import/export: ${bundle.definition.name}`);
    }
  }

  private checkEntrypoint(bundle: GeneratedBundle, body: string): void {
    const entry = runtimeEntryName(bundle.definition.name);
    const quoted = `globalThis["${entry}"]`;
    const dotted = `globalThis.${entry}`;
    if (!body.includes(quoted) && !body.includes(dotted) && !body.includes(entry)) {
      throw new Error(`Runtime bundle entrypoint is missing: ${entry}`);
    }
  }

  private checkMinifiedShape(bundle: GeneratedBundle, body: string): void {
    if (body.includes("\n/* ") || body.includes("\nfunction ")) {
      throw new Error(`Runtime bundle is not minified: ${bundle.definition.name}`);
    }
  }

  private checkRustEntrypointUsage(): void {
    const renderScript = fs.readFileSync(
      this.paths.resolve(
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_scripts.rs",
      ),
      "utf8",
    );
    if (renderScript.includes("katanaInstallMermaidZenumlRuntimeAdapter()")) {
      throw new Error("Mermaid render script must not call the ZenUML adapter installer directly");
    }
  }
}

export function renderRuntimeBundleManifest(bundles: GeneratedBundle[]): string {
  return bundles
    .map((bundle) => `${bundle.checksum}  ${bundle.definition.outputFile}`)
    .join("\n")
    .concat("\n");
}
