import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { persistFailureReport, runDiagnostic } from "./actual-svg-font-diagnostic";
import { requestSchema, sha256, validateRequest, writeFresh } from "./actual-svg-font-request";
import { browserFailure, type ObserverBrowserObservation } from "./chromium-font-capture";

test("diagnostic rejects noncanonical and invalid requests before creating output", async () => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-svg-font-diagnostic-")));
  try {
    const request = path.join(root, "request.json");
    const alias = path.join(root, "alias.json");
    const output = path.join(root, "output");
    fs.writeFileSync(request, JSON.stringify({ schema: "invalid", output }));
    fs.symlinkSync(request, alias);
    await expect(runDiagnostic("request.json")).rejects.toThrow("canonical absolute path");
    await expect(runDiagnostic(alias)).rejects.toThrow("canonical absolute path");
    await expect(runDiagnostic(request)).rejects.toThrow();
    expect(fs.existsSync(output)).toBe(false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("SVG font request pins files and refuses existing output", () => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-svg-font-request-")));
  try {
    const input = path.join(root, "input.html");
    const observer = path.join(root, "observer");
    fs.writeFileSync(input, "<svg/>");
    fs.writeFileSync(observer, "observer fixture");
    const request = {
      schema: "krr-actual-svg-font-request-v1" as const,
      input: { path: input, sha256: sha256(fs.readFileSync(input)) },
      observer: { path: observer, sha256: sha256(fs.readFileSync(observer)) },
      viewport: { width: 959, height: 648, deviceScaleFactor: 1 },
      selector: "text.frame-title",
      output: path.join(root, "capture"),
    };
    expect(validateRequest(request)).toEqual(request);
    expect(() => validateRequest({ ...request, extra: true })).toThrow();
    expect(() =>
      validateRequest({ ...request, input: { ...request.input, sha256: "0".repeat(64) } }),
    ).toThrow();
    expect(() =>
      requestSchema.parse({ ...request, viewport: { ...request.viewport, deviceScaleFactor: 0 } }),
    ).toThrow();
    fs.mkdirSync(request.output);
    expect(() => validateRequest(request)).toThrow();
    const result = path.join(root, "result.json");
    writeFresh(result, "original");
    expect(() => writeFresh(result, "replacement")).toThrow();
    expect(fs.readFileSync(result, "utf8")).toBe("original");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("SVG font request rejects symlinked pinned input", () => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-svg-font-link-")));
  try {
    const original = path.join(root, "source.html");
    const alias = path.join(root, "alias.html");
    fs.writeFileSync(original, "<svg/>");
    fs.symlinkSync(original, alias);
    const pin = { path: alias, sha256: sha256(fs.readFileSync(original)) };
    expect(() =>
      validateRequest({
        schema: "krr-actual-svg-font-request-v1",
        input: pin,
        observer: pin,
        viewport: { width: 1, height: 1, deviceScaleFactor: 1 },
        selector: "text",
        output: path.join(root, "out"),
      }),
    ).toThrow();
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("failed browser evidence preserves capture error, exit status, and stderr", () => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-svg-font-failure-")));
  try {
    const fixture = failureRequestFixture(root);
    fs.mkdirSync(fixture.request.output);
    persistFailureReport(
      fixture.requestPath,
      fixture.requestBytes,
      fixture.request,
      fixture.request.output,
      path.join(fixture.request.output, "trace.json"),
      "session",
      failureFixtureError(fixture.observerPath),
      undefined,
    );
    const report = JSON.parse(
      fs.readFileSync(path.join(fixture.request.output, "report.json"), "utf8"),
    ) as FailureReport;
    expect(report).toMatchObject({ diagnosticOnly: true, accepted: false, status: "failed" });
    expect(report.browser).toMatchObject({
      processId: 42,
      actualExitCode: 1,
      stderr: "browser stderr retained",
    });
    expect(report.failure.originalFailure).toContain("original page failure");
    expect(report.failure.cleanupFailures).toContain("browser close failed");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

type FailureReport = {
  diagnosticOnly: boolean;
  accepted: boolean;
  status: string;
  browser: { processId: number; actualExitCode: number; stderr: string };
  failure: { originalFailure: string; cleanupFailures: string[] };
};

function failureRequestFixture(root: string) {
  const inputPath = path.join(root, "input.html");
  const observerPath = path.join(root, "observer");
  const requestPath = path.join(root, "request.json");
  const requestValue = {
    schema: "krr-actual-svg-font-request-v1" as const,
    input: { path: inputPath, sha256: sha256(Buffer.from("<svg/>")) },
    observer: { path: observerPath, sha256: sha256(Buffer.from("pinned observer fixture")) },
    viewport: { width: 100, height: 80, deviceScaleFactor: 1 },
    selector: "text",
    output: path.join(root, "capture"),
  };
  fs.writeFileSync(inputPath, "<svg/>");
  fs.writeFileSync(observerPath, "pinned observer fixture");
  fs.writeFileSync(requestPath, JSON.stringify(requestValue));
  return {
    requestPath,
    requestBytes: fs.readFileSync(requestPath),
    request: validateRequest(requestValue),
    observerPath,
  };
}

function failureFixtureError(observerPath: string) {
  const original = new Error("original page failure");
  const browser: ObserverBrowserObservation = {
    processId: 42,
    actualExitCode: 1,
    signalCode: null,
    stderr: "browser stderr retained",
    stderrAvailable: true,
    argv: [observerPath, "--trace-startup=katana.font-observer"],
  };
  const failure = browserFailure(
    { browser: undefined, captured: false, failure: original },
    browser,
    ["browser close failed"],
  );
  if (!failure || failure.originalFailure !== original)
    throw new Error("original browser capture failure was not retained");
  return failure;
}
