import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { z } from "zod";

const TRACE_CATEGORY = "katana.font-observer";
const SHA256 = /^[0-9a-f]{64}$/;

const traceEventSchema = z
  .object({
    cat: z.literal(TRACE_CATEGORY),
    name: z.enum(["retained-paint", "strike-cache", "scaler-font"]),
    pid: z.number().int().positive(),
    args: z.record(z.string(), z.unknown()),
  })
  .passthrough();
const traceSchema = z.object({ traceEvents: z.array(z.unknown()) }).passthrough();

type EventArgs = z.infer<typeof traceEventSchema>["args"];
type ProcessRole = "renderer" | "gpu";
type FontAxis = { identifier: number; minimum: number; maximum: number };
type FontCoordinate = { identifier: number; value: number };

const axisSchema = z
  .object({
    identifier: z.number().int(),
    minimum: z.number().finite(),
    maximum: z.number().finite(),
  })
  .passthrough();
const coordinateSchema = z
  .object({ identifier: z.number().int(), value: z.number().finite() })
  .strict();
const actualFontSchema = z
  .object({
    postScriptName: z.string().min(1),
    physicalFileSHA256: z.string().regex(SHA256),
    physicalFileAvailable: z.literal(true),
    physicalFaceBindingStatus: z.literal("observed:unique-table-match"),
    physicalFaceTtcIndex: z.number().int().nonnegative(),
    physicalFaceFvarTablePresent: z.boolean(),
    axes: z.array(axisSchema),
    descriptorURL: z.string().min(1),
    axisInventoryAvailable: z.boolean().optional(),
  })
  .passthrough();
const coreTextCoordinatesSchema = z
  .object({
    axisInventoryRawCount: z.number().int().nonnegative().optional(),
    invalidAxisCount: z.number().int().nonnegative().optional(),
    coordinateDictionaryAvailable: z.boolean().optional(),
    coordinateDictionaryRawCount: z.number().int().nonnegative().optional(),
    invalidCoordinateCount: z.number().int().nonnegative().optional(),
    coordinatesAvailable: z.boolean().optional(),
    coordinatesStatus: z.string().optional(),
    coordinatesSource: z.string().optional(),
    coordinates: z.array(coordinateSchema).optional(),
  })
  .passthrough();
const variationPositionSchema = z
  .object({
    available: z.boolean(),
    coordinates: z.array(coordinateSchema).optional(),
    axisCount: z.number().int().nonnegative().optional(),
  })
  .passthrough();

export type ChromiumFontTraceOptions = {
  tracePath: string;
  traceSha256: string;
  sessionId: string;
};

export type ChromiumFontInstance = {
  processId: number;
  processRole: ProcessRole;
  physicalFileSHA256: string;
  physicalFilePath?: string;
  physicalFaceTtcIndex: number;
  postScriptName: string;
  fvarTablePresent: boolean;
  axes: readonly FontAxis[];
  coordinates: readonly FontCoordinate[];
  paintSerials: readonly number[];
  join: {
    retainedPaintCount: number;
    strikeCacheCount: number;
    scalerFontCount: number;
  };
};

export type ChromiumFontTrace = {
  traceSha256: string;
  sessionId: string;
  instances: readonly ChromiumFontInstance[];
};

const stringArg = (args: EventArgs, key: string): string | undefined => {
  const value = args[key];
  return typeof value === "string" ? value : undefined;
};

const parseJsonValue = (text: string | undefined): unknown => {
  if (text === undefined) return undefined;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return undefined;
  }
};

const isProcessRole = (value: unknown): value is ProcessRole =>
  value === "renderer" || value === "gpu";

const finiteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

const parsePaintSerial = (args: EventArgs): number | undefined => {
  const value = args.paintSerial;
  return typeof value === "number" && Number.isInteger(value) && value >= 0 ? value : undefined;
};

