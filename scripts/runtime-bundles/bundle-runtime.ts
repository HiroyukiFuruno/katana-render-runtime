import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { parseRuntimeBundleOptions } from "./runtime-bundle-cli";
import { RuntimeBundleCommand } from "./runtime-bundle-command";

const root = process.cwd();
const options = parseRuntimeBundleOptions(process.argv.slice(2));
const command = new RuntimeBundleCommand(root, options.mode);
if (!options.classicGenerationEvidence) {
  await command.run();
} else {
  const bundles = await command.generateCheckedBundles({ classicGeneration: true });
  const evidence = bundles.map(({ definition, content, checksum, classicGeneration }) => ({
    definition: { name: definition.name, outputFile: definition.outputFile },
    content,
    checksum,
    ...(classicGeneration ? { classicGeneration } : {}),
  }));
  const directory = path.join(root, "tmp", "quality", "classic-generation-evidence");
  fs.mkdirSync(directory, { recursive: true });
  const destination = path.join(directory, "current.json");
  const temporary = path.join(directory, `.current-${randomUUID()}.tmp`);
  fs.writeFileSync(temporary, `${JSON.stringify(evidence)}\n`, { flag: "wx" });
  try {
    fs.renameSync(temporary, destination);
  } catch (error) {
    fs.rmSync(temporary, { force: true });
    throw error;
  }
}
