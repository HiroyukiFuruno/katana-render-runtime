import { createRequire } from "node:module";
import path from "node:path";
import commonjs from "@rollup/plugin-commonjs";
import { nodeResolve } from "@rollup/plugin-node-resolve";
import swc from "@rollup/plugin-swc";
import {
  type OutputAsset,
  type OutputBundle,
  type OutputChunk,
  type Plugin,
  type PluginContext,
  type RollupBuild,
  type RollupLog,
  type RollupOutput,
  rollup,
} from "rollup";
import { minify } from "terser";
import type { RuntimeBundleDefinition } from "#shared/runtime_bundle";
import type {
  ClassicGenerationMappedSource,
  ClassicGenerationObserver,
  ClassicRollupGeneration,
} from "./runtime-bundle-types";
import { terserReservedRuntimeEntryNames } from "./runtime-entry-names";
import { RuntimeFragmentLoader } from "./runtime-fragment-loader";

interface MinifyObservation {
  readonly content?: string;
  readonly receive: (source: ClassicGenerationMappedSource) => void;
}
type ClassicGenerationPending = {
  -readonly [Key in keyof ClassicRollupGeneration]?: ClassicRollupGeneration[Key];
};

export interface RuntimeRollupModuleInspection {
  readonly id: string;
  readonly code: string | null;
  readonly importedIds: readonly string[];
  readonly dynamicallyImportedIds: readonly string[];
  readonly importers: readonly string[];
  readonly dynamicImporters: readonly string[];
  readonly isEntry: boolean;
  readonly isExternal: boolean;
  readonly isIncluded: boolean | null;
}

export interface RuntimeRollupChunkInspection {
  readonly fileName: string;
  readonly code: string;
  readonly facadeModuleId: string | null;
  readonly isDynamicEntry: boolean;
  readonly isEntry: boolean;
  readonly isImplicitEntry: boolean;
  readonly moduleIds: readonly string[];
  readonly name: string;
  readonly dynamicImports: readonly string[];
  readonly imports: readonly string[];
  readonly modules: OutputChunk["modules"];
  readonly map: OutputChunk["map"];
}

export interface RuntimeRollupSourceMapAssetInspection {
  readonly fileName: string;
  readonly source: string | Uint8Array;
}

export interface RuntimeRollupInspection {
  readonly input: string;
  readonly modules: readonly RuntimeRollupModuleInspection[];
  readonly output: RuntimeRollupChunkInspection;
  readonly sourceMapAsset: RuntimeRollupSourceMapAssetInspection;
}

interface RuntimeRollupInspectionArtifacts {
  chunk: RuntimeRollupChunkInspection | null;
  sourceMapAsset: RuntimeRollupSourceMapAssetInspection | null;
}

function collectInspectionModules(
  context: PluginContext,
  modules: RuntimeRollupModuleInspection[],
): void {
  modules.length = 0;
  for (const id of context.getModuleIds()) {
    const info = context.getModuleInfo(id);
    if (info === null) throw new Error(`Runtime bundle module info is missing: ${id}`);
    modules.push({
      id: info.id,
      code: info.code,
      importedIds: [...info.importedIds],
      dynamicallyImportedIds: [...info.dynamicallyImportedIds],
      importers: [...info.importers],
      dynamicImporters: [...info.dynamicImporters],
      isEntry: info.isEntry,
      isExternal: info.isExternal,
      isIncluded: info.isIncluded,
    });
  }
}

