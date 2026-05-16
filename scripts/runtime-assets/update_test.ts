import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DrawioWarExtractor } from "./drawio-war-extractor";
import { RuntimeAssetChecksum } from "./runtime-asset-common";
import { RuntimeSourceUpdater } from "./update";

test("Draw.io WAR から 1MiB を超える viewer.min.js を展開できる", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-drawio-war-"));
  try {
    const jsDir = path.join(root, "js");
    fs.mkdirSync(jsDir, { recursive: true });
    const sourceFile = path.join(jsDir, "viewer.min.js");
    fs.writeFileSync(sourceFile, `const app = "${"x".repeat(1_200_000)}";\n`, "utf8");

    const archive = path.join(root, "draw.war");
    const zipped = spawnSync("zip", ["-q", archive, "js/viewer.min.js"], {
      cwd: root,
      encoding: "utf8",
    });
    if (zipped.status !== 0) {
      throw new Error(`zip failed: ${zipped.stderr}`);
    }

    const target = path.join(root, "drawio.min.js");
    new DrawioWarExtractor().extract(archive, target);

    expect(fs.statSync(target).size).toBeGreaterThan(1024 * 1024);
    expect(RuntimeAssetChecksum.digestFile(target)).toBe(
      RuntimeAssetChecksum.digestFile(sourceFile),
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("Rust runtime asset version const を 1 行形式でも更新できる", () => {
  const source = 'pub const DRAWIO_JS_VERSION: &str = "29.7.10";\n';

  const updated = new RuntimeSourceUpdater().replaceConst(source, "DRAWIO_JS_VERSION", "30.0.1");

  expect(updated).toBe('pub const DRAWIO_JS_VERSION: &str = "30.0.1";\n');
});
