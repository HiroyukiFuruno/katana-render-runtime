import { afterAll, expect, spyOn, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { prepareNestedRuntimeAsset, rasterCaptureFixture } from "./raster_binding_test_fixtures";
import {
  parseRasterComparisonConfigCli,
  physicalFileIdentity,
  RasterComparisonBinding,
  writeRasterComparisonConfig,
} from "./raster-comparison-binding";
import type { JsonInput, JsonObject } from "./raster-json-input";
import {
  createRasterProductionReceipt,
  type RasterCaptureState,
  type RasterEndpoint,
  type RasterPairObservation,
  type RasterProductionReceipt,
  type RasterRole,
  rasterSha256,
} from "./raster-production-binding";
import { CliOptions } from "./reference_compare_options";
import { rawHtmlSvgViewBoxSize, svgViewBoxSize } from "./svg-viewbox-size";

// この合成fixtureは外部信頼境界の拒否検査専用で、実producerの観測証拠には使わない。
const configuredFixtureRoot = process.env.MERMAID_RASTER_BINDING_TEST_ROOT;
const allowedFixtureParents = [fs.realpathSync(os.tmpdir()), fs.realpathSync(path.resolve("tmp"))];
let fixtureParent = fs.realpathSync(os.tmpdir());
if (configuredFixtureRoot !== undefined) {
  if (!path.isAbsolute(configuredFixtureRoot)) {
    throw new Error("MERMAID_RASTER_BINDING_TEST_ROOT must be an absolute existing TMP directory");
  }
  fixtureParent = fs.realpathSync(configuredFixtureRoot);
  const isScoped = allowedFixtureParents.some((allowed) => {
    const relative = path.relative(allowed, fixtureParent);
    return (
      relative === "" ||
      (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative))
    );
  });
  if (!isScoped) {
    throw new Error("MERMAID_RASTER_BINDING_TEST_ROOT must remain under an approved TMP directory");
  }
}
const fixtureRoot = fs.mkdtempSync(path.join(fixtureParent, "katana-mermaid-raster-binding-test-"));
afterAll(() => {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
});
const hash = (text: string) => rasterSha256(Buffer.from(text));
const svg = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>');
const png = Buffer.from("unit PNG bytes: not a real browser capture");

function fixture() {
  const directory = fs.mkdtempSync(path.join(fixtureRoot as string, "synthetic-"));
  const config = {
    viewport: { width: 1, height: 1, deviceScaleFactor: 1 },
    theme: "dark",
    referenceMode: "strictVendorOriginal",
    captureResize: "capture-box-only-raw-svg-v1",
    sourceFontUrls: [],
    externalImageWaitPolicy: "svg-image-load-decode-positive-dimensions-and-frame-v2",
  };
  const capture = rasterCaptureFixture(svg, "DOM");
  const endpoints: RasterEndpoint[] = (
    ["producer", "browser", "playwright", "lock", "runtime", "font", "config"] as const
  ).map((kind) => {
    const filePath = path.join(directory, kind);
    fs.writeFileSync(filePath, `synthetic ${kind}`);
    const identity = physicalFileIdentity(filePath);
    return {
      name: kind,
      kind,
      version: "synthetic-unit-only",
      before: identity,
      end: { ...identity },
    };
  });
  const manifest = {
    sourceSvgSha256: rasterSha256(svg),
    renderedSvgSha256: rasterSha256(svg),
    pngSha256: rasterSha256(png),
    configSha256: hash(JSON.stringify(config)),
  };
  const pair = (role: RasterRole): RasterPairObservation => ({
    slug: "unit",
    role,
    sourceSvg: svg,
    renderedSvg: svg,
    png,
    publishedPng: png,
    pngDimensions: { width: 1, height: 1 },
    config,
    manifest,
    rawSvgSha256: hash("raw"),
    stageSvgSha256: manifest.renderedSvgSha256,
    canonicalSourceSha256: hash("canonical"),
    provenanceSha256: hash("provenance"),
    loadedSvgSha256: manifest.sourceSvgSha256,
    capture,
  });
  const roleBinding = (role: RasterRole) => {
    const observed = pair(role);
    const receipt: RasterProductionReceipt = {
      schema: "katana-mermaid-raster-production-v1",
      diagnosticOnly: true,
      accepted: false,
      producerSucceeded: true,
      role,
      attempt: path.join(directory, role),
      nonce: `${role}-synthetic-nonce`,
      inventory: ["unit"],
      endpoints,
      rows: [
        {
          slug: "unit",
          ...manifest,
          loadedSvgSha256: observed.loadedSvgSha256,
          rawSvgSha256: observed.rawSvgSha256,
          stageSvgSha256: observed.stageSvgSha256,
          canonicalSourceSha256: observed.canonicalSourceSha256,
          provenanceSha256: observed.provenanceSha256,
          publishedPngSha256: manifest.pngSha256,
          png: observed.pngDimensions,
          capture,
        },
      ],
    };
    const bytes = createRasterProductionReceipt(receipt);
    const receiptPath = path.join(directory, `${role}-receipt.json`);
    fs.writeFileSync(receiptPath, bytes);
    const manifestBytes = Buffer.from(
      JSON.stringify({ ...manifest, role, runtimeAssets: ["unit-runtime"] }),
    );
    fs.writeFileSync(path.join(directory, `${role}.pair.json`), manifestBytes);
    const browser = endpoints.find((row) => row.kind === "browser");
    if (browser === undefined) throw new Error("Missing unit browser endpoint");
    return {
      receiptPath,
      receiptSha256: rasterSha256(bytes),
      attempt: receipt.attempt,
      nonce: receipt.nonce,
      inventory: receipt.inventory,
      observedEndpoints: endpoints,
      observedPairManifests: [{ slug: "unit", sha256: rasterSha256(manifestBytes) }],
      observedCaptures: [{ slug: "unit", loadedSvgSha256: observed.loadedSvgSha256, capture }],
      browserProcess: {
        pid: 102,
        startIdentity: "unit-browser-start",
        producerPid: 101,
        producerStartIdentity: "100.000001",
        parentChain: [{ pid: 101, startIdentity: "100.000001" }],
        executable: browser.end,
        argv: [browser.end.path, "--unit-only"],
        version: browser.version,
        beforeEndEqual: true,
        producerExitCode: 0,
        reaped: true,
        groupGone: true,
      },
    };
  };
  const trustedConfig = {
    schema: "katana-mermaid-trusted-raster-comparison-v1",
    diagnosticOnly: true,
    accepted: false,
    trustedRepositoryRoot: directory,
    roles: { official: roleBinding("official"), candidate: roleBinding("candidate") },
  };
  const configPath = path.join(directory, "externally-pinned-unit-config.json");
  const save = () => {
    const bytes = Buffer.from(JSON.stringify(trustedConfig));
    fs.writeFileSync(configPath, bytes);
    return rasterSha256(bytes);
  };
  const originalExpectedSha = save();
  return { directory, trustedConfig, configPath, originalExpectedSha, save, pair, config };
}

