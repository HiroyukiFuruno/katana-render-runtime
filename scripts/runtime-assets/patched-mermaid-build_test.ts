import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const validSha = "a".repeat(64);
type RecipeArguments = [string, string] | [string, string, string, string];
const cases: Array<{ label: string; args: RecipeArguments; message: string }> = [
  {
    label: "old candidate",
    args: ["12.2.0", "candidate"],
    message: "12.2.0 uses its fixed verification pins",
  },
  {
    label: "old output override",
    args: ["12.2.0", "verify", validSha, "123"],
    message: "12.2.0 uses its fixed verification pins",
  },
  {
    label: "unknown version",
    args: ["99.0.0", "candidate"],
    message: "unsupported Mermaid version",
  },
  { label: "unknown mode", args: ["12.3.0", "other"], message: "unknown 12.3.0 build mode" },
  {
    label: "missing pins",
    args: ["12.3.0", "verify"],
    message: "verify requires an independently pinned",
  },
  {
    label: "invalid SHA",
    args: ["12.3.0", "verify", "not-a-sha", "123"],
    message: "verify requires an independently pinned",
  },
  {
    label: "zero bytes",
    args: ["12.3.0", "verify", validSha, "0"],
    message: "verify requires an independently pinned",
  },
  {
    label: "invalid bytes",
    args: ["12.3.0", "verify", validSha, "1x"],
    message: "verify requires an independently pinned",
  },
];

function assertRejectedBeforeWork(args: RecipeArguments, message: string): void {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-patched-mermaid-args-"));
  try {
    const tools = path.join(root, "tools");
    const work = path.join(root, "must-not-exist");
    const trap = path.join(root, "invoked");
    fs.mkdirSync(tools);
    for (const name of ["curl", "mkdir", "node", "shasum", "tar"]) {
      fs.writeFileSync(
        path.join(tools, name),
        '#!/bin/sh\nprintf "%s\\n" blocked >> "$KRR_RECIPE_TEST_TRAP"\nexit 97\n',
        { mode: 0o755 },
      );
    }
    const options = {
      cwd: process.cwd(),
      encoding: "utf8" as const,
      env: {
        PATH: `${tools}:/usr/bin:/bin`,
        KRR_PNPM_CJS_PATH: path.join(root, "not-installed"),
        KRR_RECIPE_TEST_TRAP: trap,
      },
    };
    // 実行するスクリプトを静的解析で追跡できるよう、引数の数ごとに呼び出す。
    const result =
      args.length === 2
        ? spawnSync(
            "bash",
            ["scripts/runtime-assets/mermaid-12.2.0-krr.pr6823.1.build.sh", work, args[0], args[1]],
            options,
          )
        : spawnSync(
            "bash",
            [
              "scripts/runtime-assets/mermaid-12.2.0-krr.pr6823.1.build.sh",
              work,
              args[0],
              args[1],
              args[2],
              args[3],
            ],
            options,
          );
    expect(result.status).toBe(2);
    expect(result.stderr).toContain(message);
    expect(fs.existsSync(work)).toBe(false);
    expect(fs.existsSync(trap)).toBe(false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test.each(cases)("patched Mermaid recipe rejects $label before work", ({ args, message }) => {
  assertRejectedBeforeWork(args, message);
});
