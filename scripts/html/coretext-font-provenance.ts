import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import type { JsonValue } from "./dynamic-state";

const U32_MAX = 4_294_967_295;

const variationAxisSchema = z
  .object({
    identifier: z.number().int().min(0).max(U32_MAX),
    minimumValue: z.number().finite(),
    maximumValue: z.number().finite(),
    defaultValue: z.number().finite(),
  })
  .strict()
  .superRefine((axis, context) => {
    if (axis.minimumValue > axis.maximumValue)
      context.addIssue({ code: "custom", message: "axis minimum exceeds maximum" });
    if (axis.defaultValue < axis.minimumValue || axis.defaultValue > axis.maximumValue)
      context.addIssue({ code: "custom", message: "axis default is outside its range" });
  });

const variationCoordinateSchema = z
  .object({ identifier: z.number().int().min(0).max(U32_MAX), value: z.number().finite() })
  .strict();

const coreTextDescriptorSchema = z
  .object({
    urlPath: z.string().min(1),
    postScriptName: z.string().min(1),
    variationAxes: z.array(variationAxisSchema).min(1),
    variationCoordinates: z.array(variationCoordinateSchema).min(1),
  })
  .strict()
  .superRefine((descriptor, context) => {
    const axes = new Map<number, z.infer<typeof variationAxisSchema>>();
    for (const axis of descriptor.variationAxes) {
      if (axes.has(axis.identifier))
        context.addIssue({ code: "custom", message: "duplicate variation axis identifier" });
      axes.set(axis.identifier, axis);
    }
    const coordinates = new Set<number>();
    for (const coordinate of descriptor.variationCoordinates) {
      const axis = axes.get(coordinate.identifier);
      if (coordinates.has(coordinate.identifier))
        context.addIssue({ code: "custom", message: "duplicate variation coordinate identifier" });
      coordinates.add(coordinate.identifier);
      if (!axis) {
        context.addIssue({ code: "custom", message: "coordinate references an unknown axis" });
      } else if (coordinate.value < axis.minimumValue || coordinate.value > axis.maximumValue) {
        context.addIssue({ code: "custom", message: "coordinate is outside its axis range" });
      }
    }
  });

export type CoreTextFontDescriptor = z.infer<typeof coreTextDescriptorSchema>;

const fontProvenanceSchema = z
  .object({
    scope: z.enum(["capture", "node"]),
    family: z.string().min(1),
    source: z.string().min(1),
    filePath: z.string().min(1),
    fileSha256: z.string().regex(/^[a-f0-9]{64}$/),
    faceIndex: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
    selectedNodeId: z
      .number()
      .int()
      .min(Number.MIN_SAFE_INTEGER)
      .max(Number.MAX_SAFE_INTEGER)
      .nullable()
      .optional(),
    selected: z.literal(true),
    fileBytesVerified: z.literal(true),
    coreTextDescriptor: coreTextDescriptorSchema.optional(),
  })
  .strict()
  .superRefine((face, context) => {
    if (
      face.scope === "node" &&
      (typeof face.selectedNodeId !== "number" || face.selectedNodeId <= 0)
    )
      context.addIssue({ code: "custom", message: "node font requires a positive node id" });
  });

export type VerifiedFontProvenance = z.infer<typeof fontProvenanceSchema>;

export function validateFontProvenance(
  value: JsonValue | undefined,
  allowEmpty = false,
): VerifiedFontProvenance[] {
  const parsed = z.array(fontProvenanceSchema).safeParse(value);
  if (!parsed.success || (!allowEmpty && parsed.data?.length === 0))
    throw new Error("Selected font provenance is incomplete or has unknown fields");
  for (const face of parsed.data) assertPhysicalFontFace(face);
  return parsed.data;
}

function assertPhysicalFontFace(face: VerifiedFontProvenance): void {
  if (face.scope === "node" && typeof face.selectedNodeId !== "number")
    throw new Error("Selected node font identity is missing");
  if (!path.isAbsolute(face.filePath) || !fs.existsSync(face.filePath))
    throw new Error("Selected font path is not an existing absolute file");
  const actualSha = createHash("sha256").update(fs.readFileSync(face.filePath)).digest("hex");
  if (actualSha !== face.fileSha256)
    throw new Error("Selected font file SHA differs from its provenance record");
  if (face.coreTextDescriptor)
    assertCoreTextPhysicalPath(face.filePath, face.coreTextDescriptor.urlPath);
}

function assertCoreTextPhysicalPath(filePath: string, urlPath: string): void {
  const actualPath = canonicalFontPath(filePath);
  const descriptorPath = canonicalFontPath(urlPath);
  if (descriptorPath !== actualPath)
    throw new Error("CoreText descriptor path differs from the verified font file");
}

export function assertCoreTextPostScriptOrder(
  faces: readonly VerifiedFontProvenance[],
  expectedPostScriptNames: readonly string[],
): void {
  if (
    faces.length !== expectedPostScriptNames.length ||
    faces.some(
      (face, index) =>
        face.coreTextDescriptor !== undefined &&
        face.coreTextDescriptor.postScriptName !== expectedPostScriptNames[index],
    )
  )
    throw new Error("CoreText PostScript names differ from the ordered selected-font request");
}

function canonicalFontPath(value: string): string {
  if (!path.isAbsolute(value)) throw new Error("Font descriptor path must be absolute");
  const resolved = path.resolve(value);
  if (resolved !== value || !fs.existsSync(value) || fs.lstatSync(value).isSymbolicLink())
    throw new Error("Font descriptor path is missing or noncanonical");
  const stat = fs.statSync(value);
  if (!stat.isFile()) throw new Error("Font descriptor path is not a regular file");
  const real = fs.realpathSync(value);
  if (real !== value) throw new Error("Font descriptor path does not identify its canonical file");
  return real;
}
