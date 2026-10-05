import { expect, test } from "bun:test";
import { githubCredentials, LatestVersionClient, type RuntimeAssetFetch } from "./latest-check";
import { RuntimeAssetCatalog } from "./runtime-asset-common";

class FetchStub {
  public readonly requestedUrls: string[] = [];
  public readonly requests: RequestInit[] = [];

  constructor(private readonly body: string) {}

  handler(): RuntimeAssetFetch {
    return async (url, init) => {
      this.requestedUrls.push(url);
      this.requests.push(init);
      return new Response(this.body, {
        headers: { "content-type": "application/json" },
        status: 200,
      });
    };
  }
}

test("PlantUML latest は Maven metadata の最後の version を読む", async () => {
  const fetchStub = new FetchStub(
    "<metadata><versioning><versions><version>1.2026.1</version><version>1.2026.4</version></versions></versioning></metadata>",
  );
  const client = new LatestVersionClient(fetchStub.handler());

  const latest = await client.latest(RuntimeAssetCatalog.byKind("plantuml"));

  expect(latest).toBe("1.2026.4");
  expect(fetchStub.requestedUrls).toEqual([
    "https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/maven-metadata.xml",
  ]);
});

test("ZenUML latest は npm registry の version を読む", async () => {
  const fetchStub = new FetchStub(JSON.stringify({ version: "0.2.3" }));
  const client = new LatestVersionClient(fetchStub.handler());

  const latest = await client.latest(RuntimeAssetCatalog.byKind("mermaid-zenuml"));

  expect(latest).toBe("0.2.3");
  expect(fetchStub.requestedUrls).toEqual([
    "https://registry.npmjs.org/@mermaid-js/mermaid-zenuml/latest",
  ]);
});

test("ZenUML Core latest は npm registry の version を読む", async () => {
  const fetchStub = new FetchStub(JSON.stringify({ version: "3.47.9" }));
  const client = new LatestVersionClient(fetchStub.handler());

  const latest = await client.latest(RuntimeAssetCatalog.byKind("zenuml-core"));

  expect(latest).toBe("3.47.9");
  expect(fetchStub.requestedUrls).toEqual(["https://registry.npmjs.org/@zenuml/core/latest"]);
});

test("MathJax latest は npm registry の version を読む", async () => {
  const fetchStub = new FetchStub(JSON.stringify({ version: "4.1.2" }));
  const client = new LatestVersionClient(fetchStub.handler());

  const latest = await client.latest(RuntimeAssetCatalog.byKind("mathjax"));

  expect(latest).toBe("4.1.2");
  expect(fetchStub.requestedUrls).toEqual(["https://registry.npmjs.org/mathjax/latest"]);
});

test("GitHub latest だけに認証を渡し redirect を禁止する", async () => {
  const stub = new FetchStub(JSON.stringify({ tag_name: "v31.6.1" }));
  const client = new LatestVersionClient(stub.handler(), () => "unit-test-token");
  expect(await client.latest(RuntimeAssetCatalog.byKind("drawio"))).toBe("31.6.1");
  expect(new Headers(stub.requests[0]?.headers).get("authorization")).toBe(
    "Bearer unit-test-token",
  );
  expect(stub.requests[0]?.redirect).toBe("error");
});

test("npm と Maven では認証 provider を呼ばない", async () => {
  let reads = 0;
  const stub = new FetchStub("<version>1.2026.8</version>");
  const client = new LatestVersionClient(stub.handler(), () => {
    reads += 1;
    return "unit-test-token";
  });
  await client.latest(RuntimeAssetCatalog.byKind("plantuml"));
  expect(reads).toBe(0);
  expect(new Headers(stub.requests[0]?.headers).has("authorization")).toBe(false);
});

test("npm latest に認証を送らず provider も呼ばない", async () => {
  const stub = new FetchStub(JSON.stringify({ version: "12.0.0" }));
  const client = new LatestVersionClient(stub.handler(), () => {
    throw new Error("must not read credentials");
  });
  expect(await client.latest(RuntimeAssetCatalog.byKind("mermaid"))).toBe("12.0.0");
  expect(new Headers(stub.requests[0]?.headers).has("authorization")).toBe(false);
});

test("HTTPS GitHub API の正確な origin 以外は認証を読まない", async () => {
  const stub = new FetchStub(JSON.stringify({ tag_name: "v31.6.1" }));
  const client = new LatestVersionClient(stub.handler(), () => {
    throw new Error("must not read credentials");
  });
  for (const latestUrl of [
    "http://api.github.com/repos/jgraph/drawio/releases/latest",
    "https://api.github.com.evil.example/releases/latest",
    "https://api.github.com:8443/repos/jgraph/drawio/releases/latest",
    "https://user@api.github.com/repos/jgraph/drawio/releases/latest",
    "https://github.com/repos/jgraph/drawio/releases/latest",
  ]) {
    await client.latest({ ...RuntimeAssetCatalog.byKind("drawio"), latestUrl });
  }
  expect(stub.requests.every((init) => !new Headers(init.headers).has("authorization"))).toBe(true);
});

test("注入 fetcher の既定は実環境認証を取得しない", async () => {
  const stub = new FetchStub(JSON.stringify({ tag_name: "v31.6.1" }));
  const client = new LatestVersionClient(stub.handler());
  expect(await client.latest(RuntimeAssetCatalog.byKind("drawio"))).toBe("31.6.1");
  expect(new Headers(stub.requests[0]?.headers).has("authorization")).toBe(false);
});

test("認証無しでも公開 API を利用し HTTP 失敗は隠さない", async () => {
  const requests: RequestInit[] = [];
  const client = new LatestVersionClient(
    async (_url, init) => {
      requests.push(init);
      return new Response("forbidden", { status: 403 });
    },
    () => undefined,
  );
  await expect(client.latest(RuntimeAssetCatalog.byKind("drawio"))).rejects.toThrow(": 403");
  expect(new Headers(requests[0]?.headers).has("authorization")).toBe(false);
});

test("既存環境認証は GH_TOKEN、GITHUB_TOKEN、gh auth の順で利用する", () => {
  const readToken = () => "unit-gh-auth-token";
  expect(
    githubCredentials({ GH_TOKEN: "unit-gh-token", GITHUB_TOKEN: "unit-github-token" }, readToken),
  ).toBe("unit-gh-token");
  expect(githubCredentials({ GH_TOKEN: "", GITHUB_TOKEN: "unit-github-token" }, readToken)).toBe(
    "unit-github-token",
  );
  expect(githubCredentials({}, readToken)).toBe("unit-gh-auth-token");
  expect(githubCredentials({}, () => undefined)).toBeUndefined();
});

test("環境認証があれば gh auth を起動しない", () => {
  expect(
    githubCredentials({ GITHUB_TOKEN: "unit-env-token" }, () => {
      throw new Error("must not call gh");
    }),
  ).toBe("unit-env-token");
});

test("不正な認証は値を露出せず拒否する", () => {
  for (const token of [
    "unit-token\r\ninvalid",
    "unit-token\0invalid",
    "unit-token\x7finvalid",
    "unit-token日本語",
  ]) {
    expect(() => githubCredentials({ GH_TOKEN: token }, () => undefined)).toThrow(
      "GitHub credentials must contain only visible ASCII characters",
    );
  }
});
