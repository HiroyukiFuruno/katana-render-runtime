import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileSha256 } from "./mermaid-pair-manifest";

export interface ManagedToolPin {
  kind: "bun" | "python3";
  path: string;
  sha256: string;
}

interface ManagedCommandRoute {
  kind: ManagedToolPin["kind"];
  script: string;
}

export interface ManagedCommandResult {
  label: string;
  argv: string[];
  cwd: string;
  stdoutPath: string;
  stderrPath: string;
  exitPath: string;
  exitCode: number | null;
  signal: NodeJS.Signals | null;
  spawnError: string | null;
}

export function runManagedCommand(
  outputRoot: string,
  label: string,
  argv: string[],
  cwd: string,
  tool: ManagedToolPin,
): ManagedCommandResult {
  const route = validateManagedCommand(argv, tool);
  const pathSnapshot = process.env.PATH ?? "";
  const resolvedTool = resolveManagedTool(tool, pathSnapshot);
  const childEnvironment = { ...process.env, PATH: pathSnapshot };
  const normalizedArgv = [resolvedTool.name, ...argv.slice(1)];
  const logRoot = path.join(outputRoot, "logs");
  fs.mkdirSync(logRoot, { recursive: true, mode: 0o700 });
  const stdoutPath = path.join(logRoot, `${label}.stdout.raw`);
  const stderrPath = path.join(logRoot, `${label}.stderr.raw`);
  const exitPath = path.join(logRoot, `${label}.exit.json`);
  const commandPath = path.join(logRoot, `${label}.command.json`);
  writeExclusive(commandPath, {
    argv: normalizedArgv,
    cwd,
    executable: resolvedTool.name,
    resolvedExecutablePath: resolvedTool.path,
    resolvedExecutableSha256: tool.sha256,
  });
  const stdoutFd = fs.openSync(stdoutPath, "wx", 0o600);
  let stderrFd: number | null = null;
  let exitCode: number | null = null;
  let signal: NodeJS.Signals | null = null;
  let spawnError: string | null = null;
  try {
    stderrFd = fs.openSync(stderrPath, "wx", 0o600);
    const child = spawnManagedRoute(route, argv, cwd, childEnvironment, stdoutFd, stderrFd);
    exitCode = child.status;
    signal = child.signal;
    spawnError = child.error?.message ?? null;
    if (!managedToolStillMatches(tool, resolvedTool.path)) {
      spawnError = "Managed command executable changed during execution";
    }
  } catch (error) {
    spawnError = error instanceof Error ? error.message : String(error);
  } finally {
    if (stderrFd !== null) fs.closeSync(stderrFd);
    fs.closeSync(stdoutFd);
  }
  const result: ManagedCommandResult = {
    label,
    argv: normalizedArgv,
    cwd,
    stdoutPath,
    stderrPath,
    exitPath,
    exitCode,
    signal,
    spawnError,
  };
  writeExclusive(exitPath, {
    label,
    actualExitCode: exitCode,
    signal,
    spawnError,
    stdoutPath,
    stderrPath,
  });
  return result;
}

function validateManagedCommand(argv: string[], tool: ManagedToolPin): ManagedCommandRoute {
  if (
    argv.length === 0 ||
    argv.some((argument) => argument.includes("\0")) ||
    argv[0] !== tool.path ||
    !path.isAbsolute(tool.path) ||
    !/^[a-f0-9]{64}$/.test(tool.sha256)
  ) {
    throw new Error("Managed full-run command argv or tool pin is invalid");
  }
  const script = tool.kind === "bun" ? argv[2] : argv[1];
  const route = managedCommandRoute(script ?? "");
  const expectedHeader = tool.kind === "bun" ? ["run", route.script] : [route.script];
  if (
    route.kind !== tool.kind ||
    JSON.stringify(argv.slice(1, 1 + expectedHeader.length)) !== JSON.stringify(expectedHeader)
  ) {
    throw new Error("Managed command does not match its fixed Bun/Python script route");
  }
  return route;
}

function managedCommandRoute(script: string): ManagedCommandRoute {
  switch (script) {
    case "scripts/mermaid/produce_native_cli_build_receipt.py":
    case "scripts/mermaid/produce_raster_process_evidence.py":
      return { kind: "python3", script };
    case "scripts/mermaid/raster-production-session.ts":
    case "scripts/mermaid/native-render-provenance.ts":
    case "scripts/mermaid/raster-comparison-binding.ts":
    case "scripts/mermaid/reference-compare.ts":
      return { kind: "bun", script };
    default:
      throw new Error("Managed command script is not in the fixed producer route set");
  }
}