function comparisonBuilderFixture() {
  const f = fixture(),
    fixturesDir = path.join(f.directory, "fixtures");
  fs.mkdirSync(fixturesDir);
  fs.writeFileSync(path.join(fixturesDir, "unit.md"), "# Unit\n\n```mermaid\nclassDiagram\n```\n");
  const buildProcessEvidence = (role: RasterRole) => {
    const trusted = f.trustedConfig.roles[role],
      process = trusted.browserProcess,
      ownerPath = path.join(f.directory, `${role}-owner.json`),
      lifecyclePath = path.join(f.directory, `${role}-lifecycle.json`),
      observationPaths = [0, 1].map((index) =>
        path.join(f.directory, `${role}-browser-${index}.json`),
      ),
      proofPath = path.join(f.directory, `${role}-process-evidence.json`),
      sessionConfigPath = path.join(f.directory, `${role}-session.json`),
      observerPythonPath = path.join(f.directory, `${role}-observer.py`),
      owner = {
        schema: "krr-native-cli-owner-observation-v1",
        label: `mermaid-raster-${role}`,
        cwd: f.directory,
        pid: process.producerPid,
        startedNs: "100000000000",
        endedNs: "100000010000",
        actualExit: 0,
        reaped: true,
        errors: [],
      },
      lifecycle = {
        schema: "krr-native-cli-lifecycle-v1",
        identity: {
          pid: process.producerPid,
          processGroupId: process.producerPid,
          sessionId: process.producerPid,
        },
        exitCode: 0,
        groupClosed: true,
        rawFdsClosed: true,
        failure: null,
        cleanupFailure: null,
      };
    fs.writeFileSync(ownerPath, JSON.stringify(owner));
    fs.writeFileSync(lifecyclePath, JSON.stringify(lifecycle));
    fs.writeFileSync(observerPythonPath, "# synthetic observer\n");
    const sessionConfig = {
      schema: "katana-mermaid-raster-producer-input-v1",
      diagnosticOnly: true,
      accepted: false,
      role,
      attempt: trusted.attempt,
      nonce: trusted.nonce,
      inventory: ["unit"],
      outputDir: f.directory,
      observerPython: observerPythonPath,
      endpoints: trusted.observedEndpoints,
    };
    fs.writeFileSync(sessionConfigPath, JSON.stringify(sessionConfig));
    const observation = (index: number) => ({
      schema: "katana-own-browser-identity-observation-v1",
      role,
      attempt: trusted.attempt,
      nonce: trusted.nonce,
      cdpBrowserPid: process.pid,
      osMeasurement: {
        schema: "katana-macos-owned-browser-process-v1",
        platform: "darwin",
        beforeEndEqual: true,
        producer: { pid: process.producerPid, startIdentity: "100.000001" },
        browser: { pid: process.pid, startIdentity: process.startIdentity, argv: process.argv },
        collector: { pid: 103 + index, parentPid: process.producerPid },
      },
      collectorExitCode: 0,
      collectorReaped: true,
      collectorPid: 103 + index,
      python: { path: "/usr/bin/python3" },
      collector: { path: "/tmp/collector.py" },
      endpoint: trusted.observedEndpoints.find((row) => row.kind === "browser"),
    });
    observationPaths.forEach((filePath, index) => {
      fs.writeFileSync(filePath, JSON.stringify(observation(index)));
    });
    const pin = (filePath: string) => {
      const identity = physicalFileIdentity(filePath);
      return { path: identity.path, sha256: identity.sha256, size: identity.size };
    };
    const proof = {
      schema: "krr-mermaid-raster-process-evidence-v1",
      diagnosticOnly: true,
      accepted: false,
      role,
      attempt: trusted.attempt,
      nonce: trusted.nonce,
      config: pin(sessionConfigPath),
      receipt: {
        path: trusted.receiptPath,
        sha256: rasterSha256(fs.readFileSync(trusted.receiptPath)),
        size: fs.statSync(trusted.receiptPath).size,
      },
      ownerObservation: pin(ownerPath),
      lifecycle: pin(lifecyclePath),
      browserObservations: observationPaths.map(pin),
      browserProcess: process,
      descendantClosureProved: false,
    };
    fs.writeFileSync(proofPath, JSON.stringify(proof));
    fs.copyFileSync(
      path.join(f.directory, `${role}.pair.json`),
      path.join(f.directory, "unit.pair.json"),
    );
    return {
      path: proofPath,
      sha256: rasterSha256(Buffer.from(JSON.stringify(proof))),
      size: fs.statSync(proofPath).size,
    };
  };
  const processEvidence = {
    official: buildProcessEvidence("official"),
    candidate: buildProcessEvidence("candidate"),
  };
  return {
    f,
    fixturesDir,
    processEvidence,
    input: {
      trustedRepositoryRoot: f.directory,
      fixturesDir,
      configPath: path.join(f.directory, "comparison-builder-config.json"),
      official: {
        receiptPath: f.trustedConfig.roles.official.receiptPath,
        receiptSha256: rasterSha256(fs.readFileSync(f.trustedConfig.roles.official.receiptPath)),
        processEvidencePath: processEvidence.official.path,
        processEvidenceSha256: processEvidence.official.sha256,
        pairDirectory: f.directory,
      },
      candidate: {
        receiptPath: f.trustedConfig.roles.candidate.receiptPath,
        receiptSha256: rasterSha256(fs.readFileSync(f.trustedConfig.roles.candidate.receiptPath)),
        processEvidencePath: processEvidence.candidate.path,
        processEvidenceSha256: processEvidence.candidate.sha256,
        pairDirectory: f.directory,
      },
    },
  };
}

