import { createRequire } from "node:module";
import path from "node:path";

export function collectToolFingerprintResolution(): Record<string, string | null> {
  const root = process.cwd();
  const require = createRequire(
    path.join(root, "scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts"),
  );
  const utils = require.resolve("@moonrepo/cli/utils");
  const cli = require.resolve("@moonrepo/cli/package.json");
  const module = require("@moonrepo/cli/utils") as { findMoonExe?: unknown };
  if (typeof module.findMoonExe !== "function") throw new Error("Moon resolver is unavailable");
  const moon: unknown = module.findMoonExe();
  if (typeof moon !== "string") throw new Error("Moon executable path is invalid");
  const zod = Bun.resolveSync("zod", path.join(root, "scripts/runtime-bundles"));
  const libc = process.platform === "linux" ? require.resolve("detect-libc") : null;
  return {
    zod,
    utils,
    cli,
    moon,
    libc,
    bun: process.execPath,
    platform: process.platform,
    arch: process.arch,
  };
}

if (import.meta.main) process.stdout.write(JSON.stringify(collectToolFingerprintResolution()));