function resolveManagedTool(
  tool: ManagedToolPin,
  pathSnapshot: string,
): { name: ManagedToolPin["kind"]; path: string } {
  const resolved = Bun.which(tool.kind, { PATH: pathSnapshot });
  if (!resolved || !path.isAbsolute(resolved)) {
    throw new Error("Pinned managed command tool is not available through PATH");
  }
  let canonical: string;
  try {
    canonical = fs.realpathSync(resolved);
    const stat = fs.lstatSync(canonical);
    if (
      !stat.isFile() ||
      (stat.mode & 0o111) === 0 ||
      canonical !== tool.path ||
      fileSha256(canonical) !== tool.sha256
    ) {
      throw new Error("resolved tool does not match the request pin");
    }
  } catch {
    throw new Error("Pinned managed command tool does not match its PATH resolution");
  }
  return { name: tool.kind, path: canonical };
}

function managedToolStillMatches(tool: ManagedToolPin, resolvedPath: string): boolean {
  try {
    return fs.realpathSync(resolvedPath) === tool.path && fileSha256(resolvedPath) === tool.sha256;
  } catch {
    return false;
  }
}

function spawnManagedRoute(
  route: ManagedCommandRoute,
  argv: string[],
  cwd: string,
  env: NodeJS.ProcessEnv,
  stdoutFd: number,
  stderrFd: number,
) {
  const stdio: ["ignore", number, number] = ["ignore", stdoutFd, stderrFd];
  if (route.kind === "python3") {
    return route.script === "scripts/mermaid/produce_native_cli_build_receipt.py"
      ? spawnSync(
          "python3",
          ["scripts/mermaid/produce_native_cli_build_receipt.py", ...argv.slice(2)],
          { cwd, env, stdio },
        )
      : spawnSync(
          "python3",
          ["scripts/mermaid/produce_raster_process_evidence.py", ...argv.slice(2)],
          { cwd, env, stdio },
        );
  }
  switch (route.script) {
    case "scripts/mermaid/raster-production-session.ts":
      return spawnSync(
        "bun",
        ["run", "scripts/mermaid/raster-production-session.ts", ...argv.slice(3)],
        { cwd, env, stdio },
      );
    case "scripts/mermaid/native-render-provenance.ts":
      return spawnSync(
        "bun",
        ["run", "scripts/mermaid/native-render-provenance.ts", ...argv.slice(3)],
        { cwd, env, stdio },
      );
    case "scripts/mermaid/raster-comparison-binding.ts":
      return spawnSync(
        "bun",
        ["run", "scripts/mermaid/raster-comparison-binding.ts", ...argv.slice(3)],
        { cwd, env, stdio },
      );
    case "scripts/mermaid/reference-compare.ts":
      return spawnSync("bun", ["run", "scripts/mermaid/reference-compare.ts", ...argv.slice(3)], {
        cwd,
        env,
        stdio,
      });
    default:
      throw new Error("Managed command script route was not validated");
  }
}

export function requireCommandSuccess(result: ManagedCommandResult): void {
  if (result.spawnError !== null)
    throw new Error(`${result.label} failed to spawn: ${result.spawnError}`);
  if (result.exitCode !== 0 || result.signal !== null) {
    throw new Error(
      `${result.label} exited unsuccessfully: ${String(result.exitCode)} ${String(result.signal)}`,
    );
  }
}

export function requireScoreProcessOutcome(result: ManagedCommandResult, accepted: boolean): void {
  if (
    result.spawnError !== null ||
    result.signal !== null ||
    ![0, 1].includes(result.exitCode ?? -1)
  ) {
    requireCommandSuccess(result);
  }
  const expectedExitCode = accepted ? 0 : 1;
  if (result.exitCode !== expectedExitCode) {
    throw new Error(`${result.label} exit does not match its complete score evidence`);
  }
}

export function writeExclusive(filePath: string, value: unknown): void {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, {
    flag: "wx",
    mode: 0o600,
  });
}

export function parseChildJson<T>(result: ManagedCommandResult, label: string): T {
  try {
    return JSON.parse(fs.readFileSync(result.stdoutPath, "utf8")) as T;
  } catch {
    throw new Error(`${label} stdout did not contain its required JSON response`);
  }
}