function comparisonBuilderCliArgs(
  input: ReturnType<typeof comparisonBuilderFixture>["input"],
): string[] {
  return [
    "--trusted-repository-root",
    input.trustedRepositoryRoot,
    "--fixtures",
    input.fixturesDir,
    "--config",
    input.configPath,
    "--official-receipt",
    input.official.receiptPath,
    "--official-receipt-sha256",
    input.official.receiptSha256,
    "--official-process-evidence",
    input.official.processEvidencePath,
    "--official-process-sha256",
    input.official.processEvidenceSha256,
    "--official-pair-directory",
    input.official.pairDirectory,
    "--candidate-receipt",
    input.candidate.receiptPath,
    "--candidate-receipt-sha256",
    input.candidate.receiptSha256,
    "--candidate-process-evidence",
    input.candidate.processEvidencePath,
    "--candidate-process-sha256",
    input.candidate.processEvidenceSha256,
    "--candidate-pair-directory",
    input.candidate.pairDirectory,
  ];
}

test("external caller pin permits only synthetic role bindings, without acceptance promotion", () => {
  const f = fixture();
  const binding = RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"]);
  binding.verifyRole("official", [f.pair("official")]);
  binding.verifyRole("candidate", [f.pair("candidate")]);
  binding.assertUnchanged();
  expect(f.trustedConfig.accepted).toBe(false);
});
test("comparison config builder binds both roles to lifecycle and browser proof", () => {
  const prepared = comparisonBuilderFixture();
  const result = writeRasterComparisonConfig(prepared.input);
  expect(result.inventory).toEqual(["unit"]);
  expect(result.sha256).toBe(rasterSha256(fs.readFileSync(result.path)));
  expect(() => RasterComparisonBinding.read(result.path, result.sha256, ["unit"])).not.toThrow();
});
test("comparison config builder CLI writes the same pinned config as the managed API", () => {
  const prepared = comparisonBuilderFixture();
  prepared.input.configPath = path.join(prepared.f.directory, "comparison-cli-config.json");
  const result = parseRasterComparisonConfigCli(comparisonBuilderCliArgs(prepared.input));
  expect(result.path).toBe(prepared.input.configPath);
  expect(result.inventory).toEqual(["unit"]);
  expect(result.sha256).toBe(rasterSha256(fs.readFileSync(result.path)));
});
test("comparison config builder CLI rejects missing required values and unknown options", () => {
  const missing = comparisonBuilderFixture();
  missing.input.configPath = path.join(missing.f.directory, "comparison-cli-missing.json");
  expect(() =>
    parseRasterComparisonConfigCli(comparisonBuilderCliArgs(missing.input).slice(0, -2)),
  ).toThrow();
  expect(fs.existsSync(missing.input.configPath)).toBe(false);

  const unknown = comparisonBuilderFixture();
  unknown.input.configPath = path.join(unknown.f.directory, "comparison-cli-unknown.json");
  expect(() =>
    parseRasterComparisonConfigCli([
      ...comparisonBuilderCliArgs(unknown.input),
      "--unknown-option",
      "value",
    ]),
  ).toThrow();
  expect(fs.existsSync(unknown.input.configPath)).toBe(false);
});
test.each([
  [
    "owner exit",
    (proof: JsonObject) =>
      (proof.ownerObservation = { path: "", sha256: hash("missing"), size: 0 }),
  ],
  [
    "lifecycle closure",
    (proof: JsonObject) => (proof.lifecycle = { path: "", sha256: hash("missing"), size: 0 }),
  ],
  [
    "browser observation reuse",
    (proof: JsonObject) => {
      const observations = proof.browserObservations as JsonInput[];
      proof.browserObservations = [observations[0], observations[0]];
    },
  ],
  ["collector-only proof", (proof: JsonObject) => delete proof.ownerObservation],
] as const)("comparison config builder rejects %s", (_name, mutate) => {
  const prepared = comparisonBuilderFixture(),
    proofPath = prepared.processEvidence.official.path,
    proof = JSON.parse(fs.readFileSync(proofPath, "utf8")) as JsonObject;
  mutate(proof);
  fs.writeFileSync(proofPath, JSON.stringify(proof));
  expect(() => writeRasterComparisonConfig(prepared.input)).toThrow();
});
test.each([
  [
    "owner exit",
    (owner: JsonObject, _lifecycle: JsonObject, _process: JsonObject): void => {
      owner.actualExit = 1;
    },
  ],
  [
    "owner timestamp",
    (owner: JsonObject, _lifecycle: JsonObject, _process: JsonObject): void => {
      owner.endedNs = "99999999999";
    },
  ],
  [
    "lifecycle raw fd closure",
    (_owner: JsonObject, lifecycle: JsonObject, _process: JsonObject): void => {
      lifecycle.rawFdsClosed = false;
    },
  ],
  [
    "producer start identity",
    (_owner: JsonObject, _lifecycle: JsonObject, process: JsonObject): void => {
      process.producerStartIdentity = "101.000001";
    },
  ],
  [
    "browser argv",
    (_owner: JsonObject, _lifecycle: JsonObject, process: JsonObject): void => {
      process.argv = ["/wrong/browser"];
    },
  ],
] as const)("comparison config builder rejects proof %s", (_name, mutate) => {
  const prepared = comparisonBuilderFixture(),
    proofPath = prepared.processEvidence.official.path,
    proof = JSON.parse(fs.readFileSync(proofPath, "utf8")) as JsonObject,
    ownerPin = proof.ownerObservation as JsonObject,
    lifecyclePin = proof.lifecycle as JsonObject,
    process = proof.browserProcess as JsonObject,
    owner = JSON.parse(fs.readFileSync(String(ownerPin.path), "utf8")) as JsonObject,
    lifecycle = JSON.parse(fs.readFileSync(String(lifecyclePin.path), "utf8")) as JsonObject;
  mutate(owner, lifecycle, process);
  fs.writeFileSync(String(ownerPin.path), JSON.stringify(owner));
  fs.writeFileSync(String(lifecyclePin.path), JSON.stringify(lifecycle));
  const repin = (filePath: string) => {
    const identity = physicalFileIdentity(filePath);
    return { path: identity.path, sha256: identity.sha256, size: identity.size };
  };
  proof.ownerObservation = repin(String(ownerPin.path));
  proof.lifecycle = repin(String(lifecyclePin.path));
  fs.writeFileSync(proofPath, JSON.stringify(proof));
  prepared.input.official.processEvidenceSha256 = rasterSha256(fs.readFileSync(proofPath));
  expect(() => writeRasterComparisonConfig(prepared.input)).toThrow();
});
test("config mutation cannot refresh its own externally supplied SHA", () => {
  const f = fixture();
  f.trustedConfig.roles.official.nonce = "replay";
  f.save();
  expect(() => RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"])).toThrow(
    "config SHA mismatch",
  );
});
test("receipt replacement is rejected even while the external config stays fixed", () => {
  const f = fixture();
  fs.appendFileSync(f.trustedConfig.roles.official.receiptPath, " ");
  expect(() => RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"])).toThrow(
    "receipt SHA mismatch",
  );
});
test("white PNG plus refreshed manifest cannot match original pinned output", () => {
  const f = fixture();
  const binding = RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"]);
  const changedPng = Buffer.from("white replacement"),
    original = f.pair("official");
  expect(() =>
    binding.verifyRole("official", [
      {
        ...original,
        png: changedPng,
        publishedPng: changedPng,
        manifest: { ...original.manifest, pngSha256: rasterSha256(changedPng) },
      },
    ]),
  ).toThrow("PNG differs");
});
test("missing real-process fields fail closed even under a new synthetic unit pin", () => {
  const f = fixture();
  const config: Partial<typeof f.trustedConfig.roles.official> = f.trustedConfig.roles.official;
  delete config.browserProcess;
  const expected = f.save();
  expect(() => RasterComparisonBinding.read(f.configPath, expected, ["unit"])).toThrow();
});
test("external browser ancestry and executable closure are mandatory", () => {
  for (const change of ["ancestry", "executable", "cleanup"] as const) {
    const f = fixture(),
      closure = f.trustedConfig.roles.official.browserProcess;
    if (change === "ancestry") {
      const parent = closure.parentChain[0];
      if (parent === undefined) throw new Error("Missing synthetic browser parent");
      parent.startIdentity = "unrelated";
    }
    if (change === "executable") closure.argv[0] = "/wrong/browser";
    if (change === "cleanup") closure.groupGone = false;
    expect(() => RasterComparisonBinding.read(f.configPath, f.save(), ["unit"])).toThrow();
  }
});
test("external inventory and capture observations cannot be silently taken from sidecar", () => {
  const f = fixture();
  const capture = f.trustedConfig.roles.official.observedCaptures[0];
  if (capture === undefined) throw new Error("Missing synthetic capture observation");
  capture.loadedSvgSha256 = hash("different input");
  expect(() => RasterComparisonBinding.read(f.configPath, f.save(), ["unit"])).toThrow(
    "capture observations differ",
  );
  const clean = fixture();
  expect(() =>
    RasterComparisonBinding.read(clean.configPath, clean.originalExpectedSha, ["extra", "unit"]),
  ).toThrow("inventory differs");
});
test("physical browser endpoint changed after capture is rejected", () => {
  const f = fixture();
  fs.appendFileSync(f.trustedConfig.roles.official.browserProcess.executable.path, "changed");
  expect(() => RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"])).toThrow(
    "physical endpoint differs",
  );
});
test("official-only adapter rejects stale physical endpoints before candidate binding", () => {
  const f = comparisonBuilderFixture();
  const lock = f.f.trustedConfig.roles.official.observedEndpoints.find(
    (endpoint) => endpoint.kind === "lock",
  );
  if (lock === undefined) throw new Error("Missing synthetic lock endpoint");
  fs.appendFileSync(lock.end.path, "changed");
  expect(() =>
    RasterComparisonBinding.validateOfficialRole(f.input.trustedRepositoryRoot, f.input.official, [
      "unit",
    ]),
  ).toThrow("physical endpoint differs");
});
test("official-only adapter accepts the complete role and pair inventory", () => {
  const f = comparisonBuilderFixture();
  expect(() =>
    RasterComparisonBinding.validateOfficialRole(f.input.trustedRepositoryRoot, f.input.official, [
      "unit",
    ]),
  ).not.toThrow();
});
test("official-only adapter rejects a tampered official pair artifact before candidate binding", () => {
  const f = comparisonBuilderFixture();
  fs.unlinkSync(path.join(f.input.official.pairDirectory, "unit.pair.json"));
  expect(() =>
    RasterComparisonBinding.validateOfficialRole(f.input.trustedRepositoryRoot, f.input.official, [
      "unit",
    ]),
  ).toThrow();
});
test("official-only adapter rejects a tampered official loaded SVG digest before candidate binding", () => {
  const f = comparisonBuilderFixture();
  const receiptPath = f.input.official.receiptPath;
  const receipt = JSON.parse(fs.readFileSync(receiptPath, "utf8")) as {
    rows: Array<{ loadedSvgSha256: string }>;
  };
  const row = receipt.rows[0];
  if (row === undefined) throw new Error("Missing synthetic official SVG row");
  row.loadedSvgSha256 = "f".repeat(64);
  fs.writeFileSync(receiptPath, JSON.stringify(receipt));
  expect(() =>
    RasterComparisonBinding.validateOfficialRole(f.input.trustedRepositoryRoot, f.input.official, [
      "unit",
    ]),
  ).toThrow();
});
test("comparison input drift is rejected after initial read", () => {
  const f = fixture(),
    binding = RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"]);
  const input = path.join(f.directory, "published.png");
  fs.writeFileSync(input, png);
  binding.readObservedFile(input);
  fs.writeFileSync(input, "replacement");
  expect(() => binding.assertUnchanged()).toThrow("endpoint drift");
});
test("comparison config leaf inode replacement rejects unchanged bytes SHA size and mode", () => {
  const f = fixture(),
    binding = RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"]);
  const before = physicalFileIdentity(f.configPath),
    bytes = fs.readFileSync(f.configPath),
    replacement = `${f.configPath}.replacement`;
  fs.writeFileSync(replacement, bytes, { flag: "wx", mode: before.mode });
  fs.chmodSync(replacement, before.mode);
  fs.renameSync(replacement, f.configPath);
  const after = physicalFileIdentity(f.configPath);
  expect(fs.readFileSync(f.configPath)).toEqual(bytes);
  expect({ ...after, ino: before.ino }).toEqual(before);
  expect(after.ino).not.toBe(before.ino);
  expect(() => binding.readObservedFile(f.configPath)).toThrow("input changed");
  expect(() => binding.assertUnchanged()).toThrow("endpoint drift");
});
test("symlink and directory inputs cannot stand in for pinned regular files", () => {
  const f = fixture(),
    symlink = path.join(f.directory, "symlink");
  fs.symlinkSync(f.configPath, symlink);
  expect(() => physicalFileIdentity(symlink)).toThrow("regular file");
  expect(() => physicalFileIdentity(f.directory)).toThrow("regular file");
});