function captureInspectionOutput(
  outputBundle: OutputBundle,
  artifacts: RuntimeRollupInspectionArtifacts,
): void {
  const outputs = Object.values(outputBundle);
  const chunks = outputs.filter((item): item is OutputChunk => item.type === "chunk");
  const assets = outputs.filter((item): item is OutputAsset => item.type === "asset");
  if (chunks.length !== 1 || assets.length !== 1)
    throw new Error("Runtime bundle inspection requires one chunk and one asset");
  const [chunk] = chunks;
  const [sourceMapAsset] = assets;
  if (chunk === undefined || sourceMapAsset === undefined)
    throw new Error("Runtime bundle inspection output is incomplete");
  if (chunk.map === null) throw new Error("Runtime bundle inspection source map is missing");
  if (chunk.sourcemapFileName === null)
    throw new Error("Runtime bundle inspection source map asset name is missing");
  if (sourceMapAsset.fileName !== chunk.sourcemapFileName)
    throw new Error("Runtime bundle inspection source map asset does not match chunk");
  artifacts.chunk = {
    fileName: chunk.fileName,
    code: chunk.code,
    facadeModuleId: chunk.facadeModuleId,
    isDynamicEntry: chunk.isDynamicEntry,
    isEntry: chunk.isEntry,
    isImplicitEntry: chunk.isImplicitEntry,
    moduleIds: [...chunk.moduleIds],
    name: chunk.name,
    dynamicImports: [...chunk.dynamicImports],
    imports: [...chunk.imports],
    modules: chunk.modules,
    map: chunk.map,
  };
  artifacts.sourceMapAsset = {
    fileName: sourceMapAsset.fileName,
    source: sourceMapAsset.source,
  };
}

export class RuntimeRollupBundler {
  private readonly fragments: RuntimeFragmentLoader;

  constructor(private readonly root: string) {
    this.fragments = new RuntimeFragmentLoader(root);
  }

  async bundle(
    definition: RuntimeBundleDefinition,
    observer?: ClassicGenerationObserver,
  ): Promise<string> {
    if (observer && definition.bundledEntryPath !== null)
      throw new Error("classic evidence cannot describe a module bundle");
    const generation: ClassicGenerationPending | undefined = observer
      ? { name: definition.name }
      : undefined;
    const bundle = await this.createRollupBuild(definition, undefined, generation);
    let body: string;
    try {
      const result = await bundle.generate({
        format: "iife",
        name: this.iifeName(definition),
        sourcemap: generation ? "hidden" : false,
        strict: false,
      });
      const rollupCode = this.outputCode(definition, result.output);
      let observation: MinifyObservation | undefined;
      if (generation) {
        const chunk = result.output.find((item): item is OutputChunk => item.type === "chunk");
        if (!chunk?.map) throw new Error("classic Rollup map missing");
        const { classicGenerationSource } = await import("./runtime-classic-generation");
        generation.rollup = classicGenerationSource(
          rollupCode,
          chunk.map.toString(),
        ) as ClassicGenerationMappedSource;
        observation = {
          content: chunk.map.toString(),
          receive: (source) => {
            generation.final = source;
          },
        };
      }
      const code = await this.minifyFinal(definition, rollupCode, observation);
      body = code.replace(/\r\n/g, "\n").trimEnd();
    } finally {
      await bundle.close();
    }
    if (observer && generation) {
      const { verifyClassicRollupGeneration } = await import("./runtime-classic-generation");
      observer(verifyClassicRollupGeneration(generation as ClassicRollupGeneration));
    }
    return body;
  }

  async inspect(definition: RuntimeBundleDefinition): Promise<RuntimeRollupInspection> {
    const modules: RuntimeRollupModuleInspection[] = [];
    const artifacts: RuntimeRollupInspectionArtifacts = { chunk: null, sourceMapAsset: null };
    const bundle = await this.createRollupBuild(
      definition,
      this.inspectionPlugin(modules, artifacts),
    );
    try {
      await bundle.generate({
        format: "iife",
        name: this.iifeName(definition),
        sourcemap: true,
        strict: false,
      });
      if (artifacts.chunk === null || artifacts.sourceMapAsset === null)
        throw new Error(`Runtime bundle inspection output is incomplete: ${definition.name}`);
      return {
        input: this.virtualEntryId(definition),
        modules,
        output: artifacts.chunk,
        sourceMapAsset: artifacts.sourceMapAsset,
      };
    } finally {
      await bundle.close();
    }
  }

