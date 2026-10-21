import { validateFontProvenance } from "./coretext-font-provenance";
import type { JsonValue } from "./dynamic-state";

export interface FontEquivalenceResult {
  established: boolean;
  reason: string | null;
}

export interface CatalogFontAcceptanceResult {
  acceptance: boolean;
  fontIdentityEquivalence: "established" | "not-established";
}

export function catalogFontAcceptance(
  scores: readonly number[],
  allConditions: number,
  fontIdentityEstablished: boolean,
  rejected: readonly unknown[],
): CatalogFontAcceptanceResult {
  const accepted =
    allConditions === 84 &&
    scores.length === 84 &&
    scores.every((score) => Number.isFinite(score) && score >= 90) &&
    fontIdentityEstablished &&
    rejected.length === 0;
  return {
    acceptance: accepted,
    fontIdentityEquivalence: accepted ? "established" : "not-established",
  };
}

export function compareCatalogFontFaces(
  krrValue: JsonValue | undefined,
  chromeValue: JsonValue | undefined,
): FontEquivalenceResult {
  try {
    const krrFaces = validateFontProvenance(krrValue);
    const chromeFaces = validateFontProvenance(chromeValue);
    if (krrFaces.length === 0 || chromeFaces.length === 0)
      return { established: false, reason: "selected physical font set is empty" };
    if ([...krrFaces, ...chromeFaces].some((face) => face.coreTextDescriptor !== undefined))
      return { established: false, reason: "variable font instance identity is not established" };
    const identity = (faces: readonly { fileSha256: string; faceIndex: number }[]) =>
      [...new Set(faces.map((face) => `${face.fileSha256}:${face.faceIndex}`))].sort();
    const krrIdentity = identity(krrFaces);
    const chromeIdentity = identity(chromeFaces);
    if (JSON.stringify(krrIdentity) !== JSON.stringify(chromeIdentity))
      return { established: false, reason: "physical font file and face identities differ" };
    return { established: true, reason: null };
  } catch {
    return { established: false, reason: "physical font provenance is incomplete or invalid" };
  }
}
