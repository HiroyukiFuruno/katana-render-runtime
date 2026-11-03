import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { parseJsonValue, sha256 } from "./mermaid-pair-manifest";
import {
  assertCurrentGitSourceSet,
  digestFileRecords,
  verifyFileRecord,
} from "./native-build-receipt";
import {
  captureLiveSourceObservation,
  parseLiveSourceObservation,
  verifyLiveSourceObservation,
} from "./native-live-source-observation";
import {
  captureNativeCoverageSourceInputs,
  captureNativeSourceManifest,
} from "./native-source-manifest";

test("captures current bytes and detects source changes with an unchanged HEAD", () => {
  withGitRepository((root) => {
    fs.writeFileSync(path.join(root, "source.rs"), "first bytes");
    commitFiles(root, ["source.rs"]);
    const head = git(root, ["rev-parse", "HEAD"]).trim();

    const first = captureNativeSourceManifest(root);
    fs.writeFileSync(path.join(root, "source.rs"), "other bytes");
    const second = captureNativeSourceManifest(root);

    expect(git(root, ["rev-parse", "HEAD"]).trim()).toBe(head);
    expect(first.files[0]?.sha256).toBe(sha256(Buffer.from("first bytes")));
    expect(second.files[0]?.sha256).toBe(sha256(Buffer.from("other bytes")));
    expect(second.digest).not.toBe(first.digest);
  });
});

test("includes fresh untracked files and detects their addition and removal", () => {
  withGitRepository((root) => {
    fs.writeFileSync(path.join(root, "tracked.rs"), "tracked");
    commitFiles(root, ["tracked.rs"]);

    const initial = captureNativeSourceManifest(root);
    fs.writeFileSync(path.join(root, "new.rs"), "untracked");
    const added = captureNativeSourceManifest(root);
    expect(added.files.map((file) => path.basename(file.path))).toEqual(["new.rs", "tracked.rs"]);
    expect(added.digest).not.toBe(initial.digest);

    fs.unlinkSync(path.join(root, "new.rs"));
    const removed = captureNativeSourceManifest(root);
    expect(removed.files).toEqual(initial.files);
    expect(removed.digest).toBe(initial.digest);
  });
});

test("does not include ignored private tmp or target files", () => {
  withGitRepository((root) => {
    fs.writeFileSync(path.join(root, ".gitignore"), "tmp/\ntarget/\n");
    fs.writeFileSync(path.join(root, "source.rs"), "source");
    fs.mkdirSync(path.join(root, "tmp", "private"), { recursive: true });
    fs.mkdirSync(path.join(root, "target", "debug"), { recursive: true });
    fs.writeFileSync(path.join(root, "tmp", "private", "secret"), "must not read");
    fs.writeFileSync(path.join(root, "target", "debug", "artifact"), "must not read");
    commitFiles(root, [".gitignore", "source.rs"]);

    const manifest = captureNativeSourceManifest(root);
    expect(manifest.files.map((file) => path.basename(file.path))).toEqual([
      ".gitignore",
      "source.rs",
    ]);
    expect(manifest.files.some((file) => file.path.includes(`${path.sep}tmp${path.sep}`))).toBe(
      false,
    );
    expect(manifest.files.some((file) => file.path.includes(`${path.sep}target${path.sep}`))).toBe(
      false,
    );
  });
});

test("rejects a symlink in the current tracked source set", () => {
  withGitRepository((root) => {
    fs.writeFileSync(path.join(root, "real.rs"), "source");
    fs.symlinkSync("real.rs", path.join(root, "alias.rs"));
    commitFiles(root, ["real.rs", "alias.rs"]);

    expect(() => captureNativeSourceManifest(root)).toThrow("canonical regular file");
  });
});

test("requires the canonical Git top-level rather than a repository subdirectory", () => {
  withGitRepository((root) => {
    fs.mkdirSync(path.join(root, "nested"));
    expect(() => captureNativeSourceManifest(path.join(root, "nested"))).toThrow(
      "Native source repository root must be the Git top-level directory",
    );
  });
});

test("rejects force-added tmp and target paths before opening their broken symlinks", () => {
  for (const rootSegment of ["tmp", "target"]) {
    withGitRepository((root) => {
      const forbiddenDirectory = path.join(root, rootSegment);
      fs.mkdirSync(forbiddenDirectory);
      fs.symlinkSync("missing-content", path.join(forbiddenDirectory, "source.rs"));
      commitFiles(root, [`${rootSegment}/source.rs`]);

      expect(() => captureNativeSourceManifest(root)).toThrow(
        `Git-visible source path is outside the native source set: ${rootSegment}/source.rs`,
      );
    });
  }
});