const parseDescriptor = (args: EventArgs): { sha: string; length: number } | undefined => {
  const sha = stringArg(args, "descriptorSHA256");
  const length = args.descriptorLength;
  return sha !== undefined &&
    SHA256.test(sha) &&
    typeof length === "number" &&
    Number.isInteger(length) &&
    length > 0
    ? { sha, length }
    : undefined;
};

const parseTransform = (args: EventArgs): boolean => {
  const transform = parseJsonValue(stringArg(args, "transformRowMajor"));
  return Array.isArray(transform) && transform.length === 16 && transform.every(finiteNumber);
};

type FontIdentity = Omit<ChromiumFontInstance, "paintSerials" | "join">;

export const OBSERVER_SCALER_REJECTION_CODES = [
  "observer-scaler-no-event",
  "observer-scaler-conflicting-descriptor",
  "observer-scaler-invalid-font-schema",
  "observer-scaler-invalid-variation-schema",
  "observer-scaler-missing-descriptor-path",
  "observer-scaler-file-uri-needs-normalization",
  "observer-scaler-unavailable-absolute-file",
  "observer-scaler-file-read-failed",
  "observer-scaler-file-digest-mismatch",
  "observer-scaler-static-fvar-inconsistent",
  "observer-scaler-fvar-inventory-unavailable",
  "observer-scaler-skia-position-unavailable",
  "observer-scaler-axis-count-mismatch",
  "observer-scaler-coordinate-length-mismatch",
  "observer-scaler-invalid-axis",
  "observer-scaler-duplicate-axis",
  "observer-scaler-missing-coordinate",
  "observer-scaler-excess-coordinate",
  "observer-scaler-duplicate-coordinate",
  "observer-scaler-out-of-range-coordinate",
  "observer-scaler-skia-position-contradictory",
  "observer-scaler-coretext-coordinates-origin-invalid",
  "observer-scaler-coretext-coordinates-incomplete",
  "observer-scaler-coretext-coordinates-count-invalid",
] as const;

type ScalerRejectionToken = (typeof OBSERVER_SCALER_REJECTION_CODES)[number];

type ParsedFontIdentity =
  | { identity: FontIdentity; rejection?: undefined }
  | { identity?: undefined; rejection: ScalerRejectionToken };

const identityKey = (instance: FontIdentity): string =>
  JSON.stringify([
    instance.processId,
    instance.processRole,
    instance.physicalFileSHA256,
    instance.physicalFaceTtcIndex,
    instance.axes,
    instance.coordinates,
  ]);

function validateStatic(
  actual: z.infer<typeof actualFontSchema>,
  position: z.infer<typeof variationPositionSchema>,
): boolean {
  return (
    actual.physicalFaceFvarTablePresent === false &&
    actual.axes.length === 0 &&
    (position.coordinates === undefined || position.coordinates.length === 0) &&
    position.available === false
  );
}

type ValidatedVariation = { axes: FontAxis[]; coordinates: FontCoordinate[] };

type ActualFont = z.infer<typeof actualFontSchema>;

function hasCoreTextCoordinateFields(value: unknown): boolean {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  return [
    "coordinatesAvailable",
    "coordinatesStatus",
    "coordinatesSource",
    "coordinateDictionaryAvailable",
    "coordinateDictionaryRawCount",
    "invalidCoordinateCount",
    "axisInventoryRawCount",
    "invalidAxisCount",
    "coordinates",
  ].some((field) => field in value);
}

