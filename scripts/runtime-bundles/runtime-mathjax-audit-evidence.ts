import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { z } from "zod";
import { fileSha256 } from "../mermaid/mermaid-pair-manifest";
import { sameFileRecords } from "../mermaid/native-build-receipt";
import {
  artifactNames,
  diagnosticArgv,
  environment,
  inputs,
  owned,
  record,
  source,
  suffix,
  unicode,
  verifyEvidenceDirectory,
} from "./runtime-mathjax-audit-evidence-inputs";
import { inspectUnusedBiomeReport } from "./runtime-unused-biome";
import {
  bindMathJaxOwnedDiagnostics,
  MATHJAX_SOURCE_PATH,
  readMathJaxOwnedAuditEvidence,
} from "./runtime-unused-mathjax";
import { physicalUnusedSource } from "./runtime-unused-origin";

const recordSchema = z.strictObject({
  path: z.string(),
  sha256: z.string().regex(/^[a-f0-9]{64}$/),
  size: z.number().int().nonnegative(),
});
const invocationSchema = z.strictObject({
  argv: z.array(z.string()),
  exit: z.number().int(),
  codeSha256: z.string().regex(/^[a-f0-9]{64}$/),
});
const receiptSchema = z.strictObject({
  schema: z.literal("krr-mathjax-owned-evidence-v1"),
  status: z.literal("completed"),
  root: z.string(),
  directory: z.string(),
  observedAt: z.iso.datetime(),
  argv: z.array(z.string()),
  bunVersion: z.string(),
  environment: z.record(z.string(), z.string()),
  inputsBefore: z.array(recordSchema),
  inputsAfter: z.array(recordSchema),
  artifacts: z.array(recordSchema),
  owned: invocationSchema,
  unicode: invocationSchema,
});

function readDiagnostic(
  directory: string,
  name: string,
  invocation: z.infer<typeof invocationSchema>,
) {
  const code = fs.readFileSync(path.join(directory, `${name}.js`), "utf8");
  if (fileSha256(path.join(directory, `${name}.js`)) !== invocation.codeSha256)
    throw new Error("MathJax evidence diagnostic input hash drift");
  const diagnostics = inspectUnusedBiomeReport(
    JSON.parse(fs.readFileSync(path.join(directory, `${name}.json`), "utf8")),
    invocation.exit,
  );
  return { code, diagnostics, exit: invocation.exit, codeSha256: invocation.codeSha256 };
}

function validateDiagnosticConfiguration(
  root: string,
  directory: string,
  receipt: z.infer<typeof receiptSchema>,
) {
  const config = z
    .record(z.string(), z.unknown())
    .parse(JSON.parse(fs.readFileSync(path.join(root, "biome.json"), "utf8")));
  config.files = { includes: ["**/*.js"] };
  if (
    !isDeepStrictEqual(
      JSON.parse(fs.readFileSync(path.join(directory, "biome-context-probe.json"), "utf8")),
      config,
    )
  )
    throw new Error("MathJax evidence diagnostic config drift");
  for (const [name, invocation] of [
    [owned, receipt.owned],
    [unicode, receipt.unicode],
  ] as const) {
    if (!isDeepStrictEqual(invocation.argv, diagnosticArgv(root, directory, name)))
      throw new Error("MathJax evidence diagnostic argv mismatch");
  }
}

function mathJaxEvidenceSemanticFingerprint(receipt: z.infer<typeof receiptSchema>): string {
  return createHash("sha256")
    .update(
      JSON.stringify({
        inputsAfter: receipt.inputsAfter,
        environment: receipt.environment,
        bunVersion: receipt.bunVersion,
      }),
    )
    .digest("hex");
}