function physicalReadDiagnostic(filePath: string): Record<string, unknown> {
  try {
    physicalFileIdentity(filePath);
  } catch (error) {
    if (!(error instanceof Error)) throw error;
    const prefix = `Raster comparison input changed while reading: ${filePath} `;
    if (!error.message.startsWith(prefix)) throw error;
    return JSON.parse(error.message.slice(prefix.length)) as Record<string, unknown>;
  }
  throw new Error("expected physical read rejection");
}

test("physical read rejection reports short bytes and evaluated stamps", () => {
  const f = fixture();
  const filePath = path.join(f.directory, "short-read");
  fs.writeFileSync(filePath, "abcd");
  const read = spyOn(fs, "readFileSync").mockReturnValueOnce(Buffer.from("ab"));
  try {
    const details = physicalReadDiagnostic(filePath);
    expect(details.bytesLength).toBe(2);
    expect(details.expectedSize).toBe("4");
    expect(details.canonicalBefore).toBe(fs.realpathSync(filePath));
    expect(details.canonicalAfter).toBe(details.canonicalBefore);
    expect(details.fdAfter).toEqual(details.before);
    expect(details.after).toEqual(details.before);
  } finally {
    read.mockRestore();
  }
});

test("physical read rejection keeps skipped canonical observation null", () => {
  const f = fixture();
  const filePath = path.join(f.directory, "changed-fd");
  fs.writeFileSync(filePath, "abcd");
  const original = fs.statSync(filePath, { bigint: true });
  const changed = Object.create(original) as fs.BigIntStats;
  Object.defineProperty(changed, "size", { value: original.size + 1n });
  const stat = spyOn(fs, "fstatSync").mockReturnValueOnce(original).mockReturnValueOnce(changed);
  try {
    const details = physicalReadDiagnostic(filePath);
    expect(details.canonicalAfter).toBeNull();
    expect(details.fdAfter).toMatchObject({ size: 5 });
    expect(details.bytesLength).toBe(4);
    expect(details.expectedSize).toBe("4");
  } finally {
    stat.mockRestore();
  }
});