  private async createRollupBuild(
    definition: RuntimeBundleDefinition,
    inspectionPlugin?: Plugin,
    generation?: ClassicGenerationPending,
  ): Promise<RollupBuild> {
    const input = generation
      ? `krr-classic-generated-wrapper:${definition.name}.js`
      : this.virtualEntryId(definition);
    const source = await this.entryModule(definition, generation);
    const wrapperMap = generation
      ? new (await import("magic-string")).default(source)
          .generateMap({
            source: `@classic-generated-wrapper/${definition.name}.js`,
            hires: true,
            includeContent: true,
          })
          .toString()
      : undefined;
    return rollup({
      input,
      plugins: [
        this.virtualEntryPlugin(input, source, wrapperMap),
        commonjs(),
        nodeResolve({
          browser: true,
          extensions: [".mjs", ".js", ".json", ".node", ".ts"],
          preferBuiltins: false,
        }),
        swc({
          include: ["**/*.ts"],
          swc: {
            jsc: {
              parser: {
                syntax: "typescript",
              },
              target: "es2024",
            },
            module: {
              type: "es6",
            },
          },
        }),
        ...(inspectionPlugin === undefined ? [] : [inspectionPlugin]),
      ],
      onwarn: (warning, warn) => this.handleWarning(warning, warn),
    });
  }

  private inspectionPlugin(
    modules: RuntimeRollupModuleInspection[],
    artifacts: RuntimeRollupInspectionArtifacts,
  ): Plugin {
    return {
      name: "krr-runtime-bundle-inspection",
      generateBundle(_options, outputBundle: OutputBundle) {
        collectInspectionModules(this, modules);
        captureInspectionOutput(outputBundle, artifacts);
      },
    };
  }

  private virtualEntryId(definition: RuntimeBundleDefinition): string {
    return `\0krr-runtime-bundle:${definition.name}.js`;
  }

  private virtualEntryPlugin(input: string, source: string, map?: string): Plugin {
    return {
      name: "krr-runtime-bundle-entry",
      resolveId(id) {
        if (id === input) {
          return input;
        }
        return null;
      },
      load(id) {
        if (id === input) {
          return map === undefined ? source : { code: source, map };
        }
        return null;
      },
    };
  }

  private async entryModule(
    definition: RuntimeBundleDefinition,
    generation?: ClassicGenerationPending,
  ): Promise<string> {
    if (definition.bundledEntryPath !== null) {
      const absolutePath = path.join(this.root, definition.bundledEntryPath);
      return `import ${JSON.stringify(absolutePath)};`;
    }
    let normalized: string;
    let observation: MinifyObservation | undefined;
    if (generation) {
      const provenance = this.fragments.loadDefinitionWithProvenance(definition);
      const { classicGenerationSource } = await import("./runtime-classic-generation");
      normalized = provenance.code;
      generation.normalized = classicGenerationSource(normalized);
      generation.normalizedProvenance = JSON.stringify(provenance);
      observation = {
        receive: (source) => {
          generation.inner = source;
        },
      };
    } else normalized = this.fragments.loadDefinition(definition);
    const script = await this.minifyScript(definition, normalized, observation);
    const wrapper = [
      'import parseCss from "css-tree/parser";',
      'import generateCss from "css-tree/generator";',
      'import walkCss from "css-tree/walker";',
      `import { installCssVariables, decodeCssIdentifier, cssVariableReferences } from ${JSON.stringify(path.join(this.root, "scripts/runtime-bundles/runtime-css-variables.js"))};`,
      `installCssVariables(${JSON.stringify(this.fontFamilySyntax())});`,
      `const parseDeclarations = (() => {`,
      `  const cache = new Map();`,
      `  const capacity = 256;`,
      `  return (source) => {`,
      `    const cached = cache.get(source);`,
      `    if (cached !== undefined) {`,
      `      cache.delete(source);`,
      `      cache.set(source, cached);`,
      `      return cached;`,
      `    }`,
      `    const ast = parseCss(source, { context: "declarationList", parseValue: true, parseCustomProperty: true });`,
      `    walkCss(ast, { visit: "Comment", enter(node, item, list) { if (item && list) list.remove(item); } });`,
      `    const rows = [];`,
      `    ast.children.forEach((node) => {`,
      `      if (node.type !== "Declaration") return;`,
      `      const property = decodeCssIdentifier(node.property);`,
      `      const name = property.startsWith("--") ? property : property.toLowerCase();`,
      `      const value = generateCss(node.value);`,
      `      if ((/var\\s*\\(/i.test(value) || value.includes(String.fromCharCode(92))) && cssVariableReferences(value) === undefined) return;`,
      `      rows.push(Object.freeze({ name, value, priority: node.important ? "important" : "" }));`,
      `    });`,
      `    const result = Object.freeze(rows);`,
      `    cache.set(source, result);`,
      `    if (cache.size > capacity) cache.delete(cache.keys().next().value);`,
      `    return result;`,
      `  };`,
      `})();`,
      `globalThis.katanaParseCssDeclarations = parseDeclarations;`,
      `globalThis.eval(${JSON.stringify(script)});`,
    ].join("\n");
    if (generation) {
      const { classicGenerationSource } = await import("./runtime-classic-generation");
      generation.wrapper = classicGenerationSource(wrapper);
    }
    return wrapper;
  }

