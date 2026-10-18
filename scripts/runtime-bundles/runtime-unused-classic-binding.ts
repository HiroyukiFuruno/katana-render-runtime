import fs from "node:fs";
import path from "node:path";
import MagicString from "magic-string";
import { canonicalRegularFile } from "../mermaid/native-build-receipt";
import type { GeneratedBundle } from "./runtime-bundle-types";
import {
  type ClassicUnusedBinding,
  classicCapturedConsumer,
  readUnusedClassicCapture,
  type UnusedClassicInputs,
  type UnusedClassicProgram,
  verifyClassicUnusedBindings,
  verifyUnusedClassicCapture,
} from "./runtime-unused-classic-capture";
import {
  physicalUnusedSource,
  RuntimeUnusedOrigin,
  type UnusedDiagnostic,
} from "./runtime-unused-origin";
import { Lines, mapIndex, sourceMapSchema } from "./runtime-unused-source-map";

const ownedContextNames: Readonly<Record<string, string>> = {
  "mermaid-runtime.min.js": "mermaid",
  "zenuml-runtime.min.js": "zenuml",
};

export class ClassicCaptureProgramOrigin {
  readonly map: string;
  readonly program: UnusedClassicProgram;
  private readonly lines: Lines;
  private readonly points: ReturnType<typeof mapIndex>;
  constructor(program: UnusedClassicProgram, suppliedMap?: string) {
    this.program = { ...program, file: { ...program.file }, source: { ...program.source } };
    this.verify();
    this.map =
      suppliedMap ??
      new MagicString(program.source.code)
        .generateMap({
          source: program.file.path,
          hires: true,
          includeContent: true,
        })
        .toString();
    const map = sourceMapSchema.parse(JSON.parse(this.map));
    if (map.sources.join("\0") !== program.file.path) throw new Error("classic map source differs");
    this.lines = new Lines(program.source.code);
    this.points = mapIndex(map, this.lines, new Map([[program.file.path, this.lines]]));
    for (const [offset, point] of this.points) {
      if (!point || point.source !== program.file.path || point.offset !== offset) {
        throw new Error("classic physical map is not identity");
      }
    }
  }
  verify() {
    canonicalRegularFile(this.program.file.path, "classic physical source");
    const bytes = fs.readFileSync(this.program.file.path);
    const source = physicalUnusedSource(bytes);
    if (
      source.code !== this.program.source.code ||
      source.sha256 !== this.program.source.sha256 ||
      source.sha256 !== this.program.file.sha256 ||
      bytes.length !== this.program.file.size
    ) {
      throw new Error("classic physical source changed");
    }
  }
  resolve(diagnostic: UnusedDiagnostic) {
    this.verify();
    if (diagnostic.location.path !== this.program.file.path)
      throw new Error("classic diagnostic crosses program");
    const start = this.lines.biome(
      diagnostic.location.start.line,
      diagnostic.location.start.column,
    );
    const end = this.lines.biome(diagnostic.location.end.line, diagnostic.location.end.column);
    if (end <= start) throw new Error("classic diagnostic range is empty or reversed");
    for (let offset = start; offset < end; offset++) {
      if (this.points.get(offset)?.offset !== offset)
        throw new Error("classic diagnostic is unmapped");
    }
    return {
      sourcePath: this.program.file.path,
      sourceSha256: this.program.file.sha256,
      start,
      end,
    };
  }
}
export function bindClassicOwnedBytes(
  root: string,
  program: UnusedClassicProgram,
  bundles: readonly GeneratedBundle[],
) {
  const expectedPath = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated",
    program.name,
  );
  const bundle = bundles.find((item) => item.definition.outputFile === program.name);
  if (
    !bundle ||
    path.resolve(bundle.outputPath) !== expectedPath ||
    bundle.definition.bundledEntryPath !== null ||
    physicalUnusedSource(Buffer.from(bundle.content)).sha256 !== bundle.checksum ||
    bundle.checksum !== program.file.sha256 ||
    !fs.readFileSync(expectedPath).equals(Buffer.from(bundle.content))
  ) {
    throw new Error("classic captured owned bundle bytes differ from checked generation");
  }
  return { outputPath: expectedPath, checksum: bundle.checksum };
}

export function readClassicUnusedBindings(inputs: UnusedClassicInputs): ClassicUnusedBinding[] {
  return (["normal", "zenuml"] as const).flatMap((mode) => {
    const input = inputs[mode];
    if (!input) return [];
    const capture = readUnusedClassicCapture(input, mode);
    const origins = capture.programs.map((program) => new ClassicCaptureProgramOrigin(program));
    const last = capture.programs.at(-1);
    if (!last) throw new Error("classic render consumer missing");
    classicCapturedConsumer(last);
    verifyUnusedClassicCapture(capture, mode);
    return [{ mode, capture, origins }];
  });
}

function classicProgramMetadata(
  root: string,
  origin: ClassicCaptureProgramOrigin,
  bundles: readonly GeneratedBundle[],
  contexts: readonly { name: string; origin: object }[],
) {
  origin.verify();
  const program = origin.program;
  const contextName = ownedContextNames[program.name];
  const context = contexts.find((item) => item.name === contextName);
  if (contextName && !(context?.origin instanceof RuntimeUnusedOrigin))
    throw new Error("classic fragment origin missing");
  return {
    index: program.index,
    name: program.name,
    file: program.file,
    role: contextName
      ? "owned-generated"
      : program.name === "render-mermaid.js"
        ? "actual-render-call"
        : "vendor",
    sourceSha256: program.source.sha256,
    map: origin.map,
    canonicalOwnedBundle: contextName ? bindClassicOwnedBytes(root, program, bundles) : null,
    fragmentOrigin:
      context?.origin instanceof RuntimeUnusedOrigin ? context.origin.provenance : null,
  };
}

export function classicUnusedMetadata(
  root: string,
  bindings: readonly ClassicUnusedBinding[],
  bundles: readonly GeneratedBundle[],
  contexts: readonly { name: string; origin: object }[],
) {
  verifyClassicUnusedBindings(bindings);
  return (["normal", "zenuml"] as const).map((mode) => {
    const binding = bindings.find((item) => item.mode === mode);
    if (!binding)
      return {
        mode,
        status: "not-provided",
        actualCaptureBytesBound: false,
        acceptedConsumerClosure: false,
        scopeComplete: false,
      } as const;
    const programs = binding.origins.map((origin) =>
      classicProgramMetadata(root, origin, bundles, contexts),
    );
    return {
      mode,
      status: "verified",
      actualCaptureBytesBound: true,
      captureArtifactsVerified: true,
      externalExitVerified: true,
      requestBytesVerified: true,
      materializedScriptsVerified: true,
      canonicalOwnedBundleBytesBound: true,
      vendorBytesBound: false,
      innerEvalGenerationBound: false,
      fragmentGenerationMapBound: false,
      finiteClassicConsumerBound: true,
      acceptedConsumerClosure: false,
      scopeComplete: false,
      performanceVerified: false,
      coverageVerified: false,
      programs,
      bindingsSha256: binding.capture.bindingsSha256,
      attestation: binding.capture.attestation,
      manifest: binding.capture.envelope.manifest,
      request: binding.capture.envelope.capture.request,
    } as const;
  });
}
