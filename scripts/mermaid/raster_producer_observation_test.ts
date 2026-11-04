import { afterAll, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  browserMismatchProjection,
  parseOwnedBrowserMeasurement,
  uniqueOwnBrowserPid,
} from "./raster-browser-observation";
import {
  type CaptureSnapshot,
  closeCaptureObservation,
  type LoadedSvgObservation,
} from "./raster-capture-observation";
import { physicalFileIdentity } from "./raster-comparison-binding";
import type { JsonObject } from "./raster-json-input";
import { rasterSha256 } from "./raster-production-binding";
import {
  assertScreenshotPublishedBytes,
  RasterProductionSession,
  writeCandidateRasterSessionConfig,
  writeInitialOfficialRasterSessionConfig,
  writeOfficialRasterSessionConfig,
} from "./raster-production-session";

// 実browser/CDP/OS adapterを呼ばない。ここで使うPIDとDOMは合成単位入力である。
const configuredRoot = process.env.MERMAID_RASTER_PRODUCER_TEST_ROOT;
const allowedParents = [fs.realpathSync(os.tmpdir()), fs.realpathSync(path.resolve("tmp"))];
let fixtureParent = fs.realpathSync(os.tmpdir());
if (configuredRoot !== undefined) {
  if (!path.isAbsolute(configuredRoot)) {
    throw new Error("MERMAID_RASTER_PRODUCER_TEST_ROOT must be an absolute existing TMP directory");
  }
  fixtureParent = fs.realpathSync(configuredRoot);
  const isScoped = allowedParents.some((allowed) => {
    const relative = path.relative(allowed, fixtureParent);
    return (
      relative === "" ||
      (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative))
    );
  });
  if (!isScoped) {
    throw new Error(
      "MERMAID_RASTER_PRODUCER_TEST_ROOT must remain under an approved TMP directory",
    );
  }
}
const root = fs.mkdtempSync(path.join(fixtureParent, "katana-mermaid-raster-producer-test-"));
afterAll(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

function processSample() {
  return {
    schema: "katana-macos-owned-browser-process-v1",
    platform: "darwin",
    beforeEndEqual: true,
    browser: {
      pid: 103,
      parentPid: 102,
      startIdentity: "unit-browser",
      executablePath: "/unit/browser",
      argv: ["/unit/browser", "--unit"],
    },
    producer: { pid: 101, parentPid: 100, startIdentity: "unit-producer" },
    collector: { pid: 104, parentPid: 101, startIdentity: "unit-collector" },
    parentChain: [
      { pid: 102, parentPid: 101, startIdentity: "unit-driver" },
      { pid: 101, parentPid: 100, startIdentity: "unit-producer" },
    ],
  };
}
function sessionFixture() {
  const directory = fs.mkdtempSync(path.join(root, "unit-")),
    outputDir = path.join(directory, "output");
  fs.mkdirSync(outputDir);
  const endpoints = (["producer", "playwright", "lock", "runtime", "font", "config"] as const).map(
    (kind) => {
      const filePath = path.join(directory, kind);
      fs.writeFileSync(filePath, `unit ${kind}`);
      return { name: kind, kind, version: "unit-only", path: filePath };
    },
  );
  const runtimeEndpoint = endpoints[3];
  if (runtimeEndpoint === undefined) throw new Error("Missing synthetic runtime endpoint");
  const options = {
    role: "official" as const,
    attempt: outputDir,
    outputDir,
    nonce: "unit-fresh-nonce",
    inventory: ["unit"],
    observerPython: runtimeEndpoint.path,
    endpoints,
  };
  const config = {
    schema: "katana-mermaid-raster-producer-input-v1",
    diagnosticOnly: true,
    accepted: false,
    ...options,
  };
  const configPath = path.join(directory, "external-input.json");
  const save = () => {
    const bytes = Buffer.from(JSON.stringify(config));
    fs.writeFileSync(configPath, bytes);
    return rasterSha256(bytes);
  };
  return { options, config, configPath, save, outputDir, directory };
}

function officialSeedFixture(f: ReturnType<typeof sessionFixture>, slugs = ["alpha", "zeta"]) {
  const directory = fs.mkdtempSync(path.join(f.directory, "official-seed-")),
    fixturesDir = path.join(directory, "fixtures"),
    attempt = path.join(directory, "attempt"),
    outputDir = path.join(directory, "output"),
    seedOptionsPath = path.join(directory, "seed-options.json"),
    configPath = path.join(directory, "official-session.json");
  fs.mkdirSync(fixturesDir);
  for (const slug of slugs) {
    fs.writeFileSync(
      path.join(fixturesDir, `${slug}.md`),
      `# ${slug}\n\n\`\`\`mermaid\nclassDiagram\n\`\`\`\n`,
    );
  }
  fs.mkdirSync(attempt);
  fs.mkdirSync(outputDir);
  fs.writeFileSync(
    seedOptionsPath,
    JSON.stringify({
      schema: "katana-mermaid-raster-session-seed-v1",
      observerPython: f.options.observerPython,
      endpoints: f.options.endpoints.filter((row) => row.kind !== "config"),
    }),
  );
  return { directory, fixturesDir, attempt, outputDir, seedOptionsPath, configPath };
}

function runOfficialSeedCli(
  input: ReturnType<typeof officialSeedFixture>,
  extraArgs: string[] = [],
) {
  return Bun.spawnSync([
    "bun",
    path.join(import.meta.dir, "raster-production-session.ts"),
    "--seed-official",
    "--seed-options",
    input.seedOptionsPath,
    "--fixtures",
    input.fixturesDir,
    "--attempt",
    input.attempt,
    "--output",
    input.outputDir,
    "--config",
    input.configPath,
    ...extraArgs,
  ]);
}
test("browser mismatch projection exposes only fixed comparisons", () => {
  const directory = fs.mkdtempSync(path.join(root, "browser-identity-"));
  const first = path.join(directory, "first"),
    second = path.join(directory, "second");
  fs.writeFileSync(first, "same bytes");
  fs.writeFileSync(second, "same bytes");
  const projection = browserMismatchProjection(physicalFileIdentity(first), second, 3);
  expect(projection).toEqual({
    schema: "katana-browser-mismatch-projection-v1",
    pidAndAncestryVerified: true,
    observationBeforeEndEqual: true,
    argc: 3,
    osIdentityAvailable: true,
    realpathEqual: false,
    deviceInodeEqual: false,
    sha256Equal: true,
  });
  expect(JSON.stringify(projection)).not.toContain(directory);
  expect(
    browserMismatchProjection(physicalFileIdentity(first), path.join(directory, "missing"), 3),
  ).toMatchObject({
    osIdentityAvailable: false,
    realpathEqual: null,
    deviceInodeEqual: null,
    sha256Equal: null,
  });
});
test("own-browser CDP selects only the unique browser PID", () => {
  expect(
    uniqueOwnBrowserPid({
      processInfo: [
        { type: "renderer", id: 201 },
        { type: "browser", id: 103 },
      ],
    }),
  ).toBe(103);
  for (const input of [
    {},
    { processInfo: [] },
    {
      processInfo: [
        { type: "browser", id: 103 },
        { type: "browser", id: 104 },
      ],
    },
    { processInfo: [{ type: "browser", id: 1.5 }] },
  ])
    expect(() => uniqueOwnBrowserPid(input)).toThrow();
});
test("owned OS identity parser retains PID/start/argv and linked producer ancestry", () => {
  const parsed = parseOwnedBrowserMeasurement(processSample(), 103, 101);
  expect(parsed.parentChain).toEqual([
    { pid: 102, startIdentity: "unit-driver" },
    { pid: 101, startIdentity: "unit-producer" },
  ]);
  expect(parsed.argv).toEqual(["/unit/browser", "--unit"]);
});
type ProcessMutation = "pid" | "closure" | "parent" | "start" | "collector";
function mutateProcessSample(
  input: ReturnType<typeof processSample>,
  field: ProcessMutation,
): void {
  switch (field) {
    case "pid":
      input.browser.pid = 105;
      return;
    case "closure":
      input.beforeEndEqual = false;
      return;
    case "parent": {
      const parent = input.parentChain[0];
      if (parent === undefined) throw new Error("Missing synthetic browser parent");
      parent.parentPid = 999;
      return;
    }
    case "start": {
      const producer = input.parentChain[1];
      if (producer === undefined) throw new Error("Missing synthetic producer parent");
      producer.startIdentity = "reused-producer";
      return;
    }
    case "collector":
      input.collector.parentPid = 999;
  }
}
test("OS identity rejects PID mismatch, missing closure and foreign parent chain", () => {
  for (const field of ["pid", "closure", "parent", "start", "collector"] as const) {
    const input = processSample();
    mutateProcessSample(input, field);
    expect(() => parseOwnedBrowserMeasurement(input, 103, 101)).toThrow();
  }
});
test("screenshot returned Buffer must equal both temporary and published bytes", () => {
  const original = Buffer.from("unit-image"),
    changed = Buffer.from("white-replacement");
  expect(() => assertScreenshotPublishedBytes(original, original, original)).not.toThrow();
  expect(() => assertScreenshotPublishedBytes(original, changed, original)).toThrow();
  expect(() => assertScreenshotPublishedBytes(original, original, changed)).toThrow();
});
function captureSample(): CaptureSnapshot {
  return {
    domHtml: "unit capture",
    svgHtml: "unit svg",
    bounds: { x: 0, y: 0, width: 10, height: 20 },
    viewport: { width: 1520, height: 845 },
    deviceScaleFactor: 1,
    theme: "dark",
    fontsReady: true,
    imagesDecoded: true,
  };
}
const rawSvg = Buffer.from("synthetic expected raw SVG bytes");
function loadedSample(): LoadedSvgObservation {
  return {
    policy: "browser-domparser-html-svg-outerhtml-v1",
    inputSha256: rasterSha256(rawSvg),
    expectedSvgSerialization: captureSample().svgHtml,
    assignedSvgSerialization: captureSample().svgHtml,
  };
}
test("capture closure derives hashes from independently expected and actually observed DOM", () => {
  const before = captureSample(),
    actual = closeCaptureObservation(rawSvg, loadedSample(), before, structuredClone(before));
  expect(actual.domSha256Before).toBe(actual.domSha256After);
  expect(actual.resized).toBe(false);
  expect(actual.svg.inputSha256).toBe(rasterSha256(rawSvg));
  expect(actual.svg.expectedDomSha256).toBe(actual.svg.beforeDomSha256);
  expect(() =>
    closeCaptureObservation(Buffer.from("different expected raw"), loadedSample(), before, before),
  ).toThrow("input/DOM binding");
});
test("capture closure rejects DOM/bounds/viewport/scale/theme/readiness drift", () => {
  for (const field of ["dom", "bounds", "viewport", "scale", "theme", "fonts", "images"] as const) {
    const before = captureSample(),
      after = structuredClone(before);
    if (field === "dom") after.domHtml = "changed";
    if (field === "bounds") after.bounds.width++;
    if (field === "viewport") after.viewport.width++;
    if (field === "scale") after.deviceScaleFactor = 2;
    if (field === "theme") after.theme = "light";
    if (field === "fonts") after.fontsReady = false;
    if (field === "images") after.imagesDecoded = false;
    expect(() => closeCaptureObservation(rawSvg, loadedSample(), before, after)).toThrow();
  }
});
test("session refuses to issue a receipt without actual browser observation", () => {
  const f = sessionFixture(),
    session = new RasterProductionSession(f.options);
  session.markBrowserClosed();
  expect(() => session.finish()).toThrow("actual browser/capture closure");
  expect(fs.existsSync(path.join(f.outputDir, "raster-production-receipt.json"))).toBe(false);
});
test("source input consumption and runtime physical changes fail closed", () => {
  const f = sessionFixture(),
    session = new RasterProductionSession(f.options),
    file = path.join(f.directory, "input.md");
  fs.writeFileSync(file, "raw input");
  session.pinInput(file, Buffer.from("raw input"));
  expect(() => session.pinInput(file, Buffer.from("normalized input"))).toThrow("Consumed");
  fs.writeFileSync(file, "changed input");
  expect(() => session.pinInput(file, Buffer.from("changed input"))).toThrow("changed");
});
test("producer input config requires an external pin and rejects static browser identity", () => {
  const f = sessionFixture(),
    originalSha = f.save();
  expect(() => RasterProductionSession.fromPinnedConfig(f.configPath, originalSha)).not.toThrow();
  f.config.nonce = "replayed";
  f.save();
  expect(() => RasterProductionSession.fromPinnedConfig(f.configPath, originalSha)).toThrow(
    "config SHA",
  );
  const injected: JsonObject = f.config;
  injected.browserProcess = processSample();
  expect(() => RasterProductionSession.fromPinnedConfig(f.configPath, f.save())).toThrow("fields");
});

test("candidate session config reuses pinned official endpoints and derives exact fixture inventory", () => {
  const f = sessionFixture(),
    fixturesDir = path.join(f.directory, "fixtures"),
    attempt = path.join(f.directory, "candidate-attempt"),
    outputDir = path.join(f.directory, "candidate-output"),
    configPath = path.join(f.directory, "candidate-session.json");
  fs.mkdirSync(fixturesDir);
  fs.writeFileSync(path.join(fixturesDir, "zeta.md"), "# Zeta\n\n```mermaid\nclassDiagram\n```\n");
  fs.writeFileSync(
    path.join(fixturesDir, "alpha.md"),
    "# Alpha\n\n```mermaid\nclassDiagram\n```\n",
  );
  fs.mkdirSync(attempt);
  fs.mkdirSync(outputDir);
  const result = writeCandidateRasterSessionConfig({
    templateConfigPath: f.configPath,
    templateConfigSha256: f.save(),
    fixturesDir,
    attempt,
    outputDir,
    configPath,
  });
  const bytes = fs.readFileSync(configPath),
    config = JSON.parse(bytes.toString("utf8")) as JsonObject;
  expect(result.sha256).toBe(rasterSha256(bytes));
  expect(result.role).toBe("candidate");
  expect(result.inventory).toEqual(["alpha", "zeta"]);
  expect(config).toMatchObject({
    schema: "katana-mermaid-raster-producer-input-v1",
    diagnosticOnly: true,
    accepted: false,
    role: "candidate",
    attempt,
    outputDir,
    inventory: ["alpha", "zeta"],
    endpoints: f.options.endpoints,
  });
  expect(config.nonce).toEqual(expect.any(String));
  expect(Object.keys(config).sort()).toEqual(
    [
      "schema",
      "diagnosticOnly",
      "accepted",
      "role",
      "attempt",
      "nonce",
      "inventory",
      "outputDir",
      "observerPython",
      "endpoints",
    ].sort(),
  );
  expect(() => RasterProductionSession.fromPinnedConfig(configPath, result.sha256)).not.toThrow();
});

test("official session config shares pinned template validation and emits fresh exact inputs", () => {
  const f = sessionFixture(),
    fixturesDir = path.join(f.directory, "official-fixtures"),
    attempt = path.join(f.directory, "official-attempt"),
    outputDir = path.join(f.directory, "official-output"),
    configPath = path.join(f.directory, "official-session.json");
  fs.mkdirSync(fixturesDir);
  fs.writeFileSync(path.join(fixturesDir, "zeta.md"), "# Zeta\n\n```mermaid\nclassDiagram\n```\n");
  fs.writeFileSync(
    path.join(fixturesDir, "alpha.md"),
    "# Alpha\n\n```mermaid\nclassDiagram\n```\n",
  );
  fs.mkdirSync(attempt);
  fs.mkdirSync(outputDir);
  const result = writeOfficialRasterSessionConfig({
    templateConfigPath: f.configPath,
    templateConfigSha256: f.save(),
    fixturesDir,
    attempt,
    outputDir,
    configPath,
  });
  const bytes = fs.readFileSync(configPath),
    config = JSON.parse(bytes.toString("utf8")) as JsonObject;
  expect(result.role).toBe("official");
  expect(result.inventory).toEqual(["alpha", "zeta"]);
  expect(config).toMatchObject({
    schema: "katana-mermaid-raster-producer-input-v1",
    diagnosticOnly: true,
    accepted: false,
    role: "official",
    attempt,
    outputDir,
    inventory: ["alpha", "zeta"],
    endpoints: f.options.endpoints,
  });
  expect(config.nonce).not.toBe(f.options.nonce);
  expect(fs.statSync(configPath).mode & 0o777).toBe(0o600);
  expect(() => RasterProductionSession.fromPinnedConfig(configPath, result.sha256)).not.toThrow();

  const cliAttempt = path.join(f.directory, "cli-candidate-attempt"),
    cliOutput = path.join(f.directory, "cli-candidate-output"),
    cliConfigPath = path.join(f.directory, "cli-candidate-session.json");
  fs.mkdirSync(cliAttempt);
  fs.mkdirSync(cliOutput);
  const cliResult = Bun.spawnSync([
    "bun",
    path.join(import.meta.dir, "raster-production-session.ts"),
    "--template",
    f.configPath,
    "--template-sha256",
    f.save(),
    "--fixtures",
    fixturesDir,
    "--attempt",
    cliAttempt,
    "--output",
    cliOutput,
    "--config",
    cliConfigPath,
  ]);
  expect(cliResult.exitCode).toBe(0);
  expect(JSON.parse(fs.readFileSync(cliConfigPath, "utf8"))).toMatchObject({
    role: "candidate",
    inventory: ["alpha", "zeta"],
    attempt: cliAttempt,
    outputDir: cliOutput,
  });

  const officialCliAttempt = path.join(f.directory, "cli-official-attempt"),
    officialCliOutput = path.join(f.directory, "cli-official-output"),
    officialCliConfigPath = path.join(f.directory, "cli-official-session.json");
  fs.mkdirSync(officialCliAttempt);
  fs.mkdirSync(officialCliOutput);
  const officialCliResult = Bun.spawnSync([
    "bun",
    path.join(import.meta.dir, "raster-production-session.ts"),
    "--template",
    f.configPath,
    "--template-sha256",
    f.save(),
    "--fixtures",
    fixturesDir,
    "--attempt",
    officialCliAttempt,
    "--output",
    officialCliOutput,
    "--config",
    officialCliConfigPath,
    "--role",
    "official",
  ]);
  expect(officialCliResult.exitCode).toBe(0);
  expect(JSON.parse(fs.readFileSync(officialCliConfigPath, "utf8"))).toMatchObject({
    role: "official",
    inventory: ["alpha", "zeta"],
    attempt: officialCliAttempt,
    outputDir: officialCliOutput,
  });
});

test("initial official session CLI seeds strict config without a prior template", () => {
  const f = sessionFixture(),
    input = officialSeedFixture(f),
    result = runOfficialSeedCli(input);
  expect(result.exitCode).toBe(0);
  const written = JSON.parse(result.stdout.toString("utf8")) as JsonObject,
    bytes = fs.readFileSync(input.configPath),
    config = JSON.parse(bytes.toString("utf8")) as JsonObject;
  expect(written).toMatchObject({
    path: input.configPath,
    role: "official",
    inventory: ["alpha", "zeta"],
  });
  expect(config).toMatchObject({
    schema: "katana-mermaid-raster-producer-input-v1",
    diagnosticOnly: true,
    accepted: false,
    role: "official",
    attempt: input.attempt,
    outputDir: input.outputDir,
    inventory: ["alpha", "zeta"],
  });
  expect(config.nonce).toEqual(expect.any(String));
  expect(config.endpoints).toContainEqual({
    name: "official-session-seed-options",
    kind: "config",
    version: "katana-mermaid-raster-session-seed-v1",
    path: input.seedOptionsPath,
  });
  expect(fs.statSync(input.configPath).mode & 0o777).toBe(0o600);
  expect(written.sha256).toBe(rasterSha256(bytes));
  expect(() =>
    RasterProductionSession.fromPinnedConfig(input.configPath, written.sha256 as string),
  ).not.toThrow();

  const retryAttempt = path.join(input.directory, "retry-attempt"),
    retryOutput = path.join(input.directory, "retry-output");
  fs.mkdirSync(retryAttempt);
  fs.mkdirSync(retryOutput);
  const overwrite = runOfficialSeedCli({
    ...input,
    attempt: retryAttempt,
    outputDir: retryOutput,
  });
  expect(overwrite.exitCode).toBe(1);
  expect(overwrite.stderr.toString()).toContain("new file");
  expect(fs.readFileSync(input.configPath)).toEqual(bytes);
});

test("initial official session excludes empty Mermaid fences from its inventory", () => {
  const f = sessionFixture(),
    input = officialSeedFixture(f);
  fs.writeFileSync(path.join(input.fixturesDir, "empty.md"), "# empty\n\n```mermaid\n \n\t\n```\n");
  const result = runOfficialSeedCli(input);
  expect(result.exitCode).toBe(0);
  const config = JSON.parse(fs.readFileSync(input.configPath, "utf8")) as JsonObject;
  expect(config.inventory).toEqual(["alpha", "zeta"]);
});

test("official seed API derives tool options only from its pinned seed file", () => {
  const f = sessionFixture(),
    input = officialSeedFixture(f),
    configPath = path.join(input.directory, "direct-api-session.json"),
    result = writeInitialOfficialRasterSessionConfig(
      Object.assign({}, input, {
        configPath,
        observerPython: path.join(input.directory, "unbound-python"),
        endpoints: [],
      }),
    ),
    session = RasterProductionSession.fromPinnedConfig(configPath, result.sha256);
  expect(session.options.observerPython).toBe(f.options.observerPython);
  expect(session.options.endpoints).toEqual([
    ...f.options.endpoints.filter((row) => row.kind !== "config"),
    {
      name: "official-session-seed-options",
      kind: "config",
      version: "katana-mermaid-raster-session-seed-v1",
      path: input.seedOptionsPath,
    },
  ]);
});

test("initial official session seed rejects role injection and incomplete or changed endpoints", () => {
  const f = sessionFixture(),
    wrongRole = officialSeedFixture(f),
    roleResult = runOfficialSeedCli(wrongRole, ["--role", "candidate"]);
  expect(roleResult.exitCode).toBe(1);
  expect(roleResult.stderr.toString()).toContain("supported named values");

  const missingEndpoint = officialSeedFixture(f),
    manifest = JSON.parse(fs.readFileSync(missingEndpoint.seedOptionsPath, "utf8")) as JsonObject;
  manifest.endpoints = (manifest.endpoints as JsonObject[]).filter((row) => row.kind !== "font");
  fs.writeFileSync(missingEndpoint.seedOptionsPath, JSON.stringify(manifest));
  const missingResult = runOfficialSeedCli(missingEndpoint);
  expect(missingResult.exitCode).toBe(1);
  expect(missingResult.stderr.toString()).toContain("physical tool endpoints are incomplete");
  expect(fs.existsSync(missingEndpoint.configPath)).toBe(false);

  const changedEndpoint = officialSeedFixture(f),
    changedManifest = JSON.parse(
      fs.readFileSync(changedEndpoint.seedOptionsPath, "utf8"),
    ) as JsonObject,
    runtimeRow = (changedManifest.endpoints as JsonObject[]).find((row) => row.kind === "runtime");
  if (runtimeRow === undefined || typeof runtimeRow.path !== "string") {
    throw new Error("Synthetic runtime endpoint missing");
  }
  const movedRuntimePath = `${runtimeRow.path}.moved`;
  fs.renameSync(runtimeRow.path, movedRuntimePath);
  fs.symlinkSync(movedRuntimePath, runtimeRow.path);
  const changedResult = runOfficialSeedCli(changedEndpoint);
  expect(changedResult.exitCode).toBe(1);
  expect(changedResult.stderr.toString()).toContain("regular file");
  expect(fs.existsSync(changedEndpoint.configPath)).toBe(false);
});

test("initial official session seed rejects empty inventories and unknown manifest fields", () => {
  const f = sessionFixture(),
    emptyInventory = officialSeedFixture(f, []),
    emptyResult = runOfficialSeedCli(emptyInventory);
  expect(emptyResult.exitCode).toBe(1);
  expect(emptyResult.stderr.toString()).toContain("Candidate fixture inventory must be nonempty");
  expect(fs.existsSync(emptyInventory.configPath)).toBe(false);

  const unknownField = officialSeedFixture(f),
    manifest = JSON.parse(fs.readFileSync(unknownField.seedOptionsPath, "utf8")) as JsonObject;
  manifest.role = "candidate";
  fs.writeFileSync(unknownField.seedOptionsPath, JSON.stringify(manifest));
  const unknownResult = runOfficialSeedCli(unknownField);
  expect(unknownResult.exitCode).toBe(1);
  expect(unknownResult.stderr.toString()).toContain("Unexpected raster producer config fields");
  expect(fs.existsSync(unknownField.configPath)).toBe(false);
});

test("raster config CLI rejects unsupported and duplicate options before reading inputs", () => {
  const script = path.join(import.meta.dir, "raster-production-session.ts"),
    base = [
      "--template",
      "missing",
      "--template-sha256",
      "missing",
      "--fixtures",
      "missing",
      "--attempt",
      "missing",
      "--output",
      "missing",
      "--config",
      "missing",
    ],
    unsupported = Bun.spawnSync(["bun", script, ...base, "--unknown", "value"]),
    unsupportedRole = Bun.spawnSync(["bun", script, ...base, "--role", "other"]),
    duplicate = Bun.spawnSync([
      "bun",
      script,
      ...base,
      "--role",
      "official",
      "--role",
      "candidate",
    ]);
  expect(unsupported.exitCode).toBe(1);
  expect(unsupported.stderr.toString()).toContain("supported named values");
  expect(unsupportedRole.exitCode).toBe(1);
  expect(unsupportedRole.stderr.toString()).toContain("role must be official or candidate");
  expect(duplicate.exitCode).toBe(1);
  expect(duplicate.stderr.toString()).toContain("Duplicate --role");
});

test("candidate session config rejects stale templates and nonfresh output paths", () => {
  const f = sessionFixture(),
    fixturesDir = path.join(f.directory, "fixtures");
  fs.mkdirSync(fixturesDir);
  fs.writeFileSync(path.join(fixturesDir, "one.md"), "# One\n\n```mermaid\nclassDiagram\n```\n");
  const attempt = path.join(f.directory, "candidate-attempt"),
    outputDir = path.join(f.directory, "candidate-output");
  fs.mkdirSync(attempt);
  fs.mkdirSync(outputDir);
  const input = {
    templateConfigPath: f.configPath,
    templateConfigSha256: f.save(),
    fixturesDir,
    attempt,
    outputDir,
    configPath: path.join(f.directory, "candidate-session.json"),
  };
  expect(() =>
    writeCandidateRasterSessionConfig({ ...input, templateConfigSha256: "0".repeat(64) }),
  ).toThrow("config SHA");
  fs.writeFileSync(path.join(outputDir, "stale"), "old output");
  expect(() => writeCandidateRasterSessionConfig(input)).toThrow("fresh and empty");
});
test("producer cannot read its orchestration config from the output it controls", () => {
  const f = sessionFixture(),
    configPath = path.join(f.outputDir, "self-input.json"),
    bytes = Buffer.from(JSON.stringify(f.config));
  fs.writeFileSync(configPath, bytes);
  expect(() => RasterProductionSession.fromPinnedConfig(configPath, rasterSha256(bytes))).toThrow(
    "outside producer output",
  );
});

// IR-R4-P1-02: 代入後の最初の観測から別SVGが安定していてもexpected inputとの相違を拒否する。
test("wrong stable SVG from the first capture observation cannot authenticate itself", () => {
  const before = { ...captureSample(), svgHtml: "different wrong SVG" },
    after = structuredClone(before);
  expect(() => closeCaptureObservation(rawSvg, loadedSample(), before, after)).toThrow(
    "expected parsed input",
  );
  const loaded = { ...loadedSample(), assignedSvgSerialization: before.svgHtml };
  expect(() => closeCaptureObservation(rawSvg, loaded, before, after)).toThrow("input/DOM binding");
});
test("expected and assigned DOM are separate observations, including parser normalization", () => {
  const loaded = {
    ...loadedSample(),
    expectedSvgSerialization: '<svg width="1"></svg>',
    assignedSvgSerialization: '<svg width="1"></svg>',
  };
  const before = { ...captureSample(), svgHtml: loaded.expectedSvgSerialization };
  expect(() => closeCaptureObservation(rawSvg, loaded, before, before)).not.toThrow();
  expect(() =>
    closeCaptureObservation(
      rawSvg,
      { ...loaded, assignedSvgSerialization: '<svg width="2"></svg>' },
      before,
      before,
    ),
  ).toThrow();
});
test("SVG mutation before or after screenshot rejects instead of becoming a resize claim", () => {
  const original = captureSample();
  for (const point of ["before", "after"] as const) {
    const before = structuredClone(original),
      after = structuredClone(original);
    (point === "before" ? before : after).svgHtml = "wrong SVG";
    expect(() => closeCaptureObservation(rawSvg, loadedSample(), before, after)).toThrow(
      "expected parsed input",
    );
  }
});
