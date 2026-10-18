import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import type { MermaidThemeVariables } from "./diagram_theme";
import type { OfficialReferenceMode } from "./official-reference-mode";
import type { MermaidConfig, MermaidSvgStages } from "./official-renderer-types";

interface RenderFixture {
  slug: string;
  source: string;
  canonicalSource?: string;
  title: string;
  originalFenceSource?: string;
  fenceLanguage?: "mermaid" | "zenuml";
  markdown?: string;
}

interface NormalizationRequest {
  theme: string;
  text: string;
  fill: string;
  stroke: string;
  arrow: string;
  source: string;
  background: string;
}

export interface OfficialRendererProvenanceInput {
  outputDir: string;
  fixture: RenderFixture;
  stages: MermaidSvgStages;
  themeName: string;
  themeVariables: MermaidThemeVariables;
  mermaidConfig: MermaidConfig;
  normalizationRequest: NormalizationRequest;
  mermaidJs: string;
  mermaidZenumlJs: string;
  runtimeScripts: string[];
  configSourceFiles: string[];
  referenceMode: OfficialReferenceMode;
  referenceScope: "formal" | "diagnostic";
}

interface FileProvenance {
  path: string;
  sha256: string;
  bytes: number;
  declaredVersion: string | null;
}

const Stage = {
  raw: "raw-mermaid.svg",
  restored: "i18n-restored.svg",
  normalized: "normalized.svg",
} as const;

export type OfficialRendererPrimaryStage = "rawMermaid" | "normalized";

export function primaryStageForDiagramType(diagramType: string): OfficialRendererPrimaryStage {
  return diagramType.toLowerCase() === "classdiagram" ? "rawMermaid" : "normalized";
}

export function primaryStageForReferenceMode(
  mode: OfficialReferenceMode,
  diagramType: string,
): OfficialRendererPrimaryStage {
  return mode === "strictVendorOriginal" ? "rawMermaid" : primaryStageForDiagramType(diagramType);
}

export function primarySvgForStages(stages: MermaidSvgStages): string {
  return primaryStageForDiagramType(stages.diagramType) === "rawMermaid"
    ? stages.rawSvg
    : stages.normalizedSvg;
}

export function writeOfficialRendererProvenance(input: OfficialRendererProvenanceInput): string {
  validateInput(input);
  const runtimeScripts = input.runtimeScripts.map(fileProvenance);
  const configSourceFiles = input.configSourceFiles.map(fileProvenance);
  const rendererAssets = {
    mermaid: fileProvenance(input.mermaidJs),
    zenuml: fileProvenance(input.mermaidZenumlJs),
    runtimeScripts,
    configSourceFiles,
  };

  const outputDir = path.resolve(input.outputDir);
  fs.mkdirSync(outputDir, { recursive: true });
  const provenanceRoot = path.join(outputDir, ".provenance");
  fs.mkdirSync(provenanceRoot, { recursive: true });
  const slugDigest = sha256(Buffer.from(input.fixture.slug, "utf8")).slice(0, 16);
  const attemptDir = fs.mkdtempSync(path.join(provenanceRoot, `${slugDigest}-`));

  const stageFiles = {
    rawMermaid: writeStage(attemptDir, Stage.raw, input.stages.rawSvg),
    restored: writeStage(attemptDir, Stage.restored, input.stages.restoredSvg),
    normalized: writeStage(attemptDir, Stage.normalized, input.stages.normalizedSvg),
  };
  const primaryStage = primaryStageForReferenceMode(input.referenceMode, input.stages.diagramType);
  const manifest = {
    schema: "katana-mermaid-render-provenance-v1",
    diagnosticOnly: true,
    accepted: false,
    referenceMode: input.referenceMode,
    referenceScope: input.referenceScope,
    fixture: {
      slug: input.fixture.slug,
      title: input.fixture.title,
      originalSource: input.stages.originalSource,
      originalSourceSha256: sha256(Buffer.from(input.stages.originalSource, "utf8")),
      canonicalSource: input.fixture.canonicalSource ?? input.stages.originalSource,
      canonicalSourceSha256: sha256(
        Buffer.from(input.fixture.canonicalSource ?? input.stages.originalSource, "utf8"),
      ),
      fenceLanguage: input.fixture.fenceLanguage ?? null,
      capturedFenceSource: input.fixture.originalFenceSource ?? input.stages.originalSource,
      capturedFenceSourceSha256: sha256(
        Buffer.from(input.fixture.originalFenceSource ?? input.stages.originalSource, "utf8"),
      ),
      sourceAdapterPolicy:
        input.fixture.fenceLanguage === "zenuml" ? "zenuml-prefix-v1" : "mermaid-fence-lines-v1",
      markdownSha256:
        input.fixture.markdown === undefined
          ? null
          : sha256(Buffer.from(input.fixture.markdown, "utf8")),
    },
    sources: {
      preprocessed: input.stages.preprocessedSource,
      preprocessedSha256: sha256(Buffer.from(input.stages.preprocessedSource, "utf8")),
      renderInput: input.stages.renderSource,
      renderInputSha256: sha256(Buffer.from(input.stages.renderSource, "utf8")),
      i18nReplacements: input.stages.replacements,
    },
    diagramType: input.stages.diagramType,
    primaryStage,
    primarySvgSha256: stageFiles[primaryStage].sha256,
    primarySvgBytes: stageFiles[primaryStage].bytes,
    mermaidVersion: input.stages.mermaidVersion,
    theme: { name: input.themeName, variables: input.themeVariables },
    config: {
      mermaid: input.mermaidConfig,
      normalizationRequest: input.normalizationRequest,
    },
    rendererAssets,
    stages: stageFiles,
  };
  const manifestPath = path.join(attemptDir, "manifest.json");
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx" });
  return manifestPath;
}

