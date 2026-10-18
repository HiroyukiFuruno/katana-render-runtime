import fs from "node:fs";
import path from "node:path";
import { parseSync } from "@swc/core";
import { z } from "zod";
import { FixtureRepository } from "../mermaid/diagram_update_fixtures";
import { fileSha256 } from "../mermaid/mermaid-pair-manifest";
import { canonicalRegularFile, type FileRecord } from "../mermaid/native-build-receipt";
import {
  type NativeClassicCapture,
  verifyNativeClassicCapture,
} from "../mermaid/native-classic-capture";
import { requireNativeRenderBinding } from "../mermaid/native-render-provenance";
import type { ClassicCaptureProgramOrigin } from "./runtime-unused-classic-binding";
import { astObject, physicalUnusedSource, type RawUnusedSource } from "./runtime-unused-origin";

const inputSchema = z
  .object({
    bindingsPath: z.string(),
    fixturePath: z.string(),
    slug: z.string().min(1),
    fenceLanguage: z.enum(["mermaid", "zenuml"]),
  })
  .strict();
export type UnusedClassicInput = z.infer<typeof inputSchema>;
export interface UnusedClassicInputs {
  normal?: UnusedClassicInput;
  zenuml?: UnusedClassicInput;
}
export interface UnusedClassicProgram {
  readonly index: number;
  readonly name: string;
  readonly file: FileRecord;
  readonly source: RawUnusedSource;
}
export interface UnusedClassicCapture {
  readonly input: UnusedClassicInput;
  readonly bindingsSha256: string;
  readonly attestation: FileRecord;
  readonly envelope: NativeClassicCapture;
  readonly request: RawUnusedSource;
  readonly programs: readonly UnusedClassicProgram[];
  readonly externalExitVerified: true;
}
export interface ClassicUnusedBinding {
  readonly mode: "normal" | "zenuml";
  readonly capture: UnusedClassicCapture;
  readonly origins: readonly ClassicCaptureProgramOrigin[];
}

export function unusedClassicInputs(argv: readonly string[]): UnusedClassicInputs {
  const inputs: UnusedClassicInputs = {};
  for (const mode of ["normal", "zenuml"] as const) {
    const flag = `--classic-${mode}-input`;
    const matches = argv.flatMap((value, index) => (value === flag ? [index] : []));
    if (matches.length > 1) throw new Error(`duplicate classic input: ${mode}`);
    const index = matches[0];
    if (index === undefined) continue;
    const value = argv[index + 1];
    if (value === undefined) throw new Error(`missing classic input: ${mode}`);
    inputs[mode] = inputSchema.parse(JSON.parse(value));
  }
  return inputs;
}

export function readUnusedClassicPrograms(
  envelope: NativeClassicCapture,
  artifactDir: string,
  canonicalBytes: Buffer,
): { request: RawUnusedSource; programs: readonly UnusedClassicProgram[] } {
  const verified = verifyNativeClassicCapture(envelope, artifactDir, canonicalBytes);
  const read = (record: FileRecord) => {
    const source = physicalUnusedSource(fs.readFileSync(record.path));
    if (source.sha256 !== record.sha256) throw new Error("classic source bytes changed");
    return source;
  };
  const request = read(verified.capture.request);
  const programs = verified.capture.programs.map((program) => ({
    index: program.index,
    name: program.name,
    file: { ...program.file },
    source: read(program.file),
  }));
  verifyNativeClassicCapture(envelope, artifactDir, canonicalBytes);
  return { request, programs };
}

