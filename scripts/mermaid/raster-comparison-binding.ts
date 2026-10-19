import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual, parseArgs } from "node:util";
import { FixtureRepository } from "./diagram_update_fixtures";
import type { JsonInput, JsonObject } from "./raster-json-input";
import {
  parseRasterFileIdentity,
  parseRasterProductionReceipt,
  type RasterBindingRow,
  type RasterEndpoint,
  type RasterFileIdentity,
  type RasterPairObservation,
  type RasterRole,
  rasterCaptureContext,
  rasterSha256,
  sameRasterFileIdentity,
  type TrustedRasterInput,
  verifyRasterProductionBinding,
} from "./raster-production-binding";

type DirectoryStamp = {
  path: string;
  dev: RasterFileIdentity["dev"];
  ino: RasterFileIdentity["ino"];
  mode: number;
  size: number;
  mtimeNs: string;
  ctimeNs: string;
};

interface RoleBinding {
  receiptBytes: Buffer;
  trusted: TrustedRasterInput;
  endpoints: RasterEndpoint[];
  manifests: Array<{ slug: string; sha256: string }>;
  observations: Pick<RasterBindingRow, "slug" | "loadedSvgSha256" | "capture">[];
}

export class RasterComparisonBinding {
  private repositoryRoot = "";
  private repositoryStamp: DirectoryStamp | null = null;
  private readonly relativeAssetAncestors = new Map<string, DirectoryStamp[]>();
  private readonly pins = new Map<string, RasterFileIdentity>();
  private readonly verifiedContexts = new Map<string, ReturnType<typeof rasterCaptureContext>>();
  private readonly roles = new Map<RasterRole, RoleBinding>();

  static read(configPath: string, expectedSha256: string, inventory: string[]) {
    const binding = new RasterComparisonBinding();
    const bytes = binding.readObservedFile(absolute(configPath));
    if (rasterSha256(bytes) !== digest(expectedSha256)) {
      throw new Error("Externally pinned raster comparison config SHA mismatch");
    }
    const config = object(JSON.parse(bytes.toString("utf8")), "raster comparison config");
    keys(config, ["schema", "diagnosticOnly", "accepted", "trustedRepositoryRoot", "roles"]);
    if (
      config.schema !== "katana-mermaid-trusted-raster-comparison-v1" ||
      config.diagnosticOnly !== true ||
      config.accepted !== false
    ) {
      throw new Error("Unsupported trusted raster comparison config");
    }
    binding.initializeRepositoryRoot(absolute(config.trustedRepositoryRoot));
    const roles = object(config.roles, "raster roles");
    keys(roles, ["official", "candidate"]);
    for (const role of ["official", "candidate"] as const) {
      binding.roles.set(role, binding.readRole(role, roles[role], inventory));
    }
    binding.assertUnchanged();
    return binding;
  }

  static validateOfficialRole(
    repositoryRoot: string,
    input: RasterRoleConfigInput,
    inventory: string[],
  ): void {
    const binding = new RasterComparisonBinding();
    binding.initializeRepositoryRoot(repositoryRoot);
    const pins = new Map<string, RasterFileIdentity>();
    const role = comparisonRoleConfig("official", input, inventory, binding.repositoryRoot, pins);
    binding.roles.set(
      "official",
      binding.readRole("official", role as unknown as JsonInput, inventory),
    );
    for (const slug of inventory) {
      binding.readPairManifest(
        "official",
        slug,
        path.join(absolute(input.pairDirectory), `${slug}.pair.json`),
      );
    }
    binding.assertUnchanged();
    assertPinnedInputsUnchanged(pins);
  }

  private initializeRepositoryRoot(repositoryRoot: string): void {
    this.repositoryRoot = absolute(repositoryRoot);
    this.repositoryStamp = this.observeRepositoryRoot();
  }

