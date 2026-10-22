import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { FixtureRepository } from "./diagram_update_fixtures";
import type { MermaidPairManifest } from "./mermaid-pair-manifest";
import type { BrowserHandle } from "./official-renderer-types";
import {
  collectorPath,
  type ObservedBrowserIdentity,
  observeActualBrowserIdentity,
  requireApprovedBrowserObservationRoute,
} from "./raster-browser-observation";
import { physicalFileIdentity } from "./raster-comparison-binding";
import type { JsonInput, JsonObject } from "./raster-json-input";
import {
  createRasterProductionReceipt,
  type RasterBindingRow,
  type RasterCaptureState,
  type RasterEndpoint,
  type RasterFileIdentity,
  type RasterRole,
  rasterCaptureContext,
  rasterSha256,
} from "./raster-production-binding";

export interface RasterProductionSessionOptions {
  role: RasterRole;
  attempt: string;
  nonce: string;
  inventory: string[];
  outputDir: string;
  observerPython: string;
  endpoints: Array<{
    name: string;
    kind: Exclude<RasterEndpoint["kind"], "browser">;
    version: string;
    path: string;
  }>;
}

export function assertScreenshotPublishedBytes(
  screenshot: Buffer,
  attemptPng: Buffer,
  publishedPng: Buffer,
) {
  if (
    !Buffer.isBuffer(screenshot) ||
    !screenshot.equals(attemptPng) ||
    !screenshot.equals(publishedPng)
  ) {
    throw new Error("Screenshot Buffer, attempt PNG and published PNG differ");
  }
}

