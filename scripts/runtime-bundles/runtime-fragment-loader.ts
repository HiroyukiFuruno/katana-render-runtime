import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { getLineInfo } from "acorn";
import MagicString, { Bundle } from "magic-string";
import type {
  RuntimeBundleDefinition,
  RuntimeFragment,
  RuntimeFragmentTransform,
} from "#shared/runtime_bundle";
import { RuntimeScriptNormalizer } from "./runtime-script-normalizer";
import type {
  RuntimeDefinitionProvenance,
  RuntimeFragmentInput,
  RuntimeGeneratedSpan,
  RuntimeSourceRange,
} from "./runtime-source-provenance";

export class RuntimeFragmentLoader {
  constructor(private readonly root: string) {}

  loadDefinition(definition: RuntimeBundleDefinition): string {
    if (definition.bundledEntryPath !== null) {
      throw new Error(`Runtime definition uses a module entry: ${definition.name}`);
    }
    const source = [
      ...definition.fragments.map((fragment) => this.load(fragment)),
      definition.entrypoint.trim(),
      "",
    ].join("\n");
    return new RuntimeScriptNormalizer().deduplicateFunctionDeclarations(source);
  }

  loadDefinitionWithProvenance(definition: RuntimeBundleDefinition): RuntimeDefinitionProvenance {
    if (definition.bundledEntryPath !== null) {
      throw new Error(`Runtime definition uses a module entry: ${definition.name}`);
    }
    // 通常のstring返却経路へmap生成や入力SHAのコストを持ち込まない。
    const bundle = new Bundle({ separator: "\n" });
    const inputs: RuntimeFragmentInput[] = [];
    const generated: { kind: "label" | "separator"; start: number; end: number }[] = [];
    let offset = 0;
    for (const [occurrence, fragment] of definition.fragments.entries()) {
      if (occurrence > 0) generated.push({ kind: "separator", start: offset++, end: offset });
      const bytes = fs.readFileSync(path.join(this.root, fragment.sourcePath));
      // 入力SHAは物理bytesへ結び、BOMを保持して既存のUTF16座標と一致させる。
      const raw = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
      const transformed = this.transform(raw, fragment.transform);
      const content = new MagicString(raw, { filename: fragment.sourcePath });
      const target = this.transformTarget(fragment.transform);
      const removals: RuntimeSourceRange[] = [];
      if (target !== null) {
        const start = raw.indexOf(target);
        content.remove(start, start + target.length);
        removals.push(this.range(raw, start, start + target.length));
      }
      if (content.toString() !== transformed)
        throw new Error("Runtime transform provenance mismatch");
      const prefix = `/* ${fragment.label} */\n`;
      generated.push({ kind: "label", start: offset, end: offset + prefix.length });
      offset += prefix.length + transformed.length;
      generated.push({ kind: "separator", start: offset++, end: offset });
      content.prepend(prefix).append("\n");
      bundle.addSource({ content, filename: fragment.sourcePath });
      inputs.push({
        occurrence,
        sourcePath: fragment.sourcePath,
        sourceSha256: createHash("sha256").update(bytes).digest("hex"),
        label: fragment.label,
        transform: fragment.transform,
        rawLength: raw.length,
        transformRemovals: removals,
      });
    }
    if (inputs.length > 0) generated.push({ kind: "separator", start: offset++, end: offset });
    const entrypoint = definition.entrypoint.trim();
    const entrypointStart = offset;
    const entrypointSource = `@runtime/entrypoint/${definition.name}`;
    bundle.addSource({ content: new MagicString(entrypoint), filename: entrypointSource });
    offset += entrypoint.length;
    generated.push({ kind: "separator", start: offset, end: offset + 1 });
    bundle.addSource(new MagicString(""));
    const assembled = bundle.toString();
    const normalized = new RuntimeScriptNormalizer().withProvenance(assembled);
    return {
      code: normalized.code,
      assemblyMap: bundle.generateMap({ hires: true, includeContent: true }).toString(),
      normalizationMap: normalized.map,
      rawInputs: inputs,
      entrypoint: {
        kind: "entrypoint",
        definition: definition.name,
        source: entrypointSource,
        sourceSha256: this.sha(entrypoint),
        rawSourcePath: null,
        assembledRange: this.range(assembled, entrypointStart, offset),
      },
      generatedSpans: generated.map(
        (span): RuntimeGeneratedSpan => ({
          ...this.range(assembled, span.start, span.end),
          kind: span.kind,
        }),
      ),
      duplicateRemovals: normalized.removedRanges,
    };
  }

  private sha(source: string): string {
    return createHash("sha256").update(source).digest("hex");
  }

  private range(source: string, start: number, end: number): RuntimeSourceRange {
    return {
      start,
      end,
      startPosition: getLineInfo(source, start),
      endPosition: getLineInfo(source, end),
    };
  }

  private transformTarget(transform: RuntimeFragmentTransform): string | null {
    if (transform === "verbatim") return null;
    return transform === "drawio-runtime-adapter-without-installer"
      ? "katanaInstallDrawioRuntimeAdapter();"
      : "katanaRenderDrawioSvg();";
  }

  load(fragment: RuntimeFragment): string {
    const sourcePath = path.join(this.root, fragment.sourcePath);
    const source = fs.readFileSync(sourcePath, "utf8");
    return [`/* ${fragment.label} */`, this.transform(source, fragment.transform), ""].join("\n");
  }

  private transform(source: string, transform: RuntimeFragmentTransform): string {
    if (transform === "verbatim") {
      return source;
    }
    if (transform === "drawio-runtime-adapter-without-installer") {
      return this.replaceRequiredLine(source, "katanaInstallDrawioRuntimeAdapter();", "");
    }
    return this.replaceRequiredLine(source, "katanaRenderDrawioSvg();", "");
  }

  private replaceRequiredLine(source: string, needle: string, replacement: string): string {
    if (!source.includes(needle)) {
      throw new Error(`Runtime bundle transform target was not found: ${needle}`);
    }
    return source.replace(needle, replacement);
  }
}