  private fontFamilySyntax(): {
    generic: boolean;
    properties: Record<string, string>;
    types: Record<string, string>;
  } {
    const requireCss = createRequire(import.meta.url);
    const definitions = requireCss("css-tree/definition-syntax-data") as {
      properties: Record<string, string>;
      types: Record<string, string>;
    };
    type SyntaxNode =
      | {
          type: "Type" | "Property" | "Keyword" | "AtKeyword" | "Function";
          name: string;
        }
      | { type: "String" | "Token" | "Comma" }
      | { type: "Group"; terms: SyntaxNode[] }
      | { type: "Multiplier" | "Boolean"; term: SyntaxNode };
    const definitionSyntax = requireCss("css-tree/definition-syntax") as {
      parse(source: string): SyntaxNode;
      walk(ast: SyntaxNode, visit: (node: SyntaxNode) => void): void;
    };
    const properties: Record<string, string> = {};
    const types: Record<string, string> = {};
    const collect = (kind: "Property" | "Type", name: string): void => {
      const target = kind === "Property" ? properties : types;
      const source = (kind === "Property" ? definitions.properties : definitions.types)[name];
      if (source === undefined || target[name] !== undefined) return;
      target[name] = source;
      definitionSyntax.walk(definitionSyntax.parse(source), (node) => {
        if (node.type === "Type" || node.type === "Property") collect(node.type, node.name);
      });
    };
    collect("Property", "font-family");
    if (properties["font-family"] === undefined)
      throw new Error("CSS font-family syntax is missing");
    return { generic: true, properties, types };
  }

  private outputCode(definition: RuntimeBundleDefinition, output: RollupOutput["output"]): string {
    const chunk = output.find((item): item is OutputChunk => item.type === "chunk");
    if (chunk === undefined) {
      throw new Error(`Runtime bundle output is missing: ${definition.name}`);
    }
    return chunk.code;
  }

  private async minifyScript(
    definition: RuntimeBundleDefinition,
    code: string,
    observation?: MinifyObservation,
  ): Promise<string> {
    return this.minify(definition, code, false, observation);
  }

  private async minifyFinal(
    definition: RuntimeBundleDefinition,
    code: string,
    observation?: MinifyObservation,
  ): Promise<string> {
    return this.minify(definition, code, true, observation);
  }

