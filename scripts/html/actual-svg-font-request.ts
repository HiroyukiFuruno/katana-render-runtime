import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { z } from "zod";

const absolute = z
  .string()
  .min(1)
  .refine((value) => path.isAbsolute(value) && path.normalize(value) === value);
const sha = z.string().regex(/^[a-f0-9]{64}$/);
export const requestSchema = z
  .object({
    schema: z.literal("krr-actual-svg-font-request-v1"),
    input: z.object({ path: absolute, sha256: sha }).strict(),
    observer: z.object({ path: absolute, sha256: sha }).strict(),
    viewport: z
      .object({
        width: z.number().int().positive().max(16384),
        height: z.number().int().positive().max(16384),
        deviceScaleFactor: z.number().finite().positive().max(4),
      })
      .strict(),
    selector: z.string().min(1).max(512),
    output: absolute,
  })
  .strict();
export type SvgFontRequest = z.infer<typeof requestSchema>;

export function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

export function assertPinnedFile(pin: { path: string; sha256: string }): void {
  if (
    !fs.statSync(pin.path).isFile() ||
    fs.realpathSync(pin.path) !== pin.path ||
    sha256(fs.readFileSync(pin.path)) !== pin.sha256
  )
    throw new Error("pinned file identity differs");
}

export function validateRequest(value: unknown): SvgFontRequest {
  const request = requestSchema.parse(value);
  assertPinnedFile(request.input);
  assertPinnedFile(request.observer);
  const parent = path.dirname(request.output);
  if (fs.realpathSync(parent) !== parent || fs.existsSync(request.output))
    throw new Error("output must be a fresh path under a canonical parent");
  return request;
}

export function writeFresh(file: string, bytes: Uint8Array | string): void {
  fs.writeFileSync(file, bytes, { flag: "wx" });
}