export function validateOfficialRendererStageIntegrity(
  fixture: RenderFixture,
  stages: MermaidSvgStages,
) {
  if (fixture.slug.trim() === "" || fixture.source.trim() === "") {
    throw new Error("Official renderer provenance requires a fixture slug and source");
  }
  if (stages.originalSource !== fixture.source) {
    throw new Error(
      "Official renderer provenance fixture source does not match the rendered source",
    );
  }
  if (fixture.canonicalSource !== undefined && stages.originalSource !== fixture.canonicalSource) {
    throw new Error("Official renderer canonical source does not match the rendered source");
  }
  if (stages.diagramType.trim() === "") {
    throw new Error("Official renderer provenance requires a recognized diagram type");
  }
  if (
    stages.diagramType.toLowerCase() === "classdiagram" &&
    (stages.preprocessedSource !== fixture.source || stages.renderSource !== fixture.source)
  ) {
    throw new Error("classDiagram provenance requires the original fixture source as render input");
  }
  if (stages.diagramType.toLowerCase() === "classdiagram" && stages.rawSvg !== stages.restoredSvg) {
    throw new Error("classDiagram raw and i18n-restored SVG stages must be byte-identical");
  }
  validateSvgStage("raw Mermaid", stages.rawSvg);
  validateSvgStage("restored", stages.restoredSvg);
  validateSvgStage("normalized", stages.normalizedSvg);
}

function validateInput(input: OfficialRendererProvenanceInput) {
  validateOfficialRendererStageIntegrity(input.fixture, input.stages);
  if (input.referenceMode === "strictVendorOriginal") {
    const canonicalSource = input.fixture.canonicalSource ?? input.fixture.source;
    if (
      input.stages.originalSource !== canonicalSource ||
      input.stages.preprocessedSource !== canonicalSource ||
      input.stages.renderSource !== canonicalSource ||
      input.stages.replacements.length !== 0 ||
      input.stages.rawSvg !== input.stages.restoredSvg ||
      input.stages.rawSvg !== input.stages.normalizedSvg ||
      input.runtimeScripts.length !== 0
    ) {
      throw new Error(
        "strictVendorOriginal provenance requires unchanged source and raw SVG stages",
      );
    }
  }
}

function validateSvgStage(name: string, svg: string) {
  const trimmed = svg.trimStart();
  if (!/<svg(?:\s|>)/i.test(trimmed) || !/<\/svg\s*>\s*$/i.test(svg)) {
    throw new Error(`Official renderer ${name} stage is not a complete SVG document`);
  }
}

function fileProvenance(filePath: string): FileProvenance {
  const absolutePath = path.resolve(filePath);
  const bytes = fs.readFileSync(absolutePath);
  const versionMatch = absolutePath.match(/(?:^|[\\/])(\d+\.\d+\.\d+)(?=[\\/])/);
  return {
    path: absolutePath,
    sha256: sha256(bytes),
    bytes: bytes.byteLength,
    declaredVersion: versionMatch?.[1] ?? null,
  };
}

function writeStage(directory: string, fileName: string, svg: string) {
  const bytes = Buffer.from(svg, "utf8");
  fs.writeFileSync(path.join(directory, fileName), bytes, { flag: "wx" });
  return { path: fileName, sha256: sha256(bytes), bytes: bytes.byteLength };
}

function sha256(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}
