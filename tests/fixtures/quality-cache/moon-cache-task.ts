import { appendFileSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.env.CONTRACT_ROOT;
const phase = process.argv[2];
if (!root || !phase) throw new Error("Missing contract fixture environment");
appendFileSync(join(root, ".probe/events.jsonl"), `${JSON.stringify({ phase })}\n`);
const config: unknown = JSON.parse(readFileSync(join(root, ".probe/config.json"), "utf8"));
if (typeof config !== "object" || config === null || !("staticFailure" in config)) {
  throw new Error("Invalid contract config");
}
if (phase === "failure" || (phase === "static" && config.staticFailure)) {
  process.exit(1);
}
