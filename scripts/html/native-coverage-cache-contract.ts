import { z } from "zod";
import { MOON_QUALITY_INPUTS } from "./moon-quality-inputs";

const DIRECTORY = "tmp/quality/standalone-performance-coverage";
const PRODUCERS = {
  "unit-test": {
    producer: "root",
    recipe: "root-test-and-coverage",
    deps: ["performance-coverage"],
    files: ["root.json", "root-local-result.json"],
  },
  "performance-coverage": {
    producer: "performance",
    recipe: "performance-harness-coverage",
    deps: ["heavy-native", "check"],
    files: ["baseline.json", "candidate.json", "performance-local-result.json"],
  },
} as const;

export function nativeCoverageCacheTask(name: string, task: Record<string, unknown>): boolean {
  return (
    name === "performance-coverage" ||
    (name === "unit-test" &&
      Array.isArray(task.command) &&
      task.command.join("\0") === "just\0root-test-and-coverage")
  );
}

export function validateNativeCoverageCacheTask(
  config: Record<string, unknown>,
  name: string,
  task: Record<string, unknown>,
): void {
  if (name !== "unit-test" && name !== "performance-coverage") {
    throw new Error("Unknown native coverage producer task");
  }
  const expected = PRODUCERS[name];
  z.object({
    cache: z.literal(false),
    runInCI: z.literal("always"),
    shell: z.literal(false),
  }).parse(config.taskOptions);
  z.object({
    command: z.tuple([z.literal("just"), z.literal(expected.recipe)]),
    deps: z.array(z.string()).refine((values) => values.join("\0") === expected.deps.join("\0")),
    inputs: z
      .array(z.string())
      .refine((values) => values.join("\0") === MOON_QUALITY_INPUTS.join("\0")),
    outputs: z
      .array(z.string())
      .refine(
        (values) =>
          values.join("\0") === expected.files.map((file) => `${DIRECTORY}/${file}`).join("\0"),
      ),
    options: z.object({ cache: z.literal("local") }).strict(),
    checks: z.tuple([
      z
        .object({
          check: z.literal("fingerprint"),
          script: z.literal(`just native-coverage-cache-key ${expected.producer}`),
          hash: z.literal("stdout"),
        })
        .strict(),
    ]),
  })
    .strict()
    .parse(task);
}
