import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { javaScriptTypeReader, typeScriptToolBinding } from "../quality/owned-type-projects";
import { checkHarnessContract, declaredValidationCode } from "./harness-contract";
import { executionGraph, moonStaticTaskProjection, policyContract } from "./harness-contract-graph";
import { nativeCoverageCacheTask } from "./native-coverage-cache-contract";

const moonAuthorityNames = [
  ".moon/workspace.yml",
  "moon.yml",
  "Justfile",
  "package.json",
  "bun.lock",
  "scripts/html/harness-contract-cli.ts",
  "scripts/html/harness-contract-graph.ts",
  "scripts/html/native-coverage-cache-contract.ts",
  "scripts/html/moon-quality-inputs.ts",
  "scripts/html/harness-contract-policy.json",
  "scripts/hooks/run_parallel_checks.py",
  "scripts/release/native_coverage_local_aggregate.py",
  "scripts/release/native_coverage_local_anchor.py",
  "scripts/release/native_coverage_local_context.py",
  "scripts/release/native_coverage_local_result.py",
  "scripts/release/native_coverage_linux_context.py",
  "scripts/release/native_coverage_result.py",
  "scripts/release/native_coverage_result_cli.py",
  "scripts/mermaid/native-source-manifest.ts",
  "scripts/mermaid/mermaid-pair-manifest.ts",
  "scripts/mermaid/native-build-receipt.ts",
  "scripts/mermaid/native-cli-managed-build.ts",
  "scripts/mermaid/native-cli-managed-build-schema.ts",
  "scripts/mermaid/native-cli-managed-json.ts",
] as const;
const sha256 = z.string().regex(/^[0-9a-f]{64}$/);
const fingerprintIdentity = z.strictObject({
  version: z.string().min(1),
  executableSha256: sha256,
});
const runtimeBundleToolFingerprint = z.strictObject({
  schema: z.literal("krr-runtime-bundle-tool-fingerprint-v1"),
  platform: z.string().min(1),
  arch: z.string().min(1),
  tools: z.strictObject({
    bun: fingerprintIdentity,
    just: fingerprintIdentity,
    node: fingerprintIdentity,
    bash: fingerprintIdentity,
    moon: z.strictObject({
      version: z.string().min(1),
      executableSha256: sha256,
      wrapperSha256: sha256,
      utilsSha256: sha256,
      packageSha256: sha256,
    }),
  }),
  environment: z.array(z.strictObject({ name: z.string().min(1), valueSha256: sha256 })),
});
const runtimeBundleFingerprintScript =
  "bun scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts";
const inputSha256 = z.record(z.enum(moonAuthorityNames), sha256);
const moonEntry = z.enum(["coverage", "runtime-bundle-check"]);
const nanoseconds = z.string().regex(/^(?:0|[1-9][0-9]*)$/);
const head = z.string().regex(/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/);
const moonTask = z.strictObject({
  deps: z.array(z.string()).refine((deps) => new Set(deps).size === deps.length),
  leaf: z.string().nullable(),
  options: z.looseObject({
    cache: z.union([z.boolean(), z.literal("local")]),
    shell: z.literal(false),
    allowFailure: z.literal(false),
    expectFailure: z.literal(false),
    runInCI: z.literal("always"),
  }),
});
const runContract = z.strictObject({
  schema: z.literal(1),
  project: z.literal("krr"),
  entry: moonEntry,
  version: z.literal("2.6.2"),
  inputSha256,
  tasks: z.record(z.string().min(1), moonTask),
});
const preflightContract = z.strictObject({
  schema: z.literal(1),
  staticContractOnly: z.literal(true),
  errors: z.array(z.string()).length(0),
  moonRunContract: runContract,
});
const verificationRequest = z.strictObject({
  schema: z.literal(1),
  entry: moonEntry.default("coverage"),
  preflightText: z.string(),
  reportText: z.string(),
  reportSha256: sha256,
  inputSha256Before: inputSha256,
  inputSha256After: inputSha256,
  headBefore: head,
  headAfter: head,
  startedNs: nanoseconds,
  finishedNs: nanoseconds,
  reportMtimeNs: nanoseconds,
});
const verificationSuccess = z.strictObject({
  schema: z.literal(1),
  status: z.literal("verified"),
  reportSha256: sha256,
  inputSha256,
  head,
  startedNs: nanoseconds,
  finishedNs: nanoseconds,
  reportMtimeNs: nanoseconds,
});
const moonTimestamp = z.iso
  .datetime({ local: true })
  .regex(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z?$/);
