import type { RuntimeBundleMode } from "./runtime-bundle-types";

export function parseRuntimeBundleMode(argv: string[]): RuntimeBundleMode {
  if (argv.includes("--write")) {
    return "write";
  }
  if (argv.includes("--check")) {
    return "check";
  }
  throw new Error("Usage: bun run scripts/runtime-bundles/bundle-runtime.ts --write|--check");
}
