import { createHash } from "node:crypto";
import fs from "node:fs";
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

function validateVariable(
  actual: z.infer<typeof actualFontSchema>,
  position: z.infer<typeof variationPositionSchema>,
): { axes: FontAxis[]; coordinates: FontCoordinate[] } | undefined {
  if (
    actual.physicalFaceFvarTablePresent !== true ||
    actual.axisInventoryAvailable !== true ||
    position.available !== true ||
    position.axisCount !== actual.axes.length ||
    position.coordinates === undefined ||
    position.coordinates.length !== actual.axes.length
  )
    return undefined;
  const axesValue = actual.axes;
  const coordinatesValue = position.coordinates;
  const ids = new Set<number>();
  const axes: FontAxis[] = [];
  for (const axis of axesValue) {
    if (axis.minimum > axis.maximum || ids.has(axis.identifier)) return undefined;
    ids.add(axis.identifier);
    axes.push(axis);
  }
  const seen = new Set<number>();
  const coordinates: FontCoordinate[] = [];
  for (const coordinate of coordinatesValue) {
    const axis = axes.find((item) => item.identifier === coordinate.identifier);
    if (
      axis === undefined ||
      seen.has(coordinate.identifier) ||
      coordinate.value < axis.minimum ||
      coordinate.value > axis.maximum
    )
      return undefined;
    seen.add(coordinate.identifier);
    coordinates.push(coordinate);
  }
  return seen.size === ids.size ? { axes, coordinates } : undefined;
}

function parseFontIdentity(args: EventArgs): FontIdentity | undefined {
  const actualResult = actualFontSchema.safeParse(parseJsonValue(stringArg(args, "actualFont")));
  const positionResult = variationPositionSchema.safeParse(
    parseJsonValue(stringArg(args, "skiaVariationPosition")),
  );
  const descriptor = parseDescriptor(args);
  if (!actualResult.success || !positionResult.success || descriptor === undefined)
    return undefined;
  const actual = actualResult.data;
  const position = positionResult.data;
  const physicalPath = actual.descriptorURL;
  if (typeof physicalPath !== "string" || !fs.existsSync(physicalPath)) return undefined;
  const bytes = fs.readFileSync(physicalPath);
  if (createHash("sha256").update(bytes).digest("hex") !== actual.physicalFileSHA256)
    return undefined;
  let axes: FontAxis[] = [];
  let coordinates: FontCoordinate[] = [];
  if (!validateStatic(actual, position)) {
    const variable = validateVariable(actual, position);
    if (variable === undefined) return undefined;
    axes = variable.axes;
    coordinates = variable.coordinates;
  }
  return {
    processId: args.processId as number,
    processRole: args.processRole as ProcessRole,
    physicalFileSHA256: actual.physicalFileSHA256,
    physicalFaceTtcIndex: actual.physicalFaceTtcIndex,
    postScriptName: actual.postScriptName,
    fvarTablePresent: actual.physicalFaceFvarTablePresent === true,
    axes,
    coordinates,
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
  const identity = parseFontIdentity(args);
  if (descriptor === undefined || identity === undefined) return;
  const key = descriptorKey(args, descriptor);
  if (state.conflictingDescriptors.has(key)) return;
  const previous = state.scalers.get(key);
  if (previous !== undefined && identityKey(previous.identity) !== identityKey(identity)) {
    state.conflictingDescriptors.add(key);
    state.scalers.delete(key);
  } else if (previous === undefined) state.scalers.set(key, { identity });
}

function collectState(events: z.infer<typeof traceEventSchema>[]): TraceState {
  const state: TraceState = {
    retained: new Map(),
    strikes: new Map(),
    scalers: new Map(),
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

function joinInstances(state: TraceState): ChromiumFontInstance[] {
  const instances = new Map<string, ChromiumFontInstance>();
  for (const [key, serials] of state.strikes) {
    const scaler = state.scalers.get(key);
    const [processId, processRole] = key.split(":");
    const usedSerials = [...serials].filter((serial) =>
      state.retained.has(`${processId}:${processRole}:${serial}`),
    );
    if (usedSerials.length === 0) continue;
    if (scaler === undefined) throw new Error("used strike has no joined scaler");
    for (const serial of usedSerials) {
      const retained = state.retained.get(
        `${scaler.identity.processId}:${scaler.identity.processRole}:${serial}`,
      );
      if (retained?.begin !== true || retained.end !== true)
        throw new Error("used strike has incomplete retained paint");
      const identity = identityKey(scaler.identity);
      const prior = instances.get(identity);
      instances.set(identity, {
        ...scaler.identity,
        paintSerials: [...(prior?.paintSerials ?? []), serial].sort((a, b) => a - b),
        join: {
          retainedPaintCount: (prior?.join.retainedPaintCount ?? 0) + 1,
          strikeCacheCount: (prior?.join.strikeCacheCount ?? 0) + 1,
          scalerFontCount: 1,
        },
      });
    }
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
