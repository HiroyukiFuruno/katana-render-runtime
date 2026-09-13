import { expect, test } from "bun:test";
import { runInNewContext } from "node:vm";
import type { PageHandle } from "../mermaid/official-renderer-types";
import { OfficialDrawioSourceFonts } from "./official-source-fonts";

const rockSalt = "https://fonts.googleapis.com/css?family=Rock+Salt";
const architects = "https://fonts.googleapis.com/css?family=Architects+Daughter";

function sourceWithFonts(...urls: string[]): string {
  return `<mxGraphModel><root>${urls
    .map((url) => `<mxCell style="fontSource=${encodeURIComponent(url)};fontSize=17;"/>`)
    .join("")}</root></mxGraphModel>`;
}

class FontLink extends EventTarget {
  rel = "";
  href = "";
}

class FontPage implements PageHandle {
  readonly links: FontLink[] = [];

  async evaluate<Result, Argument>(
    callback: (argument: Argument) => Result | Promise<Result>,
    argument?: Argument,
  ): Promise<Result> {
    const execute = runInNewContext(`(${callback.toString()})`, {
      document: {
        createElement: (tag: string) => {
          expect(tag).toBe("link");
          return new FontLink();
        },
        head: { appendChild: (link: FontLink) => this.links.push(link) },
      },
    }) as (value?: Argument) => Result | Promise<Result>;
    return await execute(argument);
  }

  async setContent(): Promise<void> {}
  async addScriptTag(): Promise<undefined> {}
  async close(): Promise<void> {}
  locator() {
    return { screenshot: async (): Promise<undefined> => undefined };
  }
}

test("source fontSource のURLを復号し順序を維持して重複を除去する", () => {
  expect(OfficialDrawioSourceFonts.urls(sourceWithFonts(rockSalt, architects, rockSalt))).toEqual([
    rockSalt,
    architects,
  ]);
});

test("XML属性のamp entityを復号してHTTPS stylesheet URLを保持する", () => {
  const source = '<mxCell style="fontSource=https://example.test/font?family=Demo&amp;v=2;"/>';
  expect(OfficialDrawioSourceFonts.urls(source)).toEqual([
    "https://example.test/font?family=Demo&v=2",
  ]);
});

test("single quote のXML属性でも先頭fontSourceを取得する", () => {
  const source = `<mxCell style='fontSource=${encodeURIComponent(rockSalt)};'/>`;
  expect(OfficialDrawioSourceFonts.urls(source)).toEqual([rockSalt]);
});

test("fontSourceがない入力と不正URL・非HTTPSはロード対象にしない", () => {
  expect(OfficialDrawioSourceFonts.urls("<mxGraphModel/>")).toEqual([]);
  expect(
    OfficialDrawioSourceFonts.urls(
      '<mxCell style="fontSource=%ZZ;"/><mxCell style="fontSource=javascript%3Aalert(1);"/>',
    ),
  ).toEqual([]);
  expect(OfficialDrawioSourceFonts.urls(sourceWithFonts("http://example.test/font"))).toEqual([]);
});

test("全stylesheetのload完了までinstallを解決しない", async () => {
  const page = new FontPage();
  let complete = false;
  const installation = OfficialDrawioSourceFonts.install(
    page,
    sourceWithFonts(rockSalt, architects),
  ).then(() => {
    complete = true;
  });
  expect(page.links.map((link) => [link.rel, link.href])).toEqual([
    ["stylesheet", rockSalt],
    ["stylesheet", architects],
  ]);
  await Promise.resolve();
  expect(complete).toBe(false);
  page.links[0]?.dispatchEvent(new Event("load"));
  await Promise.resolve();
  expect(complete).toBe(false);
  page.links[1]?.dispatchEvent(new Event("load"));
  await installation;
  expect(complete).toBe(true);
});

test("stylesheetのerrorを成功扱いせず呼出側へ返す", async () => {
  const page = new FontPage();
  const installation = OfficialDrawioSourceFonts.install(page, sourceWithFonts(rockSalt));
  const failure = installation.then(
    () => "unexpected success",
    (error: Error) => error.message,
  );
  expect(page.links).toHaveLength(1);
  page.links[0]?.dispatchEvent(new Event("error"));
  expect(await failure).toContain("failed to load Draw.io source font stylesheet");
});

test("重複URLは一度だけロードしfontSourceなしはlinkを追加しない", async () => {
  const page = new FontPage();
  await OfficialDrawioSourceFonts.install(page, "<mxGraphModel/>");
  expect(page.links).toHaveLength(0);
  const installation = OfficialDrawioSourceFonts.install(page, sourceWithFonts(rockSalt, rockSalt));
  expect(page.links).toHaveLength(1);
  page.links[0]?.dispatchEvent(new Event("load"));
  await installation;
});
