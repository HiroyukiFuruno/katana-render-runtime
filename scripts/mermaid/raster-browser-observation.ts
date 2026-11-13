import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserHandle } from "./official-renderer-types";
import { physicalFileIdentity } from "./raster-comparison-binding";
import type { JsonInput, JsonObject } from "./raster-json-input";
import {
  type RasterEndpoint,
  type RasterFileIdentity,
  type RasterRole,
  sameRasterFileIdentity,
} from "./raster-production-binding";

export interface BrowserObservationRequest {
  role: RasterRole;
  attempt: string;
  nonce: string;
  observerPython: string;
}
export interface ObservedBrowserIdentity {
  endpoint: RasterEndpoint;
  observationPath: string;
  pid: number;
  startIdentity: string;
  producerPid: number;
  producerStartIdentity: string;
  parentChain: Array<{ pid: number; startIdentity: string }>;
  argv: string[];
}

export function uniqueOwnBrowserPid(response: JsonInput): number {
  const root = object(response);
  if (!Array.isArray(root.processInfo))
    throw new Error("Own browser CDP process inventory is missing");
  const rows = root.processInfo.map(object).filter((row) => row.type === "browser");
  if (rows.length !== 1) throw new Error("Own browser CDP must identify exactly one browser PID");
  return pid(rows[0]?.id);
}

export function requireApprovedBrowserObservationRoute(observerPython: string) {
  if (process.platform !== "darwin")
    throw new Error(
      "Actual browser OS identity adapter is currently macOS-only; other platforms fail closed",
    );
  // 既存launch済み同じBrowserのCDP identity照会だけを使い、navigationやbrowser条件を変更しない。
  physicalFileIdentity(observerPython);
  physicalFileIdentity(collectorPath());
}

export async function observeActualBrowserIdentity(
  browser: BrowserHandle,
  request: BrowserObservationRequest,
): Promise<ObservedBrowserIdentity> {
  requireApprovedBrowserObservationRoute(request.observerPython);
  if (browser.newBrowserCDPSession === undefined || browser.version === undefined) {
    throw new Error("Actual launched browser has no supported own-browser identity API");
  }
  const session = await browser.newBrowserCDPSession();
  let browserPid: number;
  try {
    browserPid = uniqueOwnBrowserPid(await session.send("SystemInfo.getProcessInfo"));
  } finally {
    await session.detach();
  }
  const result = spawnSync(
    request.observerPython,
    [collectorPath(), String(browserPid), String(process.pid)],
    { encoding: "utf8", timeout: 10000, maxBuffer: 1024 * 1024 },
  );
  if (result.error !== undefined || result.status !== 0 || result.signal !== null) {
    throw new Error("Actual owned browser OS identity collector failed");
  }
  const measured = parseOwnedBrowserMeasurement(
    JSON.parse(result.stdout) as JsonInput,
    browserPid,
    process.pid,
  );
  if (measured.collectorPid !== result.pid)
    throw new Error("Actual collector PID differs from owned spawned child");
  const version = browser.version();
  if (version === "") throw new Error("Actual browser version is missing");
  const identity = physicalFileIdentity(measured.argv[0] as string);
  if (!sameBrowserExecutable(identity, measured.osExecutablePath))
    throw new Error(
      `Actual OS browser executable and argv differ: ${JSON.stringify(browserMismatchProjection(identity, measured.osExecutablePath, measured.argv.length))}`,
    );
  const evidence = {
    schema: "katana-own-browser-identity-observation-v1",
    role: request.role,
    attempt: request.attempt,
    nonce: request.nonce,
    cdpBrowserPid: browserPid,
    osMeasurement: JSON.parse(result.stdout) as JsonInput,
    collectorExitCode: result.status,
    collectorReaped: true,
    collectorPid: result.pid,
    python: physicalFileIdentity(request.observerPython),
    collector: physicalFileIdentity(collectorPath()),
    endpoint: { name: "actual-browser", kind: "browser", version, before: identity, end: identity },
  };
  const observationPath = path.join(
    request.attempt,
    `browser-identity-${Date.now()}-${process.hrtime.bigint()}.json`,
  );
  fs.writeFileSync(observationPath, `${JSON.stringify(evidence, null, 2)}\n`, { flag: "wx" });
  // これはproducerの実測rawであり、trusted期待SHA/configは発行しない。
  return {
    endpoint: { name: "actual-browser", kind: "browser", version, before: identity, end: identity },
    observationPath,
    pid: measured.pid,
    startIdentity: measured.startIdentity,
    producerPid: measured.producerPid,
    producerStartIdentity: measured.producerStartIdentity,
    parentChain: measured.parentChain,
    argv: measured.argv,
  };
}

