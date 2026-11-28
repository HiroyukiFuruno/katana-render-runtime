import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { type Fixture, FixtureRepository } from "./diagram_update_fixtures";
import { spawnNativeCli } from "./native-cli-process";
import { renderNativeFixtureBatch } from "./native-render-batch";

function deferred(): { promise: Promise<void>; release: () => void } {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => {
    release = resolve;
  });
  return { promise, release };
}

function createFixtures(count: number): { root: string; fixtures: Fixture[] } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "native-render-batch-"));
  for (let index = 0; index < count; index += 1) {
    fs.writeFileSync(
      path.join(root, `fixture-${index}.md`),
      `# fixture-${index}\n\n\`\`\`mermaid\nflowchart LR\nA-->B\n\`\`\`\n`,
    );
  }
  return { root, fixtures: new FixtureRepository(root).list() };
}

async function runChild(root: string, fixture: Fixture, exitCode = 0) {
  const output = path.join(root, fixture.slug);
  fs.mkdirSync(output, { recursive: true });
  const temporary = path.join(output, "tmp");
  fs.mkdirSync(temporary);
  return spawnNativeCli(
    process.execPath,
    [
      process.execPath,
      "-e",
      `process.stdout.write(${JSON.stringify(fixture.slug)}); process.exit(${exitCode})`,
    ],
    path.join(output, "stdout.log"),
    path.join(output, "stderr.log"),
    temporary,
  );
}

test("native render batch preserves order and bounds real child concurrency at two", async () => {
  const { root, fixtures } = createFixtures(4);
  try {
    const firstRelease = deferred();
    const secondStarted = deferred();
    let active = 0;
    let maximum = 0;
    const batchPromise = renderNativeFixtureBatch(fixtures, 2, async (fixture) => {
      if (fixture.slug === "fixture-0") await firstRelease.promise;
      if (fixture.slug === "fixture-1") secondStarted.release();
      active += 1;
      maximum = Math.max(maximum, active);
      try {
        const child = await runChild(root, fixture);
        expect(child.exitStatus).toBe(0);
        return { row: null, renderError: null };
      } finally {
        active -= 1;
      }
    });
    await secondStarted.promise;
    firstRelease.release();
    const batch = await batchPromise;
    expect(active).toBe(0);
    expect(maximum).toBe(2);
    expect(batch.results.map(({ fixture }) => fixture.slug)).toEqual(
      fixtures.map((fixture) => fixture.slug),
    );
    expect(batch.errors).toEqual([]);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("native render batch stops queued fixtures after a render failure and reaps started children", async () => {
  const { root, fixtures } = createFixtures(4);
  try {
    const secondRelease = deferred();
    const secondStarted = deferred();
    const failed = deferred();
    const started: string[] = [];
    const batchPromise = renderNativeFixtureBatch(fixtures, 2, async (fixture) => {
      started.push(fixture.slug);
      if (fixture.slug === "fixture-0") {
        const child = await runChild(root, fixture, 13);
        expect(child.exitStatus).toBe(13);
        failed.release();
        return { row: null, renderError: "child exited with status 13" };
      }
      secondStarted.release();
      await secondRelease.promise;
      const child = await runChild(root, fixture);
      expect(child.exitStatus).toBe(0);
      return { row: null, renderError: null };
    });
    await secondStarted.promise;
    await failed.promise;
    secondRelease.release();
    const batch = await batchPromise;
    expect(started).toEqual(["fixture-0", "fixture-1"]);
    expect(batch.results).toHaveLength(2);
    expect(batch.errors).toEqual([]);
    expect(batch.results[0]?.result.renderError).toBe("child exited with status 13");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("native render batch retains filesystem exceptions and completed sibling results", async () => {
  const { root, fixtures } = createFixtures(3);
  try {
    const secondStarted = deferred();
    const secondRelease = deferred();
    const occupied = path.join(root, "occupied.log");
    fs.writeFileSync(occupied, "occupied");
    const started: string[] = [];
    const batchPromise = renderNativeFixtureBatch(fixtures, 2, async (fixture) => {
      started.push(fixture.slug);
      if (fixture.slug === "fixture-0") {
        await spawnNativeCli(
          process.execPath,
          [process.execPath, "-e", ""],
          occupied,
          path.join(root, "unused.stderr"),
        );
      }
      secondStarted.release();
      await secondRelease.promise;
      const child = await runChild(root, fixture);
      expect(child.exitStatus).toBe(0);
      return { row: null, renderError: null };
    });
    await secondStarted.promise;
    secondRelease.release();
    const batch = await batchPromise;
    expect(started).toEqual(["fixture-0", "fixture-1"]);
    expect(batch.errors).toHaveLength(1);
    expect(batch.errors[0]).toContain("fixture-0:");
    expect(batch.results[0]?.result.renderError).toContain("EEXIST");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
