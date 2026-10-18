import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const recipePath = "scripts/runtime-assets/mermaid-12.2.0-krr.pr6823.1.build.sh";
const patchPath = "scripts/runtime-assets/mermaid-12.2.0-krr.pr6823.1.patch";
const provenancePath = "scripts/runtime-assets/mermaid-12.2.0-krr.pr6823.1.provenance.json";

test("patched Mermaid build recipe pins tools, inputs, patch, and output", () => {
  const recipe = fs.readFileSync(recipePath, "utf8");
  const provenance = JSON.parse(fs.readFileSync(provenancePath, "utf8")) as {
    patch: { sha256: string };
    build: { output_sha256: string; output_bytes: number };
  };
  const patchSha = crypto.createHash("sha256").update(fs.readFileSync(patchPath)).digest("hex");

  expect(recipe).toContain('work_dir="$(cd "$work_dir" && pwd -P)"');
  expect(recipe).toContain('export GIT_CEILING_DIRECTORIES="$work_dir"');
  expect(recipe).toContain('git -C "$source" apply --check "$patch_path"');
  expect(recipe).toContain('node --version)" != "v26.10.0"');
  expect(recipe).toContain('node "$pnpm_cjs_path" --version');
  expect(recipe).toContain("KRR_PNPM_CJS_PATH:?set KRR_PNPM_CJS_PATH");
  expect(recipe).toContain("export HUSKY=0");
  expect(recipe).toContain('node "$pnpm_cjs_path" install --frozen-lockfile');
  expect(recipe).toContain('node "$pnpm_cjs_path" build:mermaid');
  expect(recipe).toContain("shasum -a 256 -c -");
  expect(recipe).toContain("unexpected Mermaid output byte count");
  expect(recipe).not.toContain("rm -rf");
  expect(provenance.patch.sha256).toBe(patchSha);
  expect(provenance.build.output_sha256).toBe(
    "831ed9d054742315d8edcb9bb933c6e56fd79d0c187a2cdf41ad48e388583a4c",
  );
  expect(provenance.build.output_bytes).toBe(5494857);
});

test("recipe patch check is isolated from the parent repository", () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "krr-mermaid-patch-check-"));
  const grammarPath = path.join(
    fixtureRoot,
    "packages/mermaid/src/diagrams/sankey/parser/sankey.jison",
  );
  fs.mkdirSync(path.dirname(grammarPath), { recursive: true });
  try {
    fs.copyFileSync("tests/fixtures/runtime-assets/mermaid-12.2.0-sankey.jison", grammarPath);
    const before = spawnSync("git", ["status", "--porcelain", "--", "scripts/runtime-assets"], {
      encoding: "utf8",
    });
    const check = spawnSync(
      "git",
      ["-C", fixtureRoot, "apply", "--check", path.resolve(patchPath)],
      {
        encoding: "utf8",
        env: { ...process.env, GIT_CEILING_DIRECTORIES: fixtureRoot, HUSKY: "0" },
      },
    );
    const after = spawnSync("git", ["status", "--porcelain", "--", "scripts/runtime-assets"], {
      encoding: "utf8",
    });
    expect(check.status).toBe(0);
    expect(check.stderr).toBe("");
    expect(after.stdout).toBe(before.stdout);
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