  readObservedFile(filePath: string): Buffer {
    this.assertRelativeAssetBoundary(filePath);
    const { identity, bytes } = readPhysicalFile(filePath);
    if (this.relativeAssetAncestors.has(filePath) && identity.canonical !== filePath) {
      throw new Error("Relative raster runtime asset canonical target changed while reading");
    }
    const previous = this.pins.get(filePath);
    if (previous !== undefined && !sameRasterFileIdentity(previous, identity)) {
      throw new Error(`Raster comparison input changed: ${filePath}`);
    }
    this.assertRelativeAssetBoundary(filePath);
    this.pins.set(filePath, identity);
    return bytes;
  }

  readPairManifest(role: RasterRole, slug: string, filePath: string): Buffer {
    const expected = this.role(role).manifests.find((row) => row.slug === slug);
    const bytes = this.readObservedFile(filePath);
    if (expected === undefined || rasterSha256(bytes) !== expected.sha256) {
      throw new Error(`Externally pinned pair manifest differs: ${role}/${slug}`);
    }
    return bytes;
  }

  captureObservation(role: RasterRole, slug: string) {
    const observation = this.role(role).observations.find((row) => row.slug === slug);
    if (observation === undefined)
      throw new Error(`Missing trusted raster capture ${role}/${slug}`);
    return observation;
  }

  verifyRole(role: RasterRole, pairs: RasterPairObservation[]) {
    const value = this.role(role);
    verifyRasterProductionBinding(value.receiptBytes, value.trusted, pairs, value.endpoints);
    const contexts = pairs.map((pair) => rasterCaptureContext(pair.config, pair.capture));
    for (const [index, pair] of pairs.entries()) {
      const context = contexts[index];
      const other = this.verifiedContexts.get(
        `${role === "official" ? "candidate" : "official"}/${pair.slug}`,
      );
      if (other !== undefined && !isDeepStrictEqual(other, context)) {
        throw new Error(`Actual role capture context mismatch: ${pair.slug}`);
      }
    }
    for (const [index, pair] of pairs.entries()) {
      const context = contexts[index];
      if (context === undefined) throw new Error("Missing verified capture context");
      this.verifiedContexts.set(`${role}/${pair.slug}`, context);
    }
  }

  requireRepositoryRoot(expectedCanonicalRoot: string) {
    this.observeRepositoryRoot();
    if (this.repositoryRoot !== absolute(expectedCanonicalRoot)) {
      throw new Error("Trusted raster repository anchor differs from comparison repository");
    }
  }

  resolveRuntimeAsset(assetPath: string): string {
    if (path.isAbsolute(assetPath)) return absolute(assetPath);
    if (
      assetPath === "" ||
      path.normalize(assetPath) !== assetPath ||
      assetPath.split(path.sep).some((part) => part === ".." || part === ".")
    ) {
      throw new Error("Unsafe relative raster runtime asset path");
    }
    const resolved = path.resolve(this.repositoryRoot, assetPath);
    const relative = path.relative(this.repositoryRoot, resolved);
    if (
      relative === "" ||
      relative === ".." ||
      relative.startsWith(`..${path.sep}`) ||
      path.isAbsolute(relative) ||
      fs.realpathSync(resolved) !== resolved
    ) {
      throw new Error("Relative raster runtime asset escapes trusted canonical repository root");
    }
    this.observeRepositoryRoot();
    const ancestors = this.observeAssetAncestors(resolved);
    const previous = this.relativeAssetAncestors.get(resolved);
    if (previous !== undefined && !isDeepStrictEqual(previous, ancestors)) {
      throw new Error("Relative raster runtime asset ancestors changed");
    }
    this.relativeAssetAncestors.set(resolved, ancestors);
    this.readObservedFile(resolved);
    return resolved;
  }

