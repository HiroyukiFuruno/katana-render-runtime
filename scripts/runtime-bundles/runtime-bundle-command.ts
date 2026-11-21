import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { RuntimeBundleDefinition } from "#shared/runtime_bundle";
import { RuntimeBundleChecks, renderRuntimeBundleManifest } from "./runtime-bundle-checks";
import { runtimeBundleChecksum } from "./runtime-bundle-checksum";
import { runtimeBundleDefinitions } from "./runtime-bundle-definitions";
import { RuntimeBundlePaths } from "./runtime-bundle-paths";
import type {
  ClassicRollupGeneration,
  GeneratedBundle,
  RuntimeBundleMode,
} from "./runtime-bundle-types";
import { RuntimeRollupBundler } from "./runtime-rollup-bundler";

export class RuntimeBundleCommand {
  private readonly bundler: RuntimeRollupBundler;
  private readonly paths: RuntimeBundlePaths;

  constructor(
    private readonly root: string,
    private readonly mode: RuntimeBundleMode,
  ) {
    this.paths = new RuntimeBundlePaths(root);
    this.bundler = new RuntimeRollupBundler(root);
  }

  async run(): Promise<void> {
    if (this.mode === "check") {
      await this.generateCheckedBundles();
      return;
    }
    const bundles = await this.generateBundles();
    this.writeGeneratedBundles(bundles);
  }

  async generateCheckedBundles(
    options: { classicGeneration?: boolean } = {},
  ): Promise<readonly GeneratedBundle[]> {
    const pins = options.classicGeneration ? await this.generationInputPins() : undefined;
    const bundles = await this.generateBundles(options.classicGeneration === true);
    new RuntimeBundleChecks(this.paths).checkGeneratedBundles(bundles);
    if (pins) {
      for (const bundle of bundles)
        if (bundle.classicGeneration)
          await verifyClassicGenerationInputs(this.root, bundle.classicGeneration.generation);
      if (JSON.stringify(pins) !== JSON.stringify(await this.generationInputPins()))
        throw new Error("classic generation source/tool/config input drift");
    }
    return Object.freeze(
      bundles.map((bundle) => {
        const definition = Object.freeze(
          new RuntimeBundleDefinition(
            bundle.definition.name,
            bundle.definition.outputFile,
            Object.freeze(
              bundle.definition.fragments.map((fragment) => Object.freeze({ ...fragment })),
            ),
            bundle.definition.entrypoint,
            bundle.definition.bundledEntryPath,
          ),
        );
        return Object.freeze({
          ...bundle,
          definition,
          ...(bundle.classicGeneration
            ? {
                classicGeneration: Object.freeze({
                  ...bundle.classicGeneration,
                  inputPins: JSON.stringify(pins),
                }),
              }
            : {}),
        });
      }),
    );
  }

  private async generateBundles(classicGeneration = false): Promise<GeneratedBundle[]> {
    return Promise.all(
      runtimeBundleDefinitions().map((definition) => this.generate(definition, classicGeneration)),
    );
  }

  private async generate(
    definition: RuntimeBundleDefinition,
    classicGeneration = false,
  ): Promise<GeneratedBundle> {
    let generation: ClassicRollupGeneration | undefined;
    const observer =
      classicGeneration && definition.bundledEntryPath === null
        ? (value: ClassicRollupGeneration) => {
            generation = value;
          }
        : undefined;
    const bundle = await this.bundler.bundle(definition, observer);
    const content = [this.generatedHeader(definition), bundle, ""].join("\n");
    const evidence = generation
      ? await (await import("./runtime-classic-generation")).completeClassicGeneration(
          generation,
          this.generatedHeader(definition),
          content,
          definition.outputFile,
        )
      : undefined;
    return {
      definition,
      outputPath: this.paths.generatedPath(definition.outputFile),
      content,
      checksum: runtimeBundleChecksum(content),
      ...(evidence ? { classicGeneration: evidence } : {}),
    };
  }

  private async generationInputPins() {
    const { CLASSIC_GENERATION_CONTROL_INPUTS } = await import("./runtime-bundle-types");
    const relative = [
      ...CLASSIC_GENERATION_CONTROL_INPUTS,
      ...runtimeBundleDefinitions().flatMap((definition) =>
        definition.fragments.map((fragment) => fragment.sourcePath),
      ),
    ];
    const { classicGenerationInputPath } = await import("./runtime-rollup-bundler");
    const files = await Promise.all(
      relative.map((name) => classicGenerationInputPath(this.root, name)),
    );
    return [...new Set([...files, fs.realpathSync(process.execPath)])].sort().map((file) => {
      const bytes = fs.readFileSync(file);
      return {
        path: fs.realpathSync(file),
        sha256: createHash("sha256").update(bytes).digest("hex"),
        size: bytes.length,
      };
    });
  }

  async verifyGenerationInputPins(value: string, generation: ClassicRollupGeneration) {
    if (JSON.stringify(await this.generationInputPins()) !== value)
      throw new Error("classic generation source/tool/config pins differ");
    await verifyClassicGenerationInputs(this.root, generation);
  }

  private generatedHeader(definition: RuntimeBundleDefinition): string {
    return `// @generated by scripts/runtime-bundles/bundle-runtime.ts\n// bundle: ${definition.name}\n`;
  }
  private writeGeneratedBundles(bundles: GeneratedBundle[]): void {
    for (const bundle of bundles) {
      fs.mkdirSync(path.dirname(bundle.outputPath), { recursive: true });
      fs.writeFileSync(bundle.outputPath, bundle.content, "utf8");
    }
    fs.writeFileSync(
      this.paths.checksumManifestPath(),
      renderRuntimeBundleManifest(bundles),
      "utf8",
    );
  }
}

export async function verifyClassicGenerationInputs(
  root: string,
  generation: ClassicRollupGeneration,
) {
  const { physicalUnusedSource, RuntimeUnusedOrigin } = await import("./runtime-unused-origin");
  const { sourceMapSchema } = await import("./runtime-unused-source-map");
  const provenance = JSON.parse(generation.normalizedProvenance) as ConstructorParameters<
    typeof RuntimeUnusedOrigin
  >[0];
  const definition = runtimeBundleDefinitions().find((item) => item.name === generation.name);
  if (!definition) throw new Error("classic generation definition missing");
  const { classicGenerationInputPath } = await import("./runtime-rollup-bundler");
  for (const fragment of definition.fragments)
    await classicGenerationInputPath(root, fragment.sourcePath);
  const { RuntimeFragmentLoader } = await import("./runtime-fragment-loader");
  if (
    JSON.stringify(new RuntimeFragmentLoader(root).loadDefinitionWithProvenance(definition)) !==
    JSON.stringify(provenance)
  )
    throw new Error("classic generation canonical normalized provenance differs");
  const raw = new Map(
    provenance.rawInputs.map((input) => [
      input.sourcePath,
      physicalUnusedSource(fs.readFileSync(path.join(root, input.sourcePath))),
    ]),
  );
  new RuntimeUnusedOrigin(provenance, raw);
  const map = sourceMapSchema.parse(JSON.parse(generation.rollup.map));
  for (const [index, name] of map.sources.entries()) {
    if (
      name === `@classic-generated-wrapper/${generation.name}.js` &&
      map.sourcesContent[index] === generation.wrapper.code
    )
      continue;
    const file = await classicGenerationInputPath(root, name);
    if (physicalUnusedSource(fs.readFileSync(file)).code !== map.sourcesContent[index])
      throw new Error("classic generation dependency input differs");
  }
}