export class RasterProductionSession {
  static fromPinnedConfig(configPath: string, expectedSha256: string) {
    const identity = physicalFileIdentity(configPath),
      bytes = fs.readFileSync(configPath);
    if (
      !/^[a-f0-9]{64}$/.test(expectedSha256) ||
      rasterSha256(bytes) !== expectedSha256 ||
      identity.sha256 !== expectedSha256
    ) {
      throw new Error("Externally pinned raster producer config SHA differs");
    }
    const value = JSON.parse(bytes.toString("utf8")) as JsonInput;
    const options = parseRasterSessionConfig(value);
    const output = fs.realpathSync(options.outputDir),
      config = fs.realpathSync(configPath);
    const relative = path.relative(output, config);
    if (
      relative === "" ||
      (relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative))
    ) {
      throw new Error("Raster producer config must be outside producer output");
    }
    const session = new RasterProductionSession(options);
    session.pinInput(configPath, bytes);
    return session;
  }

  private readonly files = new Map<string, RasterFileIdentity>();
  private readonly endpoints: RasterEndpoint[];
  private readonly rows: RasterBindingRow[] = [];
  private browserBefore: ObservedBrowserIdentity | null = null;
  private browserEnd: ObservedBrowserIdentity | null = null;
  private closed = false;

  constructor(readonly options: RasterProductionSessionOptions) {
    for (const directory of [options.attempt, options.outputDir]) {
      if (!path.isAbsolute(directory) || path.resolve(directory) !== directory)
        throw new Error("Absolute fresh raster attempt/output required");
    }
    if (
      !options.nonce ||
      options.inventory.length === 0 ||
      new Set(options.inventory).size !== options.inventory.length ||
      !isDeepStrictEqual(options.inventory, [...options.inventory].sort())
    )
      throw new Error("Fresh nonce and ordered exact inventory required");
    if (
      new Set(options.endpoints.map((row) => row.name)).size !== options.endpoints.length ||
      ["producer", "playwright", "lock", "runtime", "font", "config"].some(
        (kind) => !options.endpoints.some((row) => row.kind === kind),
      )
    ) {
      throw new Error("Raster session physical tool endpoints are incomplete or duplicated");
    }
    this.endpoints = options.endpoints.map((spec) => {
      const identity = this.pin(spec.path);
      return {
        name: spec.name,
        kind: spec.kind,
        version: spec.version,
        before: identity,
        end: identity,
      };
    });
  }

  requireReady(role: RasterRole, outputDir: string, inventory: string[]) {
    if (
      this.options.role !== role ||
      this.options.outputDir !== outputDir ||
      !isDeepStrictEqual(inventory, this.options.inventory)
    ) {
      throw new Error("Raster session role/output/exact inventory differs");
    }
    requireApprovedBrowserObservationRoute(this.options.observerPython);
    this.pin(this.options.observerPython);
    this.pin(collectorPath());
  }

  async observeBrowserBefore(browser: BrowserHandle) {
    if (this.browserBefore !== null) throw new Error("Raster browser observation reused");
    this.browserBefore = await observeActualBrowserIdentity(browser, this.options);
    this.validateBrowserObservation(this.browserBefore);
  }

  async observeBrowserEnd(browser: BrowserHandle) {
    const observation = await observeActualBrowserIdentity(browser, this.options);
    this.validateBrowserObservation(observation);
    if (
      this.browserBefore === null ||
      !isDeepStrictEqual(browserIdentityCore(this.browserBefore), browserIdentityCore(observation))
    ) {
      throw new Error("Actual raster browser identity changed");
    }
    this.browserEnd = observation;
  }

  markBrowserClosed() {
    this.closed = true;
  }

  pinInput(filePath: string, consumed: Buffer) {
    if (rasterSha256(consumed) !== this.pin(filePath).sha256)
      throw new Error("Consumed raster input differs from observed physical source");
  }

  pinAssets(assets: Array<{ path: string; sha256: string }>) {
    for (const asset of assets) {
      const observed = this.pin(path.resolve(asset.path));
      if (observed.sha256 !== asset.sha256)
        throw new Error(`Consumed raster runtime asset differs: ${asset.path}`);
    }
  }

  recordPublishedPair(input: {
    manifest: MermaidPairManifest;
    sourceSvg: Buffer;
    renderedSvg: Buffer;
    screenshot: Buffer;
    attemptPng: Buffer;
    publishedPng: Buffer;
    capture: RasterCaptureState;
    rawSvgSha256: string;
    canonicalSourceSha256: string;
  }) {
    if (this.browserBefore === null || this.browserEnd !== null || this.closed)
      throw new Error("Raster capture has no live observed browser");
    const { manifest } = input;
    if (
      manifest.role !== this.options.role ||
      this.options.inventory[this.rows.length] !== manifest.slug
    ) {
      throw new Error("Raster publication exact order/role differs");
    }
    assertScreenshotPublishedBytes(input.screenshot, input.attemptPng, input.publishedPng);
    rasterCaptureContext(manifest.config, input.capture);
    const source = rasterSha256(input.sourceSvg),
      rendered = rasterSha256(input.renderedSvg),
      loaded = input.capture.svg.inputSha256;
    if (
      source !== rendered ||
      source !== loaded ||
      source !== manifest.sourceSvgSha256 ||
      rendered !== manifest.renderedSvgSha256 ||
      rasterSha256(input.screenshot) !== manifest.pngSha256
    )
      throw new Error("Actual raster SVG/PNG differs from published pair");
    this.pinAssets(manifest.runtimeAssets);
    for (const extension of ["svg", "rendered.svg", "png", "pair.json"])
      this.pin(path.join(this.options.outputDir, `${manifest.slug}.${extension}`));
    this.rows.push({
      slug: manifest.slug,
      sourceSvgSha256: source,
      renderedSvgSha256: rendered,
      loadedSvgSha256: loaded,
      rawSvgSha256: input.rawSvgSha256,
      stageSvgSha256: rendered,
      canonicalSourceSha256: input.canonicalSourceSha256,
      provenanceSha256: manifest.provenance.sha256,
      pngSha256: manifest.pngSha256,
      publishedPngSha256: rasterSha256(input.publishedPng),
      png: manifest.png,
      configSha256: manifest.configSha256,
      capture: input.capture,
    });
  }

  finish() {
    if (
      !this.closed ||
      this.browserBefore === null ||
      this.browserEnd === null ||
      !isDeepStrictEqual(
        this.rows.map((row) => row.slug),
        this.options.inventory,
      )
    ) {
      throw new Error(
        "Raster receipt cannot be issued without complete actual browser/capture closure",
      );
    }
    for (const [filePath, before] of this.files) {
      if (!isDeepStrictEqual(before, physicalFileIdentity(filePath)))
        throw new Error(`Raster source/tool/published input changed: ${filePath}`);
    }
    const browser = this.browserBefore.endpoint;
    const observedFiles: RasterEndpoint[] = [...this.files]
      .filter(
        ([filePath]) =>
          !this.endpoints.some((row) => row.before.path === filePath) &&
          browser.before.path !== filePath,
      )
      .map(([filePath, before], index) => ({
        name: `observed-physical-file-${index}`,
        kind: "config",
        version: "physical-observation-v1",
        before,
        end: physicalFileIdentity(filePath),
      }));
    const endpoints = [
      ...this.endpoints.map((row) => ({ ...row, end: physicalFileIdentity(row.end.path) })),
      browser,
      ...observedFiles,
    ];
    const bytes = createRasterProductionReceipt({
      schema: "katana-mermaid-raster-production-v1",
      diagnosticOnly: true,
      accepted: false,
      producerSucceeded: true,
      role: this.options.role,
      attempt: this.options.attempt,
      nonce: this.options.nonce,
      inventory: this.options.inventory,
      endpoints,
      rows: this.rows,
    });
    const receiptPath = path.join(this.options.attempt, "raster-production-receipt.json");
    fs.writeFileSync(receiptPath, bytes, { flag: "wx" });
    // 期待SHAやtrusted comparison configは発行しない。終了後の外部orchestrationが独立に固定する。
    return receiptPath;
  }

  private pin(filePath: string) {
    const identity = physicalFileIdentity(filePath),
      before = this.files.get(filePath);
    if (before !== undefined && !isDeepStrictEqual(before, identity))
      throw new Error(`Raster observed file changed: ${filePath}`);
    this.files.set(filePath, identity);
    return identity;
  }

  private validateBrowserObservation(observation: ObservedBrowserIdentity) {
    // 返却値は同じownBrowser CDPとreadonly OS collectorの実測からのみ生成される。
    if (
      observation.endpoint.kind !== "browser" ||
      !path.isAbsolute(observation.observationPath) ||
      observation.pid <= 0 ||
      observation.producerPid !== process.pid ||
      !observation.startIdentity ||
      !observation.producerStartIdentity ||
      !observation.parentChain.some(
        (row) => row.pid === process.pid && row.startIdentity === observation.producerStartIdentity,
      ) ||
      observation.argv[0] !== observation.endpoint.before.path ||
      !isDeepStrictEqual(this.pin(observation.endpoint.before.path), observation.endpoint.before)
    ) {
      throw new Error("Missing legitimate externally pinned actual browser identity");
    }
    this.pin(observation.observationPath);
  }
}