const operation = z.looseObject({
  status: z.enum([
    "cached",
    "cached-from-remote",
    "failed",
    "invalid",
    "passed",
    "running",
    "skipped",
    "timed-out",
    "aborted",
  ]),
  meta: z.looseObject({ type: z.string(), hash: sha256.optional() }),
});
const taskAction = z.looseObject({
  status: z.enum(["passed", "cached"]),
  allowFailure: z.literal(false),
  expectFailure: z.literal(false),
  error: z.never().optional(),
  nodeIndex: z.number().int().nonnegative(),
  node: z.looseObject({
    action: z.literal("run-task"),
    params: z.looseObject({
      target: z.string(),
      args: z.array(z.string()).length(0),
      env: z.record(z.string(), z.string()).refine((env) => Object.keys(env).length === 0),
      interactive: z.literal(false),
      persistent: z.literal(false),
    }),
  }),
  startedAt: moonTimestamp,
  finishedAt: moonTimestamp,
  operations: z.array(operation).min(1),
});
const completedReport = z.looseObject({
  status: z.literal("completed"),
  context: z.looseObject({
    initialTargets: z.array(z.string()).length(1),
    primaryTargets: z.array(z.string()).length(1),
    affected: z.null(),
    passthroughArgs: z.array(z.string()).length(0),
    ignoredDependencies: z
      .record(z.string(), z.array(z.string()))
      .refine((deps) => Object.keys(deps).length === 0)
      .optional(),
    targetStates: z.record(
      z.string(),
      z.discriminatedUnion("state", [
        z.strictObject({ state: z.literal("passed"), hash: sha256 }),
        z.strictObject({ state: z.literal("passthrough") }),
      ]),
    ),
  }),
  actions: z.array(taskAction).min(1),
});
type MoonRunContract = z.infer<typeof runContract>;
type MoonTaskAction = z.infer<typeof taskAction>;

function bytesSha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}
function authorityBytes(root: string, name: (typeof moonAuthorityNames)[number]): Buffer {
  const file = path.resolve(root, name);
  if (
    !file.startsWith(`${root}${path.sep}`) ||
    !fs.realpathSync(file).startsWith(`${root}${path.sep}`)
  )
    throw new Error(`external Moon authority: ${name}`);
  if (!fs.lstatSync(file).isFile()) throw new Error(`non-regular Moon authority: ${name}`);
  return fs.readFileSync(file);
}
function timestampNs(value: string): bigint {
  const withoutZone = moonTimestamp.parse(value).replace(/Z$/, "");
  const [seconds, fraction = ""] = withoutZone.split(".");
  if (!seconds) throw new Error("missing Moon timestamp seconds");
  return BigInt(Date.parse(`${seconds}Z`)) * 1_000_000n + BigInt(fraction.padEnd(9, "0"));
}
function sameInputs(
  left: z.infer<typeof inputSha256>,
  right: z.infer<typeof inputSha256>,
): boolean {
  return moonAuthorityNames.every((name) => left[name] === right[name]);
}

function repositoryIdentity(root: string): { root: string; head: string } {
  const physicalRoot = fs.realpathSync(root);
  const environment = Object.fromEntries(
    Object.entries(process.env).filter(([name]) => !name.startsWith("GIT_")),
  );
  const git = Bun.spawnSync(
    ["git", "--no-replace-objects", "rev-parse", "--show-toplevel", "HEAD"],
    { cwd: physicalRoot, env: environment },
  );
  if (git.exitCode !== 0) throw new Error("current Git repository identity is unavailable");
  const [topLevel, currentHead, ...extra] = git.stdout.toString().trim().split(/\r?\n/);
  if (!topLevel || extra.length || fs.realpathSync(topLevel) !== physicalRoot)
    throw new Error("run Moon verification from the physical Git repository root");
  return { root: physicalRoot, head: head.parse(currentHead) };
}

