import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DrawioReferencePairRepository } from "./reference-pairs";

function withReferencePairs(
  officialNames: readonly string[],
  candidateNames: readonly string[],
  check: (repository: DrawioReferencePairRepository) => void,
) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "drawio-reference-pairs-"));
  const officialDir = path.join(root, "official");
  const candidateDir = path.join(root, "candidate");

  try {
    for (const [directory, names] of [
      [officialDir, officialNames],
      [candidateDir, candidateNames],
    ] as const) {
      fs.mkdirSync(directory);
      for (const name of names) fs.writeFileSync(path.join(directory, name), name);
    }
    check(new DrawioReferencePairRepository({ officialDir, katanaDir: candidateDir }));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

const validOfficial = ["diagram-b.svg", "diagram-b.png", "diagram-a.png", "diagram-a.svg"];
const validCandidate = ["diagram-b.png", "diagram-a.png"];

test("Draw.io pairs official SVG and PNG with raster-only candidate PNG", () => {
  withReferencePairs(validOfficial, validCandidate, (repository) => {
    const pairs = repository.list();
    expect(pairs.map((pair) => pair.slug)).toEqual(["diagram-a", "diagram-b"]);
    expect(fs.existsSync(pairs[0]?.officialSvgPath ?? "")).toBe(true);
    expect(fs.existsSync(pairs[0]?.katanaSvgPath ?? "")).toBe(false);
  });
});

test.each([
  [validOfficial, ["diagram-a.png"], "candidate PNG slug mismatch; missing=diagram-b"],
  [
    validOfficial,
    [...validCandidate, "diagram-c.png"],
    "candidate PNG slug mismatch; missing=; extra=diagram-c",
  ],
  [
    [...validOfficial, "diagram-c.svg"],
    validCandidate,
    "official SVG slug mismatch; missing=; extra=diagram-c",
  ],
  [[], [], "Draw.io comparison has no official PNG fixtures"],
  [[".png"], [], "Draw.io .png inventory contains an empty slug"],
] as const)("Draw.io rejects invalid reference inventory %#", (official, candidate, message) => {
  withReferencePairs(official, candidate, (repository) => {
    expect(() => repository.list()).toThrow(message);
  });
});