export function readUnusedClassicCapture(
  value: UnusedClassicInput,
  mode: "normal" | "zenuml",
): UnusedClassicCapture {
  const input = inputSchema.parse(value);
  canonicalRegularFile(input.bindingsPath, "classic render bindings");
  canonicalRegularFile(input.fixturePath, "classic render fixture");
  const fixture = new FixtureRepository(path.dirname(input.fixturePath)).read(
    path.basename(input.fixturePath),
  );
  if (
    fixture.slug !== input.slug ||
    fixture.fenceLanguage !== input.fenceLanguage ||
    input.fenceLanguage !== (mode === "normal" ? "mermaid" : "zenuml") ||
    fixture.canonicalSource === undefined
  ) {
    throw new Error("classic fixture identity/mode differs");
  }
  const outputPath = canonicalRegularFile(
    path.join(path.dirname(input.bindingsPath), `${input.slug}.svg`),
    "classic render output",
  );
  const outputBytes = fs.readFileSync(outputPath);
  const binding = requireNativeRenderBinding(input.bindingsPath, fixture, outputBytes);
  const attestationPath = canonicalRegularFile(
    path.join(path.dirname(input.bindingsPath), binding.row.cliAttestationPath),
    "classic CLI attestation",
  );
  const bytes = fs.readFileSync(attestationPath);
  const cli = z
    .object({ artifacts: z.object({ classicCapture: z.unknown() }) })
    .parse(JSON.parse(physicalUnusedSource(bytes).code));
  const artifactDir = path.join(
    path.dirname(attestationPath),
    `${path.basename(attestationPath, ".json")}.artifacts`,
  );
  const canonicalBytes = Buffer.from(fixture.canonicalSource);
  const envelope = verifyNativeClassicCapture(
    cli.artifacts.classicCapture,
    artifactDir,
    canonicalBytes,
  );
  const physical = readUnusedClassicPrograms(envelope, artifactDir, canonicalBytes);
  const diagramType = z
    .object({ diagramType: z.string() })
    .parse(JSON.parse(physical.request.code)).diagramType;
  if ((diagramType === "zenuml") !== (mode === "zenuml"))
    throw new Error("classic request mode differs");
  if (
    fileSha256(attestationPath) !== binding.row.cliAttestationSha256 ||
    fileSha256(input.bindingsPath) !== binding.receiptSha256
  ) {
    throw new Error("classic provenance changed during read");
  }
  return {
    input,
    bindingsSha256: binding.receiptSha256,
    attestation: {
      path: attestationPath,
      sha256: physicalUnusedSource(bytes).sha256,
      size: bytes.length,
    },
    envelope,
    ...physical,
    externalExitVerified: true,
  };
}

export function verifyUnusedClassicCapture(
  capture: UnusedClassicCapture,
  mode: "normal" | "zenuml",
) {
  const current = readUnusedClassicCapture(capture.input, mode);
  if (JSON.stringify(current) !== JSON.stringify(capture))
    throw new Error("classic capture snapshot changed");
}

export function classicCapturedConsumer(program: UnusedClassicProgram) {
  const script = parseSync(program.source.code, { syntax: "ecmascript", isModule: false });
  const body = z.array(astObject).parse(script.body);
  const statement = body[0];
  if (body.length !== 1 || statement?.type !== "ExpressionStatement")
    throw new Error("classic consumer statements differ");
  const call = astObject.parse(statement.expression);
  const callee = astObject.parse(call.callee);
  const argumentsList = z.array(astObject).parse(call.arguments);
  if (
    call.type !== "CallExpression" ||
    callee.type !== "Identifier" ||
    callee.value !== "katanaRunMermaidRuntime" ||
    argumentsList.length !== 1 ||
    argumentsList[0]?.spread !== null ||
    astObject.parse(argumentsList[0]?.expression).type !== "ObjectExpression"
  ) {
    throw new Error("classic consumer call differs");
  }
  return physicalUnusedSource(Buffer.from(JSON.stringify(call))).sha256;
}

export function verifyClassicUnusedBindings(bindings: readonly ClassicUnusedBinding[]) {
  for (const binding of bindings) {
    verifyUnusedClassicCapture(binding.capture, binding.mode);
    if (
      JSON.stringify(binding.origins.map((origin) => origin.program)) !==
      JSON.stringify(binding.capture.programs)
    ) {
      throw new Error("classic mapped program inventory differs from capture");
    }
    for (const origin of binding.origins) origin.verify();
  }
}