export function moonRunContract(
  root: string,
  entry: z.infer<typeof moonEntry> = "coverage",
): MoonRunContract {
  root = repositoryIdentity(root).root;
  const contents = new Map(moonAuthorityNames.map((name) => [name, authorityBytes(root, name)]));
  const text = (name: (typeof moonAuthorityNames)[number]): string => {
    const bytes = contents.get(name);
    if (!bytes) throw new Error(`missing Moon authority: ${name}`);
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  };
  const workspace = z
    .looseObject({
      projects: z.looseObject({ krr: z.literal(".") }),
      versionConstraint: z.literal("=2.6.2"),
    })
    .parse(Bun.YAML.parse(text(".moon/workspace.yml")));
  z.looseObject({ devDependencies: z.looseObject({ "@moonrepo/cli": z.literal("2.6.2") }) }).parse(
    JSON.parse(text("package.json")),
  );
  const policy = policyContract.parse(
    JSON.parse(text("scripts/html/harness-contract-policy.json")),
  );
  if (
    policy.moon?.project !== "krr" ||
    policy.moon.entry !== "coverage" ||
    policy.moon.workspace !== ".moon/workspace.yml" ||
    policy.moon.config !== "moon.yml"
  )
    throw new Error("unexpected KRR Moon policy");
  return runContract.parse({
    schema: 1,
    project: "krr",
    entry,
    version: workspace.versionConstraint.slice(1),
    inputSha256: Object.fromEntries(
      [...contents].map(([name, bytes]) => [name, bytesSha256(bytes)]),
    ),
    tasks: moonStaticTaskProjection(text("moon.yml"), "krr", entry),
  });
}

export function runtimeBundleCacheActionAuthorized(
  contract: MoonRunContract,
  target: string,
): boolean {
  const task = contract.tasks["runtime-bundle-check"];
  return (
    target === "krr:runtime-bundle-check" &&
    task?.leaf === "runtime-bundle-check" &&
    task.deps.length === 0 &&
    task.options.cache === true
  );
}

function nativeCoverageProducerForTarget(
  contract: MoonRunContract,
  target: string,
): "root" | "performance" | undefined {
  const name =
    target === "krr:unit-test"
      ? "unit-test"
      : target === "krr:performance-coverage"
        ? "performance-coverage"
        : undefined;
  if (!name) return undefined;
  const expected = name === "unit-test" ? "root-test-and-coverage" : "performance-harness-coverage";
  const task = contract.tasks[name];
  if (!task || task.leaf !== expected || task.options.cache !== "local") return undefined;
  if (!nativeCoverageCacheTask(name, { command: ["just", expected] })) return undefined;
  const aggregate = contract.tasks.coverage;
  if (
    contract.entry !== "coverage" ||
    aggregate?.leaf !== "coverage-result-check" ||
    aggregate.options.cache !== false ||
    !aggregate.deps.includes("performance-coverage")
  )
    return undefined;
  return name === "unit-test" ? "root" : "performance";
}

function validateNativeCoverageReceipt(root: string, producer: "root" | "performance"): void {
  const result = Bun.spawnSync(
    [
      "just",
      "--command",
      "python3",
      "scripts/release/native_coverage_result_cli.py",
      "--repo",
      root,
      "local-validate",
      "--producer",
      producer,
    ],
    { cwd: root, stdout: "pipe", stderr: "pipe" },
  );
  if (result.exitCode !== 0) {
    const stderr = result.stderr.toString("utf8").slice(-2048);
    throw new Error(
      `local native coverage receipt validation failed: ${producer}; actualExit=${result.exitCode}; stderr=${stderr}`,
    );
  }
  const receipt = z
    .strictObject({
      schema: z.literal("krr-native-local-coverage-receipt-v1"),
      valid: z.literal(true),
      cacheable: z.literal(true),
      producer: z.enum(["root", "performance"]),
    })
    .parse(JSON.parse(result.stdout.toString("utf8")));
  if (receipt.producer !== producer)
    throw new Error("local native coverage receipt producer differs");
}

