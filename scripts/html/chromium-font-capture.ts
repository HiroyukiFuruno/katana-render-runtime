import { createHash } from "node:crypto";
import type { ChildProcess } from "node:child_process";
import fs from "node:fs";
import { chromium, type Browser, type BrowserServer } from "playwright";
import { type ChromiumFontTrace, readChromiumFontTrace } from "./chromium-font-trace";

export type ObserverBrowserObservation = {
  processId: number | null;
  actualExitCode: number | null;
  signalCode: NodeJS.Signals | null;
  stderr: string;
  stderrAvailable: boolean;
  argv: string[];
};

export class ObserverBrowserFailure extends Error {
  constructor(
    message: string,
    readonly originalFailure: unknown,
    readonly browser: ObserverBrowserObservation,
    readonly cleanupFailures: readonly string[],
  ) {
    super(message);
    this.name = "ObserverBrowserFailure";
    this.cause = originalFailure;
  }
}

export function observerLaunchArguments(sessionId: string, tracePath: string): string[] {
  if (sessionId.length === 0 || tracePath.length === 0)
    throw new Error("observer trace binding is incomplete");
  return [
    "--trace-startup=katana.font-observer",
    "--trace-startup-format=json",
    `--trace-startup-file=${tracePath}`,
    `--katana-actual-font-observer-session=${sessionId}`,
  ];
}

function assertExecutable(executablePath: string, executableSha256: string): void {
  if (
    fs.realpathSync(executablePath) !== executablePath ||
    createHash("sha256").update(fs.readFileSync(executablePath)).digest("hex") !== executableSha256
  )
    throw new Error("observer Chrome executable physical pin differs");
}

export async function withObserverBrowser<T>(options: {
  executablePath: string;
  executableSha256: string;
  tracePath: string;
  sessionId: string;
  capture: (browser: Awaited<ReturnType<typeof chromium.connect>>) => Promise<T>;
}): Promise<{
  value: T;
  trace: ChromiumFontTrace;
  processId: number;
  actualExitCode: number;
  stderr: string;
  argv: string[];
}> {
  const argv = observerLaunchArguments(options.sessionId, options.tracePath);
  assertExecutable(options.executablePath, options.executableSha256);
  if (fs.existsSync(options.tracePath)) throw new Error("observer trace already exists");
  const server = await chromium.launchServer({
    executablePath: options.executablePath,
    headless: true,
    args: argv,
  });
  const process = server.process();
  const monitor = monitorBrowserProcess(process);
  const attempt = await captureBrowser(server, process, options);
  const cleanupFailures = await closeBrowser(server, attempt.browser);
  const exit = await monitor.closed;
  if (monitor.failure() !== undefined) cleanupFailures.push(monitor.failure() ?? "");
  const observation = browserObservation(process, exit, monitor.stderr(), monitor.stderrAvailable);
  const failure = browserFailure(attempt, observation, cleanupFailures);
  if (failure) throw failure;
  if (!attempt.captured || observation.processId === null || observation.actualExitCode !== 0)
    throw new Error("observer browser success state is incomplete");
  try {
    assertExecutable(options.executablePath, options.executableSha256);
    const trace = readObserverTrace(options.tracePath, options.sessionId);
    return {
      value: attempt.value,
      trace,
      processId: observation.processId,
      actualExitCode: observation.actualExitCode,
      stderr: observation.stderr,
      argv: observation.argv,
    };
  } catch (error) {
    throw new ObserverBrowserFailure(
      `observer Chrome evidence validation failed: ${errorMessage(error)}`,
      error,
      observation,
      cleanupFailures,
    );
  }
}

export type CaptureAttempt<T> =
  | { browser: Browser | undefined; captured: true; value: T; failure?: undefined }
  | { browser: Browser | undefined; captured: false; value?: undefined; failure: unknown };

type ProcessExit = { code: number | null; signal: NodeJS.Signals | null };