function directCoreTextCoordinates(
  actual: ActualFont,
  actualValue: unknown,
  position: z.infer<typeof variationPositionSchema>,
): FontCoordinate[] | ScalerRejectionToken | undefined {
  if (position.coordinates !== undefined || position.axisCount !== undefined)
    return "observer-scaler-skia-position-contradictory";
  if (!hasCoreTextCoordinateFields(actualValue)) return undefined;
  const directResult = coreTextCoordinatesSchema.safeParse(actualValue);
  if (!directResult.success) return "observer-scaler-coretext-coordinates-incomplete";
  const direct = directResult.data;
  if (direct.coordinatesSource !== "CoreText.CTFontCopyVariation(raw-sparse)")
    return "observer-scaler-coretext-coordinates-origin-invalid";
  if (
    actual.axes.length === 0 ||
    direct.coordinatesAvailable !== true ||
    direct.coordinatesStatus !== "observed:complete-coordinate-dictionary" ||
    direct.coordinateDictionaryAvailable !== true ||
    direct.invalidAxisCount !== 0 ||
    direct.invalidCoordinateCount !== 0 ||
    direct.coordinates === undefined
  )
    return "observer-scaler-coretext-coordinates-incomplete";
  if (
    direct.axisInventoryRawCount !== actual.axes.length ||
    direct.coordinateDictionaryRawCount !== actual.axes.length ||
    direct.coordinates.length !== actual.axes.length
  )
    return "observer-scaler-coretext-coordinates-count-invalid";
  return direct.coordinates;
}

function validateVariablePosition(
  position: z.infer<typeof variationPositionSchema>,
  axisCount: number,
): FontCoordinate[] | ScalerRejectionToken {
  if (position.available !== true) return "observer-scaler-skia-position-unavailable";
  if (position.axisCount !== axisCount) return "observer-scaler-axis-count-mismatch";
  if (position.coordinates === undefined) return "observer-scaler-coordinate-length-mismatch";
  if (position.coordinates.length < axisCount) return "observer-scaler-missing-coordinate";
  if (position.coordinates.length > axisCount) return "observer-scaler-excess-coordinate";
  return position.coordinates;
}

function validateVariableAxes(axesValue: readonly FontAxis[]): FontAxis[] | ScalerRejectionToken {
  const ids = new Set<number>();
  const axes: FontAxis[] = [];
  for (const axis of axesValue) {
    if (axis.minimum > axis.maximum) return "observer-scaler-invalid-axis";
    if (ids.has(axis.identifier)) return "observer-scaler-duplicate-axis";
    ids.add(axis.identifier);
    axes.push(axis);
  }
  return axes;
}

function validateVariableCoordinates(
  axes: readonly FontAxis[],
  coordinatesValue: readonly FontCoordinate[],
): FontCoordinate[] | ScalerRejectionToken {
  const seen = new Set<number>();
  const coordinates: FontCoordinate[] = [];
  for (const coordinate of coordinatesValue) {
    const axis = axes.find((item) => item.identifier === coordinate.identifier);
    if (axis === undefined) return "observer-scaler-excess-coordinate";
    if (seen.has(coordinate.identifier)) return "observer-scaler-duplicate-coordinate";
    if (coordinate.value < axis.minimum || coordinate.value > axis.maximum)
      return "observer-scaler-out-of-range-coordinate";
    seen.add(coordinate.identifier);
    coordinates.push(coordinate);
  }
  return seen.size === axes.length ? coordinates : "observer-scaler-missing-coordinate";
}

function validateVariable(
  actual: ActualFont,
  actualValue: unknown,
  position: z.infer<typeof variationPositionSchema>,
): ValidatedVariation | ScalerRejectionToken {
  if (actual.physicalFaceFvarTablePresent !== true)
    return "observer-scaler-static-fvar-inconsistent";
  if (actual.axisInventoryAvailable !== true) return "observer-scaler-fvar-inventory-unavailable";
  if (position.available === false) {
    const directCoordinates = directCoreTextCoordinates(actual, actualValue, position);
    if (typeof directCoordinates === "string") return directCoordinates;
    if (directCoordinates === undefined) return "observer-scaler-skia-position-unavailable";
    const axes = validateVariableAxes(actual.axes);
    if (typeof axes === "string") return axes;
    const coordinates = validateVariableCoordinates(axes, directCoordinates);
    return typeof coordinates === "string" ? coordinates : { axes, coordinates };
  }
  const coordinates = validateVariablePosition(position, actual.axes.length);
  if (typeof coordinates === "string") return coordinates;
  const axes = validateVariableAxes(actual.axes);
  if (typeof axes === "string") return axes;
  const validatedCoordinates = validateVariableCoordinates(axes, coordinates);
  return typeof validatedCoordinates === "string"
    ? validatedCoordinates
    : { axes, coordinates: validatedCoordinates };
}

