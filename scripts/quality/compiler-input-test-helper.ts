import type { spyOn } from "bun:test";
import fs from "node:fs";
import path from "node:path";

export function changedCompilerInputRead(
  root: string,
  observe: typeof spyOn,
): { mockRestore: () => void } {
  const target = path.join(root, "node_modules/typescript/lib/tsc.js");
  const originalRead = fs.readFileSync;
  return observe(fs, "readFileSync").mockImplementation(
    new Proxy(originalRead, {
      apply(targetFunction, receiver: unknown, args: unknown[]) {
        const bytes: unknown = Reflect.apply(targetFunction, receiver, args);
        if (String(args[0]) !== target) return bytes;
        if (typeof bytes === "string") return `${bytes}\n`;
        if (Buffer.isBuffer(bytes)) return Buffer.concat([bytes, Buffer.from("\n")]);
        return bytes;
      },
    }),
  );
}
