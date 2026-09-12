import { expect, test } from "bun:test";
import fs from "node:fs";
import { type DependencyCommandRunner, DependencyUpdateAllCommand } from "./depends-update-all";
import { RuntimeAssetCatalog, type RuntimeAssetDefinition } from "./runtime-asset-common";

class LatestVersionStub {
  constructor(private readonly versions: ReadonlyMap<string, string>) {}

  async latest(definition: RuntimeAssetDefinition): Promise<string> {
    const version = this.versions.get(definition.kind);
    if (version === undefined) {
      throw new Error(`Latest version is not configured: ${definition.kind}`);
    }
    return version;
  }
}

test("更新が必要なランタイム資産だけを最新バージョンへ更新する", async () => {
  const mermaid = RuntimeAssetCatalog.byKind("mermaid");
  const mathjax = RuntimeAssetCatalog.byKind("mathjax");
  const commands: string[][] = [];
  const runner: DependencyCommandRunner = async (command, args) => {
    commands.push([command, ...args]);
  };

  await new DependencyUpdateAllCommand(
    [mermaid, mathjax],
    new LatestVersionStub(
      new Map([
        ["mermaid", "11.18.0"],
        ["mathjax", mathjax.version],
      ]),
    ),
    runner,
    () => undefined,
  ).run();

  expect(commands).toEqual([
    ["bun", "run", "scripts/runtime-assets/update.ts", "mermaid", "11.18.0"],
  ]);
});

test("一括更新はRustのmajorとpinned依存を含め、TypeScriptを旧版へ戻さない", () => {
  const justfile = fs.readFileSync("Justfile", "utf8");
  const recipeStart = justfile.indexOf("depends-update-all:");
  const recipe = justfile.slice(recipeStart, justfile.indexOf("\n\n", recipeStart));

  expect(recipe).toContain("{{CARGO}} upgrade --incompatible allow --pinned allow --recursive true");
  expect(recipe).toContain("bun update --latest");
  expect(recipe).not.toMatch(/bun add -d typescript@/);
});
