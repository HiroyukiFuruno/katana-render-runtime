import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { FixtureRepository } from "./diagram_update_fixtures";

test("zenuml 直接フェンスを Mermaid 公式参照用の source として読める", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-mermaid-fixtures-"));
  try {
    fs.writeFileSync(
      path.join(root, "29-zenuml-direct.md"),
      [
        "# 29. ZenUML Direct Fence",
        "",
        "~~~zenuml",
        "title Order Service",
        "@Actor Client",
        "Client.method()",
        "~~~",
        "",
      ].join("\n"),
      "utf8",
    );

    const [fixture] = new FixtureRepository(root).list();

    expect(fixture?.source).toBe("zenuml\ntitle Order Service\n@Actor Client\nClient.method()");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