function validateTaskAction(
  action: MoonTaskAction,
  task: z.infer<typeof moonTask>,
  targetHash: string,
  allowCached: boolean,
  startedNs: bigint,
  finishedNs: bigint,
): void {
  const started = timestampNs(action.startedAt);
  const finished = timestampNs(action.finishedAt);
  if (started < startedNs || finished < started || finished > finishedNs)
    throw new Error(`Moon task time is outside its invocation: ${action.node.params.target}`);
  const processKinds = [
    "no-operation",
    "output-hydration",
    "process-execution",
    "sync-operation",
    "task-execution",
  ];
  const final = [...action.operations].reverse().find((op) => processKinds.includes(op.meta.type));
  if (action.status === "cached") {
    validateCachedTaskAction(action, task, targetHash, allowCached, final);
    return;
  }
  if (final?.status !== "passed") throw new Error("Moon final task operation did not pass");
  const terminalMeta =
    task.leaf === null
      ? z.looseObject({ type: z.literal("no-operation") })
      : z.looseObject({ type: z.literal("task-execution"), exitCode: z.literal(0) });
  terminalMeta.parse(final.meta);
}

function validateCachedTaskAction(
  action: MoonTaskAction,
  task: z.infer<typeof moonTask>,
  targetHash: string,
  allowCached: boolean,
  final: MoonTaskAction["operations"][number] | undefined,
): void {
  if (
    !allowCached ||
    (task.options.cache !== true && task.options.cache !== "local") ||
    final?.status !== "cached"
  )
    throw new Error("cached Moon action is not authorized by the selected leaf contract");
  if (
    action.operations.some(
      (operation) => operation.status !== "passed" && operation.status !== "cached",
    )
  )
    throw new Error("cached Moon action contains a failed or skipped operation");
  if (final.meta.type !== "output-hydration" && final.meta.type !== "task-execution")
    throw new Error("unknown cached Moon terminal operation");
  const hashes = action.operations
    .filter((operation) => operation.meta.type === "hash-generation")
    .map((operation) => operation.meta.hash);
  if (hashes.length !== 1 || hashes[0] !== targetHash)
    throw new Error("cached Moon action hash differs from the current target state");
}

function validateCompletedReport(
  raw: unknown,
  contract: MoonRunContract,
  root: string,
  startedNs: bigint,
  finishedNs: bigint,
): void {
  const report = completedReport.parse(raw);
  const targets = Object.keys(contract.tasks)
    .map((name) => `krr:${name}`)
    .sort();
  const selected = `krr:${contract.entry}`;
  validateReportTargetSet(report, targets, selected);
  const actions = validateReportActions(report, contract, root, targets, startedNs, finishedNs);
  validateReportDependencyOrder(contract.tasks, actions);
}

function validateReportActions(
  report: z.infer<typeof completedReport>,
  contract: MoonRunContract,
  root: string,
  targets: string[],
  startedNs: bigint,
  finishedNs: bigint,
): Map<string, MoonTaskAction> {
  const actions = new Map<string, MoonTaskAction>();
  const indices = new Set<number>();
  const nativeCachedProducers = new Set<"root" | "performance">();
  for (const action of report.actions) {
    const target = action.node.params.target;
    const task = contract.tasks[target.replace(/^krr:/, "")];
    if (!targets.includes(target) || !task || actions.has(target) || indices.has(action.nodeIndex))
      throw new Error(`unknown or duplicate Moon task action: ${target}`);
    const targetState = report.context.targetStates[target];
    if (targetState?.state !== "passed") throw new Error(`Moon target did not pass: ${target}`);
    const nativeProducer = validateOneReportAction(
      action,
      task,
      target,
      contract,
      root,
      targetState.hash,
      startedNs,
      finishedNs,
    );
    if (action.status === "cached" && nativeProducer) nativeCachedProducers.add(nativeProducer);
    actions.set(target, action);
    indices.add(action.nodeIndex);
  }
  if ([...actions.keys()].sort().join("\0") !== targets.join("\0"))
    throw new Error("Moon task actions differ from the coverage closure");
  validateNativeCachedProducers(root, actions, nativeCachedProducers);
  return actions;
}

