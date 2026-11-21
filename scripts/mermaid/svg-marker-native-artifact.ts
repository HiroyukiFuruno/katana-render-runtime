import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { fixturePath, type parseMarkerInventory } from "./svg-marker-browser-capture";
import { readStableRgbaPng } from "./svg-marker-image";

const failureMetadata = z.strictObject({
  input: z.string().min(1),
  reference: z.string().min(1),
  width: z.number().int().positive(),
  height: z.number().int().positive(),
  differentPixels: z.number().int().positive(),
});

type MarkerInventory = ReturnType<typeof parseMarkerInventory>;

export interface NativeFailureArtifact {
  input: string;
  reference: string;
  width: number;
  height: number;
  differentPixels: number;
  pngSha256: string;
  pixels: Uint8Array;
}

export interface NativeFailureArtifactFiles {
  input: string;
  reference: string;
  width: number;
  height: number;
  differentPixels: number;
  pngPath: string;
}

export function readNativeFailureArtifacts(
  artifactRoot: string,
  repoRoot: string,
  inventory: MarkerInventory,
): Map<string, NativeFailureArtifact> {
  const files = inspectNativeFailureArtifacts(artifactRoot, repoRoot, inventory);
  return new Map<string, NativeFailureArtifact>(
    [...files].map(([input, artifact]) => {
      canonicalFile(artifact.pngPath);
      const png = readStableRgbaPng(artifact.pngPath, artifact.width, artifact.height);
      canonicalFile(artifact.pngPath);
      const loaded: NativeFailureArtifact = {
        ...artifact,
        pngSha256: png.sha256,
        pixels: png.pixels,
      };
      return [input, loaded] as const;
    }),
  );
}

export function inspectNativeFailureArtifacts(
  artifactRoot: string,
  repoRoot: string,
  inventory: MarkerInventory,
): Map<string, NativeFailureArtifactFiles> {
  const root = canonicalDirectory(artifactRoot);
  const crateRoot = path.join(repoRoot, "crates/katana-render-runtime");
  const fixtureRoot = path.join(repoRoot, "tests/fixtures/svg-engine/upstream/resvg/tests");
  const expected = new Map(
    inventory.cases.map((entry) => [
      entry.input,
      {
        input: fixturePath(fixtureRoot, entry.input),
        reference: fixturePath(fixtureRoot, entry.input.replace(/\.svg$/u, ".png")),
        width: entry.requestedWidth,
        height: entry.requestedHeight,
      },
    ]),
  );
  const directories = fs.readdirSync(root).sort();
  if (
    directories.length !== expected.size ||
    directories.some((name) => !/^svg-render-failure-[A-Za-z0-9_-]+$/u.test(name))
  ) {
    throw new Error("native failure artifact directory set is incomplete or contains extras");
  }

  const artifacts = new Map<string, NativeFailureArtifactFiles>();
  for (const directory of directories) {
    const directoryPath = canonicalDirectory(path.join(root, directory));
    const files = fs.readdirSync(directoryPath).sort();
    if (files.length !== 2 || files[0] !== "actual.png" || files[1] !== "metadata.json") {
      throw new Error("native failure artifact contents are incomplete or contain extras");
    }
    const metadataPath = canonicalFile(path.join(directoryPath, "metadata.json"));
    const pngPath = canonicalFile(path.join(directoryPath, "actual.png"));
    const metadata = parseFailureMetadata(fs.readFileSync(metadataPath, "utf8"));
    const input = resolveExpectedMetadataPath(metadata.input, crateRoot, "input");
    const reference = resolveExpectedMetadataPath(metadata.reference, crateRoot, "reference");
    const match = [...expected].find(([, value]) => value.input === input);
    if (!match || artifacts.has(match[0])) {
      throw new Error("native failure artifact input is unknown or duplicated");
    }
    const [relativeInput, dimensions] = match;
    if (
      reference !== dimensions.reference ||
      metadata.width !== dimensions.width ||
      metadata.height !== dimensions.height ||
      metadata.differentPixels > metadata.width * metadata.height
    ) {
      throw new Error("native failure artifact metadata differs from its inventory case");
    }
    artifacts.set(relativeInput, {
      input: relativeInput,
      reference,
      width: metadata.width,
      height: metadata.height,
      differentPixels: metadata.differentPixels,
      pngPath,
    });
  }
  if (artifacts.size !== expected.size) {
    throw new Error("native failure artifacts do not cover the fixed marker inventory");
  }
  return artifacts;
}

function parseFailureMetadata(text: string): z.infer<typeof failureMetadata> {
  let value: unknown;
  try {
    value = JSON.parse(text) as unknown;
    return failureMetadata.parse(value);
  } catch {
    throw new Error("native failure artifact metadata is invalid");
  }
}

function resolveExpectedMetadataPath(value: string, crateRoot: string, field: string): string {
  if (value.includes("\0")) {
    throw new Error(`native failure artifact ${field} path is unsafe`);
  }
  const resolved = path.resolve(crateRoot, value);
  if (fs.realpathSync(resolved) !== resolved) {
    throw new Error(`native failure artifact ${field} path is not canonical`);
  }
  return resolved;
}

function canonicalDirectory(directory: string): string {
  const resolved = path.resolve(directory);
  try {
    const stat = fs.lstatSync(resolved);
    if (
      !path.isAbsolute(directory) ||
      directory !== resolved ||
      stat.isSymbolicLink() ||
      !stat.isDirectory() ||
      fs.realpathSync(resolved) !== resolved
    ) {
      throw new Error("not canonical");
    }
  } catch {
    throw new Error("native failure artifact directory is missing, unsafe, or not canonical");
  }
  return resolved;
}

function canonicalFile(file: string): string {
  try {
    const stat = fs.lstatSync(file);
    if (stat.isSymbolicLink() || !stat.isFile() || fs.realpathSync(file) !== file) {
      throw new Error("not canonical");
    }
  } catch {
    throw new Error("native failure artifact file is missing, unsafe, or not canonical");
  }
  return file;
}