export function validateMathJaxAuditEvidence(root: string, value: unknown) {
  const receipt = receiptSchema.parse(value);
  const directory = receipt.directory;
  verifyEvidenceDirectory(root, receipt.root, directory);
  if (
    !isDeepStrictEqual(receipt.argv, [
      fs.realpathSync(process.execPath),
      path.join(root, source),
    ]) ||
    receipt.bunVersion !== Bun.version ||
    !isDeepStrictEqual(receipt.environment, environment())
  )
    throw new Error("MathJax evidence producer/tool/environment mismatch");
  if (
    !sameFileRecords(receipt.inputsBefore, receipt.inputsAfter) ||
    !sameFileRecords(receipt.inputsAfter, inputs(root))
  )
    throw new Error("MathJax evidence source/tool/config input drift");
  const artifacts = artifactNames().map((name) => record(path.join(directory, name)));
  if (!sameFileRecords(receipt.artifacts, artifacts))
    throw new Error("MathJax evidence artifact drift");
  validateDiagnosticConfiguration(root, directory, receipt);
  const ownedResult = readDiagnostic(directory, owned, receipt.owned);
  const raw = fs.readFileSync(path.join(root, MATHJAX_SOURCE_PATH), "utf8");
  const unicodeResult = readDiagnostic(directory, unicode, receipt.unicode);
  if (ownedResult.code !== raw || unicodeResult.code !== raw + suffix)
    throw new Error("MathJax evidence owned/Unicode program drift");
  const report = readMathJaxOwnedAuditEvidence(
    root,
    directory,
    JSON.parse(fs.readFileSync(path.join(directory, "inspection.json"), "utf8")),
    ownedResult,
    receipt.inputsBefore,
  );
  if (
    !isDeepStrictEqual(
      report,
      JSON.parse(fs.readFileSync(path.join(directory, "owned-audit.json"), "utf8")),
    )
  )
    throw new Error("MathJax evidence audit report mismatch");
  if (
    report.diagnostics.length !== 0 ||
    report.diagnosticExit !== 0 ||
    unicodeResult.diagnostics.length !== 1 ||
    unicodeResult.exit !== 1
  )
    throw new Error("MathJax evidence actual diagnostic contract mismatch");
  const binding = bindMathJaxOwnedDiagnostics(
    root,
    directory,
    unicode,
    physicalUnusedSource(Buffer.from(unicodeResult.code)),
    unicodeResult.diagnostics,
  )[0];
  if (
    binding?.rawText !== "unused_Ω" ||
    binding.start !== unicodeResult.code.indexOf("unused_Ω") ||
    binding.end !== binding.start + "unused_Ω".length
  )
    throw new Error("MathJax evidence Unicode diagnostic span drift");
  if (
    !sameFileRecords(receipt.inputsAfter, inputs(root)) ||
    !sameFileRecords(
      receipt.artifacts,
      artifactNames().map((name) => record(path.join(directory, name))),
    )
  )
    throw new Error("MathJax evidence inputs changed during validation");
  return {
    report,
    unicode: unicodeResult,
    directory,
    fingerprint: mathJaxEvidenceSemanticFingerprint(receipt),
  };
}

export function readMathJaxAuditEvidence(root: string) {
  const pointer = path.join(root, "tmp/quality/mathjax-audit-evidence/current.json");
  const before = record(pointer);
  const receipt = recordSchema.parse(JSON.parse(fs.readFileSync(pointer, "utf8")));
  if (!sameFileRecords([receipt], [record(receipt.path)]))
    throw new Error("MathJax evidence receipt drift");
  const result = validateMathJaxAuditEvidence(
    root,
    JSON.parse(fs.readFileSync(receipt.path, "utf8")),
  );
  if (
    receipt.path !== path.join(result.directory, "receipt.json") ||
    !sameFileRecords([before, receipt], [record(pointer), record(receipt.path)])
  )
    throw new Error("MathJax evidence receipt/pointer changed during validation");
  return result;
}

export function mathJaxEvidenceFingerprint(root: string): string {
  return readMathJaxAuditEvidence(root).fingerprint;
}

if (import.meta.main) {
  const root = fs.realpathSync(path.resolve(import.meta.dir, "../.."));
  if (process.argv.length === 3 && process.argv[2] === "preflight") inputs(root);
  else if (process.argv.length === 3 && process.argv[2] === "verify")
    readMathJaxAuditEvidence(root);
  else if (process.argv.length === 3 && process.argv[2] === "fingerprint")
    process.stdout.write(
      `${JSON.stringify({
        schema: "krr-mathjax-evidence-fingerprint-v1",
        fingerprint: mathJaxEvidenceFingerprint(root),
      })}\n`,
    );
  else if (process.argv.length === 2 && process.cwd() === root) {
    const { acquireMathJaxAuditEvidence } = await import(
      "./runtime-mathjax-audit-evidence-producer"
    );
    await acquireMathJaxAuditEvidence(root);
  } else
    throw new Error(
      "MathJax evidence requires repository cwd and no arguments, preflight, verify, or fingerprint",
    );
}
