import { afterEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DiagramTheme } from "./diagram_theme";
import type { OfficialRendererProvenanceInput } from "./official-renderer-provenance";
import {
  primaryStageForDiagramType,
  primaryStageForReferenceMode,
  primarySvgForStages,
  writeOfficialRendererProvenance,
} from "./official-renderer-provenance";

const temporaryDirectories: string[] = [];

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

describe("Official renderer provenance", () => {
  test("stores exact SVG stages and creates a fresh attempt directory per render", () => {
    const input = createInput("classDiagram");
    const firstManifestPath = writeOfficialRendererProvenance(input);
    const firstManifest = JSON.parse(fs.readFileSync(firstManifestPath, "utf8"));
    const attemptDirectory = path.dirname(firstManifestPath);

    expect(fs.readFileSync(path.join(attemptDirectory, "raw-mermaid.svg"), "utf8")).toBe(
      input.stages.rawSvg,
    );
    expect(firstManifest.primaryStage).toBe("rawMermaid");
    expect(primaryStageForDiagramType("classDiagram")).toBe("rawMermaid");
    expect(primarySvgForStages(input.stages)).toBe(input.stages.rawSvg);
    expect(firstManifest.stages.rawMermaid.sha256).toBe(hash(input.stages.rawSvg));
    expect(firstManifest.stages.rawMermaid.bytes).toBe(Buffer.byteLength(input.stages.rawSvg));
    expect(firstManifest.fixture.originalSourceSha256).toBe(hash(input.fixture.source));
    expect(firstManifest.fixture.canonicalSource).toBe(input.fixture.source);
    expect(firstManifest.fixture.canonicalSourceSha256).toBe(hash(input.fixture.source));
    expect(firstManifest.rendererAssets.mermaid.sha256).toBe(hashFile(input.mermaidJs));
    expect(firstManifest.rendererAssets.zenuml.sha256).toBe(hashFile(input.mermaidZenumlJs));
    expect(firstManifest.rendererAssets.runtimeScripts).toHaveLength(input.runtimeScripts.length);
    expect(firstManifest.fixture.originalSource).toBe(input.fixture.source);
    expect(firstManifest.sources.renderInput).toBe(input.fixture.source);
    expect(firstManifest.sources.renderInputSha256).toBe(hash(input.fixture.source));
    expect(firstManifest.accepted).toBe(false);
    expect(firstManifest.primarySvgSha256).toBe(firstManifest.stages.rawMermaid.sha256);

    const secondManifestPath = writeOfficialRendererProvenance(input);
    expect(path.dirname(secondManifestPath)).not.toBe(attemptDirectory);
    expect(fs.existsSync(firstManifestPath)).toBe(true);
  });

  test("keeps normalized as the primary stage for other diagram types", () => {
    const input = createInput("flowchart");
    const manifestPath = writeOfficialRendererProvenance(input);
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));

    expect(manifest.primaryStage).toBe("normalized");
    expect(primaryStageForDiagramType("flowchart")).toBe("normalized");
    expect(primarySvgForStages(input.stages)).toBe(input.stages.normalizedSvg);
  });

  test("strict mode records raw as primary for every diagram and requires identical stages", () => {
    const input = createInput("flowchart");
    input.referenceMode = "strictVendorOriginal";
    input.referenceScope = "formal";
    input.runtimeScripts = [];
    input.stages.restoredSvg = input.stages.rawSvg;
    input.stages.normalizedSvg = input.stages.rawSvg;
    const manifestPath = writeOfficialRendererProvenance(input);
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));

    expect(manifest.referenceMode).toBe("strictVendorOriginal");
    expect(manifest.referenceScope).toBe("formal");
    expect(manifest.primaryStage).toBe("rawMermaid");
    expect(primaryStageForReferenceMode("strictVendorOriginal", "flowchart")).toBe("rawMermaid");
    expect(manifest.primarySvgSha256).toBe(manifest.stages.rawMermaid.sha256);
  });

  test("strict mode rejects altered stage copies or runtime scripts before writing", () => {
    const input = createInput("flowchart");
    input.referenceMode = "strictVendorOriginal";
    input.referenceScope = "formal";
    input.runtimeScripts = [];
    input.stages.restoredSvg = input.stages.rawSvg;
    input.stages.normalizedSvg = input.stages.rawSvg;
    input.runtimeScripts.push(writeAsset(path.dirname(input.mermaidJs), "extra.js", "extra"));

    expect(() => writeOfficialRendererProvenance(input)).toThrow("unchanged source and raw SVG");
    expect(fs.existsSync(path.join(input.outputDir, ".provenance"))).toBe(false);
  });

  test("rejects classDiagram provenance when render source differs from fixture source", () => {
    const input = createInput("classDiagram");
    input.stages.renderSource = `${input.fixture.source}\n%% changed`;

    expect(() => writeOfficialRendererProvenance(input)).toThrow(
      "classDiagram provenance requires the original fixture source as render input",
    );
    expect(fs.existsSync(path.join(input.outputDir, ".provenance"))).toBe(false);
  });

  test("rejects classDiagram i18n restoration that changes raw SVG bytes", () => {
    const input = createInput("classDiagram");
    input.stages.restoredSvg = '<svg id="restored-different"></svg>\n';

    expect(() => writeOfficialRendererProvenance(input)).toThrow(
      "classDiagram raw and i18n-restored SVG stages must be byte-identical",
    );
    expect(fs.existsSync(path.join(input.outputDir, ".provenance"))).toBe(false);
  });

  test("fails closed when a vendor asset cannot be hashed", () => {
    const input = createInput("classDiagram");
    input.mermaidJs = path.join(input.outputDir, "missing-mermaid.js");

    expect(() => writeOfficialRendererProvenance(input)).toThrow();
    expect(fs.existsSync(input.outputDir)).toBe(false);
  });
});