interface RasterSessionConfigInput {
  templateConfigPath: string;
  templateConfigSha256: string;
  fixturesDir: string;
  attempt: string;
  outputDir: string;
  configPath: string;
}

export interface OfficialRasterSessionSeedInput {
  seedOptionsPath: string;
  fixturesDir: string;
  attempt: string;
  outputDir: string;
  configPath: string;
}

export function writeCandidateRasterSessionConfig(input: RasterSessionConfigInput) {
  return writeRasterSessionConfig(input, "candidate");
}

export function writeOfficialRasterSessionConfig(input: RasterSessionConfigInput) {
  return writeRasterSessionConfig(input, "official");
}

export function writeInitialOfficialRasterSessionConfig(input: OfficialRasterSessionSeedInput) {
  const seedOptionsPath = sessionAbsolute(input.seedOptionsPath),
    seedOptions = readOfficialRasterSessionSeedOptions(seedOptionsPath),
    endpoints = seedOptions.endpoints,
    reservedEndpointName = "official-session-seed-options";
  if (endpoints.some((row) => row.name === reservedEndpointName)) {
    throw new Error("Raster seed endpoint name is reserved");
  }
  const seedOptionsEndpoint = {
      name: reservedEndpointName,
      kind: "config" as const,
      version: "katana-mermaid-raster-session-seed-v1",
      path: seedOptionsPath,
    },
    configPath = newFilePath(input.configPath),
    options = rasterSessionOptions(
      input,
      {
        observerPython: seedOptions.observerPython,
        endpoints: [...endpoints, seedOptionsEndpoint],
      },
      "official",
    ),
    pinnedInputs = [
      [seedOptionsPath, seedOptions.seedOptionsIdentity] as const,
      ...[options.observerPython, ...endpoints.map((row) => row.path)].map(
        (filePath) => [filePath, physicalFileIdentity(filePath)] as const,
      ),
    ];
  new RasterProductionSession(options);
  const result = writeAndVerifyRasterConfig(configPath, options),
    readback = RasterProductionSession.fromPinnedConfig(result.path, result.sha256);
  if (!isDeepStrictEqual(readback.options, options)) {
    throw new Error("Seeded official raster config differs after strict readback");
  }
  for (const [filePath, identity] of pinnedInputs) {
    if (!isDeepStrictEqual(identity, physicalFileIdentity(filePath))) {
      throw new Error(`Raster seed endpoint changed while writing config: ${filePath}`);
    }
  }
  return result;
}

