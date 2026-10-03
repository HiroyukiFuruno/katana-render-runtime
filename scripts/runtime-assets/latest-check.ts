import { spawnSync } from "node:child_process";
import type { RuntimeAssetDefinition } from "./runtime-asset-common";

interface NpmLatestResponse {
  readonly version: string;
}

interface GitHubLatestResponse {
  readonly tag_name: string;
}

export type RuntimeAssetFetch = (url: string, init: RequestInit) => Promise<Response>;

export type GitHubCredentialsProvider = () => string | undefined;

function readGhToken(): string | undefined {
  const result = spawnSync("gh", ["auth", "token", "--hostname", "github.com"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
    timeout: 5_000,
    maxBuffer: 64 * 1024,
  });
  return result.status === 0 ? result.stdout.trim() || undefined : undefined;
}

export function githubCredentials(
  environment: NodeJS.ProcessEnv = process.env,
  readToken: GitHubCredentialsProvider = readGhToken,
): string | undefined {
  const token = environment.GH_TOKEN?.trim() || environment.GITHUB_TOKEN?.trim() || readToken();
  if (token !== undefined && /[^\x21-\x7e]/.test(token)) {
    throw new Error("GitHub credentials must contain only visible ASCII characters");
  }
  return token || undefined;
}

function isGitHubApi(url: string): boolean {
  const parsed = new URL(url);
  return parsed.origin === "https://api.github.com" && !parsed.username && !parsed.password;
}

export class LatestVersionClient {
  private readonly fetcher: RuntimeAssetFetch;

  constructor(
    fetcher?: RuntimeAssetFetch,
    private readonly credentials: GitHubCredentialsProvider = fetcher === undefined
      ? githubCredentials
      : () => undefined,
  ) {
    this.fetcher = fetcher ?? ((url, init) => fetch(url, init));
  }

  async latest(definition: RuntimeAssetDefinition): Promise<string> {
    if (definition.kind === "drawio") {
      return this.drawio(definition.latestUrl);
    }
    if (definition.kind === "plantuml") {
      return this.plantuml(definition.latestUrl);
    }
    return this.npm(definition.latestUrl);
  }

  private async npm(url: string): Promise<string> {
    const response = await this.get(url);
    const body = (await response.json()) as NpmLatestResponse;
    return body.version;
  }

  private async drawio(url: string): Promise<string> {
    const response = await this.get(url);
    const body = (await response.json()) as GitHubLatestResponse;
    return body.tag_name.replace(/^v/, "");
  }

  private async plantuml(url: string): Promise<string> {
    const response = await this.get(url);
    const body = await response.text();
    const versions = [...body.matchAll(/<version>([^<]+)<\/version>/g)].map((it) => it[1]);
    const latest = versions.at(-1);
    if (latest === undefined) {
      throw new Error(`PlantUML metadata did not include versions: ${url}`);
    }
    return latest;
  }

  private async get(url: string): Promise<Response> {
    const headers: Record<string, string> = {
      accept: "application/json",
      "user-agent": "katana-render-runtime-release-tool",
    };
    const token = isGitHubApi(url) ? this.credentials() : undefined;
    if (token !== undefined) {
      headers.authorization = `Bearer ${token}`;
    }
    const response = await this.fetcher(url, {
      headers,
      // 認証付き応答のリダイレクトで別ホストへ秘密を送らない。
      ...(token === undefined ? {} : { redirect: "error" }),
    });
    if (!response.ok) {
      throw new Error(`Failed to fetch ${url}: ${response.status}`);
    }
    return response;
  }
}