function validateOneReportAction(
  action: MoonTaskAction,
  task: MoonRunContract["tasks"][string],
  target: string,
  contract: MoonRunContract,
  root: string,
  targetHash: string,
  startedNs: bigint,
  finishedNs: bigint,
): "root" | "performance" | undefined {
  const nativeProducer = nativeCoverageProducerForTarget(contract, target);
  const allowCached =
    runtimeBundleCacheActionAuthorized(contract, target) || nativeProducer !== undefined;
  validateTaskAction(action, task, targetHash, allowCached, startedNs, finishedNs);
  if (action.status === "cached" && runtimeBundleCacheActionAuthorized(contract, target))
    validateRuntimeBundleCacheManifest(root, targetHash);
  return action.status === "cached" ? nativeProducer : undefined;
}

function validateNativeCachedProducers(
  root: string,
  actions: Map<string, MoonTaskAction>,
  producers: Set<"root" | "performance">,
): void {
  if (producers.size === 0) return;
  if (actions.get("krr:coverage")?.status !== "passed")
    throw new Error("cached native coverage requires an uncached aggregate validation");
  for (const producer of producers) validateNativeCoverageReceipt(root, producer);
}

function validateReportDependencyOrder(
  tasks: MoonRunContract["tasks"],
  actions: Map<string, MoonTaskAction>,
): void {
  for (const [name, task] of Object.entries(tasks)) {
    const consumer = actions.get(`krr:${name}`);
    if (!consumer) throw new Error(`missing Moon consumer: ${name}`);
    for (const dependency of task.deps) {
      const parent = actions.get(`krr:${dependency}`);
      if (!parent || timestampNs(parent.finishedAt) > timestampNs(consumer.startedAt))
        throw new Error(`Moon dependency order changed: ${dependency} -> ${name}`);
    }
  }
}

function validateReportTargetSet(
  report: z.infer<typeof completedReport>,
  targets: string[],
  selected: string,
): void {
  if (
    Object.keys(report.context.targetStates).sort().join("\0") !== targets.join("\0") ||
    report.context.initialTargets[0] !== selected ||
    report.context.primaryTargets[0] !== selected
  )
    throw new Error("Moon target states differ from the selected closure");
}

function validateRuntimeBundleCacheManifest(root: string, taskHash: string): void {
  const relative = `.moon/cache/blobs/${taskHash.slice(0, 2)}/${taskHash.slice(2)}`;
  const file = path.resolve(root, relative);
  if (!file.startsWith(`${root}${path.sep}`) || !fs.existsSync(file))
    throw new Error("Moon cache hash manifest is unavailable");
  const physical = fs.realpathSync(file);
  if (!physical.startsWith(`${root}${path.sep}`) || !fs.lstatSync(file).isFile())
    throw new Error("Moon cache hash manifest is not a repository file");
  const manifest = validateRuntimeBundleCacheBlob(fs.readFileSync(file), taskHash);
  validateRuntimeBundleCacheInputs(root, record(manifest.inputs));
}

export function validateRuntimeBundleCacheBlob(
  bytes: Uint8Array,
  taskHash: string,
): Record<string, unknown> {
  if (bytesSha256(bytes) !== sha256.parse(taskHash))
    throw new Error("Moon hash manifest content hash differs from task hash");
  const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  return validateRuntimeBundleCacheArray(JSON.parse(text));
}

export function validateRuntimeBundleCacheArray(raw: unknown): Record<string, unknown> {
  if (!Array.isArray(raw) || raw.length !== 2) throw new Error("unknown Moon hash manifest shape");
  const manifest = record(raw[0]);
  validateRuntimeBundleCacheManifestBinding(manifest);
  validateRuntimeBundleCheckFingerprint(raw[1]);
  return manifest;
}

function validateRuntimeBundleCheckFingerprint(raw: unknown): void {
  const binding = record(raw);
  if (Object.keys(binding).join("\0") !== "checks")
    throw new Error("unknown Moon task check fingerprint fields");
  const checks = record(binding.checks);
  if (Object.keys(checks).join("\0") !== runtimeBundleFingerprintScript)
    throw new Error("Moon task check fingerprint binding changed");
  const output = record(checks[runtimeBundleFingerprintScript]);
  if (Object.keys(output).join("\0") !== "stdout" || typeof output.stdout !== "string")
    throw new Error("Moon task check fingerprint output changed");
  runtimeBundleToolFingerprint.parse(JSON.parse(output.stdout));
}