test("uses the native manifest sort and digest convention and verifies captured records", () => {
  withGitRepository((root) => {
    fs.writeFileSync(path.join(root, "z.rs"), "z");
    fs.writeFileSync(path.join(root, "a.rs"), "a");
    commitFiles(root, ["z.rs", "a.rs"]);

    const manifest = captureNativeSourceManifest(root);
    const expected = [...manifest.files].sort((left, right) =>
      Buffer.compare(Buffer.from(left.path, "utf8"), Buffer.from(right.path, "utf8")),
    );
    expect(manifest.schema).toBe("krr-native-build-source-manifest-v1");
    expect(manifest.files).toEqual(expected);
    expect(manifest.digest).toBe(digestFileRecords(manifest.files));
    expect(() => assertCurrentGitSourceSet(root, manifest.files)).not.toThrow();
    for (const file of manifest.files)
      expect(() => verifyFileRecord(file, "test source")).not.toThrow();
  });
});

test("coverage source digest ignores only the quality task ledger bytes", () => {
  withGitRepository((root) => {
    fs.mkdirSync(path.join(root, "docs"));
    fs.writeFileSync(path.join(root, "docs/quality-gate-improvement-tasks.md"), "ledger one");
    fs.writeFileSync(path.join(root, "docs/other.md"), "other docs one");
    fs.writeFileSync(path.join(root, "source.rs"), "source one");
    commitFiles(root, ["docs/quality-gate-improvement-tasks.md", "docs/other.md", "source.rs"]);

    const initialManifest = captureNativeSourceManifest(root);
    const initialCoverage = captureNativeCoverageSourceInputs(root);
    expect(initialCoverage).toEqual({
      schema: "krr-native-coverage-source-inputs-v1",
      sourceDigest: initialManifest.digest,
      coverageInputDigest: digestFileRecords(
        initialManifest.files.filter(
          (file) => file.path !== path.join(root, "docs/quality-gate-improvement-tasks.md"),
        ),
      ),
    });

    const mutations = [
      {
        path: "docs/quality-gate-improvement-tasks.md",
        content: "ledger two",
        fullDigestChanges: true,
        coverageDigestChanges: false,
      },
      {
        path: "source.rs",
        content: "source two",
        fullDigestChanges: true,
        coverageDigestChanges: true,
      },
      {
        path: "docs/other.md",
        content: "other docs two",
        fullDigestChanges: true,
        coverageDigestChanges: true,
      },
    ];
    let previous = initialCoverage;
    for (const mutation of mutations) {
      fs.writeFileSync(path.join(root, mutation.path), mutation.content);
      const manifest = captureNativeSourceManifest(root);
      const coverage = captureNativeCoverageSourceInputs(root);
      expect(manifest.digest !== previous.sourceDigest).toBe(mutation.fullDigestChanges);
      expect(coverage.coverageInputDigest !== previous.coverageInputDigest).toBe(
        mutation.coverageDigestChanges,
      );
      previous = coverage;
    }

    git(root, ["rm", "--cached", "--", "docs/quality-gate-improvement-tasks.md"]);
    fs.unlinkSync(path.join(root, "docs/quality-gate-improvement-tasks.md"));
    expect(() => captureNativeCoverageSourceInputs(root)).toThrow(
      "Native coverage source inputs require exactly one quality task ledger",
    );
  });
});

test("CLI requires --root and prints one captured JSON manifest", () => {
  withGitRepository((root) => {
    fs.mkdirSync(path.join(root, "docs"));
    fs.writeFileSync(path.join(root, "docs/quality-gate-improvement-tasks.md"), "ledger");
    fs.writeFileSync(path.join(root, "source.rs"), "source");
    commitFiles(root, ["docs/quality-gate-improvement-tasks.md", "source.rs"]);
    const success = runNativeSourceManifestCli(["--root", root]);
    expect(success.exitCode).toBe(0);
    expect(success.stderr.toString("utf8")).toBe("");
    expect(JSON.parse(success.stdout.toString("utf8"))).toEqual(captureNativeSourceManifest(root));

    const coverageInputs = runNativeSourceManifestCli(["--root", root, "--coverage-inputs"]);
    expect(coverageInputs.exitCode).toBe(0);
    expect(coverageInputs.stderr.toString("utf8")).toBe("");
    expect(JSON.parse(coverageInputs.stdout.toString("utf8"))).toEqual(
      captureNativeCoverageSourceInputs(root),
    );

    const missingRoot = runNativeSourceManifestCli([]);
    expect(missingRoot.exitCode).toBe(1);
    expect(missingRoot.stdout.toString("utf8")).toBe("");

    const unknownArgument = runNativeSourceManifestCli(["--root", root, "--extra"]);
    expect(unknownArgument.exitCode).toBe(1);
    expect(unknownArgument.stdout.toString("utf8")).toBe("");
  });
});