export function sameBrowserExecutable(argvIdentity: RasterFileIdentity, osExecutablePath: string) {
  const osIdentity = physicalFileIdentity(fs.realpathSync(osExecutablePath));
  // macOSの実行パス表記とargvが異なっても、同じ実ファイルの証跡を照合する。
  return sameRasterFileIdentity(argvIdentity, {
    ...osIdentity,
    path: argvIdentity.path,
    canonical: argvIdentity.canonical,
  });
}

export function browserMismatchProjection(
  argvIdentity: RasterFileIdentity,
  osExecutablePath: string,
  argc: number,
) {
  try {
    const osIdentity = physicalFileIdentity(fs.realpathSync(osExecutablePath));
    return {
      schema: "katana-browser-mismatch-projection-v1",
      pidAndAncestryVerified: true,
      observationBeforeEndEqual: true,
      argc,
      osIdentityAvailable: true,
      realpathEqual: argvIdentity.canonical === osIdentity.canonical,
      deviceInodeEqual: argvIdentity.dev === osIdentity.dev && argvIdentity.ino === osIdentity.ino,
      sha256Equal: argvIdentity.sha256 === osIdentity.sha256,
    };
  } catch {
    return {
      schema: "katana-browser-mismatch-projection-v1",
      pidAndAncestryVerified: true,
      observationBeforeEndEqual: true,
      argc,
      osIdentityAvailable: false,
      realpathEqual: null,
      deviceInodeEqual: null,
      sha256Equal: null,
    };
  }
}

export function parseOwnedBrowserMeasurement(
  value: JsonInput,
  expectedBrowserPid: number,
  expectedProducerPid: number,
) {
  const root = object(value),
    browser = object(root.browser),
    producer = object(root.producer),
    collector = object(root.collector);
  if (
    root.schema !== "katana-macos-owned-browser-process-v1" ||
    root.platform !== "darwin" ||
    root.beforeEndEqual !== true ||
    pid(collector.parentPid) !== expectedProducerPid ||
    !text(collector.startIdentity) ||
    pid(browser.pid) !== expectedBrowserPid ||
    pid(producer.pid) !== expectedProducerPid ||
    expectedBrowserPid === expectedProducerPid
  ) {
    throw new Error("Actual owned browser process measurement differs");
  }
  const startIdentity = text(browser.startIdentity),
    producerStartIdentity = text(producer.startIdentity);
  const osExecutablePath = absolute(browser.executablePath);
  if (!Array.isArray(browser.argv) || browser.argv.length === 0)
    throw new Error("Actual browser argv is missing");
  const argv = browser.argv.map(text);
  absolute(argv[0]);
  if (
    !Array.isArray(root.parentChain) ||
    root.parentChain.length === 0 ||
    root.parentChain.length > 64
  )
    throw new Error("Actual browser ancestry is missing");
  const parents = root.parentChain.map((value: JsonInput) => {
    const row = object(value);
    return {
      pid: pid(row.pid),
      parentPid: pid(row.parentPid),
      startIdentity: text(row.startIdentity),
    };
  });
  if (
    new Set(parents.map((row) => row.pid)).size !== parents.length ||
    parents.some((row) => row.pid === expectedBrowserPid) ||
    pid(browser.parentPid) !== parents[0]?.pid ||
    parents.some(
      (row, index) => index < parents.length - 1 && row.parentPid !== parents[index + 1]?.pid,
    ) ||
    parents.at(-1)?.pid !== expectedProducerPid ||
    parents.at(-1)?.startIdentity !== producerStartIdentity
  ) {
    throw new Error("Actual browser parent chain does not reach the producer");
  }
  return {
    collectorPid: pid(collector.pid),
    pid: expectedBrowserPid,
    startIdentity,
    producerPid: expectedProducerPid,
    producerStartIdentity,
    osExecutablePath,
    argv,
    parentChain: parents.map(({ pid, startIdentity }) => ({ pid, startIdentity })),
  };
}

export function collectorPath() {
  return path.join(path.dirname(fileURLToPath(import.meta.url)), "browser-process-observe.py");
}
function object(value: JsonInput): JsonObject {
  if (value === null || typeof value !== "object" || Array.isArray(value))
    throw new Error("Invalid browser observation record");
  return value as JsonObject;
}
function pid(value: JsonInput): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value <= 0)
    throw new Error("Invalid observed PID");
  return value;
}
function text(value: JsonInput): string {
  if (typeof value !== "string" || value === "") throw new Error("Missing observed browser value");
  return value;
}
function absolute(value: JsonInput): string {
  const result = text(value);
  if (!path.isAbsolute(result) || path.resolve(result) !== result)
    throw new Error("Actual browser path must be absolute");
  return result;
}