  // 解決時の制約を実際のfd読取と最終閉検査へ持ち越し、外部targetを新しいpinにしない。
  private observeAssetAncestors(filePath: string) {
    const relative = path.relative(this.repositoryRoot, filePath);
    if (
      relative === "" ||
      relative === ".." ||
      relative.startsWith(`..${path.sep}`) ||
      path.isAbsolute(relative)
    ) {
      throw new Error("Relative raster runtime asset escapes trusted canonical repository root");
    }
    const directories: string[] = [];
    for (const part of relative.split(path.sep).slice(0, -1)) {
      const parent = directories.at(-1) ?? this.repositoryRoot;
      directories.push(path.join(parent, part));
    }
    return directories.map((directory) => {
      const stamp = fs.lstatSync(directory, { bigint: true });
      if (
        !stamp.isDirectory() ||
        stamp.isSymbolicLink() ||
        fs.realpathSync(directory) !== directory
      ) {
        throw new Error("Relative raster runtime asset escapes trusted canonical repository root");
      }
      return { path: directory, ...directoryStamp(stamp) };
    });
  }

  private assertRelativeAssetBoundary(filePath: string) {
    const expected = this.relativeAssetAncestors.get(filePath);
    if (expected === undefined) return;
    if (
      !isDeepStrictEqual(expected, this.observeAssetAncestors(filePath)) ||
      fs.realpathSync(filePath) !== filePath
    ) {
      throw new Error("Relative raster runtime asset boundary changed");
    }
  }

  private observeRepositoryRoot() {
    const stamp = fs.lstatSync(this.repositoryRoot, { bigint: true });
    if (
      !stamp.isDirectory() ||
      stamp.isSymbolicLink() ||
      fs.realpathSync(this.repositoryRoot) !== this.repositoryRoot
    ) {
      throw new Error("Trusted raster repository root must be a canonical directory");
    }
    const identity = { path: this.repositoryRoot, ...directoryStamp(stamp) };
    if (this.repositoryStamp !== null && !sameDirectoryStamp(identity, this.repositoryStamp)) {
      throw new Error("Trusted raster repository root changed");
    }
    return identity;
  }

  assertUnchanged() {
    for (const [filePath, identity] of this.pins) {
      this.assertRelativeAssetBoundary(filePath);
      if (!sameRasterFileIdentity(identity, physicalFileIdentity(filePath))) {
        throw new Error(`Raster comparison endpoint drift: ${filePath}`);
      }
      this.assertRelativeAssetBoundary(filePath);
    }
    this.observeRepositoryRoot();
  }

  private role(role: RasterRole): RoleBinding {
    const value = this.roles.get(role);
    if (value === undefined) throw new Error(`Missing trusted raster role ${role}`);
    return value;
  }

  private readRole(role: RasterRole, value: JsonInput, inventory: string[]): RoleBinding {
    const input = object(value, `${role} raster binding`);
    keys(input, [
      "receiptPath",
      "receiptSha256",
      "attempt",
      "nonce",
      "inventory",
      "observedEndpoints",
      "observedCaptures",
      "observedPairManifests",
      "browserProcess",
    ]);
    const receiptBytes = this.readObservedFile(absolute(input.receiptPath));
    const receiptSha256 = digest(input.receiptSha256);
    if (rasterSha256(receiptBytes) !== receiptSha256)
      throw new Error("Externally pinned raster receipt SHA mismatch");
    const receipt = parseRasterProductionReceipt(receiptBytes);
    const trusted: TrustedRasterInput = {
      receiptSha256,
      role,
      attempt: absolute(input.attempt),
      nonce: text(input.nonce),
      inventory,
    };
    if (
      !isDeepStrictEqual(input.inventory, inventory) ||
      receipt.role !== role ||
      receipt.attempt !== trusted.attempt ||
      receipt.nonce !== trusted.nonce ||
      !isDeepStrictEqual(receipt.inventory, inventory)
    ) {
      throw new Error("Trusted raster role/attempt/nonce/inventory differs");
    }
    // 期待観測は外部固定configからのみ受け取り、receiptの自己申告をそのまま信頼しない。
    if (!sameEndpointList(input.observedEndpoints, receipt.endpoints)) {
      throw new Error("Externally pinned raster endpoint observations differ");
    }
    const observations = receipt.rows.map(({ slug, loadedSvgSha256, capture }) => ({
      slug,
      loadedSvgSha256,
      capture,
    }));
    if (!isDeepStrictEqual(input.observedCaptures, observations)) {
      throw new Error("Externally pinned raster capture observations differ");
    }
    if (!Array.isArray(input.observedPairManifests))
      throw new Error("Missing externally pinned pair manifests");
    const manifests = input.observedPairManifests.map((value: JsonInput) => {
      const row = object(value, "trusted pair manifest");
      keys(row, ["slug", "sha256"]);
      return { slug: text(row.slug), sha256: digest(row.sha256) };
    });
    if (
      !isDeepStrictEqual(
        manifests.map((row) => row.slug),
        inventory,
      )
    ) {
      throw new Error("Externally pinned pair manifest inventory differs");
    }
    this.requireBrowserProcess(input.browserProcess, receipt.endpoints);
    for (const endpoint of receipt.endpoints) {
      if (
        !sameRasterFileIdentity(endpoint.before, endpoint.end) ||
        !sameRasterFileIdentity(physicalFileIdentity(endpoint.end.path), endpoint.end)
      ) {
        throw new Error(`Raster physical endpoint differs: ${endpoint.name}`);
      }
      this.readObservedFile(endpoint.end.path);
    }
    return { receiptBytes, trusted, endpoints: receipt.endpoints, observations, manifests };
  }