function hasMissingDescriptorPath(actual: unknown): boolean {
  if (typeof actual !== "object" || actual === null || Array.isArray(actual)) return true;
  const descriptorURL = (actual as Record<string, unknown>).descriptorURL;
  return typeof descriptorURL !== "string" || descriptorURL.length === 0;
}

function descriptorPathRejection(
  descriptorURL: string,
  physicalFileSha256: string,
): ScalerRejectionToken | undefined {
  if (!fs.existsSync(descriptorURL)) {
    if (descriptorURL.startsWith("file:")) return "observer-scaler-file-uri-needs-normalization";
    if (path.isAbsolute(descriptorURL)) return "observer-scaler-unavailable-absolute-file";
    return "observer-scaler-missing-descriptor-path";
  }
  try {
    const bytes = fs.readFileSync(descriptorURL);
    return createHash("sha256").update(bytes).digest("hex") === physicalFileSha256
      ? undefined
      : "observer-scaler-file-digest-mismatch";
  } catch {
    throw new Error("observer-scaler-file-read-failed");
  }
}

function canonicalPhysicalFilePath(descriptorURL: string): string | undefined {
  if (!path.isAbsolute(descriptorURL)) return undefined;
  try {
    const canonical = fs.realpathSync(descriptorURL);
    return path.isAbsolute(canonical) && fs.statSync(canonical).isFile() ? canonical : undefined;
  } catch {
    return undefined;
  }
}

function parseFontIdentity(args: EventArgs): ParsedFontIdentity {
  const actualValue = parseJsonValue(stringArg(args, "actualFont"));
  const actualResult = actualFontSchema.safeParse(actualValue);
  const positionResult = variationPositionSchema.safeParse(
    parseJsonValue(stringArg(args, "skiaVariationPosition")),
  );
  const descriptor = parseDescriptor(args);
  if (!actualResult.success)
    return {
      rejection: hasMissingDescriptorPath(actualValue)
        ? "observer-scaler-missing-descriptor-path"
        : "observer-scaler-invalid-font-schema",
    };
  if (!positionResult.success) return { rejection: "observer-scaler-invalid-variation-schema" };
  if (descriptor === undefined) return { rejection: "observer-scaler-invalid-font-schema" };
  const actual = actualResult.data;
  const position = positionResult.data;
  const pathRejection = descriptorPathRejection(actual.descriptorURL, actual.physicalFileSHA256);
  if (pathRejection !== undefined) return { rejection: pathRejection };
  let axes: FontAxis[] = [];
  let coordinates: FontCoordinate[] = [];
  if (!validateStatic(actual, position)) {
    const variable = validateVariable(actual, actualValue, position);
    if (typeof variable === "string") return { rejection: variable };
    axes = variable.axes;
    coordinates = variable.coordinates;
  }
  const physicalFilePath = canonicalPhysicalFilePath(actual.descriptorURL);
  return {
    identity: {
      processId: args.processId as number,
      processRole: args.processRole as ProcessRole,
      physicalFileSHA256: actual.physicalFileSHA256,
      ...(physicalFilePath === undefined ? {} : { physicalFilePath }),
      physicalFaceTtcIndex: actual.physicalFaceTtcIndex,
      postScriptName: actual.postScriptName,
      fvarTablePresent: actual.physicalFaceFvarTablePresent === true,
      axes,
      coordinates,
    },
  };
}

