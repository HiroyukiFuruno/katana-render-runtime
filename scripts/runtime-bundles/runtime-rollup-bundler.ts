import path from "node:path";
import { nodeResolve } from "@rollup/plugin-node-resolve";
import typescript from "@rollup/plugin-typescript";
import { type OutputChunk, type Plugin, type RollupLog, type RollupOutput, rollup } from "rollup";
import { minify } from "terser";
import type { RuntimeBundleDefinition } from "#shared/runtime_bundle";
import { terserReservedRuntimeEntryNames } from "./runtime-entry-names";
import { RuntimeFragmentLoader } from "./runtime-fragment-loader";
import { RuntimeScriptNormalizer } from "./runtime-script-normalizer";

export class RuntimeRollupBundler {
  private readonly fragments: RuntimeFragmentLoader;
  private readonly normalizer: RuntimeScriptNormalizer;

  constructor(private readonly root: string) {
    this.fragments = new RuntimeFragmentLoader(root);
    this.normalizer = new RuntimeScriptNormalizer();
  }

  async bundle(definition: RuntimeBundleDefinition): Promise<string> {
    const input = this.virtualEntryId(definition);
    const source = await this.entryModule(definition);
    const bundle = await rollup({
      input,
      plugins: [
        this.virtualEntryPlugin(input, source),
        nodeResolve({
          browser: true,
          extensions: [".mjs", ".js", ".json", ".node", ".ts"],
          preferBuiltins: false,
        }),
        typescript({
          tsconfig: path.join(this.root, "tsconfig.json"),
          compilerOptions: {
            declaration: false,
            declarationMap: false,
            noEmit: false,
            sourceMap: false,
          },
        }),
      ],
      onwarn: (warning, warn) => this.handleWarning(warning, warn),
    });
    try {
      const result = await bundle.generate({
        format: "iife",
        name: this.iifeName(definition),
        sourcemap: false,
        strict: false,
      });
      const code = await this.minifyFinal(definition, this.outputCode(definition, result.output));
      return code.replace(/\r\n/g, "\n").trimEnd();
    } finally {
      await bundle.close();
    }
  }

  private virtualEntryId(definition: RuntimeBundleDefinition): string {
    return `\0krr-runtime-bundle:${definition.name}.js`;
  }

  private virtualEntryPlugin(input: string, source: string): Plugin {
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
          return source;
        }
        return null;
      },
    };
  }

  private async entryModule(definition: RuntimeBundleDefinition): Promise<string> {
    if (definition.bundledEntryPath !== null) {
      const absolutePath = path.join(this.root, definition.bundledEntryPath);
      return `import ${JSON.stringify(absolutePath)};`;
    }
    const source = [
      ...definition.fragments.map((fragment) => this.fragments.load(fragment)),
      definition.entrypoint.trim(),
      "",
    ].join("\n");
    const normalized = this.normalizer.deduplicateFunctionDeclarations(source);
    const script = await this.minifyScript(definition, normalized);
    return `globalThis.eval(${JSON.stringify(script)});`;
  }

  private outputCode(definition: RuntimeBundleDefinition, output: RollupOutput["output"]): string {
    const chunk = output.find((item): item is OutputChunk => item.type === "chunk");
    if (chunk === undefined) {
      throw new Error(`Runtime bundle output is missing: ${definition.name}`);
    }
    return chunk.code;
  }

  private async minifyScript(definition: RuntimeBundleDefinition, code: string): Promise<string> {
    return this.minify(definition, code, false);
  }

  private async minifyFinal(definition: RuntimeBundleDefinition, code: string): Promise<string> {
    return this.minify(definition, code, true);
  }

  private async minify(
    definition: RuntimeBundleDefinition,
    code: string,
    mangle: boolean,
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
      sourceMap: false,
      toplevel: true,
    });
    if (result.code === undefined) {
      throw new Error(`Runtime bundle minify output is missing: ${definition.name}`);
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
