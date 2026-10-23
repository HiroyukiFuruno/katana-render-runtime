import { createHash } from "node:crypto";
import fs from "node:fs";
import { chromium } from "playwright";
import { type ChromiumFontTrace, readChromiumFontTrace } from "./chromium-font-trace";

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
  let browser: Awaited<ReturnType<typeof chromium.connect>> | undefined;
  let value: T;
  try {
    if (!process.pid) throw new Error("observer Chrome process id is unavailable");
    if (!process.spawnfile || fs.realpathSync(process.spawnfile) !== options.executablePath)
      throw new Error("observer Chrome executable path mismatch");
    assertExecutable(process.spawnfile, options.executableSha256);
    browser = await chromium.connect(server.wsEndpoint());
    value = await options.capture(browser);
  } finally {
    try {
      if (browser) await browser.close();
    } finally {
      await server.close();
    }
  }
  const actualExitCode = process.exitCode;
  if (actualExitCode !== 0 || process.signalCode !== null || !process.pid)
    throw new Error(`observer Chrome exited with ${actualExitCode}`);
  assertExecutable(options.executablePath, options.executableSha256);
  const bytes = fs.readFileSync(options.tracePath);
  const traceSha256 = createHash("sha256").update(bytes).digest("hex");
  const trace = readChromiumFontTrace({
    tracePath: options.tracePath,
    traceSha256,
    sessionId: options.sessionId,
  });
  return { value, trace, processId: process.pid, actualExitCode, argv: process.spawnargs };
}
