import { afterEach, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { verifySvgEngineProvenance } from "./svg-engine-provenance";
import { parseProvenance, SOURCE_FILE_COUNT } from "./svg-engine-provenance-schema";

const repository = path.resolve(import.meta.dir, "../..");
const vendorRelative = "crates/katana-render-runtime/vendor/svg-engine";
const temporaryRoots: string[] = [];

afterEach(() => {
  for (const root of temporaryRoots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});

function fixture(): string {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-svg-engine-")));
  const destination = path.join(root, vendorRelative);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.cpSync(path.join(repository, vendorRelative), destination, { recursive: true });
  temporaryRoots.push(root);
  return root;
}

function sourceFile(root: string): string {
  return path.join(root, vendorRelative, "usvg/lib.rs");
}

test("承認commitのSVG engine provenanceは完全な48 file inventoryを検証する", () => {
  expect(verifySvgEngineProvenance(repository)).toHaveLength(SOURCE_FILE_COUNT);
});

test("未知・欠落・改変・外部symlinkを拒否する", () => {
  const mutations: Array<(root: string) => void> = [
    (root) => fs.writeFileSync(path.join(root, vendorRelative, "unexpected.rs"), "unknown"),
    (root) => fs.rmSync(sourceFile(root)),
    (root) => fs.appendFileSync(sourceFile(root), "\nchanged"),
    (root) => fs.appendFileSync(path.join(root, vendorRelative, "PROVENANCE.json"), "\nchanged"),
    (root) => fs.appendFileSync(path.join(root, vendorRelative, "README.md"), "\nchanged"),
    (root) => {
      const outside = path.join(root, "outside.rs");
      fs.writeFileSync(outside, "outside");
      fs.rmSync(sourceFile(root));
      fs.symlinkSync(outside, sourceFile(root));
    },
  ];
  for (const mutate of mutations) {
    const root = fixture();
    mutate(root);
    expect(() => verifySvgEngineProvenance(root)).toThrow();
  }
});

test("licenseのSHA256改変を拒否する", () => {
  const root = fixture();
  fs.appendFileSync(path.join(root, vendorRelative, "LICENSE-MIT"), "\nchanged");
  expect(() => verifySvgEngineProvenance(root)).toThrow("license changed");
});

test("license destinationの重複をschema境界で拒否する", () => {
  const document = JSON.parse(
    fs.readFileSync(path.join(repository, vendorRelative, "PROVENANCE.json"), "utf8"),
  ) as { licenses: Array<Record<string, unknown>> };
  const firstLicense = document.licenses.at(0);
  if (firstLicense === undefined) throw new Error("provenance fixture has no license");
  document.licenses = [firstLicense, firstLicense];
  expect(() => parseProvenance(document)).toThrow("license destinations must be unique");
});
