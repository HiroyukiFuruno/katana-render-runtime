import { RuntimeAssetCatalog, type RuntimeAssetDefinition } from "./runtime-asset-common";

interface MermaidLatestResponse {
  readonly version: string;
}

interface GitHubLatestResponse {
  readonly tag_name: string;
}

class LatestVersionClient {
  async latest(definition: RuntimeAssetDefinition): Promise<string> {
    if (definition.kind === "mermaid") {
      return this.mermaid(definition.latestUrl);
    }
    return this.drawio(definition.latestUrl);
  }

  private async mermaid(url: string): Promise<string> {
    const response = await this.get(url);
    const body = (await response.json()) as MermaidLatestResponse;
    return body.version;
  }

  private async drawio(url: string): Promise<string> {
    const response = await this.get(url);
    const body = (await response.json()) as GitHubLatestResponse;
    return body.tag_name.replace(/^v/, "");
  }

  private async get(url: string): Promise<Response> {
    const response = await fetch(url, {
      headers: {
        accept: "application/json",
        "user-agent": "katana-diagram-renderer-release-tool",
      },
    });
    if (!response.ok) {
      throw new Error(`Failed to fetch ${url}: ${response.status}`);
    }
    return response;
  }
}

class LatestCheckCommand {
  constructor(
    private definitions: RuntimeAssetDefinition[],
    private client: LatestVersionClient,
  ) {}

  async run() {
    for (const definition of this.definitions) {
      await this.print(definition);
    }
  }

  private async print(definition: RuntimeAssetDefinition) {
    const latest = await this.client.latest(definition);
    console.log(definition.displayName);
    console.log(`current=${definition.version}`);
    console.log(`latest=${latest}`);
    console.log(`update_hint=${this.updateHint(definition, latest)}`);
  }

  private updateHint(definition: RuntimeAssetDefinition, latest: string): string {
    if (latest === definition.version) {
      return "none";
    }
    return `just ${definition.kind}-update ${latest}`;
  }
}

class CliOptions {
  static definitions(argv: string[]): RuntimeAssetDefinition[] {
    const kind = argv.at(0);
    if (kind === undefined || kind === "all") {
      return RuntimeAssetCatalog.all();
    }
    return [RuntimeAssetCatalog.byKind(kind)];
  }
}

await new LatestCheckCommand(
  CliOptions.definitions(process.argv.slice(2)),
  new LatestVersionClient(),
).run();
