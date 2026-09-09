import { expect, test } from "bun:test";
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

async function dependsUpdateRecipeLines(): Promise<string[]> {
  const justfile = await Bun.file(new URL("../../Justfile", import.meta.url)).text();
  const recipe = justfile.match(/^depends-update-all:\n([\s\S]*?)(?=^\S)/m)?.[1];
  if (recipe === undefined) {
    throw new Error("depends-update-all recipe is missing from Justfile");
  }
  return recipe
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"));
}

test("depends-update-all は既存の更新・品質・比較ステップを各1回保持する", async () => {
  const lines = await dependsUpdateRecipeLines();
  const requiredSteps = [
    "{{CARGO}} upgrade -i",
    "{{CARGO}} update",
    "bun update --latest",
    "bun run scripts/runtime-assets/depends-update-all.ts",
    'bun run scripts/drawio/resource-update.ts --resources "{{DRAWIO_RESOURCE_DIR}}" --manifest "{{DRAWIO_RESOURCE_MANIFEST}}"',
    "bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --write",
    "just fmt",
    "just runtime-bundle-build",
    "just check",
    "just coverage",
    "just mermaid-reference-all",
    "just mermaid-compare-full",
    "just mermaid-compare-ci",
    "just drawio-reference-all",
    "just drawio-compare-full",
    "just drawio-compare-ci",
  ];

  for (const step of requiredSteps) {
    expect(lines.filter((line) => line === step)).toHaveLength(1);
  }
  expect(lines.filter((line) => /^bun add -d typescript@[^\s]+$/.test(line))).toHaveLength(1);
});

test("depends-update-all は資産生成・整形・ビルド・品質検査・比較の順で実行する", async () => {
  const lines = await dependsUpdateRecipeLines();
  const stages = [
    [
      "bun run scripts/runtime-assets/depends-update-all.ts",
      'bun run scripts/drawio/resource-update.ts --resources "{{DRAWIO_RESOURCE_DIR}}" --manifest "{{DRAWIO_RESOURCE_MANIFEST}}"',
      "bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --write",
    ],
    ["just fmt"],
    ["just runtime-bundle-build"],
    ["just check"],
    ["just coverage"],
    [
      "just mermaid-reference-all",
      "just mermaid-compare-full",
      "just mermaid-compare-ci",
      "just drawio-reference-all",
      "just drawio-compare-full",
      "just drawio-compare-ci",
    ],
  ];
  let previousStageEnd = -1;
  for (const stage of stages) {
    const positions = stage.map((step) => lines.indexOf(step));
    expect(positions.every((position) => position > previousStageEnd)).toBe(true);
    previousStageEnd = Math.max(...positions);
  }
});