  private requireBrowserProcess(value: JsonInput, endpoints: RasterEndpoint[]) {
    const process = object(value, "trusted browser process closure");
    keys(process, [
      "pid",
      "startIdentity",
      "producerPid",
      "producerStartIdentity",
      "parentChain",
      "executable",
      "argv",
      "version",
      "beforeEndEqual",
      "producerExitCode",
      "reaped",
      "groupGone",
    ]);
    const pid = positiveInteger(process.pid),
      producerPid = positiveInteger(process.producerPid);
    text(process.startIdentity);
    const producerStart = text(process.producerStartIdentity);
    if (
      pid === producerPid ||
      !Array.isArray(process.parentChain) ||
      process.parentChain.length === 0
    ) {
      throw new Error("Trusted browser has no producer ancestry");
    }
    const parents = process.parentChain.map((value: JsonInput) => {
      const parent = object(value, "browser parent");
      keys(parent, ["pid", "startIdentity"]);
      return { pid: positiveInteger(parent.pid), startIdentity: text(parent.startIdentity) };
    });
    if (
      new Set(parents.map((row) => row.pid)).size !== parents.length ||
      parents.some((row) => row.pid === pid) ||
      !parents.some((row) => row.pid === producerPid && row.startIdentity === producerStart)
    ) {
      throw new Error("Trusted browser producer ancestry differs");
    }
    const browser = endpoints.filter((row) => row.kind === "browser");
    const browserEndpoint = browser[0];
    let executable: RasterFileIdentity;
    try {
      executable = parseRasterFileIdentity(process.executable);
    } catch {
      throw new Error("Missing or inconsistent trusted browser process evidence");
    }
    if (
      browser.length !== 1 ||
      browserEndpoint === undefined ||
      !sameRasterFileIdentity(executable, browserEndpoint.end) ||
      process.version !== browserEndpoint.version ||
      !Array.isArray(process.argv) ||
      process.argv.length === 0 ||
      process.argv.some((value: JsonInput) => typeof value !== "string" || value.length === 0) ||
      process.argv[0] !== browserEndpoint.end.path ||
      process.beforeEndEqual !== true ||
      process.producerExitCode !== 0 ||
      process.reaped !== true ||
      process.groupGone !== true
    ) {
      throw new Error("Missing or inconsistent trusted browser process evidence");
    }
  }
}

export interface RasterRoleConfigInput {
  receiptPath: string;
  receiptSha256: string;
  processEvidencePath: string;
  processEvidenceSha256: string;
  pairDirectory: string;
}

interface RasterComparisonConfigInput {
  trustedRepositoryRoot: string;
  fixturesDir: string;
  configPath: string;
  official: RasterRoleConfigInput;
  candidate: RasterRoleConfigInput;
}