  private async minify(
    definition: RuntimeBundleDefinition,
    code: string,
    mangle: boolean,
    observation?: MinifyObservation,
  ): Promise<string> {
    const result = await minify(code, {
      compress: {
        passes: 2,
        toplevel: true,
      },
      format: {
        comments: false,
      },
      keep_classnames: true,
      keep_fnames: true,
      mangle: mangle
        ? {
            reserved: terserReservedRuntimeEntryNames(),
            toplevel: true,
          }
        : false,
      module: false,
      sourceMap: observation
        ? {
            includeSources: true,
            ...(observation.content === undefined ? {} : { content: observation.content }),
          }
        : false,
      toplevel: true,
    });
    if (result.code === undefined) {
      throw new Error(`Runtime bundle minify output is missing: ${definition.name}`);
    }
    if (observation) {
      if (typeof result.map !== "string") throw new Error("classic Terser map missing");
      const { classicGenerationSource } = await import("./runtime-classic-generation");
      observation.receive(
        classicGenerationSource(result.code, result.map) as ClassicGenerationMappedSource,
      );
    }
    return result.code;
  }

  private iifeName(definition: RuntimeBundleDefinition): string {
    if (definition.name === "mermaid") {
      return "KatanaMermaidRuntimeBundle";
    }
    if (definition.name === "drawio") {
      return "KatanaDrawioRuntimeBundle";
    }
    if (definition.name === "zenuml") {
      return "KatanaZenumlRuntimeBundle";
    }
    return "KatanaMathJaxRuntimeBundle";
  }

  private handleWarning(warning: RollupLog, warn: (warning: RollupLog) => void): void {
    if (warning.code === "MODULE_LEVEL_DIRECTIVE") {
      return;
    }
    if (this.isMathJaxModuleWarning(warning)) {
      return;
    }
    warn(warning);
  }

  private isMathJaxModuleWarning(warning: RollupLog): boolean {
    const id = warning.id;
    if (warning.code === "THIS_IS_UNDEFINED" && id !== undefined) {
      return id.includes("node_modules/@mathjax/src/");
    }
    const ids = warning.ids;
    if (warning.code === "CIRCULAR_DEPENDENCY" && Array.isArray(ids)) {
      return ids.every((entry) => entry.includes("node_modules/@mathjax/src/"));
    }
    return false;
  }
}

export async function classicGenerationInputPath(root: string, name: string) {
  if (
    !path.isAbsolute(root) ||
    path.resolve(root) !== root ||
    path.isAbsolute(name) ||
    path.win32.isAbsolute(name) ||
    name.includes("\\") ||
    path.posix.normalize(name) !== name ||
    name.split("/").includes("..")
  )
    throw new Error("classic generation input path is not canonical within root");
  const file = path.join(root, name);
  if (!file.startsWith(`${root}${path.sep}`))
    throw new Error("classic generation input escapes root");
  const { canonicalRegularFile } = await import("../mermaid/native-build-receipt");
  return canonicalRegularFile(file, "classic generation input");
}

export async function classicPhysicalMap(
  source: ClassicGenerationMappedSource,
  header: string,
  file: string,
) {
  const { parsedClassicGenerationMap } = await import("./runtime-classic-generation");
  const { SourceMapConsumer, SourceMapGenerator } = await import("source-map-js");
  const { Lines } = await import("./runtime-unused-source-map");
  const value = parsedClassicGenerationMap(source);
  const consumer = new SourceMapConsumer(value as unknown as import("source-map-js").RawSourceMap);
  const generator = new SourceMapGenerator({ file });
  const body = source.code.replace(/\r\n/g, "\n").trimEnd();
  const lines = new Lines(body);
  const offset = header.split("\n").length;
  for (const [index, name] of value.sources.entries()) {
    generator.setSourceContent(name, value.sourcesContent[index] ?? "");
  }
  consumer.eachMapping((item) => {
    try {
      lines.offset(item.generatedLine, item.generatedColumn);
    } catch {
      return;
    }
    if (item.source === null || item.originalLine === null || item.originalColumn === null) return;
    generator.addMapping({
      generated: { line: item.generatedLine + offset, column: item.generatedColumn },
      original: { line: item.originalLine, column: item.originalColumn },
      source: item.source,
      ...(item.name === null ? {} : { name: item.name }),
    });
  });
  return generator.toString();
}
