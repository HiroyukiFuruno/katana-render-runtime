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

type FontInstanceVariation = ReadonlyArray<readonly [number, number]>;

export interface FontInstanceSetDifference {
  chromeCount: number;
  krrCount: number;
  groups: {
    faceOrdinal: number;
    faceIndex: number;
    chromeOnly: ReadonlyArray<FontInstanceVariation>;
    krrOnly: ReadonlyArray<FontInstanceVariation>;
  }[];
  truncated: boolean;
}

const MAX_DIFFERENCE_FACE_GROUPS = 16;
const MAX_DIFFERENCE_VARIATIONS = 16;
const MAX_DIFFERENCE_AXES = 32;

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

interface DifferenceGroup {
  faceIndex: number;
  chrome: Map<string, FontInstanceVariation>;
  krr: Map<string, FontInstanceVariation>;
}

function signatureProjection(signature: string): {
  physicalKey: string;
  faceIndex: number;
  variation: FontInstanceVariation;
} {
  const value: unknown = JSON.parse(signature);
  if (
    !Array.isArray(value) ||
    typeof value[0] !== "string" ||
    !Number.isSafeInteger(value[1]) ||
    !Array.isArray(value[2]) ||
    !value[2].every(
      (axis) =>
        Array.isArray(axis) &&
        axis.length === 2 &&
        Number.isInteger(axis[0]) &&
        Number.isFinite(axis[1]),
    )
  )
    throw new Error("invalid internal font instance signature");
  return {
    physicalKey: `${value[0]}:${value[1]}`,
    faceIndex: value[1],
    variation: value[2].map(([identifier, coordinate]) => [identifier, coordinate] as const),
  };
}

function boundedVariations(
  variations: Iterable<FontInstanceVariation>,
  state: { truncated: boolean },
): FontInstanceVariation[] {
  const result: FontInstanceVariation[] = [];
  for (const variation of variations) {
    if (result.length === MAX_DIFFERENCE_VARIATIONS) {
      state.truncated = true;
      break;
    }
    if (variation.length > MAX_DIFFERENCE_AXES) state.truncated = true;
    result.push(variation.slice(0, MAX_DIFFERENCE_AXES));
  }
  return result;
}

function fontInstanceSetDifference(
  observed: ReadonlySet<string>,
  expected: ReadonlySet<string>,
): FontInstanceSetDifference {
  const groups = new Map<string, DifferenceGroup>();
  const groupFor = (projection: ReturnType<typeof signatureProjection>): DifferenceGroup => {
    let group = groups.get(projection.physicalKey);
    if (group === undefined) {
      group = { faceIndex: projection.faceIndex, chrome: new Map(), krr: new Map() };
      groups.set(projection.physicalKey, group);
    }
    return group;
  };
  for (const signature of observed) {
    if (expected.has(signature)) continue;
    const projection = signatureProjection(signature);
    groupFor(projection).chrome.set(signature, projection.variation);
  }
  for (const signature of expected) {
    if (observed.has(signature)) continue;
    const projection = signatureProjection(signature);
    groupFor(projection).krr.set(signature, projection.variation);
  }
  const state = { truncated: false };
  const differences = [...groups.entries()]
    .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
    .flatMap(([_, group], faceOrdinal) => {
      const chromeOnly = boundedVariations(group.chrome.values(), state);
      const krrOnly = boundedVariations(group.krr.values(), state);
      return chromeOnly.length === 0 && krrOnly.length === 0
        ? []
        : [{ faceOrdinal, faceIndex: group.faceIndex, chromeOnly, krrOnly }];
    });
  if (differences.length > MAX_DIFFERENCE_FACE_GROUPS) state.truncated = true;
  return {
    chromeCount: observed.size,
    krrCount: expected.size,
    groups: differences.slice(0, MAX_DIFFERENCE_FACE_GROUPS),
    truncated: state.truncated,
  };
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
  onMismatch?: (difference: FontInstanceSetDifference) => void,
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
    ) {
      onMismatch?.(fontInstanceSetDifference(observed, expected));
      return { established: false, reason: "Chrome and KRR font instance sets differ" };
    }
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