export function writeRasterComparisonConfig(input: RasterComparisonConfigInput) {
  const root = absolute(input.trustedRepositoryRoot);
  const configPath = absolute(input.configPath);
  const parent = path.dirname(configPath);
  if (fs.realpathSync(parent) !== parent || fs.existsSync(configPath))
    throw new Error("Raster comparison config requires a new canonical output path");
  const inventory = new FixtureRepository(absolute(input.fixturesDir))
    .list()
    .map((fixture) => fixture.slug);
  const pins = new Map<string, RasterFileIdentity>();
  const roles = {
    official: comparisonRoleConfig("official", input.official, inventory, root, pins),
    candidate: comparisonRoleConfig("candidate", input.candidate, inventory, root, pins),
  };
  const config = {
    schema: "katana-mermaid-trusted-raster-comparison-v1",
    diagnosticOnly: true,
    accepted: false,
    trustedRepositoryRoot: root,
    roles,
  };
  const bytes = Buffer.from(`${JSON.stringify(config)}\n`);
  fs.writeFileSync(configPath, bytes, { flag: "wx", mode: 0o600 });
  const sha256 = rasterSha256(bytes);
  const binding = RasterComparisonBinding.read(configPath, sha256, inventory);
  for (const role of ["official", "candidate"] as const)
    for (const slug of inventory)
      binding.readPairManifest(
        role,
        slug,
        path.join(input[role].pairDirectory, `${slug}.pair.json`),
      );
  binding.assertUnchanged();
  assertPinnedInputsUnchanged(pins);
  return { path: configPath, sha256, inventory };
}

function assertPinnedInputsUnchanged(pins: Map<string, RasterFileIdentity>): void {
  for (const [file, pin] of pins) {
    if (!sameRasterFileIdentity(physicalFileIdentity(file), pin)) {
      throw new Error(`Raster builder input changed: ${file}`);
    }
  }
}

function pinnedBuilderInput(
  file: string,
  expected: string | undefined,
  pins: Map<string, RasterFileIdentity>,
) {
  const canonical = absolute(file),
    value = readPhysicalFile(canonical);
  if (expected !== undefined && value.identity.sha256 !== digest(expected))
    throw new Error(`Raster builder input SHA differs: ${file}`);
  const previous = pins.get(canonical);
  if (previous !== undefined && !sameRasterFileIdentity(previous, value.identity))
    throw new Error(`Raster builder input changed: ${file}`);
  pins.set(canonical, value.identity);
  return value;
}

function proofBuilderInput(value: JsonInput, pins: Map<string, RasterFileIdentity>) {
  const pin = object(value, "raster process input pin");
  keys(pin, ["path", "sha256", "size"]);
  const read = pinnedBuilderInput(absolute(pin.path), digest(pin.sha256), pins);
  if (read.bytes.length !== pin.size) throw new Error("Raster process input size differs");
  return read;
}

