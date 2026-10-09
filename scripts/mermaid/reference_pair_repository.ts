import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { FixtureRepository } from "./diagram_update_fixtures";
import {
  type JsonValue,
  requireReferenceMode,
  requireStrictRenderedPair,
  sha256,
  verifyMermaidPair,
} from "./mermaid-pair-manifest";
import {
  requireNativeRenderInventory,
  type VerifiedNativeRenderBinding,
} from "./native-render-provenance";
import type { CliParsedOptions } from "./reference_compare_options";

export interface ReferencePair {
  slug: string;
  officialPath: string;
  katanaPath: string;
}

export interface MermaidFixtureInventory {
  contract: "explicit-fixture-directory-and-glob" | "observed-official-png-subset";
  expected: string[];
  scored: string[];
  skippedEmpty: Array<{ slug: string; reason: string }>;
}

export class ReferencePairRepository {
  private nativeBindings = new Map<string, VerifiedNativeRenderBinding>();

  constructor(private options: CliParsedOptions) {}

  list(inventory = this.inventory()): ReferencePair[] {
    if (this.options.referenceMode === "strictVendorOriginal") {
      this.assertExactPairInventory(this.options.officialDir, inventory.expected, "official");
      this.assertExactPairInventory(this.options.katanaDir, inventory.expected, "candidate");
      this.loadNativeBindings(inventory.expected);
    }
    const pairs = inventory.expected.map((slug) => this.buildPair(`${slug}.png`));
    for (const pair of pairs) this.validatePair(pair);
    if (pairs.length === 0) throw new Error("Mermaid comparison has no expected nonempty fixtures");
    return pairs;
  }

  inventory(): MermaidFixtureInventory {
    if (
      this.options.referenceMode === "strictVendorOriginal" &&
      (this.options.fixturesDir === undefined || this.options.fixtureGlob === undefined)
    ) {
      throw new Error("Strict formal comparison requires explicit fixture directory and glob");
    }
    if (this.options.fixturesDir !== undefined && this.options.fixtureGlob !== undefined) {
      const fixtures = new FixtureRepository(this.options.fixturesDir)
        .list()
        .filter((fixture) => matchesGlob(fixture.fileName, this.options.fixtureGlob as string));
      if (fixtures.length === 0) {
        throw new Error("Mermaid fixture inventory is empty for the supplied directory and glob");
      }
      const skippedEmpty = fixtures
        .filter((fixture) => isEmptyFixture(fixture.originalFenceSource ?? ""))
        .map((fixture) => ({ slug: fixture.slug, reason: "empty Mermaid source" }));
      const expected = fixtures
        .filter((fixture) => !isEmptyFixture(fixture.originalFenceSource ?? ""))
        .map((fixture) => fixture.slug)
        .sort();
      return {
        contract: "explicit-fixture-directory-and-glob",
        expected,
        scored: expected,
        skippedEmpty,
      };
    }
    if (this.options.fixturesDir !== undefined || this.options.fixtureGlob !== undefined) {
      throw new Error("Mermaid full inventory requires both --fixtures-dir and --fixture-glob");
    }
    const expected = fs
      .readdirSync(this.options.officialDir)
      .filter((fileName) => fileName.endsWith(".png"))
      .map((fileName) => path.basename(fileName, ".png"))
      .sort();
    return {
      contract: "observed-official-png-subset",
      expected,
      scored: expected,
      skippedEmpty: [],
    };
  }

  private buildPair(fileName: string): ReferencePair {
    return {
      slug: path.basename(fileName, ".png"),
      officialPath: path.join(this.options.officialDir, fileName),
      katanaPath: path.join(this.options.katanaDir, fileName),
    };
  }

  private assertExactPairInventory(directory: string, expected: string[], role: string) {
    const actual = fs
      .readdirSync(directory)
      .filter((fileName) => fileName.endsWith(".pair.json"))
      .map((fileName) => fileName.slice(0, -".pair.json".length))
      .sort();
    const sortedExpected = [...expected].sort();
    if (JSON.stringify(actual) !== JSON.stringify(sortedExpected)) {
      throw new Error(
        `Strict ${role} pair inventory mismatch; expected=${sortedExpected.join(",")}; actual=${actual.join(",")}`,
      );
    }
  }

