import { expect, spyOn, test } from "bun:test";
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

async function withDefaultRunner(
  exit: number,
  check: (run: DependencyCommandRunner, launches: unknown[][]) => Promise<void>,
): Promise<void> {
  const launches: unknown[][] = [];
  const spawn = spyOn(Bun, "spawn").mockImplementation(
    new Proxy<typeof Bun.spawn>(
      () => {
        throw new Error("Non-executing unit mock target");
      },
      {
        apply(_target, _receiver, args) {
          launches.push(args);
          return { exited: Promise.resolve(exit) };
        },
      },
    ),
  );
  try {
    const command = new DependencyUpdateAllCommand([]);
    const runner: unknown = Reflect.get(command, "runner");
    if (typeof runner !== "function") throw new Error("Default runner is not a function");
    await check(async (command, args) => {
      const result: unknown = Reflect.apply(runner, undefined, [command, args]);
      if (!(result instanceof Promise)) throw new Error("Default runner is not asynchronous");
      await result;
    }, launches);
  } finally {
    spawn.mockRestore();
  }
}

test.each(
  [[], ["mermaid", "12.1.0"], ["日本語", "space quote '\"", "--data", ""]].map((suffix) => [
    suffix,
  ]),
)("default updater forwards suffix data unchanged: %j", async (suffix) => {
  await withDefaultRunner(0, async (run, launches) => {
    const args = ["run", "scripts/runtime-assets/update.ts", ...suffix];
    await run("bun", args);
    expect(launches).toEqual([[["bun", ...args], { stdout: "inherit", stderr: "inherit" }]]);
  });
});

test.each([
  ["node", ["run", "scripts/runtime-assets/update.ts"]],
  ["bun", ["eval", "scripts/runtime-assets/update.ts"]],
  ["bun", ["run", "scripts/hidden.ts"]],
  ["bun", []],
])("default updater rejects an unknown prefix: %s %j", async (command, args) => {
  await withDefaultRunner(0, async (run, launches) => {
    await expect(run(command, args)).rejects.toThrow(
      `Unsupported dependency update command: ${command} ${args.join(" ")}`,
    );
    expect(launches).toEqual([]);
  });
});

test("default updater preserves nonzero exit errors", async () => {
  await withDefaultRunner(7, async (run, launches) => {
    const args = ["run", "scripts/runtime-assets/update.ts", "mermaid", "12.1.0"];
    await expect(run("bun", args)).rejects.toThrow(`Command failed: bun ${args.join(" ")}`);
    expect(launches).toHaveLength(1);
  });
});

test("更新が必要なランタイム資産だけを最新バージョンへ更新する", async () => {
  const { source: _patchedSource, ...ordinaryMermaid } = RuntimeAssetCatalog.byKind("mermaid");
  const mermaid: RuntimeAssetDefinition = { ...ordinaryMermaid, version: "11.17.2" };
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

test("patched Mermaid は upstream latest と一致しても通常更新しない", async () => {
  const commands: string[][] = [];
  await new DependencyUpdateAllCommand(
    [RuntimeAssetCatalog.byKind("mermaid")],
    new LatestVersionStub(new Map([["mermaid", "12.2.0"]])),
    async (command, args) => {
      commands.push([command, ...args]);
    },
    () => undefined,
  ).run();
  expect(commands).toEqual([]);
});

test("patched Mermaid は upstream の新しい latest に通常更新しない", async () => {
  await expect(
    new DependencyUpdateAllCommand(
      [RuntimeAssetCatalog.byKind("mermaid")],
      new LatestVersionStub(new Map([["mermaid", "12.3.0"]])),
      async () => undefined,
      () => undefined,
    ).run(),
  ).rejects.toThrow("use explicit patched adoption");
});

test("一括更新はRustのmajorとpinned依存を含め、TypeScriptを旧版へ戻さない", () => {
  const justfile = fs.readFileSync("Justfile", "utf8");
  const recipeStart = justfile.indexOf("depends-update-all:");
  const recipe = justfile.slice(recipeStart, justfile.indexOf("\n\n", recipeStart));

  expect(recipe).toContain('python3 scripts/release/update_html5ever_pair.py --cargo "{{CARGO}}"');
  expect(recipe).toContain("{{CARGO}} upgrade -i allow --pinned allow --exclude skrifa");
  expect(recipe).toContain(
    'python3 scripts/release/update_usvg_skrifa_pair.py --cargo "{{CARGO}}"',
  );
  expect(recipe).toContain("bun update --latest");
  expect(recipe).not.toMatch(/bun add -d typescript@/);
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
    "{{CARGO}} upgrade -i allow --pinned allow --exclude skrifa",
    'python3 scripts/release/update_usvg_skrifa_pair.py --cargo "{{CARGO}}"',
    'python3 scripts/release/update_html5ever_pair.py --cargo "{{CARGO}}"',
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
  const rustUpdateStages = [
    "{{CARGO}} upgrade -i allow --pinned allow --exclude skrifa",
    'python3 scripts/release/update_html5ever_pair.py --cargo "{{CARGO}}"',
    "{{CARGO}} update",
  ];
  expect(rustUpdateStages.map((step) => lines.indexOf(step))).toEqual([0, 1, 2]);
  const pairUpdate = 'python3 scripts/release/update_usvg_skrifa_pair.py --cargo "{{CARGO}}"';
  expect(lines.indexOf(pairUpdate)).toBe(3);
  expect(lines.indexOf(pairUpdate)).toBeLessThan(lines.indexOf("bun update --latest"));
  expect(lines.some((line) => /^bun add -d typescript@/.test(line))).toBe(false);
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