function comparisonRoleConfig(
  role: RasterRole,
  input: RasterRoleConfigInput,
  inventory: string[],
  root: string,
  pins: Map<string, RasterFileIdentity>,
) {
  const receiptInput = pinnedBuilderInput(input.receiptPath, input.receiptSha256, pins);
  const receipt = parseRasterProductionReceipt(receiptInput.bytes);
  const proofInput = pinnedBuilderInput(
    input.processEvidencePath,
    input.processEvidenceSha256,
    pins,
  );
  const proof = object(JSON.parse(proofInput.bytes.toString("utf8")), "raster process proof");
  if (
    proof.schema !== "krr-mermaid-raster-process-evidence-v1" ||
    proof.diagnosticOnly !== true ||
    proof.accepted !== false ||
    proof.descendantClosureProved !== false ||
    proof.role !== role ||
    receipt.role !== role ||
    proof.attempt !== receipt.attempt ||
    proof.nonce !== receipt.nonce ||
    !isDeepStrictEqual(receipt.inventory, inventory)
  )
    throw new Error("Raster process proof does not bind the requested producer");
  const proofReceipt = proofBuilderInput(proof.receipt, pins);
  if (!sameRasterFileIdentity(proofReceipt.identity, receiptInput.identity))
    throw new Error("Raster process proof receipt differs");
  const process = validatedBuilderProcess(proof, role, root, pins);
  const manifests = inventory.map((slug) => ({
    slug,
    sha256: pinnedBuilderInput(
      path.join(absolute(input.pairDirectory), `${slug}.pair.json`),
      undefined,
      pins,
    ).identity.sha256,
  }));
  return {
    receiptPath: receiptInput.identity.path,
    receiptSha256: receiptInput.identity.sha256,
    attempt: receipt.attempt,
    nonce: receipt.nonce,
    inventory,
    observedEndpoints: receipt.endpoints,
    observedCaptures: receipt.rows.map(({ slug, loadedSvgSha256, capture }) => ({
      slug,
      loadedSvgSha256,
      capture,
    })),
    observedPairManifests: manifests,
    browserProcess: process,
  };
}

function validatedBuilderProcess(
  proof: JsonObject,
  role: RasterRole,
  root: string,
  pins: Map<string, RasterFileIdentity>,
) {
  const owner = object(
    JSON.parse(proofBuilderInput(proof.ownerObservation, pins).bytes.toString("utf8")),
    "raster owner",
  );
  const lifecycle = object(
    JSON.parse(proofBuilderInput(proof.lifecycle, pins).bytes.toString("utf8")),
    "raster lifecycle",
  );
  const process = object(proof.browserProcess, "raster browser process");
  const identity = object(lifecycle.identity, "raster owned child");
  if (
    owner.actualExit !== 0 ||
    owner.reaped !== true ||
    owner.cwd !== root ||
    owner.label !== `mermaid-raster-${role}` ||
    !isDeepStrictEqual(owner.errors, []) ||
    lifecycle.exitCode !== 0 ||
    lifecycle.groupClosed !== true ||
    lifecycle.rawFdsClosed !== true ||
    lifecycle.failure !== null ||
    lifecycle.cleanupFailure !== null ||
    owner.pid !== identity.pid ||
    identity.processGroupId !== owner.pid ||
    identity.sessionId !== owner.pid ||
    process.producerPid !== owner.pid ||
    process.producerExitCode !== owner.actualExit ||
    process.reaped !== owner.reaped ||
    process.groupGone !== lifecycle.groupClosed
  )
    throw new Error("Raster producer exit/reap/group proof differs");
  const start = text(process.producerStartIdentity).match(/^(\d+)\.(\d{6})$/);
  if (start === null) throw new Error("Invalid raster producer start identity");
  const startUs = BigInt(start[1] as string) * 1_000_000n + BigInt(start[2] as string);
  if (
    startUs < builderNanoseconds(owner.startedNs) / 1000n ||
    startUs > builderNanoseconds(owner.endedNs) / 1000n
  )
    throw new Error("Raster producer identity is outside the owned spawn interval");
  validateBuilderBrowserObservations(proof, process, pins);
  const config = object(
    JSON.parse(proofBuilderInput(proof.config, pins).bytes.toString("utf8")),
    "raster session input",
  );
  if (config.role !== role || config.attempt !== proof.attempt || config.nonce !== proof.nonce)
    throw new Error("Raster process session input differs");
  return process;
}

function builderNanoseconds(value: JsonInput) {
  const decimal = text(value);
  if (!/^(0|[1-9]\d{0,19})$/.test(decimal)) throw new Error("Invalid raster owner timestamp");
  return BigInt(decimal);
}

