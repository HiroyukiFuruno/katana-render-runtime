import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { markerInventoryFixture } from "./svg-marker-inventory-test-fixture";
import { inspectNativeFailureArtifacts } from "./svg-marker-native-artifact";

interface Fixture {
  root: string;
  repoRoot: string;
  crateRoot: string;
  fixtureRoot: string;
  artifactRoot: string;
  inventory: ReturnType<typeof markerInventoryFixture>;
  artifactDirectories: string[];
}

function withFixture(run: (fixture: Fixture) => void): void {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-native-svg-artifact-")));
  try {
    const repoRoot = path.join(root, "repo");
    const crateRoot = path.join(repoRoot, "crates/katana-render-runtime");
    const fixtureRoot = path.join(repoRoot, "tests/fixtures/svg-engine/upstream/resvg/tests");
    const artifactRoot = path.join(root, "artifacts");
    fs.mkdirSync(crateRoot, { recursive: true });
    fs.mkdirSync(artifactRoot, { recursive: true });
    const inventory = markerInventoryFixture();
    const artifactDirectories = inventory.cases.map((entry, index) => {
      const input = path.join(fixtureRoot, entry.input);
      const reference = path.join(fixtureRoot, entry.input.replace(/\.svg$/u, ".png"));
      fs.mkdirSync(path.dirname(input), { recursive: true });
      fs.writeFileSync(input, "fixture svg");
      fs.writeFileSync(reference, "fixture reference png");
      const directory = path.join(
        artifactRoot,
        `svg-render-failure-${String(index).padStart(4, "0")}`,
      );
      fs.mkdirSync(directory);
      fs.writeFileSync(path.join(directory, "actual.png"), "unread placeholder png");
      fs.writeFileSync(
        path.join(directory, "metadata.json"),
        JSON.stringify({
          input: path.relative(crateRoot, input),
          reference: path.relative(crateRoot, reference),
          width: entry.requestedWidth,
          height: entry.requestedHeight,
          differentPixels: 1,
        }),
      );
      return directory;
    });
    run({
      root,
      repoRoot,
      crateRoot,
      fixtureRoot,
      artifactRoot,
      inventory,
      artifactDirectories,
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test("matches all 48 native failure artifacts to inventory input, reference, and dimensions", () => {
  withFixture((fixture) => {
    const artifacts = inspectNativeFailureArtifacts(
      fixture.artifactRoot,
      fixture.repoRoot,
      fixture.inventory,
    );
    expect(artifacts.size).toBe(48);
    expect(artifacts.get("tests/painting/marker/percent-values.svg")).toMatchObject({
      width: 300,
      height: 150,
      differentPixels: 1,
    });
  });
});

function artifactDirectory(fixture: Fixture, index: number): string {
  const directory = fixture.artifactDirectories[index];
  if (!directory) throw new Error("artifact fixture is incomplete");
  return directory;
}

function updateFirstMetadata(
  fixture: Fixture,
  update: (metadata: Record<string, unknown>) => void,
): string {
  const metadataPath = path.join(artifactDirectory(fixture, 0), "metadata.json");
  const metadata = JSON.parse(fs.readFileSync(metadataPath, "utf8")) as Record<string, unknown>;
  update(metadata);
  fs.writeFileSync(metadataPath, JSON.stringify(metadata));
  return fixture.artifactRoot;
}

function duplicateInput(fixture: Fixture): string {
  const firstMetadata = path.join(artifactDirectory(fixture, 0), "metadata.json");
  const secondMetadata = path.join(artifactDirectory(fixture, 1), "metadata.json");
  fs.writeFileSync(secondMetadata, fs.readFileSync(firstMetadata));
  return fixture.artifactRoot;
}

function symlinkArtifactRoot(fixture: Fixture): string {
  const alias = path.join(fixture.root, "artifact-root-alias");
  fs.symlinkSync(fixture.artifactRoot, alias, "dir");
  return alias;
}

function symlinkArtifactDirectory(fixture: Fixture): string {
  const directory = artifactDirectory(fixture, 0);
  const moved = path.join(fixture.root, "saved-artifact-directory");
  fs.renameSync(directory, moved);
  fs.symlinkSync(moved, directory, "dir");
  return fixture.artifactRoot;
}

function symlinkArtifactFile(fixture: Fixture): string {
  const pngPath = path.join(artifactDirectory(fixture, 0), "actual.png");
  const target = path.join(fixture.root, "saved-actual.png");
  fs.renameSync(pngPath, target);
  fs.symlinkSync(target, pngPath);
  return fixture.artifactRoot;
}

type ArtifactMutation = readonly [
  label: string,
  root: (fixture: Fixture) => string,
  expectedError: string,
];

const artifactMutations: readonly ArtifactMutation[] = [
  [
    "missing artifact",
    (fixture) => {
      fs.rmSync(artifactDirectory(fixture, 0), { recursive: true });
      return fixture.artifactRoot;
    },
    "native failure artifact directory set is incomplete or contains extras",
  ],
  [
    "extra artifact",
    (fixture) => {
      fs.mkdirSync(path.join(fixture.artifactRoot, "svg-render-failure-extra"));
      return fixture.artifactRoot;
    },
    "native failure artifact directory set is incomplete or contains extras",
  ],
  [
    "extra artifact file",
    (fixture) => {
      fs.writeFileSync(path.join(artifactDirectory(fixture, 0), "extra.txt"), "extra");
      return fixture.artifactRoot;
    },
    "native failure artifact contents are incomplete or contain extras",
  ],
  ["duplicate input", duplicateInput, "native failure artifact input is unknown or duplicated"],
  [
    "unsafe metadata path",
    (fixture) =>
      updateFirstMetadata(fixture, (metadata) => {
        metadata.input = "\0outside.svg";
      }),
    "native failure artifact input path is unsafe",
  ],
  [
    "wrong reference",
    (fixture) =>
      updateFirstMetadata(fixture, (metadata) => {
        const otherCase = fixture.inventory.cases[1];
        if (!otherCase) throw new Error("artifact inventory fixture is incomplete");
        const otherReference = path.join(
          fixture.fixtureRoot,
          otherCase.input.replace(/\.svg$/u, ".png"),
        );
        metadata.reference = path.relative(fixture.crateRoot, otherReference);
      }),
    "native failure artifact metadata differs from its inventory case",
  ],
  [
    "wrong dimensions",
    (fixture) =>
      updateFirstMetadata(fixture, (metadata) => {
        metadata.height = 149;
      }),
    "native failure artifact metadata differs from its inventory case",
  ],
  [
    "unknown metadata field",
    (fixture) =>
      updateFirstMetadata(fixture, (metadata) => {
        metadata.binarySha256 = "f".repeat(64);
      }),
    "native failure artifact metadata is invalid",
  ],
  [
    "symlink artifact root",
    symlinkArtifactRoot,
    "native failure artifact directory is missing, unsafe, or not canonical",
  ],
  [
    "symlink artifact directory",
    symlinkArtifactDirectory,
    "native failure artifact directory is missing, unsafe, or not canonical",
  ],
  [
    "symlink artifact file",
    symlinkArtifactFile,
    "native failure artifact file is missing, unsafe, or not canonical",
  ],
];

test.each(artifactMutations)(
  "rejects invalid native failure artifact inventory: %s",
  (_label, mutate, expectedError) => {
    withFixture((fixture) => {
      const artifactRoot = mutate(fixture);
      expect(() =>
        inspectNativeFailureArtifacts(artifactRoot, fixture.repoRoot, fixture.inventory),
      ).toThrow(expectedError);
    });
  },
);
