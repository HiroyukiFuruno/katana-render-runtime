import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { RuntimeAssetCatalog } from "../runtime-assets/runtime-asset-common";

const PLANTUML_JAR_VERSION = RuntimeAssetCatalog.byKind("plantuml").version;

interface CliParsedOptions {
  fixtures: string;
  output: string;
  jar: string;
  darkMode: boolean;
}

class PlantUmlJarPath {
  default(): string {
    if (process.env.KDR_PLANTUML_JAR !== undefined) {
      return process.env.KDR_PLANTUML_JAR;
    }
    if (process.env.PLANTUML_JAR !== undefined) {
      return process.env.PLANTUML_JAR;
    }
    return path.join(this.cacheRoot(), PLANTUML_JAR_VERSION, "plantuml.jar");
  }

  private cacheRoot(): string {
    if (process.env.KDR_PLANTUML_CACHE_DIR !== undefined) {
      return process.env.KDR_PLANTUML_CACHE_DIR;
    }
    if (process.platform === "darwin") {
      return path.join(this.homeOrTemp(), "Library", "Caches", "kdr", "plantuml");
    }
    if (process.platform === "win32") {
      return path.join(
        process.env.LOCALAPPDATA ?? path.join(this.homeOrTemp(), "AppData", "Local"),
        "kdr",
        "plantuml",
      );
    }
    return path.join(
      process.env.XDG_CACHE_HOME ?? path.join(this.homeOrTemp(), ".cache"),
      "kdr",
      "plantuml",
    );
  }

  private homeOrTemp(): string {
    return process.env.HOME ?? process.env.USERPROFILE ?? process.env.TMPDIR ?? "/tmp";
  }
}

const CliOptions = {
  parse(argv: string[]): CliParsedOptions {
    return {
      fixtures: path.resolve(
        CliOptions.get(argv, "--fixtures", "tests/fixtures/plantuml/official"),
      ),
      output: path.resolve(CliOptions.get(argv, "--output", "tmp/kdr-plantuml-official")),
      jar: path.resolve(CliOptions.get(argv, "--jar", new PlantUmlJarPath().default())),
      darkMode: argv.includes("--dark-mode"),
    };
  },

  get(argv: string[], name: string, fallback: string): string {
    const index = argv.indexOf(name);
    return index >= 0 ? (argv.at(index + 1) ?? fallback) : fallback;
  },
};

class PlantUmlFixtureRepository {
  constructor(private fixtures: string) {}

  list(): string[] {
    return fs
      .readdirSync(this.fixtures)
      .filter((fileName) => this.isFixture(fileName))
      .sort();
  }

  read(fileName: string): string {
    const source = fs.readFileSync(path.join(this.fixtures, fileName), "utf8");
    return fileName.endsWith(".md") ? extractPlantUmlMarkdown(source) : source;
  }

  private isFixture(fileName: string): boolean {
    return fileName !== "README.md" && (fileName.endsWith(".puml") || fileName.endsWith(".md"));
  }
}

function extractPlantUmlMarkdown(source: string): string {
  const match = source.match(/```plantuml\s*\n([\s\S]*?)\n```/);
  return match?.[1] ?? source;
}

class PlantUmlOfficialRenderer {
  constructor(private options: CliParsedOptions) {}

  run() {
    this.prepareOutput();
    const repository = new PlantUmlFixtureRepository(this.options.fixtures);
    for (const fileName of repository.list()) {
      this.render(fileName, repository.read(fileName));
    }
  }

  private prepareOutput() {
    fs.rmSync(this.options.output, { recursive: true, force: true });
    fs.mkdirSync(this.options.output, { recursive: true });
  }

  private render(fileName: string, source: string) {
    const args = ["-jar", this.options.jar, "-pipe", "-tsvg"];
    if (this.options.darkMode) {
      args.push("--dark-mode");
    }
    const result = spawnSync("java", args, { input: source, encoding: "utf8" });
    const svg = String(result.stdout);
    if (result.status !== 0 || !svg.includes("<svg")) {
      throw new Error(this.failureMessage(fileName, result.stderr));
    }
    fs.writeFileSync(this.svgPath(fileName), svg, "utf8");
    console.log(`rendered official PlantUML: ${fileName}`);
  }

  private failureMessage(fileName: string, stderr: string): string {
    return [`PlantUML official render failed: ${fileName}`, stderr.trim()]
      .filter((it) => it.length > 0)
      .join("\n");
  }

  private svgPath(fileName: string): string {
    return path.join(this.options.output, `${path.basename(fileName, path.extname(fileName))}.svg`);
  }
}

new PlantUmlOfficialRenderer(CliOptions.parse(process.argv.slice(2))).run();