function validateRuntimeBundleCacheManifestBinding(manifest: Record<string, unknown>): void {
  const expectedKeys = [
    "args",
    "command",
    "deps",
    "env",
    "inputEnv",
    "inputs",
    "outputs",
    "projectDeps",
    "target",
    "toolchains",
    "version",
  ];
  if (Object.keys(manifest).sort().join("\0") !== expectedKeys.sort().join("\0"))
    throw new Error("unknown Moon hash manifest fields");
  if (
    manifest.command !== "just" ||
    JSON.stringify(manifest.args) !== JSON.stringify(["runtime-bundle-check"]) ||
    JSON.stringify(manifest.deps) !== "{}" ||
    JSON.stringify(manifest.env) !== "{}" ||
    JSON.stringify(manifest.inputEnv) !== "{}" ||
    JSON.stringify(manifest.outputs) !== "[]" ||
    JSON.stringify(manifest.projectDeps) !== "[]" ||
    manifest.target !== "krr:runtime-bundle-check" ||
    JSON.stringify(manifest.toolchains) !== JSON.stringify(["system"]) ||
    manifest.version !== "3"
  )
    throw new Error("Moon hash manifest task binding changed");
}

function validateRuntimeBundleCacheInputs(root: string, inputs: Record<string, unknown>): void {
  const requiredInputs = [
    ".moon/workspace.yml",
    "moon.yml",
    "package.json",
    "bun.lock",
    "Justfile",
    "scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mermaid-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/drawio-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/zenuml-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mathjax-runtime.min.js",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/generated/runtime-bundles.sha256",
    "crates/katana-render-runtime/src/renderer/backends/html_runtime/parser_body_onload.ts",
    "crates/katana-render-runtime/src/renderer/backends/html_runtime/parser_body_onload.js",
  ];
  if (requiredInputs.some((name) => !(name in inputs)))
    throw new Error("Moon cache hash manifest omits a required runtime bundle input");
  for (const [name, value] of Object.entries(inputs))
    validateRuntimeBundleCacheInput(root, name, value);
}

function validateRuntimeBundleCacheInput(root: string, name: string, value: unknown): void {
  if (
    name.startsWith("tmp/") ||
    name.startsWith(".moon/cache/") ||
    typeof value !== "string" ||
    !/^[0-9a-f]{64}$/.test(value)
  )
    throw new Error(`invalid or excluded Moon cache input: ${name}`);
  const current = path.resolve(root, name);
  if (
    !current.startsWith(`${root}${path.sep}`) ||
    path.relative(root, current).replaceAll(path.sep, "/") !== name ||
    !fs.existsSync(current)
  )
    throw new Error(`Moon cache input is missing: ${name}`);
  const real = fs.realpathSync(current);
  if (!real.startsWith(`${root}${path.sep}`) || !fs.lstatSync(current).isFile())
    throw new Error(`Moon cache input is not a repository file: ${name}`);
  if (bytesSha256(fs.readFileSync(current)) !== value)
    throw new Error(`Moon cache input changed: ${name}`);
}

function validateActualMoonReport(root: string, reportText: string, reportMtimeNs: bigint): void {
  const file = path.resolve(root, ".moon/cache/runReport.json");
  if (!file.startsWith(`${root}${path.sep}`) || !fs.existsSync(file))
    throw new Error("cached Moon report is not the current on-disk run report");
  const physical = fs.realpathSync(file);
  if (!physical.startsWith(`${root}${path.sep}`) || !fs.lstatSync(file).isFile())
    throw new Error("cached Moon report is not a repository file");
  if (!fs.readFileSync(file).equals(Buffer.from(reportText, "utf8")))
    throw new Error("cached Moon report bytes differ from the current on-disk run report");
  if (fs.statSync(file, { bigint: true }).mtimeNs !== reportMtimeNs)
    throw new Error("cached Moon report timestamp differs from the current on-disk run report");
}

