import fs from "node:fs";
import path from "node:path";
import { LatestVersionClient } from "./latest-check";
import {
  isPatchedRuntimeAsset,
  RuntimeAssetCatalog,
  type RuntimeAssetDefinition,
  RuntimeAssetPaths,
  runtimeAssetUpstreamVersion,
} from "./runtime-asset-common";

export type DependencyCommandRunner = (command: string, args: readonly string[]) => Promise<void>;

interface RuntimeAssetLatestClient {
  latest(definition: RuntimeAssetDefinition): Promise<string>;
}

export function runtimeAssetNeedsRepair(
  definition: RuntimeAssetDefinition,
  exists: (path: string) => boolean = fs.existsSync,
): boolean {
  if (definition.kind !== "drawio") return false;
  return !exists(path.join(RuntimeAssetPaths.vendorDir(definition), "LICENSE"));
}

class BunCommandRunner {
  async run(command: string, args: readonly string[]): Promise<void> {
    if (command !== "bun" || args[0] !== "run" || args[1] !== "scripts/runtime-assets/update.ts") {
      throw new Error(`Unsupported dependency update command: ${command} ${args.join(" ")}`);
    }
    const process = Bun.spawn(
      ["bun", "run", "scripts/runtime-assets/update.ts", ...args.slice(2)],
      {
        stdout: "inherit",
        stderr: "inherit",
      },
    );
    if ((await process.exited) !== 0) {
      throw new Error(`Command failed: ${command} ${args.join(" ")}`);
    }
  }
}

export class DependencyUpdateAllCommand {
  constructor(
    private readonly definitions: readonly RuntimeAssetDefinition[] = RuntimeAssetCatalog.all(),
    private readonly client: RuntimeAssetLatestClient = new LatestVersionClient(),
    private readonly runner: DependencyCommandRunner = (command, args) =>
      new BunCommandRunner().run(command, args),
    private readonly report: (message: string) => void = console.log,
    private readonly fileExists: (path: string) => boolean = fs.existsSync,
  ) {}

  async run(): Promise<void> {
    for (const definition of this.definitions) {
      await this.updateRuntimeAsset(definition);
    }
  }

  private async updateRuntimeAsset(definition: RuntimeAssetDefinition): Promise<void> {
    const latest = await this.client.latest(definition);
    if (isPatchedRuntimeAsset(definition)) {
      const upstreamVersion = runtimeAssetUpstreamVersion(definition);
      if (latest === upstreamVersion) {
        this.report(
          `${definition.displayName}: patched ${definition.version} pinned to upstream ${upstreamVersion}`,
        );
        return;
      }
      throw new Error(
        `${definition.displayName}: patched asset ${definition.version} cannot be replaced by ordinary update ${latest}; use explicit patched adoption`,
      );
    }
    if (latest === definition.version && !runtimeAssetNeedsRepair(definition, this.fileExists)) {
      this.report(`${definition.displayName}: already latest (${latest})`);
      return;
    }
    const action =
      latest === definition.version
        ? "repairing missing files"
        : `${definition.version} -> ${latest}`;
    this.report(`${definition.displayName}: ${action}`);
    await this.runner("bun", ["run", "scripts/runtime-assets/update.ts", definition.kind, latest]);
  }
}

if (import.meta.main) {
  await new DependencyUpdateAllCommand().run();
}
