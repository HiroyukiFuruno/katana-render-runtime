import { type SpawnSyncReturns, spawnSync } from "node:child_process";
import fs from "node:fs";

export class DrawioWarExtractor {
  private static readonly DRAWIO_SCRIPT_PATHS = ["js/viewer.min.js", "js/app.min.js"];
  private static readonly MAX_EXTRACT_BYTES = 64 * 1024 * 1024;

  extract(archive: string, target: string, sourceLabel = archive) {
    const errors: string[] = [];
    for (const scriptPath of DrawioWarExtractor.DRAWIO_SCRIPT_PATHS) {
      const extracted = this.unzip(archive, scriptPath);
      if (extracted.status === 0) {
        fs.writeFileSync(target, extracted.stdout);
        return;
      }
      errors.push(`${scriptPath}: ${this.errorDetail(extracted)}`);
    }
    throw new Error(`Failed to extract Draw.io runtime from ${sourceLabel}: ${errors.join("; ")}`);
  }

  private unzip(archive: string, scriptPath: string): SpawnSyncReturns<Buffer> {
    return spawnSync("unzip", ["-p", archive, scriptPath], {
      encoding: "buffer",
      maxBuffer: DrawioWarExtractor.MAX_EXTRACT_BYTES,
    });
  }

  private errorDetail(result: SpawnSyncReturns<Buffer>): string {
    if (result.error !== undefined) {
      return result.error.message;
    }
    const stderr = result.stderr.toString("utf8").trim();
    return stderr.length > 0 ? stderr : `exit status ${result.status}`;
  }
}
