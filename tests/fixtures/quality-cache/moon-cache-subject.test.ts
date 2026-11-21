import { expect, test } from "bun:test";
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { increment } from "./moon-cache-subject";

const root = process.env.CONTRACT_ROOT;
const phase = process.env.CONTRACT_PHASE;
if (!root || !phase) throw new Error("Missing contract fixture environment");
const config: unknown = JSON.parse(readFileSync(join(root, ".probe/config.json"), "utf8"));
const lock: unknown = JSON.parse(readFileSync(join(root, ".probe/lock.json"), "utf8"));
if (typeof config !== "object" || config === null || !("revision" in config)) {
  throw new Error("Invalid contract config");
}
if (typeof lock !== "object" || lock === null || !("revision" in lock)) {
  throw new Error("Invalid contract lock");
}
mkdirSync(join(root, ".outputs"), { recursive: true });
appendFileSync(join(root, ".probe/events.jsonl"), `${JSON.stringify({ phase })}\n`);
writeFileSync(
  join(root, `.outputs/${phase}.json`),
  JSON.stringify({ config, lock, environment: process.env.CACHE_CONTRACT_ENV }),
);

test("real Bun test executes the declared input", () => {
  expect(increment(1)).toBe(2);
  expect(config.revision).toBeString();
  expect(lock.revision).toBeString();
});
