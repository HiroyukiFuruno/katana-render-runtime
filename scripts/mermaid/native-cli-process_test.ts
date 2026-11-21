import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnNativeCli } from "./native-cli-process";

function streamFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-native-stream-fd-"));
  return {
    root,
    stdout: path.join(root, "stdout.log"),
    stderr: path.join(root, "stderr.log"),
    missingBinary: path.join(root, "missing-binary"),
  };
}

function streamDescriptors(filePath: string): number[] {
  const expected = fs.statSync(filePath);
  const descriptors: number[] = [];
  for (let fd = 0; fd < 1024; fd += 1) {
    try {
      const actual = fs.fstatSync(fd);
      if (actual.dev === expected.dev && actual.ino === expected.ino) descriptors.push(fd);
    } catch (error) {
      if (!(error instanceof Error) || !("code" in error) || error.code !== "EBADF") throw error;
    }
  }
  return descriptors;
}

function onlyStreamDescriptor(filePath: string): number {
  const descriptors = streamDescriptors(filePath);
  expect(descriptors).toHaveLength(1);
  const descriptor = descriptors[0];
  if (descriptor === undefined) throw new Error("Expected an owned stream descriptor");
  return descriptor;
}

test("native CLI closes stdout when exclusive stderr acquisition fails", async () => {
  const fixture = streamFixture();
  fs.writeFileSync(fixture.stderr, "existing stderr");
  try {
    await expect(
      spawnNativeCli(
        fixture.missingBinary,
        [fixture.missingBinary],
        fixture.stdout,
        fixture.stderr,
      ),
    ).rejects.toThrow("EEXIST");
    const remaining = streamDescriptors(fixture.stdout);
    // 失敗した旧実装でも、回帰検査自身は取得されたFDを残さない。
    for (const fd of remaining) fs.closeSync(fd);
    expect(remaining).toEqual([]);
    expect(fs.readFileSync(fixture.stderr, "utf8")).toBe("existing stderr");
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native CLI retains a real spawn error and closes both streams", async () => {
  const fixture = streamFixture();
  try {
    const result = await spawnNativeCli(
      fixture.missingBinary,
      [fixture.missingBinary],
      fixture.stdout,
      fixture.stderr,
    );
    expect(result.spawnError).toContain("ENOENT");
    expect(streamDescriptors(fixture.stdout)).toEqual([]);
    expect(streamDescriptors(fixture.stderr)).toEqual([]);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native CLI preserves the real child completion and stream output", async () => {
  const fixture = streamFixture();
  try {
    const result = await spawnNativeCli(
      process.execPath,
      [process.execPath, "--version"],
      fixture.stdout,
      fixture.stderr,
    );
    expect(result.pid).toBeGreaterThan(0);
    expect(result.exitStatus).toBe(0);
    expect(result.signal).toBeNull();
    expect(result.spawnError).toBeNull();
    expect(fs.readFileSync(fixture.stdout, "utf8").trim()).toMatch(/^\d+\.\d+\.\d+/);
    expect(fs.readFileSync(fixture.stderr, "utf8")).toBe("");
    expect(streamDescriptors(fixture.stdout)).toEqual([]);
    expect(streamDescriptors(fixture.stderr)).toEqual([]);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native CLI uses two isolated temporary directories without changing the parent environment", async () => {
  const fixture = streamFixture();
  const tempRoots = [
    fs.realpathSync(fs.mkdtempSync(path.join(fixture.root, "first-temp-"))),
    fs.realpathSync(fs.mkdtempSync(path.join(fixture.root, "second-temp-"))),
  ];
  const parentTempEnvironment = {
    TMPDIR: process.env.TMPDIR,
    TMP: process.env.TMP,
    TEMP: process.env.TEMP,
    PATH: process.env.PATH,
  };
  const childProgram = [
    'const os = require("node:os");',
    "process.stdout.write(JSON.stringify({",
    "tempDir: os.tmpdir(),",
    "TMPDIR: process.env.TMPDIR ?? null,",
    "TMP: process.env.TMP ?? null,",
    "TEMP: process.env.TEMP ?? null,",
    "PATH: process.env.PATH ?? null,",
    "}));",
  ].join("\n");
  try {
    for (const [index, tempRoot] of tempRoots.entries()) {
      const stdoutPath = path.join(fixture.root, `override-${index}.stdout.log`);
      const stderrPath = path.join(fixture.root, `override-${index}.stderr.log`);
      const result = await spawnNativeCli(
        process.execPath,
        [process.execPath, "-e", childProgram],
        stdoutPath,
        stderrPath,
        tempRoot,
      );
      expect(result.pid).toBeGreaterThan(0);
      expect(result.exitStatus).toBe(0);
      expect(result.signal).toBeNull();
      expect(result.spawnError).toBeNull();
      expect(fs.readFileSync(stderrPath, "utf8")).toBe("");
      expect(streamDescriptors(stdoutPath)).toEqual([]);
      expect(streamDescriptors(stderrPath)).toEqual([]);
      const childEnvironment = JSON.parse(fs.readFileSync(stdoutPath, "utf8")) as {
        tempDir: string;
        TMPDIR: string | null;
        TMP: string | null;
        TEMP: string | null;
        PATH: string | null;
      };
      expect(childEnvironment.tempDir).toBe(tempRoot);
      expect(childEnvironment.PATH).toBe(parentTempEnvironment.PATH ?? null);
      if (process.platform === "win32") {
        expect(childEnvironment.TMP).toBe(tempRoot);
        expect(childEnvironment.TEMP).toBe(tempRoot);
        expect(childEnvironment.TMPDIR).toBe(parentTempEnvironment.TMPDIR ?? null);
      } else {
        expect(childEnvironment.TMPDIR).toBe(tempRoot);
        expect(childEnvironment.TMP).toBe(parentTempEnvironment.TMP ?? null);
        expect(childEnvironment.TEMP).toBe(parentTempEnvironment.TEMP ?? null);
      }
    }
    expect(tempRoots[0]).not.toBe(tempRoots[1]);
    expect({
      TMPDIR: process.env.TMPDIR,
      TMP: process.env.TMP,
      TEMP: process.env.TEMP,
      PATH: process.env.PATH,
    }).toEqual(parentTempEnvironment);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

for (const missingBinary of [false, true]) {
  test(`native CLI closes stderr after a real stdout close error with spawn failure ${missingBinary}`, async () => {
    const fixture = streamFixture();
    const binary = missingBinary ? fixture.missingBinary : process.execPath;
    try {
      const completion = spawnNativeCli(
        binary,
        [binary, "--version"],
        fixture.stdout,
        fixture.stderr,
      );
      const stdoutFd = onlyStreamDescriptor(fixture.stdout);
      const stderrFd = onlyStreamDescriptor(fixture.stderr);
      fs.closeSync(stdoutFd);
      let failure: unknown;
      try {
        await completion;
      } catch (error) {
        failure = error;
      }
      // 先のcloseが失敗しても、残る取得済みFDのcloseを必ず試す。
      const remaining = streamDescriptors(fixture.stderr);
      for (const fd of remaining) fs.closeSync(fd);
      expect(remaining).toEqual([]);
      expect(() => fs.fstatSync(stderrFd)).toThrow("EBADF");
      expect(failure).toBeInstanceOf(AggregateError);
      if (!(failure instanceof AggregateError)) throw new Error("Expected stream cleanup failure");
      expect(failure.message).toContain("EBADF");
      expect(failure.cause instanceof Error ? failure.cause.message : "").toContain(
        missingBinary ? "ENOENT" : "EBADF",
      );
      if (missingBinary) {
        expect(
          failure.errors.some(
            (error: unknown) => error instanceof Error && error.message.includes("ENOENT"),
          ),
        ).toBe(true);
        expect(failure.cause).toBeInstanceOf(Error);
      } else {
        expect(fs.readFileSync(fixture.stdout, "utf8").trim()).toMatch(/^\d+\.\d+\.\d+/);
      }
    } finally {
      fs.rmSync(fixture.root, { recursive: true, force: true });
    }
  });
}