test("records and independently parses clean, dirty, untracked and staged live source", () => {
  withGitRepository((root) => {
    fs.writeFileSync(path.join(root, "source.rs"), "committed\n");
    commitFiles(root, ["source.rs"]);
    const clean = captureLiveSourceObservation(root);
    expect(clean.statusBase64).toBe("");
    expect(clean.head).toBe(git(root, ["rev-parse", "HEAD"]).trim());
    expect(clean.manifest.files[0]?.sha256).toBe(sha256(Buffer.from("committed\n")));
    const json = (input: object) => parseJsonValue(JSON.stringify(input), "observation");
    expect(verifyLiveSourceObservation(root, json(clean))).toEqual(clean);

    fs.writeFileSync(path.join(root, "source.rs"), "modified\n");
    fs.writeFileSync(path.join(root, "extra.rs"), "untracked\n");
    const dirty = captureLiveSourceObservation(root);
    expect(dirty.head).toBe(clean.head);
    expect(dirty.indexSha256).toBe(clean.indexSha256);
    expect(dirty.statusBase64).toBe(
      execFileSync("git", ["status", "--porcelain=v1", "-z", "--untracked-files=all"], {
        cwd: root,
        env: { ...process.env, GIT_OPTIONAL_LOCKS: "0" },
      }).toString("base64"),
    );
    expect(dirty.manifest.files.map((file) => path.basename(file.path))).toEqual([
      "extra.rs",
      "source.rs",
    ]);
    expect(dirty.manifest.files[1]?.sha256).toBe(sha256(Buffer.from("modified\n")));
    expect(verifyLiveSourceObservation(root, json(dirty))).toEqual(dirty);
    expect(parseLiveSourceObservation(json(clean))).toEqual(clean);
    expect(() => verifyLiveSourceObservation(root, json(clean))).toThrow();

    git(root, ["add", "source.rs"]);
    const staged = captureLiveSourceObservation(root);
    expect(staged.indexSha256).not.toBe(dirty.indexSha256);
    expect(staged.indexSha256).toBe(sha256(fs.readFileSync(path.join(root, ".git", "index"))));
    expect(staged.manifest).toEqual(dirty.manifest);
    expect(() => verifyLiveSourceObservation(root, json(dirty))).toThrow();
  });
});

test("rejects noncanonical roots and malformed live observations", () => {
  withGitRepository((root) => {
    fs.writeFileSync(path.join(root, "source.rs"), "source\n");
    commitFiles(root, ["source.rs"]);
    const observed = captureLiveSourceObservation(root);
    const json = (input: object) => parseJsonValue(JSON.stringify(input), "observation");
    const symlink = path.join(root, "alias");
    fs.symlinkSync(root, symlink);
    for (const noncanonical of [".", `${root}/.`, `${root}/missing/..`, symlink]) {
      expect(() => captureLiveSourceObservation(noncanonical)).toThrow();
      expect(() => verifyLiveSourceObservation(noncanonical, json(observed))).toThrow();
    }
    fs.unlinkSync(symlink);
    const mutations = [
      { ...observed, head: "0".repeat(40) },
      { ...observed, indexSha256: "0".repeat(64) },
      { ...observed, statusBase64: "AA==" },
      { ...observed, manifest: { ...observed.manifest, digest: "0".repeat(64) } },
      { ...observed, accepted: true },
    ];
    for (const mutation of mutations) {
      expect(() => verifyLiveSourceObservation(root, json(mutation))).toThrow();
    }
    expect(() => parseLiveSourceObservation(json(mutations[3] ?? {}))).toThrow();
    expect(() => parseLiveSourceObservation(json(mutations[4] ?? {}))).toThrow();
  });
});

function withGitRepository(action: (root: string) => void): void {
  const root = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), "krr-native-source-manifest-")),
  );
  try {
    git(root, ["init", "--quiet"]);
    git(root, ["config", "user.email", "krr-test@example.invalid"]);
    git(root, ["config", "user.name", "KRR test"]);
    action(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function git(root: string, args: string[]): string {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" });
}

function runNativeSourceManifestCli(args: string[]) {
  return Bun.spawnSync(["bun", path.join(import.meta.dir, "native-source-manifest.ts"), ...args]);
}

function commitFiles(root: string, files: string[]): void {
  git(root, ["add", "--", ...files]);
  git(root, ["commit", "-m", "initial", "--quiet"]);
}
