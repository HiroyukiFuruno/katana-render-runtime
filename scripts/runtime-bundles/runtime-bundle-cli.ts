import type { RuntimeBundleMode } from "./runtime-bundle-types";

export interface RuntimeBundleOptions {
  readonly mode: RuntimeBundleMode;
  readonly classicGenerationEvidence: boolean;
}

export function parseRuntimeBundleMode(argv: string[]): RuntimeBundleMode {
  return parseRuntimeBundleOptions(argv).mode;
}

export function parseRuntimeBundleOptions(argv: string[]): RuntimeBundleOptions {
  const modes = argv.filter((argument) => argument === "--write" || argument === "--check");
  const evidenceCount = argv.filter(
    (argument) => argument === "--classic-generation-evidence",
  ).length;
  if (modes.length === 0)
    throw new Error("Usage: bun run scripts/runtime-bundles/bundle-runtime.ts --write|--check");
  if (modes.length !== 1) throw new Error("Specify exactly one runtime bundle mode");
  if (modes.length + evidenceCount !== argv.length)
    throw new Error("Unknown runtime bundle argument");
  if (evidenceCount > 1) throw new Error("Duplicate classic generation evidence flag");
  const mode: RuntimeBundleMode = argv.includes("--write") ? "write" : "check";
  if (evidenceCount === 1 && mode !== "check")
    throw new Error("--classic-generation-evidence requires --check");
  return { mode, classicGenerationEvidence: evidenceCount === 1 };
}
