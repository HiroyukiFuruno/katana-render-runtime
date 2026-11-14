import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { type ChromiumFontTrace, readChromiumFontTrace } from "./chromium-font-trace";
import { sha256File } from "./reference-capture";

const sha = z.string().regex(/^[a-f0-9]{64}$/);
const bindingSchema = z
  .object({
    schema: z.literal("krr-chromium-font-capture-v1"),
    conditionId: z.string().min(1),
    sourceSha256: sha,
    viewport: z
      .object({
        width: z.number().int().positive(),
        height: z.number().int().positive(),
        scale: z.number().finite().positive(),
      })
      .strict(),
    browserExecutablePath: z.string().min(1),
    browserExecutableSha256: sha,
    capturePath: z.string().min(1),
    captureSha256: sha,
    tracePath: z.string().min(1),
    traceSha256: sha,
    sessionId: z.string().min(1),
    argv: z.array(z.string()).min(5),
    processId: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
    actualExitCode: z.literal(0),
  })
  .strict();

export type ChromiumFontBindingExpected = Pick<
  z.infer<typeof bindingSchema>,
  | "conditionId"
  | "sourceSha256"
  | "viewport"
  | "browserExecutablePath"
  | "browserExecutableSha256"
  | "capturePath"
  | "captureSha256"
>;

function assertPhysicalPin(file: string, digest: string): void {
  if (
    !path.isAbsolute(file) ||
    fs.realpathSync(file) !== file ||
    fs.lstatSync(file).isSymbolicLink() ||
    !fs.statSync(file).isFile() ||
    sha256File(file) !== digest
  )
    throw new Error("Chromium font observation physical pin differs");
}

export function readChromiumFontBinding(
  value: unknown,
  expected: ChromiumFontBindingExpected,
): ChromiumFontTrace {
  const binding = bindingSchema.parse(value);
  const fields = [
    "conditionId",
    "sourceSha256",
    "browserExecutablePath",
    "browserExecutableSha256",
    "capturePath",
    "captureSha256",
  ] as const;
  if (
    fields.some((field) => binding[field] !== expected[field]) ||
    binding.viewport.width !== expected.viewport.width ||
    binding.viewport.height !== expected.viewport.height ||
    binding.viewport.scale !== expected.viewport.scale
  )
    throw new Error("Chromium font observation condition binding differs");
  assertPhysicalPin(binding.browserExecutablePath, binding.browserExecutableSha256);
  assertPhysicalPin(binding.capturePath, binding.captureSha256);
  assertPhysicalPin(binding.tracePath, binding.traceSha256);
  if (fs.realpathSync(binding.argv[0] as string) !== binding.browserExecutablePath)
    throw new Error("Chromium font observation process executable differs");
  const required = [
    "--trace-startup=katana.font-observer",
    "--trace-startup-format=json",
    `--trace-startup-file=${binding.tracePath}`,
    `--katana-actual-font-observer-session=${binding.sessionId}`,
  ];
  if (required.some((flag) => binding.argv.filter((argument) => argument === flag).length !== 1))
    throw new Error("Chromium font observation process arguments differ");
  return readChromiumFontTrace({
    tracePath: binding.tracePath,
    traceSha256: binding.traceSha256,
    sessionId: binding.sessionId,
  });
}
