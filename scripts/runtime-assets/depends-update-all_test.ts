import { expect, spyOn, test } from "bun:test";
import {
  type DependencyCommandRunner,
  DependencyUpdateAllCommand,
  runtimeAssetNeedsRepair,
} from "./depends-update-all";
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

test("Draw.io の同一version更新はvendor LICENSE欠落を修復する", async () => {
  const drawio = RuntimeAssetCatalog.byKind("drawio");
  expect(runtimeAssetNeedsRepair(drawio, () => false)).toBe(true);
  expect(runtimeAssetNeedsRepair(drawio, () => true)).toBe(false);

  const commands: string[][] = [];
  await new DependencyUpdateAllCommand(
    [drawio],
    new LatestVersionStub(new Map([["drawio", drawio.version]])),
    async (command, args) => {
      commands.push([command, ...args]);
    },
    () => undefined,
    () => false,
  ).run();

  expect(commands).toEqual([
    ["bun", "run", "scripts/runtime-assets/update.ts", "drawio", drawio.version],
  ]);

  const existingCommands: string[][] = [];
  await new DependencyUpdateAllCommand(
    [drawio],
    new LatestVersionStub(new Map([["drawio", drawio.version]])),
    async (command, args) => {
      existingCommands.push([command, ...args]);
    },
    () => undefined,
    () => true,
  ).run();
  expect(existingCommands).toEqual([]);
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

function justRecipeLines(justfile: string, recipeName: string): string[] {
  const lines = justfile.split("\n");
  const headerIndex = lines.findIndex((line) => line.startsWith(`${recipeName}:`));
  if (headerIndex < 0) {
    throw new Error(`${recipeName} recipe is missing from Justfile`);
  }
  const recipeLines: string[] = [];
  for (const line of lines.slice(headerIndex + 1)) {
    if (line.trim().length === 0 || line.trimStart().startsWith("#")) continue;
    if (!/^\s/.test(line)) break;
    recipeLines.push(line.trim());
  }
  return recipeLines;
}

async function dependsUpdateCommandGroups(): Promise<{
  mutationLines: string[];
  recipeLines: string[];
}> {
  const justfile = await Bun.file(new URL("../../Justfile", import.meta.url)).text();
  return {
    mutationLines: justRecipeLines(justfile, "depends-update-mutations"),
    recipeLines: justRecipeLines(justfile, "depends-update-all"),
  };
}

const mutationLockCommand =
  "python3 scripts/hooks/run_locked_command.py --protect-root-producer -- just depends-update-mutations";
const dependencyMutationSteps = [
  "{{CARGO}} upgrade -i allow --pinned allow --exclude skrifa",
  'python3 scripts/release/update_html5ever_pair.py --cargo "{{CARGO}}"',
  "{{CARGO}} update",
  'python3 scripts/release/update_html_performance_harness_lock.py --cargo "{{CARGO}}"',
  "bun update --latest '!@moonrepo/*'",
  "bun run scripts/runtime-assets/depends-update-all.ts",
  'bun run scripts/drawio/resource-update.ts --resources "{{DRAWIO_RESOURCE_DIR}}" --manifest "{{DRAWIO_RESOURCE_MANIFEST}}"',
  "bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --write",
  "just fmt",
  "just runtime-bundle-build",
];
const dependencyValidationSteps = [
  "just check",
  "just mermaid-reference-all",
  "just mermaid-compare-full",
  "just mermaid-compare-ci",
  "just drawio-reference-all",
  "just drawio-compare-full",
  "just drawio-compare-ci",
];

test("一括更新はRustのmajorとpinned依存を含め、TypeScriptを旧版へ戻さない", async () => {
  const { mutationLines } = await dependsUpdateCommandGroups();
  expect(mutationLines).toContain(
    'python3 scripts/release/update_html5ever_pair.py --cargo "{{CARGO}}"',
  );
  expect(mutationLines).toContain("{{CARGO}} upgrade -i allow --pinned allow --exclude skrifa");
  expect(mutationLines).toContain("bun update --latest '!@moonrepo/*'");
  expect(mutationLines).not.toContain("bun add -d typescript@");
});

test("depends-update-all は更新/buildをlock内で行い、解放後に全検証を行う", async () => {
  const { mutationLines, recipeLines } = await dependsUpdateCommandGroups();
  expect(mutationLines).toEqual(dependencyMutationSteps);
  expect(recipeLines).toEqual([mutationLockCommand, ...dependencyValidationSteps]);
});

test("depends-update-all は資産生成・整形・ビルド・品質検査・比較をこの順で保持する", async () => {
  const { mutationLines, recipeLines } = await dependsUpdateCommandGroups();
  const orderedSteps = [...mutationLines, ...recipeLines.slice(1)];
  const stages = [
    [
      "bun run scripts/runtime-assets/depends-update-all.ts",
      'bun run scripts/drawio/resource-update.ts --resources "{{DRAWIO_RESOURCE_DIR}}" --manifest "{{DRAWIO_RESOURCE_MANIFEST}}"',
      "bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --write",
    ],
    ["just fmt"],
    ["just runtime-bundle-build"],
    ["just check"],
    dependencyValidationSteps.slice(1),
  ];
  let previousStageEnd = -1;
  for (const stage of stages) {
    const positions = stage.map((step) => orderedSteps.indexOf(step));
    expect(positions.every((position) => position > previousStageEnd)).toBe(true);
    previousStageEnd = Math.max(...positions);
  }
  expect(orderedSteps).not.toContain("just coverage");
});