test("strict CLI requires a caller-supplied path and SHA together", () => {
  expect(() => CliOptions.parse([])).toThrow("externally pinned");
  expect(() => CliOptions.parse(["--raster-binding-config", "/trusted/config.json"])).toThrow(
    "supplied together",
  );
  expect(() => CliOptions.parse(["--raster-binding-config-sha", hash("config")])).toThrow(
    "supplied together",
  );
  const options = CliOptions.parse([
    "--raster-binding-config",
    "/trusted/config.json",
    "--raster-binding-config-sha",
    hash("config"),
  ]);
  expect(options.rasterBindingConfigSha256).toBe(hash("config"));
});

test("manifest-only metadata replacement is rejected against external whole-file pin", () => {
  const f = fixture(),
    binding = RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"]);
  const file = path.join(f.directory, "official.pair.json");
  const manifest = JSON.parse(fs.readFileSync(file, "utf8")) as JsonObject;
  manifest.runtimeAssets = [];
  fs.writeFileSync(file, JSON.stringify(manifest));
  expect(() => binding.readPairManifest("official", "unit", file)).toThrow("pair manifest differs");
});

function repinSyntheticCapture(
  f: ReturnType<typeof fixture>,
  role: RasterRole,
  actual: RasterCaptureState,
  declared = f.config,
) {
  const binding = f.trustedConfig.roles[role];
  const receipt = JSON.parse(
    fs.readFileSync(binding.receiptPath, "utf8"),
  ) as RasterProductionReceipt;
  const row = receipt.rows[0];
  if (row === undefined) throw new Error("Missing unit row");
  row.capture = actual;
  row.configSha256 = hash(JSON.stringify(declared));
  const bytes = createRasterProductionReceipt(receipt);
  fs.writeFileSync(binding.receiptPath, bytes);
  binding.receiptSha256 = rasterSha256(bytes);
  const observedCapture = binding.observedCaptures[0];
  if (observedCapture === undefined) throw new Error("Missing synthetic capture observation");
  observedCapture.capture = actual;
  const original = f.pair(role);
  return {
    ...original,
    config: declared,
    capture: actual,
    manifest: { ...original.manifest, configSha256: row.configSha256 },
  };
}

