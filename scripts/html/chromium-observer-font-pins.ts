import path from "node:path";
import { z } from "zod";
import type { ChromiumFontInstance, ChromiumFontTrace } from "./chromium-font-trace";
import type { JsonValue } from "./dynamic-state";

const observerFaceSchema = z.strictObject({
  postScriptName: z.string().min(1),
  filePath: z.string().refine(path.isAbsolute),
  fileSha256: z.string().regex(/^[a-f0-9]{64}$/),
  faceIndex: z.number().int().min(0).max(4_294_967_295),
});

export type ChromeObserverFontFace = z.infer<typeof observerFaceSchema>;

interface SelectedFontName {
  postScriptName: string;
  isCustomFont: boolean;
}

function uniqueObservedFace(
  postScriptName: string,
  instances: readonly ChromiumFontInstance[],
): ChromeObserverFontFace | undefined {
  const matches = instances.filter((instance) => instance.postScriptName === postScriptName);
  const first = matches[0];
  if (
    !first?.physicalFilePath ||
    matches.some(
      (instance) =>
        instance.physicalFilePath !== first.physicalFilePath ||
        instance.physicalFileSHA256 !== first.physicalFileSHA256 ||
        instance.physicalFaceTtcIndex !== first.physicalFaceTtcIndex,
    )
  )
    return undefined;
  return observerFaceSchema.parse({
    postScriptName,
    filePath: first.physicalFilePath,
    fileSha256: first.physicalFileSHA256,
    faceIndex: first.physicalFaceTtcIndex,
  });
}

export function selectChromeObserverFontFaces(
  selectedFonts: readonly SelectedFontName[],
  instances: readonly ChromiumFontInstance[],
): ChromeObserverFontFace[] {
  const frequencies = new Map<string, number>();
  for (const { postScriptName } of selectedFonts)
    frequencies.set(postScriptName, (frequencies.get(postScriptName) ?? 0) + 1);
  const faces: ChromeObserverFontFace[] = [];
  for (const selected of selectedFonts) {
    if (selected.isCustomFont || frequencies.get(selected.postScriptName) !== 1) continue;
    const face = uniqueObservedFace(selected.postScriptName, instances);
    if (face) faces.push(face);
  }
  return faces;
}

export function parseChromeObserverFontFaces(value: unknown): ChromeObserverFontFace[] {
  if (value === undefined) return [];
  const parsed = z.array(observerFaceSchema).safeParse(value);
  if (!parsed.success) throw new Error("Invalid Chrome observer font face bindings");
  if (new Set(parsed.data.map((face) => face.postScriptName)).size !== parsed.data.length)
    throw new Error("Duplicate Chrome observer font face bindings");
  return parsed.data;
}

const OBSERVER_PROVENANCE_SUFFIX =
  "; Chrome retained font observation; exact PostScript-name match; physical file SHA and face index verified";

export function assertChromeObserverFontFaceBindings(
  requested: unknown,
  selectedFonts: readonly (SelectedFontName & { familyName: string })[],
  trace: ChromiumFontTrace | undefined,
  response: readonly Record<string, JsonValue>[],
): void {
  const faces = parseChromeObserverFontFaces(requested);
  if (faces.length > 0) {
    const actual = trace && selectChromeObserverFontFaces(selectedFonts, trace.instances);
    if (!actual || JSON.stringify(faces) !== JSON.stringify(actual))
      throw new Error("Chrome observer font face bindings differ from the retained trace");
  }
  for (const [index, font] of response.entries()) {
    if (typeof font.source !== "string" || !font.source.endsWith(OBSERVER_PROVENANCE_SUFFIX))
      continue;
    const selected = selectedFonts[index];
    const face = faces.find((item) => item.postScriptName === selected?.postScriptName);
    if (
      !selected ||
      selected.isCustomFont ||
      !face ||
      font.scope !== "capture" ||
      font.family !== selected.familyName ||
      font.filePath !== face.filePath ||
      font.fileSha256 !== face.fileSha256 ||
      font.faceIndex !== face.faceIndex
    )
      throw new Error("Chrome observer font probe result differs from its selected physical face");
  }
}