function createInput(diagramType: string): OfficialRendererProvenanceInput {
  const outputDir = temporaryDirectory();
  const assetDirectory = path.join(outputDir, "assets");
  fs.mkdirSync(assetDirectory);
  const mermaidJs = writeAsset(assetDirectory, "mermaid.js", "mermaid asset");
  const mermaidZenumlJs = writeAsset(assetDirectory, "zenuml.js", "zenuml asset");
  const runtimeScripts = [writeAsset(assetDirectory, "runtime.js", "runtime asset")];
  const configSourceFiles = [writeAsset(assetDirectory, "config.ts", "config source")];
  const source = "classDiagram\nAnimal <|-- Duck";
  const rawSvg = '<svg id="raw"></svg>\n';
  const theme = DiagramTheme.parse("dark");
  return {
    outputDir: path.join(outputDir, "output"),
    fixture: { slug: "fixture-1", title: "Fixture", source },
    stages: {
      diagramType,
      rawSvg,
      restoredSvg:
        diagramType.toLowerCase() === "classdiagram" ? rawSvg : '<svg id="restored"></svg>\n',
      normalizedSvg: '<svg id="normalized"></svg>\n',
      originalSource: source,
      preprocessedSource: source,
      renderSource: source,
      replacements: [],
      mermaidVersion: "12.1.0",
    },
    themeName: theme.name,
    themeVariables: theme.variables(),
    mermaidConfig: {
      startOnLoad: false,
      securityLevel: "loose",
      htmlLabels: false,
      theme: theme.mermaidTheme,
      flowchart: { htmlLabels: false, useMaxWidth: false },
      sequence: { useMaxWidth: false },
      themeVariables: theme.variables(),
    },
    normalizationRequest: {
      theme: theme.mermaidTheme,
      text: theme.text,
      fill: theme.getFill(),
      stroke: theme.getStroke(),
      arrow: theme.getArrow(),
      source: "",
      background: "transparent",
    },
    mermaidJs,
    mermaidZenumlJs,
    runtimeScripts,
    configSourceFiles,
    referenceMode: "legacyNormalized",
    referenceScope: "diagnostic",
  };
}

function temporaryDirectory(): string {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mermaid-provenance-test-"));
  temporaryDirectories.push(directory);
  return directory;
}

function writeAsset(directory: string, name: string, content: string): string {
  const filePath = path.join(directory, name);
  fs.writeFileSync(filePath, content);
  return filePath;
}

function hash(content: string): string {
  return createHash("sha256").update(Buffer.from(content, "utf8")).digest("hex");
}

function hashFile(filePath: string): string {
  return createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}
