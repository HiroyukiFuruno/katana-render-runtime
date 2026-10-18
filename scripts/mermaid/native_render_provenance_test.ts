import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { FixtureRepository } from "./diagram_update_fixtures";
import { fileSha256, type JsonValue, sha256 } from "./mermaid-pair-manifest";
import {
  canonicalRegularFile,
  digestFileRecords,
  digestStringRecord,
  type FileRecord,
  sameFileRecords,
  validateNativeCapacityProtection,
  verifyNativeBuildReceipt,
} from "./native-build-receipt";
import { managedBuildSchemaName } from "./native-cli-managed-build-schema";
import {
  type CliAttestationExpected,
  requireNativeRenderInventory,
  validateCliAttestation,
} from "./native-render-provenance";

test("native source manifest digest binds sorted path, hash, and size records", () => {
  const records = [
    { path: "/repo/A.rs", sha256: "a".repeat(64), size: 12 },
    { path: "/repo/b.rs", sha256: "b".repeat(64), size: 34 },
  ];
  const material = records
    .map((record) => `${record.path}\0${record.sha256}\0${record.size}\n`)
    .join("");
  expect(digestFileRecords(records)).toBe(sha256(Buffer.from(material, "utf8")));
});

test("native receipt helpers enforce canonical files and ordered file records", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "native-receipt-helpers-"));
  try {
    const canonicalRoot = fs.realpathSync(root);
    const regularFile = path.join(canonicalRoot, "source.rs");
    const symbolicLink = path.join(canonicalRoot, "source-link.rs");
    fs.writeFileSync(regularFile, "source");
    fs.symlinkSync(regularFile, symbolicLink);
    expect(canonicalRegularFile(regularFile, "source")).toBe(regularFile);
    expect(() => canonicalRegularFile(symbolicLink, "source")).toThrow(
      "source is not a canonical regular file",
    );
    expect(() => canonicalRegularFile(canonicalRoot, "source")).toThrow(
      "source is not a canonical regular file",
    );
    expect(() => canonicalRegularFile("source.rs", "source")).toThrow(
      "source path is not absolute and canonical",
    );

    const first = { path: "/repo/a.rs", sha256: "a".repeat(64), size: 12 };
    const second = { path: "/repo/b.rs", sha256: "b".repeat(64), size: 34 };
    const recordCases = [
      { left: [first, second], right: [first, second], expected: true },
      { left: [first, second], right: [second, first], expected: false },
      { left: [first], right: [first, second], expected: false },
      { left: [first], right: [{ ...first, path: "/repo/other.rs" }], expected: false },
      { left: [first], right: [{ ...first, sha256: "c".repeat(64) }], expected: false },
      { left: [first], right: [{ ...first, size: 13 }], expected: false },
    ];
    for (const { left, right, expected } of recordCases) {
      expect(sameFileRecords(left, right)).toBe(expected);
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("managed native schema cannot fall back to a valid legacy v1 receipt", () => {
  const fixture = createBuildFixture(true);
  try {
    const original = fs.readFileSync(fixture.receiptPath, "utf8");
    const receipt = JSON.parse(original) as { [key: string]: JsonValue };
    receipt.schema = managedBuildSchemaName;
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow(/expected|Invalid/);
    fs.writeFileSync(fixture.receiptPath, original);
    expect(
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ).binarySha256,
    ).toBe(fixture.binary.sha256);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native build receipt accepts the writer-shaped artifact without artifact-level success", () => {
  const fixture = createBuildFixture(true);
  try {
    const receipt = JSON.parse(fs.readFileSync(fixture.receiptPath, "utf8")) as {
      compilerArtifact: { success?: boolean };
    };
    expect(receipt.compilerArtifact).not.toHaveProperty("success");
    const verified = verifyNativeBuildReceipt(
      fixture.receiptPath,
      fixture.binaryPath,
      fixture.runtimePath,
      fixture.repository,
    );
    expect(verified.binarySha256).toBe(fixture.binary.sha256);
    expect(verified.sourceFileCount).toBe(5);
    expect(verified.runtimeAssetPins).toEqual([fixture.runtime]);
    const otherBinaryPath = path.join(fixture.root, "bin", "other-krr");
    const otherRuntimePath = path.join(fixture.root, "runtime", "other-mermaid.js");
    fs.writeFileSync(otherBinaryPath, "other binary");
    fs.writeFileSync(otherRuntimePath, "other runtime");
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        otherBinaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("Native build receipt points to a different CLI binary");
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        otherRuntimePath,
        fixture.repository,
      ),
    ).toThrow("Native build receipt does not pin the selected Mermaid runtime");
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("legacy v1 keeps ordinary duplicate-key JSON behavior", () => {
  const fixture = createBuildFixture(true);
  try {
    const raw = fs.readFileSync(fixture.receiptPath, "utf8");
    fs.writeFileSync(
      fixture.receiptPath,
      `{"schema":"krr-native-build-receipt-v1",${raw.slice(1)}`,
    );
    expect(
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ).binarySha256,
    ).toBe(fixture.binary.sha256);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native Cargo artifact receipt accepts fresh false as a valid cache-hit result", () => {
  const fixture = createBuildFixture(false);
  try {
    const verified = verifyNativeBuildReceipt(
      fixture.receiptPath,
      fixture.binaryPath,
      fixture.runtimePath,
      fixture.repository,
    );
    expect(verified.binarySha256).toBe(fixture.binary.sha256);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native build receipt rejects a nonboolean Cargo freshness claim", () => {
  const fixture = createBuildFixture(true);
  try {
    const receipt = JSON.parse(fs.readFileSync(fixture.receiptPath, "utf8")) as {
      compilerArtifact: { artifact: { fresh: boolean | string } };
    };
    receipt.compilerArtifact.artifact.fresh = "fresh";
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("Cargo artifact proof is missing a successful matching artifact event");
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native build receipt rejects a metadata observation outside the writer probe label", () => {
  const fixture = createBuildFixture(true, "cargo-metadata");
  try {
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("Cargo metadata process label does not match the writer probe convention");
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native build receipt rejects mismatched environment and owned process evidence", () => {
  const fixture = createBuildFixture();
  try {
    const receipt = JSON.parse(fs.readFileSync(fixture.receiptPath, "utf8")) as {
      build: { environment: { [key: string]: string }; environmentSha256: string; pid: number };
    };
    receipt.build.environmentSha256 = "0".repeat(64);
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("Native build environment digest is invalid");
    receipt.build.environmentSha256 = digestStringRecord(receipt.build.environment);
    receipt.build.pid = 0;
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("Native build receipt does not contain a successful owned build");
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native build receipt requires a reaped proxy-owned krr-build child", () => {
  const fixture = createBuildFixture();
  try {
    const receipt = JSON.parse(fs.readFileSync(fixture.receiptPath, "utf8")) as {
      build: { argv: string[]; errors: string[]; reaped: boolean };
    };
    receipt.build.reaped = false;
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("Native build receipt does not contain a successful owned build");

    receipt.build.reaped = true;
    receipt.build.argv[1] = "shell";
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("Native build receipt does not identify the owned krr-build target");
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native build receipt binds normalized guard to the owned stderr trace", () => {
  const fixture = createBuildFixture();
  try {
    const receipt = JSON.parse(fs.readFileSync(fixture.receiptPath, "utf8")) as {
      binaryProvenance: { rawGuard: { current_bytes: number } };
    };
    receipt.binaryProvenance.rawGuard.current_bytes = 2048;
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("Raw binary guard report differs from the owned build logs");
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native build receipt rejects changed source bytes", () => {
  const fixture = createBuildFixture();
  try {
    fs.appendFileSync(path.join(fixture.repository, "src/main.rs"), "changed\n");
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("changed after its receipt was written");
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native build receipt rejects over-capacity while allowing an attested cache hit", () => {
  const fixture = createBuildFixture();
  try {
    type Identity = FileRecord & {
      device: number;
      inode: number;
      mtimeNs: string;
      ctimeNs: string;
    };
    const receipt = JSON.parse(fs.readFileSync(fixture.receiptPath, "utf8")) as {
      capacityPreflight: { current_bytes: number };
      binaryProvenance: { binaryBefore: Identity | null; binaryAfter: Identity };
    };
    receipt.capacityPreflight.current_bytes = 4726095873;
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(() =>
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ),
    ).toThrow("capacity preflight does not prove an under-threshold repository target");

    receipt.capacityPreflight.current_bytes = 1024;
    receipt.binaryProvenance.binaryBefore = { ...receipt.binaryProvenance.binaryAfter };
    fs.writeFileSync(fixture.receiptPath, JSON.stringify(receipt));
    expect(
      verifyNativeBuildReceipt(
        fixture.receiptPath,
        fixture.binaryPath,
        fixture.runtimePath,
        fixture.repository,
      ).binarySha256,
    ).toBe(fixture.binary.sha256);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native render inventory rejects legacy receipts and duplicate or unbound temp roots", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-native-render-temp-binding-"));
  try {
    const fixtureDir = path.join(root, "fixtures");
    const svgDir = path.join(root, "rendered");
    fs.mkdirSync(fixtureDir);
    fs.mkdirSync(svgDir);
    fs.writeFileSync(
      path.join(fixtureDir, "flow.md"),
      "# Flow\n\n```mermaid\nflowchart LR\n  A --> B\n```\n",
    );
    const fixture = new FixtureRepository(fixtureDir).read("flow.md");
    const tempRoot = fs.realpathSync(fs.mkdtempSync(path.join(svgDir, "flow.temp-")));
    const row = (slug: string, attestedTempDir = tempRoot) => ({
      slug,
      markdownPath: path.join(fixtureDir, "flow.md"),
      inputMarkdownSha256: "0".repeat(64),
      inputPath: path.join(svgDir, `${slug}.request.mmd`),
      inputSha256: "0".repeat(64),
      inputSha256After: "0".repeat(64),
      originalFenceSourceSha256: "0".repeat(64),
      canonicalSourceSha256: "0".repeat(64),
      sourceAdapterPolicy: "mermaid-fence-lines-v1",
      sourceParserSha256Before: "0".repeat(64),
      sourceParserSha256After: "0".repeat(64),
      binaryPath: "/bin/krr",
      binarySha256Before: "0".repeat(64),
      binarySha256: "0".repeat(64),
      binarySha256After: "0".repeat(64),
      runtimePath: "/runtime/mermaid.js",
      runtimeSha256: "0".repeat(64),
      buildReceiptSha256: "0".repeat(64),
      cliAttemptToken: slug,
      argv: ["/bin/krr"],
      pid: 123,
      exitStatus: 0,
      signal: null,
      spawnError: null,
      stdoutPath: `${slug}.stdout.log`,
      stdoutSha256: "0".repeat(64),
      stderrPath: `${slug}.stderr.log`,
      stderrSha256: "0".repeat(64),
      outputPath: `${slug}.svg`,
      outputSvgSha256: "0".repeat(64),
      inputMarkdownSha256After: "0".repeat(64),
      cliAttestationPath: `${slug}.cli-attestation.json`,
      cliAttestationSha256: "0".repeat(64),
      tempRoot,
      attestedTempDir,
      actualRenderSourceSha256: "0".repeat(64),
      cliActualRenderInput: null,
      renderError: null,
    });
    const bindingsPath = path.join(svgDir, "render-bindings.json");
    fs.writeFileSync(
      bindingsPath,
      JSON.stringify({
        schema: "katana-mermaid-native-render-bindings-v1",
        status: "completed",
        referenceMode: "strictVendorOriginal",
        referenceScope: "formal",
        diagnosticOnly: true,
        accepted: false,
        formalAccepted: false,
        rows: [],
        skippedEmpty: [],
      }),
    );
    expect(() => requireNativeRenderInventory(bindingsPath, [fixture], svgDir)).toThrow(
      "Native render bindings have an unsupported schema or status",
    );

    const receipt = {
      schema: "katana-mermaid-native-render-bindings-v1",
      status: "completed",
      referenceMode: "strictVendorOriginal",
      referenceScope: "formal",
      diagnosticOnly: true,
      accepted: false,
      formalAccepted: false,
      effectiveVendorConfigObserved: false,
      vendorCacheBranchObserved: false,
      externalRuntimeConfigObserved: false,
      rows: [row("flow"), row("flow-copy")],
      skippedEmpty: [],
    };
    fs.writeFileSync(bindingsPath, JSON.stringify(receipt));
    expect(() => requireNativeRenderInventory(bindingsPath, [fixture], svgDir)).toThrow(
      "Native render bindings contain duplicate CLI temporary directories",
    );

    receipt.rows[1] = row("flow-copy", path.join(svgDir, "different-temp"));
    fs.writeFileSync(bindingsPath, JSON.stringify(receipt));
    expect(() => requireNativeRenderInventory(bindingsPath, [fixture], svgDir)).toThrow(
      "Native render bindings contain an unbound CLI temporary directory",
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test.each(["normal", "zen", "optional string profile description"] as const)(
  "CLI attestation accepts %s RenderOutput metadata and rejects malformed fields",
  (capture) => {
    const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "katana-cli-attestation-")));
    try {
      const source = Buffer.from("flowchart LR\n  A --> B\n", "utf8");
      const inputPath = path.join(root, "flow.request.mmd");
      const outputPath = path.join(root, "flow.svg");
      const runtimePath = path.join(root, "mermaid.js");
      const binaryPath = path.join(root, "krr");
      const attestationPath = path.join(root, "flow.cli-attestation.json");
      const artifactDir = path.join(root, "flow.cli-attestation.artifacts");
      const tempRoot = fs.realpathSync(fs.mkdtempSync(path.join(root, "flow-cli-temp-")));
      fs.mkdirSync(artifactDir);
      fs.writeFileSync(inputPath, source);
      fs.writeFileSync(outputPath, '<svg xmlns="http://www.w3.org/2000/svg"/>');
      fs.writeFileSync(runtimePath, "synthetic runtime");
      fs.writeFileSync(binaryPath, "synthetic executable");
      const rawInputPath = path.join(artifactDir, "raw-input.bin");
      const renderSourcePath = path.join(artifactDir, "request-source.mmd");
      const renderInputPath = path.join(artifactDir, "render-input.json");
      const outputArtifactPath = path.join(artifactDir, "output.svg");
      fs.writeFileSync(rawInputPath, source);
      fs.writeFileSync(renderSourcePath, source);
      fs.copyFileSync(outputPath, outputArtifactPath);
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
      fs.writeFileSync(renderInputPath, JSON.stringify(renderInput));
      const input = recordFile(inputPath);
      const runtime = recordFile(runtimePath);
      const executable = recordFile(binaryPath);
      const output = recordFile(outputPath);
      const rawInput = recordFile(rawInputPath);
      const renderSource = recordFile(renderSourcePath);
      const renderInputFile = recordFile(renderInputPath);
      const outputArtifact = recordFile(outputArtifactPath);
      const bindings = { input, runtime, executable };
      const token = "attempt-contract-token";
      const expected: CliAttestationExpected = {
        cliAttemptToken: token,
        pid: 31415,
        inputPath,
        inputSha256: input.sha256,
        binarySha256: executable.sha256,
        runtimeSha256: runtime.sha256,
        outputSvgSha256: output.sha256,
        cliAttestationSha256: null,
        actualRenderSourceSha256: sha256(source),
        tempRoot,
      };
      const attestation = {
        schema: "katana-native-render-attestation-v1",
        status: "output-written",
        accepted: false,
        actualExit: null,
        requiresExternalExitEvidence: true,
        attemptToken: token,
        pid: expected.pid,
        inputFormat: "diagram-source",
        actualRenderSourceSha256: sha256(source),
        cwd: path.resolve(process.cwd()),
        tempDir: tempRoot,
        cliVersion: "0.4.0",
        runtimeMetadata: {
          declaredRuntime: { name: "Mermaid.js", version: "12.1.0", checksum: runtime.sha256 },
          profile: {
            id: "katana-mermaid",
            description:
              capture === "optional string profile description" ? "Mermaid default renderer" : null,
          },
        },
        diagnostics: { warnings: [], errors: [] },
        scope:
          "CLI RenderInput boundary; vendor prepared source/config and cache branch are not observed",
        input,
        runtime,
        executable,
        output,
        artifacts: { rawInput, renderSource, renderInput: renderInputFile, output: outputArtifact },
        actualRenderInput: renderInput,
        bindingsBefore: bindings,
        bindingsAfter: bindings,
      };
      fs.writeFileSync(attestationPath, JSON.stringify(attestation));
      expected.cliAttestationSha256 = fileSha256(attestationPath);

      expect(() =>
        validateCliAttestation(
          attestationPath,
          expected,
          source,
          outputPath,
          runtimePath,
          binaryPath,
        ),
      ).not.toThrow();

      const malformedAttestations: Array<
        [string, (candidate: { [key: string]: JsonValue }) => void]
      > = [
        [
          "scalar declared runtime",
          (candidate) => {
            candidate.runtimeMetadata = {
              declaredRuntime: "Mermaid.js",
              profile: { id: "katana-mermaid", description: null },
            };
          },
        ],
        [
          "missing runtime name",
          (candidate) => {
            candidate.runtimeMetadata = {
              declaredRuntime: { version: "12.1.0", checksum: runtime.sha256 },
              profile: { id: "katana-mermaid", description: null },
            };
          },
        ],
        [
          "missing checksum",
          (candidate) => {
            candidate.runtimeMetadata = {
              declaredRuntime: { name: "Mermaid.js", version: "12.1.0" },
              profile: { id: "katana-mermaid", description: null },
            };
          },
        ],
        [
          "nullable checksum",
          (candidate) => {
            candidate.runtimeMetadata = {
              declaredRuntime: { name: "Mermaid.js", version: "12.1.0", checksum: null },
              profile: { id: "katana-mermaid", description: null },
            };
          },
        ],
        [
          "mismatched checksum",
          (candidate) => {
            candidate.runtimeMetadata = {
              declaredRuntime: {
                name: "Mermaid.js",
                version: "12.1.0",
                checksum: "0".repeat(64),
              },
              profile: { id: "katana-mermaid", description: null },
            };
          },
        ],
        [
          "scalar profile",
          (candidate) => {
            candidate.runtimeMetadata = {
              declaredRuntime: { name: "Mermaid.js", version: "12.1.0", checksum: runtime.sha256 },
              profile: "katana-mermaid",
            };
          },
        ],
        [
          "invalid profile description",
          (candidate) => {
            candidate.runtimeMetadata = {
              declaredRuntime: { name: "Mermaid.js", version: "12.1.0", checksum: runtime.sha256 },
              profile: { id: "katana-mermaid", description: 1 },
            };
          },
        ],
        [
          "missing profile description",
          (candidate) => {
            candidate.runtimeMetadata = {
              declaredRuntime: { name: "Mermaid.js", version: "12.1.0", checksum: runtime.sha256 },
              profile: { id: "katana-mermaid" },
            };
          },
        ],
        [
          "mismatched temporary directory",
          (candidate) => {
            candidate.tempDir = fs.realpathSync(os.tmpdir());
          },
        ],
        [
          "legacy diagnostics array",
          (candidate) => {
            candidate.diagnostics = [];
          },
        ],
        [
          "non-string warning",
          (candidate) => {
            candidate.diagnostics = { warnings: [1], errors: [] };
          },
        ],
        [
          "missing errors array",
          (candidate) => {
            candidate.diagnostics = { warnings: [] };
          },
        ],
      ];
      for (const [label, mutate] of malformedAttestations) {
        const candidate = JSON.parse(JSON.stringify(attestation)) as { [key: string]: JsonValue };
        mutate(candidate);
        fs.writeFileSync(attestationPath, JSON.stringify(candidate));
        expected.cliAttestationSha256 = fileSha256(attestationPath);
        let rejected = false;
        try {
          validateCliAttestation(
            attestationPath,
            expected,
            source,
            outputPath,
            runtimePath,
            binaryPath,
          );
        } catch {
          rejected = true;
        }
        if (!rejected) throw new Error(`Accepted malformed CLI RenderOutput field: ${label}`);
      }
      fs.writeFileSync(attestationPath, JSON.stringify(attestation));
      expected.cliAttestationSha256 = fileSha256(attestationPath);

      attestation.attemptToken = "different-attempt";
      fs.writeFileSync(attestationPath, JSON.stringify(attestation));
      expect(() =>
        validateCliAttestation(
          attestationPath,
          expected,
          source,
          outputPath,
          runtimePath,
          binaryPath,
        ),
      ).toThrow("CLI render attestation does not prove the fixed canonical source/output");
      attestation.attemptToken = token;
      fs.writeFileSync(attestationPath, JSON.stringify(attestation));
      expected.cliAttestationSha256 = fileSha256(attestationPath);
      fs.appendFileSync(outputPath, "tampered");
      expect(() =>
        validateCliAttestation(
          attestationPath,
          expected,
          source,
          outputPath,
          runtimePath,
          binaryPath,
        ),
      ).toThrow("CLI output bytes differ from CLI attestation");
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  },
);

function createCapacityProtectionFixture(refCount = 34) {
  const root = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), "katana-capacity-protection-")),
  );
  const physical = (filePath: string) => {
    const stats = fs.statSync(filePath, { bigint: true });
    return {
      ...recordFile(filePath),
      device: Number(stats.dev),
      inode: Number(stats.ino),
      mode: Number(stats.mode & 0o7777n),
      mtimeNs: stats.mtimeNs.toString(),
      ctimeNs: stats.ctimeNs.toString(),
    };
  };
  const refs = Array.from({ length: refCount }, (_, index) => {
    const filePath = path.join(root, `protected-${index}`);
    fs.writeFileSync(filePath, `ordinary pure fixture ${index}`);
    return physical(filePath);
  });
  const registryPath = path.join(root, "registry.json");
  fs.writeFileSync(
    registryPath,
    JSON.stringify({
      schema: "krr-protected-refs-successor-v1",
      physicalRefCount: refCount,
      refs: refs.map((item) => ({
        path: item.path,
        sha256: item.sha256,
        size: item.size,
        dev: item.device,
        ino: item.inode,
        mode: item.mode,
      })),
    }),
  );
  const registry = physical(registryPath);
  const environmentValue = JSON.stringify(refs.map((item) => item.path));
  const environment = {
    PATH: "/ordinary/no-command",
    HOME: root,
    RUSTFLAGS: "-D warnings",
    CARGO_CAPACITY_PROTECTED_REFS: environmentValue,
  };
  const pins: { [key: string]: JsonValue } = {
    capacityProtection: { registry, refCount, refs },
  };
  const proof: { [key: string]: JsonValue } = {
    schema: "krr-native-capacity-protection-v1",
    refCount,
    registryBefore: registry,
    registryAfter: structuredClone(registry),
    refsBefore: refs,
    refsAfter: structuredClone(refs),
    environmentValue,
    environmentSha256: sha256(Buffer.from(environmentValue, "utf8")),
    parentEnvironmentValue: null,
    attemptToken: "protection-unit-attempt",
    beforeNs: "99",
    afterNs: "151",
    capacityPolicy: {
      metric: "unique-inode-allocated-bytes",
      metric_version: 1,
      baseline_bytes: 2363047936,
      threshold_bytes: 4726095872,
      factor: 2,
      target: path.join(root, "target"),
    },
  };
  const build: { [key: string]: JsonValue } = {
    cwd: root,
    attemptToken: "protection-unit-attempt",
    startedNs: "100",
    endedNs: "150",
    environment: { ...environment },
  };
  const artifact: { [key: string]: JsonValue } = {
    ownedCargo: { environment: { ...environment } },
    pinsBefore: structuredClone(pins),
    pinsAfter: structuredClone(pins),
  };
  const request: { [key: string]: JsonValue } = {
    pins: structuredClone(pins),
    environment: { ...environment },
    buildEnvironment: { ...environment },
  };
  return { root, refs, registryPath, proof, build, artifact, request, environmentValue, physical };
}

function requireFirst<T>(values: readonly T[], description: string): T {
  const first = values[0];
  if (first === undefined) {
    throw new Error(`Expected at least one entry in ${description}`);
  }
  return first;
}

function capacityObject(value: JsonValue | undefined): { [key: string]: JsonValue } {
  if (value === null || value === undefined || Array.isArray(value) || typeof value !== "object") {
    throw new Error("Expected capacity test object");
  }
  return value;
}

function rewriteCapacityRegistry(
  fixture: ReturnType<typeof createCapacityProtectionFixture>,
  mutate: (registry: { [key: string]: JsonValue }) => void,
) {
  const registry = capacityObject(
    JSON.parse(fs.readFileSync(fixture.registryPath, "utf8")) as JsonValue,
  );
  mutate(registry);
  fs.writeFileSync(fixture.registryPath, JSON.stringify(registry));
  fixture.proof.registryBefore = fixture.physical(fixture.registryPath);
  fixture.proof.registryAfter = structuredClone(fixture.proof.registryBefore);
  for (const pins of [
    fixture.request.pins,
    fixture.artifact.pinsBefore,
    fixture.artifact.pinsAfter,
  ]) {
    capacityObject(pins).capacityProtection = {
      registry: capacityObject(fixture.proof.registryBefore),
      refCount: fixture.refs.length,
      refs: fixture.proof.refsBefore as JsonValue[],
    };
  }
}

for (const refCount of [1, 2, 34, 44]) {
  test(`capacity protection binds all ${refCount} registry references`, () => {
    const fixture = createCapacityProtectionFixture(refCount);
    try {
      expect(
        validateNativeCapacityProtection(
          fixture.proof,
          fixture.build,
          fixture.artifact,
          fixture.request,
        ),
      ).toBe(fixture.environmentValue);
      fixture.proof.parentEnvironmentValue = fixture.environmentValue;
      expect(
        validateNativeCapacityProtection(
          fixture.proof,
          fixture.build,
          fixture.artifact,
          fixture.request,
        ),
      ).toBe(fixture.environmentValue);
    } finally {
      fs.rmSync(fixture.root, { recursive: true, force: true });
    }
  });
}

test("capacity protection legacy absence remains optional but env or pins cannot downgrade", () => {
  const fixture = createCapacityProtectionFixture();
  try {
    expect(() =>
      validateNativeCapacityProtection(undefined, fixture.build, fixture.artifact, fixture.request),
    ).toThrow("Native capacity protection evidence is missing");
    const legacyBuild = { environment: {} };
    const legacyArtifact = { ownedCargo: { environment: {} }, pinsBefore: {}, pinsAfter: {} };
    const legacyRequest = { environment: {}, buildEnvironment: {}, pins: {} };
    expect(
      validateNativeCapacityProtection(undefined, legacyBuild, legacyArtifact, legacyRequest),
    ).toBeUndefined();
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

const capacityProtectionNegatives: [
  string,
  (fixture: ReturnType<typeof createCapacityProtectionFixture>) => void,
][] = [
  [
    "registry count missing",
    (f) => {
      rewriteCapacityRegistry(f, (r) => {
        delete r.physicalRefCount;
      });
    },
  ],
  ...[
    ["zero", 0],
    ["negative", -1],
    ["fractional", 1.5],
    ["unsafe integer", Number.MAX_SAFE_INTEGER + 1],
  ].map(
    ([label, count]) =>
      [
        `registry count ${label}`,
        (f: ReturnType<typeof createCapacityProtectionFixture>) => {
          rewriteCapacityRegistry(f, (r) => {
            r.physicalRefCount = count as number;
          });
        },
      ] as [string, (fixture: ReturnType<typeof createCapacityProtectionFixture>) => void],
  ),
  [
    "registry wrong physical count with current SHA",
    (f) => {
      rewriteCapacityRegistry(f, (r) => {
        r.physicalRefCount = 32;
      });
    },
  ],
  [
    "registry wrong schema with current SHA",
    (f) => {
      rewriteCapacityRegistry(f, (r) => {
        r.schema = "unreviewed";
      });
    },
  ],
  [
    "registry duplicate refs with current SHA",
    (f) => {
      f.proof.refsBefore = Array(34).fill(requireFirst(f.refs, "capacity fixture refs"));
      f.proof.refsAfter = structuredClone(f.proof.refsBefore);
      rewriteCapacityRegistry(f, (r) => {
        const rows = r.refs as JsonValue[];
        r.refs = Array(34).fill(requireFirst(rows, "capacity registry refs"));
      });
    },
  ],
  [
    "registry order mismatch with current SHA",
    (f) => {
      rewriteCapacityRegistry(f, (r) => {
        r.refs = [...(r.refs as JsonValue[])].reverse();
      });
    },
  ],
  [
    "unsupported schema",
    (f) => {
      f.proof.schema = "unreviewed";
    },
  ],
  [
    "wrong physical count",
    (f) => {
      f.proof.refCount = 32;
    },
  ],
  [
    "proof count differs from registry",
    (f) => {
      f.proof.refCount = 33;
    },
  ],
  [
    "refs before count differs from registry",
    (f) => {
      f.proof.refsBefore = (f.proof.refsBefore as JsonValue[]).slice(1);
      f.proof.refsAfter = structuredClone(f.proof.refsBefore);
    },
  ],
  [
    "refs after count differs from registry",
    (f) => {
      f.proof.refsAfter = (f.proof.refsAfter as JsonValue[]).slice(1);
    },
  ],
  [
    "missing parent observation",
    (f) => {
      delete f.proof.parentEnvironmentValue;
    },
  ],
  [
    "parent mismatch",
    (f) => {
      f.proof.parentEnvironmentValue = "[]";
    },
  ],
  [
    "noncanonical environment JSON",
    (f) => {
      f.proof.environmentValue = ` ${f.environmentValue}`;
    },
  ],
  [
    "wrong environment SHA",
    (f) => {
      f.proof.environmentSha256 = "0".repeat(64);
    },
  ],
  [
    "build token mismatch",
    (f) => {
      f.proof.attemptToken = "other";
    },
  ],
  [
    "before after build start",
    (f) => {
      f.proof.beforeNs = "101";
    },
  ],
  [
    "end before build completion",
    (f) => {
      f.proof.afterNs = "149";
    },
  ],
  [
    "raised threshold",
    (f) => {
      capacityObject(f.proof.capacityPolicy).threshold_bytes = 4726095873;
    },
  ],
  [
    "wrong baseline",
    (f) => {
      capacityObject(f.proof.capacityPolicy).baseline_bytes = 1;
    },
  ],
  [
    "wrong capacity target",
    (f) => {
      capacityObject(f.proof.capacityPolicy).target = f.root;
    },
  ],
  [
    "missing refs end",
    (f) => {
      delete f.proof.refsAfter;
    },
  ],
  [
    "different refs end",
    (f) => {
      f.proof.refsAfter = [];
    },
  ],
  [
    "registry identity end drift",
    (f) => {
      capacityObject(f.proof.registryAfter).inode = 0;
    },
  ],
  [
    "physical mode drift",
    (f) => {
      const ref = requireFirst(f.refs, "capacity fixture refs");
      fs.chmodSync(ref.path, ref.mode === 0o600 ? 0o644 : 0o600);
    },
  ],
  [
    "physical bytes drift",
    (f) => {
      fs.appendFileSync(requireFirst(f.refs, "capacity fixture refs").path, "changed");
    },
  ],
  [
    "replaced inode with same bytes",
    (f) => {
      const ref = requireFirst(f.refs, "capacity fixture refs");
      const replacement = path.join(f.root, "replacement");
      fs.writeFileSync(replacement, fs.readFileSync(ref.path));
      fs.renameSync(replacement, ref.path);
    },
  ],
  [
    "symlink reference",
    (f) => {
      const ref = requireFirst(f.refs, "capacity fixture refs");
      const moved = path.join(f.root, "moved-ref");
      fs.renameSync(ref.path, moved);
      fs.symlinkSync(moved, ref.path);
    },
  ],
  [
    "registry contents drift",
    (f) => {
      fs.appendFileSync(f.registryPath, " ");
    },
  ],
  [
    "outer parent env mismatch",
    (f) => {
      capacityObject(f.build.environment).CARGO_CAPACITY_PROTECTED_REFS = "[]";
    },
  ],
  [
    "request outer env mismatch",
    (f) => {
      capacityObject(f.request.buildEnvironment).CARGO_CAPACITY_PROTECTED_REFS = "[]";
    },
  ],
  [
    "request Cargo env mismatch",
    (f) => {
      capacityObject(f.request.environment).CARGO_CAPACITY_PROTECTED_REFS = "[]";
    },
  ],
  [
    "actual Cargo env missing",
    (f) => {
      delete capacityObject(capacityObject(f.artifact.ownedCargo).environment)
        .CARGO_CAPACITY_PROTECTED_REFS;
    },
  ],
  [
    "request pins missing",
    (f) => {
      delete capacityObject(f.request.pins).capacityProtection;
    },
  ],
  [
    "Cargo before pins missing",
    (f) => {
      delete capacityObject(f.artifact.pinsBefore).capacityProtection;
    },
  ],
  [
    "Cargo end pins missing",
    (f) => {
      delete capacityObject(f.artifact.pinsAfter).capacityProtection;
    },
  ],
];
for (const [label, mutate] of capacityProtectionNegatives) {
  test(`capacity protection rejects ${label}`, () => {
    const fixture = createCapacityProtectionFixture();
    try {
      mutate(fixture);
      expect(() =>
        validateNativeCapacityProtection(
          fixture.proof,
          fixture.build,
          fixture.artifact,
          fixture.request,
        ),
      ).toThrow();
    } finally {
      fs.rmSync(fixture.root, { recursive: true, force: true });
    }
  });
}

function createBuildFixture(fresh = true, metadataLabel = "probe-3-cargo-metadata") {
  const root = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), "katana-native-build-receipt-")),
  );
  const observerScriptPath = path.join(root, "cargo_observer.py");
  fs.writeFileSync(observerScriptPath, "# synthetic pinned observer code\n");
  const repository = path.join(root, "repo");
  fs.mkdirSync(path.join(repository, "src"), { recursive: true });
  fs.writeFileSync(path.join(repository, "Cargo.lock"), "version = 4\n");
  fs.writeFileSync(
    path.join(repository, "Cargo.toml"),
    '[workspace]\nmembers = ["crates/katana-render-runtime-cli"]\n',
  );
  fs.writeFileSync(path.join(repository, "src/main.rs"), "fn main() {}\n");
  const packageManifestPath = path.join(repository, "crates/katana-render-runtime-cli/Cargo.toml");
  const targetSourcePath = path.join(repository, "crates/katana-render-runtime-cli/src/main.rs");
  fs.mkdirSync(path.dirname(targetSourcePath), { recursive: true });
  fs.writeFileSync(packageManifestPath, '[package]\nname = "katana-render-runtime-cli"\n');
  fs.writeFileSync(targetSourcePath, "fn main() {}\n");
  execFileSync("git", ["init", "--quiet"], { cwd: repository });
  execFileSync("git", ["add", "Cargo.lock", "Cargo.toml", "src/main.rs", "crates"], {
    cwd: repository,
  });

  const binaryPath = path.join(root, "bin", "krr");
  const runtimePath = path.join(root, "runtime", "mermaid.js");
  const toolPaths = {
    rtk: path.join(root, "toolchain/rtk"),
    just: path.join(root, "toolchain/just"),
    git: path.join(root, "toolchain/git"),
    cargo: path.join(root, "toolchain/cargo"),
    rustc: path.join(root, "toolchain/rustc"),
    rustup: path.join(root, "toolchain/rustup"),
    python3: path.join(root, "toolchain/python3"),
    bash: path.join(root, "toolchain/bash"),
  };
  const cargoPath = toolPaths.cargo;
  const rustcPath = toolPaths.rustc;
  const stdoutPath = path.join(root, "logs", "build.stdout");
  const stderrPath = path.join(root, "logs", "build.stderr");
  for (const filePath of [
    binaryPath,
    runtimePath,
    ...Object.values(toolPaths),
    stdoutPath,
    stderrPath,
  ]) {
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
  }
  fs.writeFileSync(binaryPath, "synthetic binary");
  fs.writeFileSync(runtimePath, "synthetic runtime");
  for (const [name, filePath] of Object.entries(toolPaths))
    fs.writeFileSync(filePath, `synthetic ${name}`);
  fs.writeFileSync(
    stdoutPath,
    `Compiling katana-render-runtime-cli v0.4.0 (${path.join(repository, "crates/katana-render-runtime-cli")})\n`,
  );
  const rawGuard = {
    ...capacityReport(repository),
    cleanup_status: "not-needed",
    removed: [],
  };
  fs.writeFileSync(stderrPath, `${JSON.stringify(rawGuard)}\n`);

  const sourceFiles = [
    path.join(repository, "Cargo.lock"),
    path.join(repository, "Cargo.toml"),
    path.join(repository, "src/main.rs"),
    packageManifestPath,
    targetSourcePath,
  ];
  const sourceRecords = sortedRecords(sourceFiles);
  const sourceDigest = digestFileRecords(sourceRecords);
  const manifestPath = path.join(root, "source-manifest.json");
  fs.writeFileSync(
    manifestPath,
    JSON.stringify({
      schema: "krr-native-build-source-manifest-v1",
      digest: sourceDigest,
      files: sourceRecords,
    }),
  );
  const manifestSha256 = fileSha256(manifestPath);
  const childEnvironment = {
    PATH: "/usr/bin:/bin",
    HOME: root,
    RUSTFLAGS: "-D warnings",
  };
  const environment = {
    ...childEnvironment,
    CARGO: `${toolPaths.python3} ${observerScriptPath}`,
    KRR_BUILD_OBSERVER_REQUEST: path.join(root, "logs/cargo-observer/request.json"),
    PYTHONDONTWRITEBYTECODE: "1",
  };
  const binary = recordFile(binaryPath);
  const binaryStats = fs.statSync(binaryPath, { bigint: true });
  const cargoLock = recordFile(path.join(repository, "Cargo.lock"));
  const runtime = recordFile(runtimePath);
  const stdout = recordFile(stdoutPath);
  const stderr = recordFile(stderrPath);
  const rustc = recordFile(rustcPath);
  const cargo = recordFile(cargoPath);
  const attemptToken = "build-attempt-contract-token";
  const packageId = `path+file://${repository}/crates/katana-render-runtime-cli#katana-render-runtime-cli@0.4.0`;
  const expectedPackage = {
    id: packageId,
    name: "katana-render-runtime-cli",
    manifestPath: packageManifestPath,
  };
  const expectedTarget = { name: "krr", kind: ["bin"], srcPath: targetSourcePath };
  const artifact = {
    reason: "compiler-artifact",
    package_id: packageId,
    target: { name: "krr", kind: ["bin"], src_path: targetSourcePath },
    profile: { test: false },
    features: [],
    filenames: [binaryPath],
    executable: binaryPath,
    fresh,
  };
  const buildFinished = { reason: "build-finished", success: true };
  const rawJsonPath = path.join(root, "logs", "cargo-messages.jsonl");
  const rawStderrPath = path.join(root, "logs", "cargo.stderr");
  fs.writeFileSync(rawJsonPath, `${JSON.stringify(artifact)}\n${JSON.stringify(buildFinished)}\n`);
  fs.writeFileSync(rawStderrPath, "");
  const rawJson = recordFile(rawJsonPath);
  const rawStderr = recordFile(rawStderrPath);
  const metadataPath = path.join(root, "logs", "cargo-metadata.json");
  fs.writeFileSync(
    metadataPath,
    JSON.stringify({
      version: 1,
      workspace_root: repository,
      packages: [
        {
          id: packageId,
          name: "katana-render-runtime-cli",
          manifest_path: packageManifestPath,
          targets: [{ name: "krr", kind: ["bin"], src_path: targetSourcePath }],
        },
      ],
    }),
  );
  const metadata = recordFile(metadataPath);
  const metadataStderrPath = path.join(root, "logs", "cargo-metadata.stderr");
  fs.writeFileSync(metadataStderrPath, "");
  const metadataObservationPath = path.join(root, "logs", "cargo-metadata-observation.json");
  const metadataObservation = {
    label: metadataLabel,
    argv: [
      toolPaths.rtk,
      "proxy",
      cargoPath,
      "metadata",
      "--frozen",
      "--format-version",
      "1",
      "--no-deps",
      "--manifest-path",
      path.join(repository, "Cargo.toml"),
    ],
    cwd: repository,
    pid: 27180,
    attemptToken,
    environment: childEnvironment,
    environmentSha256: digestStringRecord(childEnvironment),
    actualExit: 0,
    startedNs: "80",
    endedNs: "99",
    reaped: true,
    errors: [],
    stdout: metadata,
    stderr: recordFile(metadataStderrPath),
  };
  fs.writeFileSync(metadataObservationPath, JSON.stringify(metadataObservation));
  const metadataObservationFile = recordFile(metadataObservationPath);
  const sourceManifest = recordFile(manifestPath);
  const pins = {
    source: sourceRecords,
    toolchain: { rustc, cargo, rustcVersionVv: "rustc 1.90.0\n", cargoVersion: "cargo 1.90.0\n" },
    runtimeAssetPins: [runtime],
    codePins: [recordFile(observerScriptPath)],
  };
  const toolchain = pins.toolchain;
  const codeManifestPath = path.join(root, "reviewed-code-manifest.json");
  fs.writeFileSync(
    codeManifestPath,
    JSON.stringify({ schema: "krr-native-build-producer-code-v1", files: pins.codePins }),
  );
  const reviewedCodeManifest = recordFile(codeManifestPath);
  const statePath = path.join(root, "state.json");
  fs.writeFileSync(statePath, JSON.stringify({ diagnostic: true }));
  const rtkRecord = recordFile(toolPaths.rtk);
  const justRecord = recordFile(toolPaths.just);
  const resolutionPins = Object.fromEntries(
    Object.entries(toolPaths).map(([name, filePath]) => [
      name,
      { invocationPath: path.join("/usr/local/bin", name), actual: recordFile(filePath) },
    ]),
  );
  const snapshotPins = {
    runtime: [runtime],
    toolchain,
    resolutions: resolutionPins,
    state: recordFile(statePath),
    code: pins.codePins,
    reviewedCodeManifest,
    config: { unsupportedCargoConfigsAbsent: [], supplementalFileHashes: [] },
    cargoLock,
  };
  const snapshotBeforePath = path.join(root, "snapshot-before.json");
  const snapshotAfterPath = path.join(root, "snapshot-after.json");
  const snapshotBase = {
    files: sourceRecords,
    cargoLock,
    toolchain,
    runtimeAssetPins: [runtime],
    binaryIdentity: null,
    pins: snapshotPins,
  };
  fs.writeFileSync(
    snapshotBeforePath,
    JSON.stringify({ ...snapshotBase, label: "before", binary: null }),
  );
  fs.writeFileSync(snapshotAfterPath, JSON.stringify({ ...snapshotBase, label: "after", binary }));
  const observationPins = {
    before: recordFile(snapshotBeforePath),
    after: recordFile(snapshotAfterPath),
  };
  const requestPath = path.join(root, "logs", "cargo-observer", "request.json");
  fs.mkdirSync(path.dirname(requestPath), { recursive: true });
  fs.writeFileSync(
    requestPath,
    JSON.stringify({
      schema: "krr-native-cargo-observer-request-v1",
      attemptToken,
      sourceManifest,
      sourceManifestSha256: manifestSha256,
      sourceDigest,
      expectedPackage,
      expectedTarget,
      cargo,
      metadata,
      metadataObservation: metadataObservationFile,
      cwd: repository,
      binaryPath,
      buildEnvironment: environment,
      environment: childEnvironment,
      pins,
    }),
  );
  const request = recordFile(requestPath);
  const observerReceiptPath = path.join(root, "logs", "cargo-observer.json");
  const ownedCargo = {
    argv: [
      cargoPath,
      "build",
      "-p",
      "katana-render-runtime-cli",
      "--message-format=json-render-diagnostics",
    ],
    cwd: repository,
    pid: 27183,
    actualExit: 0,
    attemptToken,
    environment: childEnvironment,
    environmentSha256: digestStringRecord(childEnvironment),
    stdout: rawJson,
    stderr: rawStderr,
    startedNs: "101",
    endedNs: "149",
    reaped: true,
    errors: [],
  };
  const observer = {
    schema: "krr-native-cargo-observer-receipt-v1",
    status: "completed",
    diagnosticOnly: true,
    accepted: false,
    attemptToken,
    request,
    rawJson,
    rawStderr,
    cargo,
    metadata,
    metadataObservation: metadataObservationFile,
    sourceManifestSha256: manifestSha256,
    sourceDigest,
    expectedPackage,
    expectedTarget,
    ownedCargo,
    artifact,
    buildFinished,
    pinsBefore: pins,
    pinsAfter: pins,
    errors: [],
  };
  fs.writeFileSync(observerReceiptPath, JSON.stringify(observer));
  const observerReceipt = recordFile(observerReceiptPath);
  const receiptPath = path.join(root, "build-receipt.json");
  fs.writeFileSync(
    receiptPath,
    JSON.stringify({
      schema: "krr-native-build-receipt-v1",
      status: "completed",
      diagnosticOnly: true,
      accepted: false,
      formalAccepted: false,
      childIssued: true,
      errors: [],
      codePins: pins.codePins,
      reviewedCodeManifest,
      observationPins,
      scope: "Synthetic filesystem contract fixture only; no native build or render evidence",
      atomicSourceFreezeObserved: false,
      registryContentAttested: false,
      build: {
        argv: [rtkRecord.path, "proxy", justRecord.path, "krr-build"],
        cwd: repository,
        pid: 27182,
        attemptToken,
        actualExit: 0,
        label: "build",
        reaped: true,
        errors: [],
        startedNs: "100",
        endedNs: "150",
        stdout,
        stderr,
        flags: ["build", "-p", "katana-render-runtime-cli"],
        environment,
        environmentSha256: digestStringRecord(environment),
      },
      binary,
      cargoLock,
      toolchain,
      source: {
        manifestPath,
        manifestSha256,
        sourceDigest,
        filesBefore: sourceRecords,
        filesAfter: sourceRecords,
      },
      capacityPreflight: capacityReport(repository),
      capacityPost: capacityReport(repository),
      binaryProvenance: {
        binaryBefore: null,
        binaryAfter: {
          ...binary,
          device: Number(binaryStats.dev),
          inode: Number(binaryStats.ino),
          mtimeNs: binaryStats.mtimeNs.toString(),
          ctimeNs: binaryStats.ctimeNs.toString(),
        },
        guard: rawGuard,
        rawGuard,
      },
      compilerArtifact: {
        schema: "krr-native-cargo-artifact-v1",
        attemptToken,
        observerReceipt,
        request,
        rawJson,
        rawStderr,
        cargo,
        metadata,
        metadataObservation: metadataObservationFile,
        sourceManifestSha256: manifestSha256,
        sourceDigest,
        expectedPackage,
        expectedTarget,
        ownedCargo,
        artifact,
        buildFinished,
        pinsBefore: pins,
        pinsAfter: pins,
      },
      runtimeAssetPins: [runtime],
    }),
  );
  return { root, repository, receiptPath, binaryPath, runtimePath, binary, runtime };
}

function sortedRecords(filePaths: string[]): FileRecord[] {
  return filePaths
    .map(recordFile)
    .sort((left, right) =>
      Buffer.compare(Buffer.from(left.path, "utf8"), Buffer.from(right.path, "utf8")),
    );
}

function recordFile(filePath: string): FileRecord {
  const contents = fs.readFileSync(filePath);
  return { path: filePath, sha256: sha256(contents), size: contents.length };
}

function capacityReport(repository: string) {
  return {
    metric: "unique-inode-allocated-bytes",
    metric_version: 1,
    baseline_bytes: 2363047936,
    factor: 2,
    threshold_bytes: 4726095872,
    current_bytes: 1024,
    over_capacity: false,
    metric_available: true,
    target: path.join(repository, "target"),
  };
}
