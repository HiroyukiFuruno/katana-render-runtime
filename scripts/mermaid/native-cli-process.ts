import { spawn } from "node:child_process";
import fs from "node:fs";

export async function spawnNativeCli(
  binary: string,
  argv: string[],
  stdoutPath: string,
  stderrPath: string,
  temporaryDirectory?: string,
): Promise<{
  pid: number | null;
  exitStatus: number | null;
  signal: string | null;
  spawnError: string | null;
}> {
  let stdoutFd: number | undefined;
  let stderrFd: number | undefined;
  let primaryError: unknown;
  let pid: number | null = null;
  let exitStatus: number | null = null;
  let signal: string | null = null;
  let spawnError: string | null = null;
  try {
    stdoutFd = fs.openSync(stdoutPath, "wx");
    stderrFd = fs.openSync(stderrPath, "wx");
    const childEnvironment =
      temporaryDirectory === undefined ? undefined : nativeCliEnvironment(temporaryDirectory);
    const child = spawn(binary, argv.slice(1), {
      cwd: process.cwd(),
      stdio: ["ignore", stdoutFd, stderrFd],
      ...(childEnvironment === undefined ? {} : { env: childEnvironment }),
    });
    pid = child.pid ?? null;
    const completion = await new Promise<{
      code: number | null;
      signal: NodeJS.Signals | null;
      error: Error | null;
    }>((resolve) => {
      let childError: Error | null = null;
      child.once("error", (error) => {
        childError = error;
      });
      child.once("close", (code, childSignal) =>
        resolve({ code, signal: childSignal, error: childError }),
      );
    });
    exitStatus = completion.code;
    signal = completion.signal;
    primaryError = completion.error;
    spawnError = completion.error?.message ?? null;
  } catch (error) {
    primaryError = error;
    // 取得途中の失敗は、従来どおり呼出し元へ原因を返す。
    if (stdoutFd === undefined || stderrFd === undefined) throw error;
    spawnError = error instanceof Error ? error.message : String(error);
  } finally {
    closeNativeStreams([stdoutFd, stderrFd], primaryError);
  }
  return { pid, exitStatus, signal, spawnError };
}

function nativeCliEnvironment(temporaryDirectory: string): NodeJS.ProcessEnv {
  const environment = { ...process.env };
  if (process.platform === "win32") {
    environment.TMP = temporaryDirectory;
    environment.TEMP = temporaryDirectory;
  } else {
    environment.TMPDIR = temporaryDirectory;
  }
  return environment;
}

function closeNativeStreams(descriptors: Array<number | undefined>, primaryError: unknown): void {
  const closeErrors: unknown[] = [];
  for (const descriptor of descriptors) {
    if (descriptor === undefined) continue;
    try {
      fs.closeSync(descriptor);
    } catch (error) {
      closeErrors.push(error);
    }
  }
  if (closeErrors.length === 0) return;
  const errors =
    primaryError === undefined || primaryError === null
      ? closeErrors
      : [primaryError, ...closeErrors];
  // 一方のclose失敗で他方の解放や先行した処理の原因を失わない。
  throw new AggregateError(
    errors,
    `Native CLI stream cleanup failed: ${errors.map((error) => (error instanceof Error ? error.message : String(error))).join("; ")}`,
    { cause: errors[0] },
  );
}
