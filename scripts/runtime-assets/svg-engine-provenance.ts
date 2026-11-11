import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { validateManifest } from "./svg-engine-provenance-manifest";
import { METADATA, type Provenance, readProvenance } from "./svg-engine-provenance-schema";

const VENDOR_RELATIVE = "crates/katana-render-runtime/vendor/svg-engine";
const PROVENANCE_SHA256 = "4f8d6b372446f6602b0f425c38b085454f69e6730f7fd60761c75450fbf30cee";
const README_SHA256 = "0eb3c8eca056cf62fd51cda3471ca73bdf6d8e50bd6e989523d895d9c58e6a23";

function fail(message: string): never {
  throw new Error(`SVG engine provenance: ${message}`);
}

function digest(file: string): string {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function regularFile(file: string, label: string): void {
  const stat = fs.lstatSync(file, { throwIfNoEntry: false });
  if (stat === undefined || !stat.isFile() || stat.isSymbolicLink()) {
    fail(`${label} is not a regular file`);
  }
}

function tree(directory: string, root: string): { files: string[]; directories: string[] } {
  const stat = fs.lstatSync(directory, { throwIfNoEntry: false });
  if (stat === undefined || !stat.isDirectory() || stat.isSymbolicLink()) {
    fail(`unsafe directory: ${directory}`);
  }
  const files: string[] = [];
  const directories: string[] = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const current = path.join(directory, entry.name);
    const relative = path.relative(root, current).split(path.sep).join("/");
    const currentStat = fs.lstatSync(current);
    if (currentStat.isSymbolicLink()) fail(`symlink is not allowed: ${relative}`);
    if (currentStat.isDirectory()) {
      directories.push(relative);
      const nested = tree(current, root);
      files.push(...nested.files);
      directories.push(...nested.directories);
    } else if (currentStat.isFile()) {
      files.push(relative);
    } else {
      fail(`unsupported filesystem entry: ${relative}`);
    }
  }
  return { files, directories };
}

function verifyFiles(
  vendor: string,
  provenance: Provenance,
  destinations: Set<string>,
  actual: string[],
): string[] {
  const expected = [...destinations, ...METADATA].sort();
  if (JSON.stringify([...actual].sort()) !== JSON.stringify(expected)) {
    fail("vendor inventory has unknown or missing files");
  }
  for (const entry of provenance.files) {
    const file = path.join(vendor, entry.destination);
    regularFile(file, entry.destination);
    const bytes = fs.readFileSync(file);
    if (bytes.length !== entry.transplantedBytes || digest(file) !== entry.transplantedSha256) {
      fail(`adapted source changed: ${entry.destination}`);
    }
  }
  return [...destinations].sort();
}

function verifyLicenses(vendor: string, provenance: Provenance): void {
  const expected = new Set(["LICENSE-APACHE", "LICENSE-MIT"]);
  if (provenance.licenses.length !== expected.size) fail("license inventory is incomplete");
  for (const license of provenance.licenses) {
    if (
      !expected.has(license.destination) ||
      license.source !== license.destination ||
      license.sourceSha256 !== license.transplantedSha256 ||
      !/^[0-9a-f]{64}$/.test(license.sourceSha256) ||
      !/^[0-9a-f]{64}$/.test(license.transplantedSha256)
    ) {
      fail("license provenance is invalid");
    }
    const file = path.join(vendor, license.destination);
    regularFile(file, license.destination);
    if (digest(file) !== license.transplantedSha256)
      fail(`license changed: ${license.destination}`);
  }
}

export function verifySvgEngineProvenance(
  repositoryRoot = path.resolve(import.meta.dir, "../.."),
): string[] {
  const root = path.resolve(repositoryRoot);
  const vendor = path.resolve(root, VENDOR_RELATIVE);
  if (path.relative(root, vendor).startsWith("..")) fail("vendor path escapes repository root");
  const provenanceFile = path.join(vendor, "PROVENANCE.json");
  const readmeFile = path.join(vendor, "README.md");
  regularFile(provenanceFile, "PROVENANCE.json");
  regularFile(readmeFile, "README.md");
  if (digest(provenanceFile) !== PROVENANCE_SHA256) fail("PROVENANCE.json changed");
  if (digest(readmeFile) !== README_SHA256) fail("README.md changed");
  const provenance = readProvenance(provenanceFile);
  const destinations = validateManifest(provenance);
  const inventory = tree(vendor, vendor);
  const expectedDirectories = new Set([
    "usvg",
    "resvg",
    ...[...destinations]
      .map((file) => path.posix.dirname(file))
      .filter((directory) => directory !== "."),
  ]);
  if (inventory.directories.some((directory) => !expectedDirectories.has(directory))) {
    fail("vendor inventory has an unknown directory");
  }
  const verifiedFiles = verifyFiles(vendor, provenance, destinations, inventory.files);
  verifyLicenses(vendor, provenance);
  return verifiedFiles.map((file) => `${VENDOR_RELATIVE}/${file}`);
}

if (import.meta.main) {
  try {
    const files = verifySvgEngineProvenance(process.argv[2]);
    console.log(`SVG engine provenance verified: ${files.length} source files`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