  private validatePair(pair: ReferencePair) {
    for (const filePath of [pair.officialPath, pair.katanaPath]) {
      if (!fs.existsSync(filePath))
        throw new Error(`Mermaid fixture inventory missing image: ${filePath}`);
    }
    const official = verifyMermaidPair(this.options.officialDir, pair.slug, "official");
    const candidate = verifyMermaidPair(this.options.katanaDir, pair.slug, "candidate");
    if (this.options.referenceMode !== undefined && this.options.referenceScope !== undefined) {
      requireReferenceMode(official, this.options.referenceMode, this.options.referenceScope);
      requireReferenceMode(candidate, this.options.referenceMode, this.options.referenceScope);
    }
    if (this.options.referenceMode === "strictVendorOriginal") {
      requireStrictRenderedPair(official);
      requireStrictRenderedPair(candidate);
      this.validateCanonicalSourceBinding(pair.slug, official, candidate);
      this.validateCaptureContext(pair.slug, official.config, candidate.config);
    }
    const candidateSvgPath = path.join(
      this.options.katanaSvgDir ?? this.options.katanaDir,
      `${pair.slug}.svg`,
    );
    if (!fs.existsSync(candidateSvgPath) || fs.lstatSync(candidateSvgPath).isSymbolicLink()) {
      throw new Error(
        `Mermaid fixture inventory missing or unsafe candidate SVG: ${candidateSvgPath}`,
      );
    }
    const candidateSvgSha = createHash("sha256")
      .update(fs.readFileSync(candidateSvgPath))
      .digest("hex");
    if (candidateSvgSha !== candidate.sourceSvgSha256) {
      throw new Error(`Mermaid candidate SVG differs from rasterized source for ${pair.slug}`);
    }
  }

  private validateCanonicalSourceBinding(
    slug: string,
    official: ReturnType<typeof verifyMermaidPair>,
    candidate: ReturnType<typeof verifyMermaidPair>,
  ) {
    if (this.options.fixturesDir === undefined) {
      throw new Error(`Strict source binding has no fixture directory: ${slug}`);
    }
    const fixture = new FixtureRepository(this.options.fixturesDir).read(`${slug}.md`);
    const fenceSource = fixture.originalFenceSource;
    const canonicalSource = fixture.canonicalSource;
    if (fenceSource === undefined || canonicalSource === undefined) {
      throw new Error(`Strict source binding is incomplete for ${slug}`);
    }
    const expected = {
      originalFenceSourceSha256: sha256(Buffer.from(fenceSource, "utf8")),
      canonicalSourceSha256: sha256(Buffer.from(canonicalSource, "utf8")),
      originalFenceLanguage: fixture.fenceLanguage,
      sourceAdapterPolicy:
        fixture.fenceLanguage === "zenuml" ? "zenuml-prefix-v1" : "mermaid-fence-lines-v1",
      sourceMarkdownSha256: sha256(Buffer.from(fixture.markdown, "utf8")),
      referenceScope: this.options.referenceScope ?? "formal",
    };
    for (const [key, value] of Object.entries(expected)) {
      if (
        configValue(official.config, key) !== value ||
        configValue(candidate.config, key) !== value
      ) {
        throw new Error(`Strict source binding mismatch for ${slug}: ${key}`);
      }
    }
    this.validateNativePairBinding(slug, candidate);
    this.validateOfficialSourceProvenance(slug, fenceSource, canonicalSource, expected, official);
  }

  private validateNativePairBinding(slug: string, candidate: ReturnType<typeof verifyMermaidPair>) {
    const nativeBinding = this.nativeBindings.get(slug);
    if (nativeBinding === undefined) {
      throw new Error(`Strict native render binding is missing ${slug}`);
    }
    const nativeExpected = {
      nativeRenderBindingsSha256: nativeBinding.receiptSha256,
      nativeRenderBinarySha256: nativeBinding.receipt.binarySha256After,
      nativeSourceParserSha256: nativeBinding.receipt.sourceParserSha256After,
      nativeInputMarkdownSha256: nativeBinding.row.inputMarkdownSha256,
      nativeOutputSvgSha256: nativeBinding.row.outputSvgSha256,
    };
    for (const [key, value] of Object.entries(nativeExpected)) {
      if (configValue(candidate.config, key) !== value) {
        throw new Error(`Strict native render pair binding mismatch for ${slug}: ${key}`);
      }
    }
  }

  private validateOfficialSourceProvenance(
    slug: string,
    fenceSource: string,
    canonicalSource: string,
    expected: {
      originalFenceSourceSha256: string;
      canonicalSourceSha256: string;
      originalFenceLanguage: string | undefined;
      sourceAdapterPolicy: string;
      sourceMarkdownSha256: string;
      referenceScope: "formal" | "diagnostic";
    },
    official: ReturnType<typeof verifyMermaidPair>,
  ) {
    validateStrictOfficialSourceProvenance({
      slug,
      document: official.provenance.document,
      sourceSvgSha256: official.sourceSvgSha256,
      fenceSource,
      canonicalSource,
      expected,
    });
  }

