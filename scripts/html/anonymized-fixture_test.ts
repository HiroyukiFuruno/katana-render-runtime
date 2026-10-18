import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";

const fixture = fs.readFileSync(
  new URL("../../tests/fixtures/html/anonymized-requirements.html", import.meta.url),
  "utf8",
);

test("anonymized requirements retains its document layout and executable resources", () => {
  expect(fixture.match(/<\/?[a-zA-Z][^>]*>/g)?.length).toBe(1207);
  const hashes = {
    style: "02eded3ec6cf174f1ef53726f94d644133a09934d1e4a317984e29e1c6cb7e49",
    script: "cecc469558ca2e4f587d67b10a79abef2b0f1531dbd51933d7c241beba02f93e",
  };
  for (const [tag, expected] of Object.entries(hashes)) {
    const body = fixture.match(new RegExp(`<${tag}[^>]*>([\\s\\S]*?)</${tag}>`))?.[1];
    expect(body).toBeDefined();
    expect(
      createHash("sha256")
        .update(body ?? "")
        .digest("hex"),
    ).toBe(expected);
  }
});

test("anonymized requirements retains the ordered table of contents and all destinations", () => {
  const navigation = fixture.match(/<nav\b[^>]*>([\s\S]*?)<\/nav>/)?.[1] ?? "";
  const destinations = Array.from(navigation.matchAll(/href="#([^"]+)"/g), (match) => match[1]);
  expect(destinations).toEqual([
    "ch1",
    "s11",
    "s12",
    "s13",
    "s14",
    "s15",
    "ch2",
    "s21",
    "s22",
    "s23",
    "s24",
    "s25",
    "s26",
    "s27",
    "s28",
    "changelog",
  ]);
  for (const destination of destinations) {
    expect(fixture.match(new RegExp(`id="${destination}"`, "g"))?.length).toBe(1);
  }
  expect(navigation.match(/class="toc-item sub"/g)?.length).toBe(13);
});