function rawViewportPair(
  f: ReturnType<typeof fixture>,
  role: RasterRole,
  rawSvg: string,
  viewport: { width: number; height: number },
) {
  const source = Buffer.from(rawSvg);
  const actual = rasterCaptureFixture(source, "raw viewport DOM");
  actual.viewport = viewport;
  actual.bounds = { x: 0, y: 0, ...viewport };
  const declared = {
    ...f.config,
    captureResize: "raw-svg-viewbox-viewport-before-assignment-v1",
    viewport: { ...viewport, deviceScaleFactor: 1 },
  };
  const pair = f.pair(role);
  const roleConfig = f.trustedConfig.roles[role];
  const receipt = JSON.parse(
    fs.readFileSync(roleConfig.receiptPath, "utf8"),
  ) as RasterProductionReceipt;
  const row = receipt.rows[0];
  if (row === undefined) throw new Error("Missing raw viewport test row");
  const sourceHash = rasterSha256(source);
  row.capture = actual;
  row.configSha256 = hash(JSON.stringify(declared));
  row.sourceSvgSha256 = sourceHash;
  row.renderedSvgSha256 = sourceHash;
  row.loadedSvgSha256 = sourceHash;
  row.stageSvgSha256 = sourceHash;
  const observed = roleConfig.observedCaptures[0];
  if (observed === undefined) throw new Error("Missing raw viewport test observation");
  observed.capture = actual;
  observed.loadedSvgSha256 = sourceHash;
  const bytes = createRasterProductionReceipt(receipt);
  fs.writeFileSync(roleConfig.receiptPath, bytes);
  roleConfig.receiptSha256 = rasterSha256(bytes);
  return {
    ...pair,
    config: declared,
    capture: actual,
    sourceSvg: source,
    renderedSvg: source,
    loadedSvgSha256: sourceHash,
    stageSvgSha256: sourceHash,
    manifest: {
      ...pair.manifest,
      sourceSvgSha256: sourceHash,
      renderedSvgSha256: sourceHash,
      configSha256: row.configSha256,
    },
  };
}