function readOfficialRasterSessionSeedOptions(seedOptionsPath: string) {
  const configPath = sessionAbsolute(seedOptionsPath),
    before = physicalFileIdentity(configPath),
    bytes = fs.readFileSync(configPath),
    after = physicalFileIdentity(configPath);
  if (before.sha256 !== rasterSha256(bytes) || !isDeepStrictEqual(before, after)) {
    throw new Error("Raster seed options changed while reading");
  }
  const root = sessionObject(JSON.parse(bytes.toString("utf8")) as JsonInput, [
    "schema",
    "observerPython",
    "endpoints",
  ]);
  if (root.schema !== "katana-mermaid-raster-session-seed-v1") {
    throw new Error("Unsupported raster session seed schema");
  }
  return {
    observerPython: sessionAbsolute(root.observerPython),
    seedOptionsIdentity: after,
    endpoints: parseRasterSessionEndpoints(root.endpoints),
  };
}

function writeRasterSessionConfig(input: RasterSessionConfigInput, role: RasterRole) {
  const template = RasterProductionSession.fromPinnedConfig(
    input.templateConfigPath,
    input.templateConfigSha256,
  );
  if (template.options.role !== "official") {
    throw new Error("Raster config template must be an externally pinned official session");
  }
  const options = rasterSessionOptions(input, template.options, role);
  new RasterProductionSession(options);
  return writeAndVerifyRasterConfig(input.configPath, options);
}

function rasterSessionOptions(
  input: Pick<RasterSessionConfigInput, "fixturesDir" | "attempt" | "outputDir" | "configPath">,
  template: Pick<RasterProductionSessionOptions, "observerPython" | "endpoints">,
  role: RasterRole,
) {
  const fixturesDir = canonicalDirectory(input.fixturesDir),
    attempt = emptyCanonicalDirectory(input.attempt),
    outputDir = emptyCanonicalDirectory(input.outputDir),
    configPath = newFilePath(input.configPath);
  if (
    inside(attempt, outputDir) ||
    inside(outputDir, attempt) ||
    inside(configPath, attempt) ||
    inside(configPath, outputDir)
  ) {
    throw new Error("Raster config, attempt and output paths must remain separate");
  }
  const inventory = new FixtureRepository(fixturesDir)
    .list()
    .filter((fixture) => (fixture.originalFenceSource ?? "").trim() !== "")
    .map((fixture) => fixture.slug);
  if (inventory.length === 0 || new Set(inventory).size !== inventory.length) {
    throw new Error("Candidate fixture inventory must be nonempty and unique");
  }
  return {
    ...template,
    role,
    attempt,
    outputDir,
    nonce: randomUUID(),
    inventory,
  };
}

