import type { RuntimeBundleDefinition } from "#shared/runtime_bundle";

export type RuntimeBundleMode = "write" | "check";

export interface GeneratedBundle {
  readonly definition: RuntimeBundleDefinition;
  readonly outputPath: string;
  readonly content: string;
  readonly checksum: string;
}