  private loadNativeBindings(expected: string[]) {
    if (this.options.fixturesDir === undefined || this.options.fixtureGlob === undefined) {
      throw new Error("Strict native render binding requires explicit fixture inventory");
    }
    const fixtureRepository = new FixtureRepository(this.options.fixturesDir);
    const fixtures = fixtureRepository
      .list()
      .filter((fixture) => matchesGlob(fixture.fileName, this.options.fixtureGlob ?? ""));
    const expectedFromSource = fixtures
      .filter((fixture) => !isEmptyFixture(fixture.originalFenceSource ?? ""))
      .map((fixture) => fixture.slug)
      .sort();
    if (JSON.stringify(expectedFromSource) !== JSON.stringify([...expected].sort())) {
      throw new Error("Strict native render inventory differs from the source fixture inventory");
    }
    const svgDirectory = this.options.katanaSvgDir ?? this.options.katanaDir;
    this.nativeBindings = requireNativeRenderInventory(
      path.join(svgDirectory, "render-bindings.json"),
      fixtures,
      svgDirectory,
    );
  }

  private validateCaptureContext(slug: string, official: JsonValue, candidate: JsonValue) {
    for (const key of [
      "theme",
      "viewport",
      "captureResize",
      "sourceFontUrls",
      "externalImageWaitPolicy",
    ] as const) {
      if (
        JSON.stringify(configValue(official, key)) !== JSON.stringify(configValue(candidate, key))
      ) {
        throw new Error(`Strict capture context mismatch for ${slug}: ${key}`);
      }
    }
  }
}

export function validateStrictOfficialSourceProvenance(input: {
  slug: string;
  document: JsonValue;
  sourceSvgSha256: string;
  fenceSource: string;
  canonicalSource: string;
  expected: {
    originalFenceSourceSha256: string;
    canonicalSourceSha256: string;
    originalFenceLanguage: string | undefined;
    sourceAdapterPolicy: string;
    sourceMarkdownSha256: string;
    referenceScope: "formal" | "diagnostic";
  };
}) {
  const document = recordValue(input.document, "document");
  const fixture = recordValue(document.fixture ?? null, "fixture");
  const sources = recordValue(document.sources ?? null, "sources");
  const rendererAssets = recordValue(document.rendererAssets ?? null, "renderer assets");
  const raw = recordValue(
    recordValue(document.stages ?? null, "stages").rawMermaid ?? null,
    "raw stage",
  );
  const restored = recordValue(
    recordValue(document.stages ?? null, "stages").restored ?? null,
    "restored stage",
  );
  const normalized = recordValue(
    recordValue(document.stages ?? null, "stages").normalized ?? null,
    "normalized stage",
  );
  const runtimeScripts = rendererAssets.runtimeScripts;
  const rawSha = stringValue(raw.sha256, "raw stage hash");
  if (
    document.schema !== "katana-mermaid-render-provenance-v1" ||
    document.diagnosticOnly !== true ||
    document.accepted !== false ||
    document.referenceMode !== "strictVendorOriginal" ||
    document.referenceScope !== input.expected.referenceScope ||
    document.primaryStage !== "rawMermaid" ||
    document.primarySvgSha256 !== rawSha ||
    rawSha !== input.sourceSvgSha256 ||
    restored.sha256 !== rawSha ||
    normalized.sha256 !== rawSha ||
    !Array.isArray(runtimeScripts) ||
    runtimeScripts.length !== 0
  ) {
    throw new Error(`Official strict stage provenance is inconsistent for ${input.slug}`);
  }
  if (
    fixture.capturedFenceSource !== input.fenceSource ||
    fixture.capturedFenceSourceSha256 !== input.expected.originalFenceSourceSha256 ||
    fixture.originalSource !== input.canonicalSource ||
    fixture.canonicalSource !== input.canonicalSource ||
    fixture.canonicalSourceSha256 !== input.expected.canonicalSourceSha256 ||
    fixture.sourceAdapterPolicy !== input.expected.sourceAdapterPolicy ||
    fixture.fenceLanguage !== (input.expected.originalFenceLanguage ?? null) ||
    fixture.markdownSha256 !== input.expected.sourceMarkdownSha256 ||
    sources.preprocessed !== input.canonicalSource ||
    sources.preprocessedSha256 !== input.expected.canonicalSourceSha256 ||
    sources.renderInput !== input.canonicalSource ||
    sources.renderInputSha256 !== input.expected.canonicalSourceSha256
  ) {
    throw new Error(`Official strict source provenance does not match fixture ${input.slug}`);
  }
}

function isEmptyFixture(source: string): boolean {
  return source.trim().length === 0;
}

function configValue(config: JsonValue, key: string): JsonValue | undefined {
  if (typeof config !== "object" || config === null || Array.isArray(config)) return undefined;
  return config[key];
}

function recordValue(value: JsonValue, label: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`Invalid strict provenance ${label}`);
  }
  return value;
}

function stringValue(value: JsonValue | undefined, label: string): string {
  if (typeof value !== "string") throw new Error(`Invalid strict provenance ${label}`);
  return value;
}

function matchesGlob(fileName: string, glob: string): boolean {
  const escaped = glob
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  return new RegExp(`^${escaped}$`).test(fileName);
}
