import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { capturePage } from "./actual-svg-font-page";
import { assertPinnedFile, sha256, validateRequest, writeFresh } from "./actual-svg-font-request";
import { ObserverBrowserFailure, withObserverBrowser } from "./chromium-font-capture";

type PageCapture = Awaited<ReturnType<typeof capturePage>>;
type BrowserCapture = Awaited<ReturnType<typeof withObserverBrowser<PageCapture>>>;

export async function runDiagnostic(requestPath: string): Promise<void> {
  if (!path.isAbsolute(requestPath) || fs.realpathSync(requestPath) !== requestPath)
    throw new Error("request must be a canonical absolute path");
  const requestBytes = fs.readFileSync(requestPath);
  const request = validateRequest(JSON.parse(requestBytes.toString("utf8")) as unknown);
  fs.mkdirSync(request.output);
  const tracePath = path.join(request.output, "trace.json");
  const sessionId = randomUUID();
  let captured: BrowserCapture | undefined;
  try {
    captured = await withObserverBrowser({
      executablePath: request.observer.path,
      executableSha256: request.observer.sha256,
      tracePath,
      sessionId,
      capture: (browser) => capturePage(browser, request),
    });
    assertPinnedFile(request.input);
    assertPinnedFile(request.observer);
    const framePath = path.join(request.output, "frame.png");
    writeFresh(framePath, captured.value.png);
    const report = {
      schema: "krr-actual-svg-font-diagnostic-v1",
      diagnosticOnly: true,
      accepted: false,
      request: { path: requestPath, sha256: sha256(requestBytes), value: request },
      browser: {
        version: captured.value.browserVersion,
        executable: request.observer,
        processId: captured.processId,
        actualExitCode: captured.actualExitCode,
        stderr: captured.stderr,
        argv: captured.argv,
      },
      viewport: captured.value.viewport,
      frame: { path: framePath, sha256: sha256(captured.value.png) },
      trace: {
        path: tracePath,
        sha256: captured.trace.traceSha256,
        sessionId,
        instances: captured.trace.instances,
      },
      target: captured.value.observation,
      platformFonts: captured.value.platformFonts,
    };
    writeFresh(path.join(request.output, "report.json"), JSON.stringify(report, null, 2));
  } catch (error) {
    persistFailureReport(
      requestPath,
      requestBytes,
      request,
      request.output,
      tracePath,
      sessionId,
      error,
      captured,
    );
    throw error;
  }
}

export function persistFailureReport(
  requestPath: string,
  requestBytes: Buffer,
  request: ReturnType<typeof validateRequest>,
  output: string,
  tracePath: string,
  sessionId: string,
  error: unknown,
  captured: BrowserCapture | undefined,
): void {
  const report = {
    schema: "krr-actual-svg-font-diagnostic-v1",
    diagnosticOnly: true,
    accepted: false,
    status: "failed",
    request: { path: requestPath, sha256: sha256(requestBytes), value: request },
    browser: browserFailureState(request.observer, error, captured),
    trace: traceFailureState(tracePath, sessionId, captured),
    failure: errorState(error),
  };
  try {
    writeFresh(path.join(output, "report.json"), JSON.stringify(report, null, 2));
  } catch (reportError) {
    process.stderr.write(
      `Could not save diagnostic failure report: ${errorMessage(reportError)}\n`,
    );
  }
}

function browserFailureState(
  observer: { path: string; sha256: string },
  error: unknown,
  captured: BrowserCapture | undefined,
): object {
  if (captured)
    return {
      executable: observer,
      processId: captured.processId,
      actualExitCode: captured.actualExitCode,
      stderr: captured.stderr,
      argv: captured.argv,
      version: captured.value.browserVersion,
    };
  if (error instanceof ObserverBrowserFailure) return { executable: observer, ...error.browser };
  return { executable: observer };
}

function traceFailureState(
  tracePath: string,
  sessionId: string,
  captured: BrowserCapture | undefined,
): object {
  if (captured)
    return {
      path: tracePath,
      sha256: captured.trace.traceSha256,
      sessionId,
      instances: captured.trace.instances,
    };
  if (!fs.existsSync(tracePath)) return { path: tracePath, sessionId, available: false };
  try {
    const bytes = fs.readFileSync(tracePath);
    return { path: tracePath, sha256: sha256(bytes), sessionId, available: true };
  } catch (error) {
    return { path: tracePath, sessionId, available: false, readError: errorMessage(error) };
  }
}

function errorState(error: unknown): object {
  if (error instanceof ObserverBrowserFailure)
    return {
      name: error.name,
      message: error.message,
      originalFailure: errorMessage(error.originalFailure),
      cleanupFailures: error.cleanupFailures,
      stack: error.stack,
    };
  return {
    name: error instanceof Error ? error.name : "UnknownError",
    message: errorMessage(error),
    stack: error instanceof Error ? error.stack : undefined,
  };
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

if (import.meta.main) {
  if (process.argv.length !== 3 || !process.argv[2])
    throw new Error("one request JSON path is required");
  await runDiagnostic(process.argv[2]);
}