export function verifyMoonRun(root: string, raw: unknown): z.infer<typeof verificationSuccess> {
  const request = verificationRequest.parse(raw);
  const contract = moonRunContract(root, request.entry);
  const preflight = preflightContract.parse(JSON.parse(request.preflightText));
  if (
    !sameInputs(request.inputSha256Before, request.inputSha256After) ||
    !sameInputs(request.inputSha256Before, contract.inputSha256) ||
    JSON.stringify(preflight.moonRunContract) !== JSON.stringify(contract)
  )
    throw new Error("Moon preflight or authority inputs changed");
  if (bytesSha256(Buffer.from(request.reportText, "utf-8")) !== request.reportSha256)
    throw new Error("Moon report bytes differ from their proof");
  const currentHead = repositoryIdentity(root).head;
  if (request.headBefore !== request.headAfter || request.headBefore !== currentHead)
    throw new Error("Git HEAD changed during Moon execution");
  const startedNs = BigInt(request.startedNs);
  const finishedNs = BigInt(request.finishedNs);
  const mtimeNs = BigInt(request.reportMtimeNs);
  if (finishedNs < startedNs || mtimeNs <= startedNs || mtimeNs > finishedNs)
    throw new Error("Moon report time is outside its invocation");
  // 重複キーはPythonのstrict_json境界で拒否し、ここでは型と意味を検査する。
  const report: unknown = JSON.parse(request.reportText);
  if (
    typeof report === "object" &&
    report !== null &&
    Array.isArray((report as { actions?: unknown }).actions) &&
    (report as { actions: unknown[] }).actions.some(
      (action) =>
        typeof action === "object" &&
        action !== null &&
        (action as { status?: unknown }).status === "cached" &&
        (action as { node?: { params?: { target?: unknown } } }).node?.params?.target ===
          "krr:runtime-bundle-check",
    )
  )
    validateActualMoonReport(
      repositoryIdentity(root).root,
      request.reportText,
      BigInt(request.reportMtimeNs),
    );
  validateCompletedReport(report, contract, repositoryIdentity(root).root, startedNs, finishedNs);
  return verificationSuccess.parse({
    schema: 1,
    status: "verified",
    reportSha256: request.reportSha256,
    inputSha256: contract.inputSha256,
    head: currentHead,
    startedNs: request.startedNs,
    finishedNs: request.finishedNs,
    reportMtimeNs: request.reportMtimeNs,
  });
}

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("expected object");
  return value as Record<string, unknown>;
}
function strings(value: unknown): string[] {
  if (!Array.isArray(value) || !value.every((member) => typeof member === "string"))
    throw new Error("expected string array");
  return value as string[];
}
function readJson(root: string, name: string): Record<string, unknown> {
  const file = path.resolve(root, name);
  if (
    !file.startsWith(`${root}${path.sep}`) ||
    !fs.realpathSync(file).startsWith(`${root}${path.sep}`)
  )
    throw new Error(`external configuration: ${name}`);
  return record(JSON.parse(fs.readFileSync(file, "utf8")));
}

