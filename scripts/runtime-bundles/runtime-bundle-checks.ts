import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { RuntimeBundlePaths } from "./runtime-bundle-paths";
import type { GeneratedBundle } from "./runtime-bundle-types";
import { runtimeEntryName } from "./runtime-entry-names";

const MODULE_KEYWORD_PATTERN = /(^|[^\w$])(import|export)([^\w$])/;

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
    if (MODULE_KEYWORD_PATTERN.test(body)) {
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
