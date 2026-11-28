import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { sha256 } from "./mermaid-pair-manifest";
import type { FileRecord } from "./native-build-receipt";
import { CLASSIC_CLI_ATTESTATION_SCOPE } from "./native-classic-capture";
import { type CliAttestationExpected, validateCliAttestation } from "./native-render-provenance";

export function writeFile(filePath: string, bytes: string | Buffer): FileRecord {
  fs.writeFileSync(filePath, bytes);
  const saved = fs.readFileSync(filePath);
  return { path: filePath, size: saved.length, sha256: sha256(saved) };
}

function createFixture(zen = false) {
  // reader の物理ファイル拒否契約用であり、native 実行や E2E の証跡には使わない。
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "classic-reader-unit-")));
  const attestationPath = path.join(root, "flow.cli-attestation.json");
  const artifactDir = path.join(root, "flow.cli-attestation.artifacts");
  fs.mkdirSync(artifactDir);
  const source = Buffer.from(`${zen ? "zenuml\n" : "flowchart LR\n"} A --> B😀é\r\n`);
  const input = writeFile(path.join(root, "flow.request.mmd"), source);
  const output = writeFile(path.join(root, "flow.svg"), "<svg/>");
  const runtime = writeFile(path.join(root, "runtime.js"), "unit runtime");
  const executable = writeFile(path.join(root, "krr"), "unit binary");
  const request: Record<string, unknown> = {
    source: source.toString("utf8"),
    svgId: "unit",
    theme: "dark",
    background: "",
    fill: "",
    text: "",
    stroke: "",
    arrow: "",
    diagramType: zen ? "zenuml" : "flowchart",
  };
  const requestText = JSON.stringify(request, null, 2);
  const requestFile = writeFile(path.join(artifactDir, "classic-request.json"), requestText);
  const names = ["mermaid-runtime.min.js", "mermaid.min.js", "mermaid-zenuml.min.js"];
  if (zen) names.push("zenuml-runtime.min.js");
  names.push("render-mermaid.js");
  const programs = names.map((name, index) => ({
    index,
    name,
    completed: true,
    file: writeFile(
      path.join(artifactDir, `classic-script-${String(index).padStart(2, "0")}.js`),
      index === names.length - 1
        ? `katanaRunMermaidRuntime(${requestText});`
        : `var unit${index} = "😀";`,
    ),
  }));
  const capture = {
    schema: "katana-mermaid-classic-capture-v1",
    status: "evaluated",
    eventTarget: "katana_render_runtime::mermaid_classic_capture",
    scriptKind: "individual classic programs in one V8 realm",
    request: requestFile,
    programs,
    requiresExternalExitEvidence: true,
    acceptedConsumerClosure: false,
  };
  const envelope = {
    manifest: writeFile(path.join(artifactDir, "classic-capture.json"), JSON.stringify(capture)),
    capture,
  };
  const renderInput = {
    kind: "Mermaid",
    source: source.toString("utf8"),
    config: { vendor_config: null },
    context: { document_id: null, theme: null, theme_fingerprint: null },
    policy: {
      background: null,
      cache_profile: null,
      max_height: null,
      max_width: null,
      padding: null,
    },
  };
  const artifacts: Record<string, unknown> = {
    rawInput: writeFile(path.join(artifactDir, "raw-input.bin"), source),
    renderSource: writeFile(path.join(artifactDir, "request-source.mmd"), source),
    renderInput: writeFile(
      path.join(artifactDir, "render-input.json"),
      JSON.stringify(renderInput),
    ),
    output: writeFile(path.join(artifactDir, "output.svg"), "<svg/>"),
    classicCapture: envelope,
  };
  const expected: CliAttestationExpected = {
    cliAttemptToken: "unit-attempt",
    pid: 31415,
    inputPath: input.path,
    inputSha256: input.sha256,
    binarySha256: executable.sha256,
    runtimeSha256: runtime.sha256,
    outputSvgSha256: output.sha256,
    cliAttestationSha256: null,
    actualRenderSourceSha256: sha256(source),
  };
  const bindings = { input, runtime, executable };
  const attestation: Record<string, unknown> = {
    schema: "katana-native-render-attestation-v1",
    status: "output-written",
    accepted: false,
    actualExit: null,
    requiresExternalExitEvidence: true,
    attemptToken: expected.cliAttemptToken,
    pid: expected.pid,
    inputFormat: "diagram-source",
    actualRenderSourceSha256: sha256(source),
    cwd: path.resolve(process.cwd()),
    tempDir: fs.realpathSync(os.tmpdir()),
    cliVersion: "0.4.0",
    runtimeMetadata: {
      declaredRuntime: { name: "Mermaid.js", version: "12.1.0", checksum: runtime.sha256 },
      profile: { id: "katana-mermaid", description: null },
    },
    diagnostics: { warnings: [], errors: [] },
    scope: CLASSIC_CLI_ATTESTATION_SCOPE,
    input,
    runtime,
    executable,
    output,
    artifacts,
    actualRenderInput: renderInput,
    bindingsBefore: bindings,
    bindingsAfter: bindings,
  };
  return {
    root,
    artifactDir,
    source,
    request,
    envelope,
    attestation,
    artifacts,
    expected,
    attestationPath,
    outputPath: output.path,
    runtimePath: runtime.path,
    binaryPath: executable.path,
  };
}

export type CaptureFixture = ReturnType<typeof createFixture>;

export function syncManifest(f: CaptureFixture) {
  f.envelope.manifest = writeFile(f.envelope.manifest.path, JSON.stringify(f.envelope.capture));
}

export function validate(f: CaptureFixture) {
  f.expected.cliAttestationSha256 = writeFile(
    f.attestationPath,
    JSON.stringify(f.attestation),
  ).sha256;
  validateCliAttestation(
    f.attestationPath,
    f.expected,
    f.source,
    f.outputPath,
    f.runtimePath,
    f.binaryPath,
  );
}

export function withFixture(run: (f: CaptureFixture) => void, zen = false) {
  const f = createFixture(zen);
  try {
    run(f);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
}

export function program(f: CaptureFixture, index: number) {
  const entry = f.envelope.capture.programs[index];
  if (!entry) throw new Error("unit program missing");
  return entry;
}

export function replaceRequest(f: CaptureFixture) {
  const bytes = JSON.stringify(f.request, null, 2);
  f.envelope.capture.request = writeFile(f.envelope.capture.request.path, bytes);
  const last = program(f, f.envelope.capture.programs.length - 1);
  last.file = writeFile(last.file.path, `katanaRunMermaidRuntime(${bytes});`);
}