export function resolveRepositoryCode(
  root: string,
  codeRoots: string[],
  reference: string,
  importer: string | undefined,
): string {
  const resolved = Bun.resolveSync(reference, path.resolve(root, importer ?? "package.json"));
  const physical = fs.realpathSync(resolved);
  if (!physical.startsWith(`${root}${path.sep}`))
    throw new Error(`external code resolution: ${reference}`);
  const name = path.relative(root, physical).replaceAll(path.sep, "/");
  if (!codeRoots.some((codeRoot) => name.startsWith(codeRoot)))
    throw new Error(`unmanaged code resolution: ${reference}`);
  return name;
}
export function repositorySnapshot(root: string, declaredCode: string[]) {
  const policy = policyContract.parse(readJson(root, "scripts/html/harness-contract-policy.json"));
  if (
    policy.schema !== 1 ||
    !policy.codeRoots.every((name) => ["scripts/", "crates/"].includes(name))
  )
    throw new Error("invalid contract policy");
  const read = (name: string): string | undefined => {
    const file = path.resolve(root, name);
    if (!file.startsWith(`${root}${path.sep}`)) return undefined;
    if (!fs.existsSync(file)) return undefined;
    if (!fs.realpathSync(file).startsWith(`${root}${path.sep}`))
      throw new Error(`external source symlink: ${name}`);
    return fs.readFileSync(file, "utf8");
  };
  const pkg = readJson(root, "package.json");
  const scripts = Object.fromEntries(
    Object.entries(record(pkg.scripts)).map(([key, value]) => {
      if (typeof value !== "string") throw new Error("invalid package script");
      return [key, value];
    }),
  );
  const biome = record(readJson(root, "biome.json").files);
  const types = readJson(root, "tsconfig.json");
  const git = Bun.spawnSync(["git", "ls-files", "--cached", "-z"], { cwd: root });
  if (git.exitCode !== 0) throw new Error("Git tracked-file authority unavailable");
  const codeFiles = [
    ...new Bun.Glob("scripts/html/**/*.{ts,tsx,js,mjs,cjs,py,sh}").scanSync({ cwd: root }),
  ];
  const includeMatches = (patterns: string[]): Record<string, number> =>
    Object.fromEntries(
      patterns
        .filter(
          (pattern) =>
            !pattern.startsWith("!") &&
            /^(scripts\/|crates\/|tests\/fixtures\/quality-cache\/)/.test(pattern),
        )
        .map((pattern) => [
          pattern,
          [...new Bun.Glob(pattern).scanSync({ cwd: root, onlyFiles: true, followSymlinks: false })]
            .length,
        ]),
    );
  const tracked = new Set(git.stdout.toString().split("\0").filter(Boolean));
  const javaScriptTyped = javaScriptTypeReader(root, { scripts }, tracked);
  const externalToolResolver = typeScriptToolBinding(root);
  return {
    root,
    policy,
    read,
    graph: executionGraph(
      policy,
      read,
      scripts,
      new Set(Object.keys(record(pkg.devDependencies))),
      codeFiles,
      (reference, importer) => resolveRepositoryCode(root, policy.codeRoots, reference, importer),
      externalToolResolver,
    ),
    codeFiles,
    declaredCode,
    tracked,
    javaScriptTyped,
    externalToolResolver,
    biomeIncludes: strings(biome.includes),
    typeIncludes: strings(types.include),
    typeExcludes: strings(types.exclude),
    scopeMatches: {
      biome: includeMatches(strings(biome.includes)),
      types: includeMatches(strings(types.include)),
    },
    authorityFiles: [
      "scripts/html/harness-contract-policy.json",
      "Justfile",
      "package.json",
      "biome.json",
      "tsconfig.json",
      policy.ciWorkflow,
      policy.localBridge.source,
      ...(policy.moon ? [policy.moon.workspace, policy.moon.config] : []),
      ...(policy.rust ? [policy.rust.workspace] : []),
    ],
  };
}

if (import.meta.main) {
  const root = fs.realpathSync(process.cwd());
  if (process.argv[2] === "--verify-moon-run") {
    if (process.argv.length !== 3) throw new Error("unexpected Moon verification arguments");
    const request: unknown = JSON.parse(await Bun.stdin.text());
    console.log(JSON.stringify(verifyMoonRun(root, request)));
    process.exit(0);
  }
  const declared: string[] = [];
  let entry: z.infer<typeof moonEntry> = "coverage";
  for (let index = 2; index < process.argv.length; index += 2) {
    const flag = process.argv[index];
    const value = process.argv[index + 1];
    if (!value) throw new Error(`missing value for ${flag}`);
    if (flag === "--validation-config")
      declared.push(...declaredValidationCode(readJson(root, value)));
    else if (flag === "--moon-entry" && value === "runtime-bundle-check") entry = value;
    else if (flag === "--code-path") declared.push(value);
    else throw new Error(`unknown option: ${flag}`);
  }
  const errors = checkHarnessContract(repositorySnapshot(root, declared));
  console.log(
    JSON.stringify(
      {
        schema: 1,
        staticContractOnly: true,
        errors,
        moonRunContract: moonRunContract(root, entry),
      },
      null,
      2,
    ),
  );
  if (errors.length) process.exitCode = 1;
}
