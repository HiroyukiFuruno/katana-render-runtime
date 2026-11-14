import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { sha256 } from "./mermaid-pair-manifest";
import { canonicalRegularFile, type FileRecord } from "./native-build-receipt";

export const LEGACY_CLI_ATTESTATION_SCOPE =
  "CLI RenderInput boundary; vendor prepared source/config and cache branch are not observed";
export const CLASSIC_CLI_ATTESTATION_SCOPE =
  "CLI RenderInput and actual Mermaid request/classic program evaluation; vendor build provenance, external exit and performance require separate evidence";

const fileSchema = z
  .object({
    path: z.string(),
    sha256: z.string().regex(/^[0-9a-f]{64}$/),
    size: z.number().int().nonnegative().safe(),
  })
  .strict();
const programSchema = z
  .object({
    index: z.number().int().nonnegative().safe(),
    name: z.string(),
    completed: z.literal(true),
    file: fileSchema,
  })
  .strict();
const captureSchema = z
  .object({
    schema: z.literal("katana-mermaid-classic-capture-v1"),
    status: z.literal("evaluated"),
    eventTarget: z.literal("katana_render_runtime::mermaid_classic_capture"),
    scriptKind: z.literal("individual classic programs in one V8 realm"),
    request: fileSchema,
    programs: z.array(programSchema),
    requiresExternalExitEvidence: z.literal(true),
    acceptedConsumerClosure: z.literal(false),
  })
  .strict();
const envelopeSchema = z.object({ manifest: fileSchema, capture: captureSchema }).strict();
const requestSchema = z
  .object({
    source: z.string(),
    svgId: z.string(),
    theme: z.string(),
    background: z.string(),
    fill: z.string(),
    text: z.string(),
    stroke: z.string(),
    arrow: z.string(),
    diagramType: z.string(),
  })
  .strict();

export type NativeClassicCapture = z.infer<typeof envelopeSchema>;

export function verifyNativeClassicCapture(
  value: unknown,
  artifactDir: string,
  canonicalBytes: Buffer,
  otherFiles: readonly FileRecord[] = [],
): NativeClassicCapture {
  const envelope = envelopeSchema.parse(value);
  assertCanonicalDirectory(artifactDir);
  const roles = new Set<string>();
  for (const item of otherFiles) roles.add(fileIdentity(item.path));
  const records: Array<{ record: FileRecord; identity: string }> = [];
  const read = (record: FileRecord, basename: string): Buffer => {
    if (record.path !== path.join(artifactDir, basename)) {
      throw new Error("Classic capture path differs from the owned attempt");
    }
    const { bytes, identity } = readRecord(record);
    if (roles.has(identity)) throw new Error("Classic capture aliases another artifact role");
    roles.add(identity);
    records.push({ record, identity });
    return bytes;
  };
  const manifest = captureSchema.parse(
    JSON.parse(utf8(read(envelope.manifest, "classic-capture.json"))),
  );
  if (JSON.stringify(manifest) !== JSON.stringify(envelope.capture)) {
    throw new Error("Classic capture manifest differs from embedded capture");
  }
  const requestBytes = read(manifest.request, "classic-request.json");
  const request = requestSchema.parse(JSON.parse(utf8(requestBytes)));
  if (request.source !== utf8(canonicalBytes)) throw new Error("Classic capture source differs");
  const names = ["mermaid-runtime.min.js", "mermaid.min.js", "mermaid-zenuml.min.js"];
  if (request.diagramType === "zenuml") names.push("zenuml-runtime.min.js");
  names.push("render-mermaid.js");
  if (manifest.programs.length !== names.length)
    throw new Error("Classic capture program count differs");
  for (const [index, program] of manifest.programs.entries()) {
    if (program.index !== index || program.name !== names[index]) {
      throw new Error("Classic capture program order differs");
    }
    const bytes = read(program.file, `classic-script-${String(index).padStart(2, "0")}.js`);
    utf8(bytes);
    if (index === names.length - 1) {
      verifyRequestProgram(bytes, requestBytes);
    }
  }
  for (const { record, identity } of records) {
    if (readRecord(record).identity !== identity)
      throw new Error("Classic capture artifact identity changed");
  }
  return envelope;
}

function verifyRequestProgram(bytes: Buffer, requestBytes: Buffer) {
  const expected = Buffer.concat([
    Buffer.from("katanaRunMermaidRuntime("),
    requestBytes,
    Buffer.from(");"),
  ]);
  if (!bytes.equals(expected)) throw new Error("Classic capture request program differs");
}

function utf8(bytes: Buffer): string {
  return new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
}

function assertCanonicalDirectory(directory: string) {
  if (!path.isAbsolute(directory) || path.resolve(directory) !== directory) {
    throw new Error("Classic capture directory is not absolute and canonical");
  }
  let ancestor = directory;
  while (true) {
    const stat = fs.lstatSync(ancestor);
    if (!stat.isDirectory() || stat.isSymbolicLink())
      throw new Error("Classic capture directory is unsafe");
    const parent = path.dirname(ancestor);
    if (parent === ancestor) break;
    ancestor = parent;
  }
  if (fs.realpathSync(directory) !== directory)
    throw new Error("Classic capture directory is not canonical");
}

function fileIdentity(filePath: string): string {
  canonicalRegularFile(filePath, "classic capture artifact");
  const stat = fs.lstatSync(filePath, { bigint: true });
  return `${stat.dev}:${stat.ino}`;
}

function readRecord(record: FileRecord): { bytes: Buffer; identity: string } {
  const identity = fileIdentity(record.path);
  const bytes = fs.readFileSync(record.path);
  if (
    bytes.length !== record.size ||
    sha256(bytes) !== record.sha256 ||
    fileIdentity(record.path) !== identity
  ) {
    throw new Error("Classic capture artifact bytes or identity differ");
  }
  return { bytes, identity };
}
