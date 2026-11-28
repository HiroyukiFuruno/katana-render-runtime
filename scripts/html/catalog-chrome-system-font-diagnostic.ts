import { randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { Browser } from "playwright";
import { withObserverBrowser } from "./chromium-font-capture";
import type { ChromiumFontInstance } from "./chromium-font-trace";
import { type CatalogCase, loadFeatureCatalog } from "./feature-catalog";
import { sha256File } from "./reference-capture";
import { createCatalogStage90ConditionPlan } from "./stage-score-suite";

const CONDITION_IDS = new Set([
  "decoration/translucent/960x720@1",
  "events/expected-state/960x720@1",
]);
const SYSTEM_ROOTS = ["/System/Library/", "/Library/Apple/System/Library/"];
const SHA256 = /^[a-f0-9]{64}$/;
const FONT_FILE = /^[\p{L}\p{M}\p{N} ._-]{1,124}\.(?:ttf|ttc|otf|otc)$/iu;
const POSTSCRIPT_NAME = /^[a-z0-9._-]{1,128}$/i;
const MAX_AXES = 32;
const MAX_GLYPHS = 1_000_000;

type Projection = {
  conditionId: string;
  fonts: SystemFont[];
  unprojectedCount: number;
};

type SystemFont = {
  basename: string;
  postScriptName: string;
  faceIndex: number;
  axes: Array<{ tag: number; coordinate: number }>;
  glyphCount?: number;
};

type SelectedFont = { postScriptName: string; isCustomFont: boolean; glyphCount: number };
type CaptureReference = (
  browser: Browser,
  entry: CatalogCase,
  tuple: ReturnType<typeof createCatalogStage90ConditionPlan>["conditions"][number],
  screenshot: string,
) => Promise<readonly SelectedFont[]>;

export function parseSystemFontObserver(argv: readonly string[]): { path: string; sha256: string } {
  const [executable, sha256] = argv;
  if (
    argv.length !== 2 ||
    !executable ||
    !sha256 ||
    !path.isAbsolute(executable) ||
    !SHA256.test(sha256)
  )
    throw new Error("invalid observer arguments");
  return { path: executable, sha256 };
}

function systemFont(
  instance: ChromiumFontInstance,
  selectedFonts: readonly SelectedFont[],
): SystemFont | undefined {
  const filePath = instance.physicalFilePath;
  if (!filePath || !SYSTEM_ROOTS.some((root) => filePath.startsWith(root))) return undefined;
  const basename = path.basename(filePath);
  if (!FONT_FILE.test(basename) || !POSTSCRIPT_NAME.test(instance.postScriptName)) return undefined;
  if (!Number.isSafeInteger(instance.physicalFaceTtcIndex) || instance.physicalFaceTtcIndex < 0)
    return undefined;
  if (
    instance.coordinates.length > MAX_AXES ||
    instance.coordinates.some(
      ({ identifier, value }) =>
        !Number.isInteger(identifier) ||
        identifier < 0 ||
        identifier > 0xffffffff ||
        !Number.isFinite(value),
    )
  )
    return undefined;
  const matches = selectedFonts.filter(
    (selected) => !selected.isCustomFont && selected.postScriptName === instance.postScriptName,
  );
  const [match] = matches;
  const glyphCount =
    matches.length === 1 && match && match.glyphCount > 0 && match.glyphCount <= MAX_GLYPHS
      ? match.glyphCount
      : undefined;
  return {
    basename,
    postScriptName: instance.postScriptName,
    faceIndex: instance.physicalFaceTtcIndex,
    axes: instance.coordinates.map(({ identifier, value }) => ({
      tag: identifier,
      coordinate: value,
    })),
    ...(glyphCount === undefined ? {} : { glyphCount }),
  };
}

async function capture(
  observer: { path: string; sha256: string },
  captureReference: CaptureReference,
): Promise<Projection[]> {
  const catalog = loadFeatureCatalog();
  const entries = [...catalog.visualAspects, ...catalog.scriptCases];
  const tuples = createCatalogStage90ConditionPlan().conditions.filter((tuple) =>
    CONDITION_IDS.has(tuple.conditionId),
  );
  if (tuples.length !== CONDITION_IDS.size) throw new Error("exact catalog conditions are absent");
  const directory = fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()), "krr-system-font-"));
  try {
    const projected: Projection[] = [];
    for (const tuple of tuples) {
      const entry = entries.find((candidate) => candidate.id === tuple.caseId);
      if (!entry || sha256File(tuple.fixturePath) !== tuple.sourceSha256)
        throw new Error("catalog fixture pin differs");
      const screenshot = path.join(directory, `${tuple.caseId}.png`);
      const observed = await withObserverBrowser({
        executablePath: observer.path,
        executableSha256: observer.sha256,
        tracePath: `${screenshot}.trace.json`,
        sessionId: randomUUID(),
        capture: (browser) => captureReference(browser, entry, tuple, screenshot),
      });
      const instances = observed.trace.instances;
      const fonts = instances
        .map((font) =>
          systemFont(
            font,
            instances.some(
              (other) => other !== font && other.postScriptName === font.postScriptName,
            )
              ? []
              : observed.value,
          ),
        )
        .filter((font) => font !== undefined);
      projected.push({
        conditionId: tuple.conditionId,
        fonts,
        unprojectedCount: instances.length - fonts.length,
      });
    }
    return projected;
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

export async function reportCatalogSystemFonts(
  observer: { path: string; sha256: string },
  captureReference: CaptureReference,
): Promise<void> {
  try {
    const conditions = await capture(observer, captureReference);
    process.stdout.write(`${JSON.stringify({ conditions })}\n`);
  } catch {
    throw new Error("macOS system font observation failed");
  }
}
