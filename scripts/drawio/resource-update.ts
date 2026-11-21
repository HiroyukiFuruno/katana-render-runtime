import fs from "node:fs";
import path from "node:path";
import { getOptionValue, printHelpIfRequested } from "./cli-options";
import { DrawioResourceInventory } from "./resource-common";

interface CliParsedOptions {
  readonly resourcesDir: string;
  readonly manifestPath: string;
}

const CliOptions = {
  parse(argv: string[]): CliParsedOptions {
    if (
      printHelpIfRequested(
        argv,
        "Usage: bun run scripts/drawio/resource-update.ts [--resources DIR] [--manifest FILE]",
      )
    ) {
      process.exit(0);
    }
    const resourcesDir = path.resolve(
      getOptionValue(
        argv,
        "--resources",
        "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources",
      ),
    );
    return {
      resourcesDir,
      manifestPath: path.resolve(
        getOptionValue(
          argv,
          "--manifest",
          path.join(resourcesDir, "drawio-resource-manifest.json"),
        ),
      ),
    };
  },
};

class DrawioResourceUpdate {
  constructor(private options: CliParsedOptions) {}

  run() {
    this.validate();
    const manifest = new DrawioResourceInventory(this.options.resourcesDir).manifest();
    fs.mkdirSync(path.dirname(this.options.manifestPath), { recursive: true });
    fs.writeFileSync(this.options.manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
    console.log(`updated ${path.relative(process.cwd(), this.options.manifestPath)}`);
    console.log(`resources: ${manifest.resources.length}`);
  }

  private validate() {
    if (!fs.existsSync(this.options.resourcesDir)) {
      throw new Error(`Draw.io resources directory not found: ${this.options.resourcesDir}`);
    }
  }
}

new DrawioResourceUpdate(CliOptions.parse(process.argv.slice(2))).run();