function parseSessionEvent(
  event: unknown,
  sessionId: string,
): z.infer<typeof traceEventSchema> | undefined {
  if (typeof event !== "object" || event === null || Array.isArray(event)) return undefined;
  const raw = event as Record<string, unknown>;
  if (raw.cat !== TRACE_CATEGORY) return undefined;
  const args = raw.args;
  if (typeof args !== "object" || args === null || Array.isArray(args))
    throw new Error("font observer event has no args object");
  if ((args as Record<string, unknown>).observerSessionId !== sessionId) return undefined;
  const parsed = traceEventSchema.safeParse(event);
  if (!parsed.success) throw new Error("font observer event schema mismatch");
  if (
    !isProcessRole(parsed.data.args.processRole) ||
    parsed.data.args.processId !== parsed.data.pid
  )
    throw new Error("font observer event process binding mismatch");
  return parsed.data;
}

function readEvents(tracePath: string, sessionId: string): z.infer<typeof traceEventSchema>[] {
  const trace = traceSchema.parse(JSON.parse(fs.readFileSync(tracePath, "utf8")));
  return trace.traceEvents.flatMap((event) => {
    const parsed = parseSessionEvent(event, sessionId);
    return parsed === undefined ? [] : [parsed];
  });
}

type TraceState = {
  retained: Map<string, { begin: boolean; end: boolean }>;
  strikes: Map<string, Set<number>>;
  scalers: Map<string, { identity: FontIdentity }>;
  scalerEvents: Set<string>;
  scalerRejections: Map<string, Set<ScalerRejectionToken>>;
  conflictingDescriptors: Set<string>;
};

const descriptorKey = (args: EventArgs, descriptor: { sha: string; length: number }): string =>
  `${args.processId}:${args.processRole}:${descriptor.sha}:${descriptor.length}`;

function recordRetained(state: TraceState, args: EventArgs, serial: number): void {
  const key = `${args.processId}:${args.processRole}:${serial}`;
  const value = state.retained.get(key) ?? { begin: false, end: false };
  if (args.paintPhase === "begin" && parseTransform(args)) value.begin = true;
  if (args.paintPhase === "end") value.end = true;
  state.retained.set(key, value);
}

function recordStrike(state: TraceState, args: EventArgs, serial: number): void {
  const descriptor = parseDescriptor(args);
  if (descriptor === undefined) return;
  const key = descriptorKey(args, descriptor);
  const serials = state.strikes.get(key) ?? new Set<number>();
  serials.add(serial);
  state.strikes.set(key, serials);
}

function recordScaler(state: TraceState, args: EventArgs): void {
  const descriptor = parseDescriptor(args);
  if (descriptor === undefined) return;
  const key = descriptorKey(args, descriptor);
  state.scalerEvents.add(key);
  const parsed = parseFontIdentity(args);
  if (parsed.identity === undefined) {
    const rejections = state.scalerRejections.get(key) ?? new Set<ScalerRejectionToken>();
    rejections.add(parsed.rejection);
    state.scalerRejections.set(key, rejections);
    return;
  }
  const identity = parsed.identity;
  if (state.conflictingDescriptors.has(key)) return;
  const previous = state.scalers.get(key);
  if (previous !== undefined && identityKey(previous.identity) !== identityKey(identity)) {
    state.conflictingDescriptors.add(key);
    state.scalers.delete(key);
  } else if (previous === undefined) state.scalers.set(key, { identity });
  else if (
    previous.identity.physicalFilePath !== identity.physicalFilePath ||
    previous.identity.postScriptName !== identity.postScriptName
  )
    delete previous.identity.physicalFilePath;
}

function collectState(events: z.infer<typeof traceEventSchema>[]): TraceState {
  const state: TraceState = {
    retained: new Map(),
    strikes: new Map(),
    scalers: new Map(),
    scalerEvents: new Set(),
    scalerRejections: new Map(),
    conflictingDescriptors: new Set(),
  };
  for (const event of events) {
    const serial = parsePaintSerial(event.args);
    if (serial === undefined) continue;
    if (event.name === "retained-paint") recordRetained(state, event.args, serial);
    else if (event.name === "strike-cache") recordStrike(state, event.args, serial);
    else recordScaler(state, event.args);
  }
  return state;
}