function validateBuilderBrowserObservations(
  proof: JsonObject,
  process: JsonObject,
  pins: Map<string, RasterFileIdentity>,
) {
  if (!Array.isArray(proof.browserObservations) || proof.browserObservations.length !== 2)
    throw new Error("Raster process proof requires both live browser observations");
  const paths = new Set<string>();
  for (const pin of proof.browserObservations) {
    const input = proofBuilderInput(pin, pins);
    if (paths.has(input.identity.path)) throw new Error("Raster browser observation was reused");
    paths.add(input.identity.path);
    const value = object(JSON.parse(input.bytes.toString("utf8")), "live browser observation");
    const measured = object(value.osMeasurement, "browser OS measurement");
    const producer = object(measured.producer, "live producer");
    const browser = object(measured.browser, "live browser");
    if (
      value.schema !== "katana-own-browser-identity-observation-v1" ||
      value.role !== proof.role ||
      value.attempt !== proof.attempt ||
      value.nonce !== proof.nonce ||
      value.collectorExitCode !== 0 ||
      value.collectorReaped !== true ||
      measured.beforeEndEqual !== true ||
      producer.pid !== process.producerPid ||
      producer.startIdentity !== process.producerStartIdentity ||
      browser.pid !== process.pid ||
      browser.startIdentity !== process.startIdentity ||
      !isDeepStrictEqual(browser.argv, process.argv)
    )
      throw new Error("Live raster browser/producer observations differ");
  }
}

export function physicalFileIdentity(filePath: string): RasterFileIdentity {
  return readPhysicalFile(filePath).identity;
}

function readPhysicalFile(filePath: string): { identity: RasterFileIdentity; bytes: Buffer } {
  absolute(filePath);
  const before = fs.lstatSync(filePath, { bigint: true }),
    canonical = fs.realpathSync(filePath);
  if (!before.isFile() || before.isSymbolicLink())
    throw new Error(`Raster input must be a regular file: ${filePath}`);
  const fd = fs.openSync(filePath, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0));
  try {
    const opened = fs.fstatSync(fd, { bigint: true });
    const sameStamp = (stamp: fs.BigIntStats) =>
      stamp.isFile() &&
      stamp.dev === before.dev &&
      stamp.ino === before.ino &&
      stamp.mode === before.mode &&
      stamp.size === before.size &&
      stamp.mtimeNs === before.mtimeNs &&
      stamp.ctimeNs === before.ctimeNs;
    if (!sameStamp(opened))
      throw new Error(`Raster comparison input changed while opening: ${filePath}`);
    const bytes = fs.readFileSync(fd),
      after = fs.lstatSync(filePath, { bigint: true });
    if (
      after.isSymbolicLink() ||
      !sameStamp(after) ||
      !sameStamp(fs.fstatSync(fd, { bigint: true })) ||
      fs.realpathSync(filePath) !== canonical ||
      BigInt(bytes.length) !== before.size
    ) {
      throw new Error(`Raster comparison input changed while reading: ${filePath}`);
    }
    return {
      bytes,
      identity: {
        path: filePath,
        canonical,
        sha256: rasterSha256(bytes),
        size: boundedStatNumber(before.size, "size"),
        dev: before.dev.toString(),
        ino: before.ino.toString(),
        mode: boundedStatNumber(before.mode, "mode"),
      },
    };
  } finally {
    fs.closeSync(fd);
  }
}

function boundedStatNumber(value: bigint, label: string): number {
  const result = Number(value);
  if (!Number.isSafeInteger(result) || result < 0) {
    throw new Error(`Raster ${label} exceeds safe integer`);
  }
  return result;
}

function directoryStamp(stamp: fs.BigIntStats): Omit<DirectoryStamp, "path"> {
  return {
    dev: stamp.dev.toString(),
    ino: stamp.ino.toString(),
    mode: boundedStatNumber(stamp.mode, "mode"),
    size: boundedStatNumber(stamp.size, "size"),
    mtimeNs: stamp.mtimeNs.toString(),
    ctimeNs: stamp.ctimeNs.toString(),
  };
}

function sameDirectoryStamp(left: DirectoryStamp, right: DirectoryStamp): boolean {
  return (
    left.path === right.path &&
    String(left.dev) === String(right.dev) &&
    String(left.ino) === String(right.ino) &&
    left.mode === right.mode &&
    left.size === right.size &&
    left.mtimeNs === right.mtimeNs &&
    left.ctimeNs === right.ctimeNs
  );
}

