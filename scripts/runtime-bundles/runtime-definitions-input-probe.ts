import fs from "node:fs";
import path from "node:path";
import { runtimeUnusedImportScan } from "./runtime-unused-import-scan";

const root = process.cwd();
const canonical = (file: string, label: string): string => {
  if (
    !path.isAbsolute(file) ||
    path.resolve(file) !== file ||
    !fs.lstatSync(file).isFile() ||
    fs.realpathSync(file) !== file
  )
    throw new Error(`${label} is not a canonical regular file`);
  return file;
};
const inputs = runtimeUnusedImportScan(
  root,
  ["scripts/runtime-bundles/runtime-bundle-definitions_test.ts"],
  canonical,
);
process.stdout.write(JSON.stringify(inputs));