function missingScalerError(state: TraceState, key: string): Error {
  const tokens = new Set<ScalerRejectionToken>();
  if (!state.scalerEvents.has(key)) tokens.add("observer-scaler-no-event");
  if (state.conflictingDescriptors.has(key)) tokens.add("observer-scaler-conflicting-descriptor");
  for (const token of state.scalerRejections.get(key) ?? []) tokens.add(token);
  if (tokens.size === 0) tokens.add("observer-scaler-no-event");
  const ordered = OBSERVER_SCALER_REJECTION_CODES.filter((token) => tokens.has(token));
  return new Error(`used strike has no joined scaler: ${ordered.join(" ")}`);
}

function usedStrikeSerials(state: TraceState, key: string, serials: ReadonlySet<number>): number[] {
  const [processId, processRole] = key.split(":");
  return [...serials].filter((serial) =>
    state.retained.has(`${processId}:${processRole}:${serial}`),
  );
}

function mergeJoinedInstance(
  instances: Map<string, ChromiumFontInstance>,
  identity: FontIdentity,
  serial: number,
): void {
  const key = identityKey(identity);
  const prior = instances.get(key);
  const physicalFilePath =
    prior === undefined ||
    (prior.physicalFilePath === identity.physicalFilePath &&
      prior.postScriptName === identity.postScriptName)
      ? identity.physicalFilePath
      : undefined;
  const { physicalFilePath: _discardedPhysicalFilePath, ...identityWithoutPhysicalPath } = identity;
  instances.set(key, {
    ...identityWithoutPhysicalPath,
    ...(physicalFilePath === undefined ? {} : { physicalFilePath }),
    paintSerials: [...(prior?.paintSerials ?? []), serial].sort((a, b) => a - b),
    join: {
      retainedPaintCount: (prior?.join.retainedPaintCount ?? 0) + 1,
      strikeCacheCount: (prior?.join.strikeCacheCount ?? 0) + 1,
      scalerFontCount: 1,
    },
  });
}

function joinUsedStrikes(
  instances: Map<string, ChromiumFontInstance>,
  retained: TraceState["retained"],
  identity: FontIdentity,
  serials: readonly number[],
): void {
  for (const serial of serials) {
    const paint = retained.get(`${identity.processId}:${identity.processRole}:${serial}`);
    if (paint?.begin !== true || paint.end !== true)
      throw new Error("used strike has incomplete retained paint");
    mergeJoinedInstance(instances, identity, serial);
  }
}

function joinInstances(state: TraceState): ChromiumFontInstance[] {
  const instances = new Map<string, ChromiumFontInstance>();
  for (const [key, serials] of state.strikes) {
    const scaler = state.scalers.get(key);
    const usedSerials = usedStrikeSerials(state, key, serials);
    if (usedSerials.length === 0) continue;
    if (scaler === undefined) throw missingScalerError(state, key);
    joinUsedStrikes(instances, state.retained, scaler.identity, usedSerials);
  }
  return [...instances.values()];
}

export function readChromiumFontTrace(options: ChromiumFontTraceOptions): ChromiumFontTrace {
  if (!SHA256.test(options.traceSha256) || options.sessionId.length === 0)
    throw new Error("invalid trace contract");
  const bytes = fs.readFileSync(options.tracePath);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== options.traceSha256) throw new Error("trace SHA256 mismatch");
  const instances = joinInstances(collectState(readEvents(options.tracePath, options.sessionId)));
  if (instances.length === 0) throw new Error("no joined retained-paint font instance");
  return {
    traceSha256: options.traceSha256,
    sessionId: options.sessionId,
    instances,
  };
}
