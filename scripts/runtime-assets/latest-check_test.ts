import { expect, test } from "bun:test";
import { LatestVersionClient, type RuntimeAssetFetch } from "./latest-check";
import { RuntimeAssetCatalog } from "./runtime-asset-common";

class FetchStub {
  public readonly requestedUrls: string[] = [];

  constructor(private readonly body: string) {}

  handler(): RuntimeAssetFetch {
    return async (url) => {
      this.requestedUrls.push(url);
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

test("MathJax latest は npm registry の version を読む", async () => {
  const fetchStub = new FetchStub(JSON.stringify({ version: "4.1.2" }));
  const client = new LatestVersionClient(fetchStub.handler());

  const latest = await client.latest(RuntimeAssetCatalog.byKind("mathjax"));

  expect(latest).toBe("4.1.2");
  expect(fetchStub.requestedUrls).toEqual(["https://registry.npmjs.org/mathjax/latest"]);
});