function monitorBrowserProcess(process: ChildProcess): {
  closed: Promise<ProcessExit>;
  stderr: () => string;
  stderrAvailable: boolean;
  failure: () => string | undefined;
} {
  const chunks: Buffer[] = [];
  let processFailure: string | undefined;
  let stderrFailure: string | undefined;
  const stderrAvailable = process.stderr !== null;
  process.stderr?.on("data", (chunk: Buffer | string) => chunks.push(Buffer.from(chunk)));
  process.stderr?.on("error", (error) => {
    stderrFailure = errorMessage(error);
  });
  process.on("error", (error) => {
    processFailure = errorMessage(error);
  });
  const closed = new Promise<ProcessExit>((resolve) => {
    process.once("close", (code, signal) => resolve({ code, signal }));
  });
  return {
    closed,
    stderr: () => Buffer.concat(chunks).toString("utf8"),
    stderrAvailable,
    failure: () => processFailure ?? stderrFailure,
  };
}

async function captureBrowser<T>(
  server: BrowserServer,
  process: ChildProcess,
  options: {
    executablePath: string;
    executableSha256: string;
    capture: (browser: Browser) => Promise<T>;
  },
): Promise<CaptureAttempt<T>> {
  let browser: Browser | undefined;
  try {
    if (!process.pid) throw new Error("observer Chrome process id is unavailable");
    if (!process.stderr) throw new Error("observer Chrome stderr stream is unavailable");
    if (!process.spawnfile || fs.realpathSync(process.spawnfile) !== options.executablePath)
      throw new Error("observer Chrome executable path mismatch");
    assertExecutable(process.spawnfile, options.executableSha256);
    browser = await chromium.connect(server.wsEndpoint());
    return { browser, captured: true, value: await options.capture(browser) };
  } catch (failure) {
    return { browser, captured: false, value: undefined, failure };
  }
}

async function closeBrowser(
  server: BrowserServer,
  browser: Browser | undefined,
): Promise<string[]> {
  const failures: string[] = [];
  if (browser) {
    try {
      await browser.close();
    } catch (error) {
      failures.push(errorMessage(error));
    }
  }
  let closed = false;
  try {
    await server.close();
    closed = true;
  } catch (error) {
    failures.push(errorMessage(error));
  }
  if (!closed) {
    try {
      await server.kill();
    } catch (error) {
      failures.push(errorMessage(error));
    }
  }
  return failures;
}

function browserObservation(
  process: ChildProcess,
  exit: ProcessExit,
  stderr: string,
  stderrAvailable: boolean,
): ObserverBrowserObservation {
  return {
    processId: process.pid ?? null,
    actualExitCode: exit.code,
    signalCode: exit.signal,
    stderr,
    stderrAvailable,
    argv: process.spawnargs.slice(),
  };
}

export function browserFailure<T>(
  attempt: CaptureAttempt<T>,
  browser: ObserverBrowserObservation,
  cleanupFailures: string[],
): ObserverBrowserFailure | undefined {
  if (
    attempt.captured &&
    cleanupFailures.length === 0 &&
    browser.actualExitCode === 0 &&
    browser.signalCode === null &&
    browser.processId !== null &&
    browser.stderrAvailable
  )
    return undefined;
  const cause = attempt.captured
    ? (cleanupFailures[0] ??
      new Error(
        `observer Chrome exited with ${browser.actualExitCode ?? browser.signalCode ?? "unknown status"}`,
      ))
    : attempt.failure;
  return new ObserverBrowserFailure(
    `observer Chrome capture failed: ${errorMessage(cause)}`,
    cause,
    browser,
    cleanupFailures,
  );
}

function readObserverTrace(tracePath: string, sessionId: string): ChromiumFontTrace {
  const bytes = fs.readFileSync(tracePath);
  const traceSha256 = createHash("sha256").update(bytes).digest("hex");
  return readChromiumFontTrace({ tracePath, traceSha256, sessionId });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}