test("new viewport policy binds each verified raw SVG while allowing role dimensions to differ", () => {
  const f = fixture();
  const official = rawViewportPair(
    f,
    "official",
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100.2 80"/>',
    { width: 125, height: 104 },
  );
  const candidate = rawViewportPair(
    f,
    "candidate",
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 90"/>',
    { width: 144, height: 114 },
  );
  const binding = RasterComparisonBinding.read(f.configPath, f.save(), ["unit"]);
  expect(() => binding.verifyRole("official", [official])).not.toThrow();
  expect(() => binding.verifyRole("candidate", [candidate])).not.toThrow();
});

test("raw HTML viewport accepts Kanban xlink while native XML stays strict", () => {
  const raw =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 80"><a xlink:href="https://github.com/mermaid-js/mermaid/issues/2038"><text>issue</text></a></svg>';
  const f = fixture();
  const pair = rawViewportPair(f, "official", raw, { width: 124, height: 104 });
  const binding = RasterComparisonBinding.read(f.configPath, f.save(), ["unit"]);
  expect(() => binding.verifyRole("official", [pair])).not.toThrow();
  expect(() => svgViewBoxSize(raw)).toThrow();
  expect(rawHtmlSvgViewBoxSize(` \n<!-- comment -->${raw} `)).toEqual({ width: 100, height: 80 });
});

for (const [label, raw] of [
  ["multiple roots", '<svg viewBox="0 0 1 1"></svg><svg viewBox="0 0 1 1"></svg>'],
  ["text beside root", 'unexpected<svg viewBox="0 0 1 1"></svg>'],
  ["non SVG root", '<div viewBox="0 0 1 1"></div>'],
  ["foreign namespace", '<math><svg viewBox="0 0 1 1"></svg></math>'],
  ["incorrect SVG declaration", '<svg xmlns="urn:wrong" viewBox="0 0 1 1"></svg>'],
] as const) {
  test(`raw HTML viewport rejects ${label}`, () => {
    expect(() => rawHtmlSvgViewBoxSize(raw)).toThrow();
  });
}

for (const [rawSvg, error] of [
  ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 80"/>', "Raw SVG viewport differs"],
  ['<svg xmlns="http://www.w3.org/2000/svg"/>', "Raw SVG viewBox missing"],
  ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 0 80"/>', "invalid SVG viewBox"],
] as const) {
  test(`new viewport policy rejects inconsistent or absent raw geometry: ${rawSvg}`, () => {
    const f = fixture();
    const pair = rawViewportPair(f, "official", rawSvg, { width: 125, height: 104 });
    const binding = RasterComparisonBinding.read(f.configPath, f.save(), ["unit"]);
    expect(() => binding.verifyRole("official", [pair])).toThrow(error);
  });
}

