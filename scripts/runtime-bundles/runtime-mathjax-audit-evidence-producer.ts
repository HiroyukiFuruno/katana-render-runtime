import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import {
  readMathJaxAuditEvidence,
  validateMathJaxAuditEvidence,
} from "./runtime-mathjax-audit-evidence";
import {
  artifactNames,
  environment,
  inputs,
  record,
  suffix,
  unicode,
} from "./runtime-mathjax-audit-evidence-inputs";
import {
  createUnusedBiomeConfiguration,
  runUnusedContextDiagnostics,
} from "./runtime-unused-check";
import { auditMathJaxOwnedSource, MATHJAX_SOURCE_PATH } from "./runtime-unused-mathjax";
export async function acquireMathJaxAuditEvidence(root: string) {
  const before = inputs(root);
  const observedEnvironment = environment();
  const evidenceRoot = path.join(root, "tmp/quality/mathjax-audit-evidence");
  const directory = path.join(evidenceRoot, "runs", randomUUID());
  fs.mkdirSync(directory, { recursive: true });
  const config = createUnusedBiomeConfiguration(root, directory);
  let ownedInvocation: ReturnType<typeof runUnusedContextDiagnostics> | undefined;
  const report = await auditMathJaxOwnedSource(
    root,
    directory,
    (name, code) => {
      ownedInvocation = runUnusedContextDiagnostics(root, directory, config, name, code);
      return ownedInvocation;
    },
    (inspection) =>
      fs.writeFileSync(path.join(directory, "inspection.json"), JSON.stringify(inspection), {
        flag: "wx",
      }),
  );
  fs.writeFileSync(path.join(directory, "owned-audit.json"), JSON.stringify(report), {
    flag: "wx",
  });
  const unicodeInvocation = runUnusedContextDiagnostics(
    root,
    directory,
    config,
    unicode,
    fs.readFileSync(path.join(root, MATHJAX_SOURCE_PATH), "utf8") + suffix,
  );
  if (!ownedInvocation) throw new Error("MathJax actual owned diagnostic acquisition incomplete");
  const invocation = (result: ReturnType<typeof runUnusedContextDiagnostics>) => ({
    argv: result.argv,
    exit: result.exit,
    codeSha256: result.codeSha256,
  });
  const receipt = {
    schema: "krr-mathjax-owned-evidence-v1",
    status: "completed",
    root,
    directory,
    observedAt: new Date().toISOString(),
    argv: process.argv,
    bunVersion: Bun.version,
    environment: observedEnvironment,
    inputsBefore: before,
    inputsAfter: inputs(root),
    artifacts: artifactNames().map((name) => record(path.join(directory, name))),
    owned: invocation(ownedInvocation),
    unicode: invocation(unicodeInvocation),
  };
  validateMathJaxAuditEvidence(root, receipt);
  const receiptPath = path.join(directory, "receipt.json");
  fs.writeFileSync(receiptPath, JSON.stringify(receipt), { flag: "wx" });
  const pointer = path.join(evidenceRoot, `${randomUUID()}.json`);
  fs.writeFileSync(pointer, JSON.stringify(record(receiptPath)), { flag: "wx" });
  fs.renameSync(pointer, path.join(evidenceRoot, "current.json"));
  readMathJaxAuditEvidence(root);
}