function writeAndVerifyRasterConfig(
  configFile: string,
  options: RasterProductionSession["options"],
) {
  const configPath = newFilePath(configFile);
  const config = {
    schema: "katana-mermaid-raster-producer-input-v1",
    diagnosticOnly: true,
    accepted: false,
    ...options,
  };
  const bytes = Buffer.from(`${JSON.stringify(config)}\n`);
  fs.writeFileSync(configPath, bytes, { flag: "wx", mode: 0o600 });
  const sha256 = rasterSha256(bytes);
  RasterProductionSession.fromPinnedConfig(configPath, sha256);
  return { path: configPath, sha256, inventory: options.inventory, role: options.role };
}

function canonicalDirectory(value: string) {
  if (!path.isAbsolute(value) || path.resolve(value) !== value) {
    throw new Error("Raster config paths must be absolute and normalized");
  }
  const real = fs.realpathSync(value),
    stat = fs.lstatSync(value);
  if (!stat.isDirectory() || stat.isSymbolicLink() || real !== value) {
    throw new Error("Raster config path must be a canonical directory");
  }
  return real;
}

function emptyCanonicalDirectory(value: string) {
  const directory = canonicalDirectory(value);
  if (fs.readdirSync(directory).length !== 0) {
    throw new Error("Raster attempt/output directories must be fresh and empty");
  }
  return directory;
}

function newFilePath(value: string) {
  if (!path.isAbsolute(value) || path.resolve(value) !== value) {
    throw new Error("Raster config output path must be absolute and normalized");
  }
  const parent = path.dirname(value);
  if (fs.realpathSync(parent) !== parent || fs.existsSync(value)) {
    throw new Error("Raster config output must be a new file in a canonical directory");
  }
  return value;
}

function inside(filePath: string, directory: string) {
  const relative = path.relative(directory, filePath);
  return (
    relative === "" ||
    (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative))
  );
}

function parseRasterConfigCli(args: string[]): RasterSessionConfigInput & { role: RasterRole } {
  const allowed = new Set([
    "--template",
    "--template-sha256",
    "--fixtures",
    "--attempt",
    "--output",
    "--config",
    "--role",
  ]);
  const values = new Map<string, string>();
  for (let index = 0; index < args.length; index += 2) {
    const name = args[index];
    const value = args[index + 1];
    if (
      !name?.startsWith("--") ||
      !allowed.has(name) ||
      value === undefined ||
      value.startsWith("--")
    ) {
      throw new Error("Raster config CLI accepts only the supported named values");
    }
    if (values.has(name)) throw new Error(`Duplicate ${name}`);
    values.set(name, value);
  }
  const hasRole = values.has("--role");
  if (values.size !== 6 + Number(hasRole)) {
    throw new Error("Raster config CLI requires all six named values and optional --role");
  }
  const role = values.get("--role") ?? "candidate";
  if (role !== "official" && role !== "candidate") {
    throw new Error("Raster config role must be official or candidate");
  }
  const value = (name: string) => {
    const result = values.get(name);
    if (result === undefined) throw new Error(`Missing ${name}`);
    return result;
  };
  return {
    templateConfigPath: value("--template"),
    templateConfigSha256: value("--template-sha256"),
    fixturesDir: value("--fixtures"),
    attempt: value("--attempt"),
    outputDir: value("--output"),
    configPath: value("--config"),
    role,
  };
}

type OfficialRasterSessionSeedCliInput = Pick<
  OfficialRasterSessionSeedInput,
  "seedOptionsPath" | "fixturesDir" | "attempt" | "outputDir" | "configPath"
>;

