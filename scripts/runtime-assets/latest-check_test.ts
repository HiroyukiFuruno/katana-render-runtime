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

test("ZenUML latest は npm registry の version を読む", async () => {
  const fetchStub = new FetchStub(JSON.stringify({ version: "0.2.3" }));
  const client = new LatestVersionClient(fetchStub.handler());

  const latest = await client.latest(RuntimeAssetCatalog.byKind("mermaid-zenuml"));

  expect(latest).toBe("0.2.3");
  expect(fetchStub.requestedUrls).toEqual([
    "https://registry.npmjs.org/@mermaid-js/mermaid-zenuml/latest",
  ]);
});