function object(value: JsonInput, label: string): JsonObject {
  if (value === null || typeof value !== "object" || Array.isArray(value))
    throw new Error(`Invalid ${label}`);
  return value as JsonObject;
}
function keys(value: JsonObject, expected: string[]) {
  if (!isDeepStrictEqual(Object.keys(value).sort(), [...expected].sort()))
    throw new Error("Unexpected raster binding fields");
}
function text(value: JsonInput): string {
  if (typeof value !== "string" || value.length === 0)
    throw new Error("Missing raster binding text");
  return value;
}
function absolute(value: JsonInput): string {
  const result = text(value);
  if (!path.isAbsolute(result) || path.resolve(result) !== result)
    throw new Error("Absolute normalized raster input path required");
  return result;
}
function digest(value: JsonInput): string {
  const result = text(value);
  if (!/^[0-9a-f]{64}$/.test(result)) throw new Error("Raster expected SHA256 required");
  return result;
}
function positiveInteger(value: JsonInput): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value <= 0)
    throw new Error("Invalid trusted process identity");
  return value;
}

function sameEndpointList(value: JsonInput, expected: RasterEndpoint[]): boolean {
  if (!Array.isArray(value) || value.length !== expected.length) return false;
  return value.every((candidate, index) => {
    const target = expected[index];
    if (target === undefined) return false;
    try {
      const row = object(candidate, "observed raster endpoint");
      keys(row, ["name", "kind", "version", "before", "end"]);
      const before = parseRasterFileIdentity(row.before);
      const end = parseRasterFileIdentity(row.end);
      return (
        text(row.name) === target.name &&
        row.kind === target.kind &&
        text(row.version) === target.version &&
        sameRasterFileIdentity(before, target.before) &&
        sameRasterFileIdentity(end, target.end)
      );
    } catch {
      return false;
    }
  });
}

function requiredCliValue(
  values: Record<string, string | boolean | undefined>,
  name: string,
): string {
  const value = values[name];
  if (typeof value !== "string" || value.length === 0)
    throw new Error(`Missing required --${name}`);
  return value;
}

export function parseRasterComparisonConfigCli(args: string[]) {
  const { values } = parseArgs({
    args,
    options: {
      "trusted-repository-root": { type: "string" },
      fixtures: { type: "string" },
      config: { type: "string" },
      "official-receipt": { type: "string" },
      "official-receipt-sha256": { type: "string" },
      "official-process-evidence": { type: "string" },
      "official-process-sha256": { type: "string" },
      "official-pair-directory": { type: "string" },
      "candidate-receipt": { type: "string" },
      "candidate-receipt-sha256": { type: "string" },
      "candidate-process-evidence": { type: "string" },
      "candidate-process-sha256": { type: "string" },
      "candidate-pair-directory": { type: "string" },
    },
    strict: true,
    allowPositionals: false,
  });
  const value = (name: string) => requiredCliValue(values, name);
  return writeRasterComparisonConfig({
    trustedRepositoryRoot: value("trusted-repository-root"),
    fixturesDir: value("fixtures"),
    configPath: value("config"),
    official: {
      receiptPath: value("official-receipt"),
      receiptSha256: value("official-receipt-sha256"),
      processEvidencePath: value("official-process-evidence"),
      processEvidenceSha256: value("official-process-sha256"),
      pairDirectory: value("official-pair-directory"),
    },
    candidate: {
      receiptPath: value("candidate-receipt"),
      receiptSha256: value("candidate-receipt-sha256"),
      processEvidencePath: value("candidate-process-evidence"),
      processEvidenceSha256: value("candidate-process-sha256"),
      pairDirectory: value("candidate-pair-directory"),
    },
  });
}

if (import.meta.main) {
  try {
    const result = parseRasterComparisonConfigCli(process.argv.slice(2));
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${detail}\n`);
    process.exitCode = 1;
  }
}