const contextMutations: Array<{
  name: string;
  change: (actual: RasterCaptureState) => RasterCaptureState;
}> = [
  { name: "theme", change: (actual) => ({ ...actual, theme: "light" }) },
  {
    name: "viewport width",
    change: (actual) => ({ ...actual, viewport: { ...actual.viewport, width: 2 } }),
  },
  {
    name: "viewport height",
    change: (actual) => ({ ...actual, viewport: { ...actual.viewport, height: 2 } }),
  },
  { name: "device scale", change: (actual) => ({ ...actual, deviceScaleFactor: 2 }) },
  { name: "SVG resize", change: (actual) => ({ ...actual, resized: true }) },
];
for (const mutation of contextMutations) {
  test(`truthfully pinned ${mutation.name} mismatch rejects one role and both roles`, () => {
    for (const roles of [["candidate"], ["official", "candidate"]] as const) {
      const f = fixture();
      const changed = roles.map((role) =>
        repinSyntheticCapture(f, role, mutation.change(f.pair(role).capture)),
      );
      const binding = RasterComparisonBinding.read(f.configPath, f.save(), ["unit"]);
      for (const pair of changed) {
        expect(() => binding.verifyRole(pair.role, [pair])).toThrow(
          "context differs from manifest config",
        );
      }
    }
  });
}
test("both declared and actual conditions may match per role but cross-role mismatch rejects", () => {
  const f = fixture(),
    original = f.pair("candidate");
  const declared = { ...f.config, theme: "light" };
  const changed = repinSyntheticCapture(
    f,
    "candidate",
    { ...original.capture, theme: "light" },
    declared,
  );
  const binding = RasterComparisonBinding.read(f.configPath, f.save(), ["unit"]);
  binding.verifyRole("official", [f.pair("official")]);
  expect(() => binding.verifyRole("candidate", [changed])).toThrow("role capture context mismatch");
});

test("existing relative runtime assets resolve only within externally pinned canonical repository root", () => {
  const f = fixture(),
    nested = path.join(f.directory, "scripts");
  fs.mkdirSync(nested);
  const file = path.join(nested, "runtime.js");
  fs.writeFileSync(file, "unit runtime");
  const binding = RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"]);
  binding.requireRepositoryRoot(f.directory);
  expect(() => binding.requireRepositoryRoot(nested)).toThrow("anchor differs");
  expect(binding.resolveRuntimeAsset("scripts/runtime.js")).toBe(file);
  expect(binding.readObservedFile(binding.resolveRuntimeAsset("scripts/runtime.js"))).toEqual(
    Buffer.from("unit runtime"),
  );
  for (const relative of [
    "../runtime.js",
    "scripts/../runtime.js",
    "./scripts/runtime.js",
    "scripts//runtime.js",
  ]) {
    expect(() => binding.resolveRuntimeAsset(relative)).toThrow();
  }
  const alias = path.join(f.directory, "alias");
  fs.symlinkSync(nested, alias);
  expect(() => binding.resolveRuntimeAsset("alias/runtime.js")).toThrow(
    "trusted canonical repository root",
  );
});
test("trusted repository root cannot be a symlink alias even under an explicitly supplied synthetic pin", () => {
  const f = fixture(),
    alias = path.join(f.directory, "root-alias");
  fs.symlinkSync(f.directory, alias);
  f.trustedConfig.trustedRepositoryRoot = alias;
  expect(() => RasterComparisonBinding.read(f.configPath, f.save(), ["unit"])).toThrow(
    "canonical directory",
  );
});

test("preserves the exact macOS font inode when ArialHB is available", () => {
  const fontPath = "/System/Library/Fonts/ArialHB.ttc";
  if (!fs.existsSync(fontPath)) return;
  const identity = physicalFileIdentity(fontPath);
  const exact = fs.statSync(fontPath, { bigint: true });
  expect(identity.dev).toBe(exact.dev.toString());
  expect(identity.ino).toBe(exact.ino.toString());
});

// 独立IR-R4-P1-01のpath解決→祖先symlink差替→actual読取を、実FSだけで再現する。
test("resolved relative asset cannot rebaseline an outside target through an ancestor swap", () => {
  const f = fixture(),
    nested = path.join(f.directory, "scripts"),
    outside = fs.mkdtempSync(path.join(fixtureRoot as string, "outside-"));
  fs.mkdirSync(nested);
  fs.writeFileSync(path.join(nested, "runtime.js"), "same bytes");
  fs.writeFileSync(path.join(outside, "runtime.js"), "same bytes");
  const binding = RasterComparisonBinding.read(f.configPath, f.originalExpectedSha, ["unit"]);
  const resolved = binding.resolveRuntimeAsset("scripts/runtime.js");
  fs.renameSync(nested, `${nested}-saved`);
  fs.symlinkSync(outside, nested);
  expect(() => binding.readObservedFile(resolved)).toThrow("trusted canonical repository root");
});
test("relative asset ancestors stay closed at final validation even when bytes and leaf inode match", () => {
  const { binding, nested } = prepareNestedRuntimeAsset(fixture());
  binding.readObservedFile(binding.resolveRuntimeAsset("scripts/runtime.js"));
  fs.renameSync(nested, `${nested}-saved`);
  fs.mkdirSync(nested);
  fs.linkSync(path.join(`${nested}-saved`, "runtime.js"), path.join(nested, "runtime.js"));
  expect(() => binding.assertUnchanged()).toThrow("boundary changed");
});
test("ancestor alias inserted after a valid read fails final closure", () => {
  const { binding, nested } = prepareNestedRuntimeAsset(fixture());
  binding.readObservedFile(binding.resolveRuntimeAsset("scripts/runtime.js"));
  fs.renameSync(nested, `${nested}-saved`);
  fs.symlinkSync(`${nested}-saved`, nested);
  expect(() => binding.assertUnchanged()).toThrow("trusted canonical repository root");
});