function parseOfficialRasterSeedCli(args: string[]): OfficialRasterSessionSeedCliInput {
  const allowed = new Set(["--seed-options", "--fixtures", "--attempt", "--output", "--config"]),
    values = new Map<string, string>();
  for (let index = 0; index < args.length; index += 2) {
    const name = args[index],
      value = args[index + 1];
    if (
      !name?.startsWith("--") ||
      !allowed.has(name) ||
      value === undefined ||
      value.startsWith("--")
    ) {
      throw new Error("Official raster seed CLI accepts only supported named values");
    }
    if (values.has(name)) throw new Error(`Duplicate ${name}`);
    values.set(name, value);
  }
  if (values.size !== allowed.size) {
    throw new Error("Official raster seed CLI requires all five named values");
  }
  const value = (name: string) => {
    const result = values.get(name);
    if (result === undefined) throw new Error(`Missing ${name}`);
    return result;
  };
  return {
    seedOptionsPath: value("--seed-options"),
    fixturesDir: value("--fixtures"),
    attempt: value("--attempt"),
    outputDir: value("--output"),
    configPath: value("--config"),
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--help") {
    console.log(
      "Usage: bun run scripts/mermaid/raster-production-session.ts --template PATH --template-sha256 SHA --fixtures DIR --attempt DIR --output DIR --config PATH [--role official|candidate]\n       bun run scripts/mermaid/raster-production-session.ts --seed-official --seed-options PATH --fixtures DIR --attempt DIR --output DIR --config PATH",
    );
  } else {
    try {
      const result =
        args[0] === "--seed-official"
          ? (() => {
              const input = parseOfficialRasterSeedCli(args.slice(1));
              return writeInitialOfficialRasterSessionConfig(input);
            })()
          : (() => {
              const input = parseRasterConfigCli(args),
                { role, ...configInput } = input;
              return writeRasterSessionConfig(configInput, role);
            })();
      process.stdout.write(`${JSON.stringify(result)}\n`);
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    }
  }
}

function browserIdentityCore(observation: ObservedBrowserIdentity) {
  const { observationPath: _rawEvidencePath, ...identity } = observation;
  return identity;
}

function parseRasterSessionConfig(value: JsonInput): RasterProductionSessionOptions {
  const root = sessionObject(value, [
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
  ]);
  if (
    root.schema !== "katana-mermaid-raster-producer-input-v1" ||
    root.diagnosticOnly !== true ||
    root.accepted !== false ||
    (root.role !== "official" && root.role !== "candidate") ||
    !Array.isArray(root.inventory) ||
    !Array.isArray(root.endpoints)
  ) {
    throw new Error("Invalid externally pinned raster producer config");
  }
  const endpoints = parseRasterSessionEndpoints(root.endpoints);
  return {
    role: root.role,
    attempt: sessionAbsolute(root.attempt),
    nonce: sessionText(root.nonce),
    inventory: root.inventory.map(sessionText),
    outputDir: sessionAbsolute(root.outputDir),
    observerPython: sessionAbsolute(root.observerPython),
    endpoints,
  };
}

function parseRasterSessionEndpoints(
  value: JsonInput,
): RasterProductionSessionOptions["endpoints"] {
  if (!Array.isArray(value)) throw new Error("Raster producer endpoints must be an array");
  return value.map((item: JsonInput) => {
    const row = sessionObject(item, ["name", "kind", "version", "path"]),
      kind = row.kind;
    if (
      kind !== "producer" &&
      kind !== "playwright" &&
      kind !== "lock" &&
      kind !== "runtime" &&
      kind !== "font" &&
      kind !== "config"
    ) {
      throw new Error("Browser identity cannot be supplied in static producer config");
    }
    return {
      name: sessionText(row.name),
      kind,
      version: sessionText(row.version),
      path: sessionAbsolute(row.path),
    };
  });
}
function sessionObject(value: JsonInput, keys: string[]): JsonObject {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    !isDeepStrictEqual(Object.keys(value).sort(), [...keys].sort())
  )
    throw new Error("Unexpected raster producer config fields");
  return value as JsonObject;
}
function sessionText(value: JsonInput): string {
  if (typeof value !== "string" || value === "")
    throw new Error("Missing raster producer config text");
  return value;
}
function sessionAbsolute(value: JsonInput) {
  const result = sessionText(value);
  if (!path.isAbsolute(result) || path.resolve(result) !== result)
    throw new Error("Absolute raster producer config path required");
  return result;
}
