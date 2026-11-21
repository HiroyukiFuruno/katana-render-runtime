import type { ChromiumFontInstance } from "./chromium-font-trace";
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

const openTypeTag = (tag: string): number => {
  if (!/^[\x20-\x7e]{4}$/.test(tag)) throw new Error("invalid OpenType variation tag");
  return (
    ((tag.charCodeAt(0) << 24) |
      (tag.charCodeAt(1) << 16) |
      (tag.charCodeAt(2) << 8) |
      tag.charCodeAt(3)) >>>
    0
  );
};

function coordinatesMatch(
  instance: ChromiumFontInstance,
  coordinates: readonly { tag: string; value: number }[],
): boolean {
  const expected = new Map(
    instance.coordinates.map((coordinate) => [coordinate.identifier, coordinate.value]),
  );
  const observed = new Map<number, number>();
  for (const coordinate of coordinates) {
    const identifier = openTypeTag(coordinate.tag);
    if (observed.has(identifier) || !Number.isFinite(coordinate.value)) return false;
    observed.set(identifier, coordinate.value);
  }
  if (instance.axes.length !== observed.size || instance.coordinates.length !== observed.size)
    return false;
  return (
    instance.axes.every((axis) => observed.has(axis.identifier)) &&
    [...expected].every(([identifier, value]) => Object.is(observed.get(identifier), value))
  );
}

function validChromiumInstance(instance: ChromiumFontInstance): boolean {
  const axisIds = new Set<number>();
  for (const axis of instance.axes) {
    if (
      !Number.isInteger(axis.identifier) ||
      !Number.isFinite(axis.minimum) ||
      !Number.isFinite(axis.maximum) ||
      axis.minimum > axis.maximum ||
      axisIds.has(axis.identifier)
    )
      return false;
    axisIds.add(axis.identifier);
  }
  const coordinateIds = new Set<number>();
  for (const coordinate of instance.coordinates) {
    const axis = instance.axes.find((item) => item.identifier === coordinate.identifier);
    if (
      !Number.isInteger(coordinate.identifier) ||
      !Number.isFinite(coordinate.value) ||
      axis === undefined ||
      coordinateIds.has(coordinate.identifier) ||
      coordinate.value < axis.minimum ||
      coordinate.value > axis.maximum
    )
      return false;
    coordinateIds.add(coordinate.identifier);
  }
  if (instance.fvarTablePresent) return axisIds.size > 0 && coordinateIds.size === axisIds.size;
  return axisIds.size === 0 && coordinateIds.size === 0;
}

function coordinateSignature(coordinates: readonly { tag: string; value: number }[]): string {
  return JSON.stringify(
    coordinates
      .map((coordinate) => [openTypeTag(coordinate.tag), coordinate.value])
      .sort(([left], [right]) => Number(left) - Number(right)),
  );
}

function instanceSignature(instance: ChromiumFontInstance): string {
  return JSON.stringify([
    instance.physicalFileSHA256,
    instance.physicalFaceTtcIndex,
    instance.coordinates
      .map((coordinate) => [coordinate.identifier, coordinate.value])
      .sort(([left], [right]) => Number(left) - Number(right)),
  ]);
}

export function compareChromiumFontInstanceToLayoutRuns(
  instance: ChromiumFontInstance,
  krrValue: JsonValue | undefined,
): FontEquivalenceResult {
  try {
    const faces = validateFontProvenance(krrValue);
    const matchingRuns = faces
      .filter(
        (face) =>
          face.fileSha256 === instance.physicalFileSHA256 &&
          face.faceIndex === instance.physicalFaceTtcIndex,
      )
      .flatMap((face) => face.layoutRuns ?? []);
    if (matchingRuns.length === 0)
      return { established: false, reason: "matching physical font has no observed layout run" };
    if (!validChromiumInstance(instance))
      return { established: false, reason: "Chromium font instance is incomplete or invalid" };
    if (
      !matchingRuns.some((run) => coordinatesMatch(instance, run.derivedAppliedDesignCoordinates))
    )
      return { established: false, reason: "applied design coordinates differ" };
    return { established: true, reason: null };
  } catch {
    return { established: false, reason: "font identity or layout provenance is incomplete" };
  }
}

export function compareChromiumFontInstancesToLayoutRuns(
  instances: readonly ChromiumFontInstance[],
  krrValue: JsonValue | undefined,
): FontEquivalenceResult {
  try {
    if (instances.length === 0 || instances.some((instance) => !validChromiumInstance(instance)))
      return { established: false, reason: "Chromium font instance set is incomplete or invalid" };
    const faces = validateFontProvenance(krrValue);
    if (faces.some((face) => face.layoutRuns === undefined || face.layoutRuns.length === 0))
      return { established: false, reason: "KRR font face has no observed layout run" };
    const observed = new Set(instances.map(instanceSignature));
    const expected = new Set(
      faces.flatMap((face) =>
        (face.layoutRuns ?? []).map((run) =>
          JSON.stringify([
            face.fileSha256,
            face.faceIndex,
            JSON.parse(coordinateSignature(run.derivedAppliedDesignCoordinates)),
          ]),
        ),
      ),
    );
    if (
      expected.size === 0 ||
      observed.size !== expected.size ||
      [...observed].some((signature) => !expected.has(signature))
    )
      return { established: false, reason: "Chrome and KRR font instance sets differ" };
    return { established: true, reason: null };
  } catch {
    return { established: false, reason: "font identity or layout provenance is incomplete" };
  }
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
