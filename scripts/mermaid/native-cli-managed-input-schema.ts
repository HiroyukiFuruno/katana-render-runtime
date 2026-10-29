import { z } from "zod";

export const digest = z.string().regex(/^[0-9a-f]{64}$/);
export const natural = z.number().int().nonnegative().safe();
export const positive = natural.positive();
export const ns = z.string().regex(/^(0|[1-9][0-9]*)$/);
export const strings = z.array(z.string());
export const environment = z.record(z.string(), z.string());
export const managedFileSchema = z
  .object({ path: z.string(), sha256: digest, size: natural })
  .strict();
export const rustToolLauncherSchema = z
  .object({ path: z.string(), resolvedPath: z.string(), sha256: digest, resolutionArgv: strings })
  .strict()
  .superRefine((launcher, context) => {
    if (launcher.resolutionArgv.length === 0) return;
    const name = launcher.path.slice(launcher.path.lastIndexOf("/") + 1);
    if (
      (name !== "cargo" && name !== "rustc") ||
      launcher.resolutionArgv.length !== 3 ||
      launcher.resolutionArgv[0] !== launcher.resolvedPath ||
      launcher.resolutionArgv[1] !== "which" ||
      launcher.resolutionArgv[2] !== name
    ) {
      context.addIssue({
        code: "custom",
        message: "Rustup resolution argv differs",
        path: ["resolutionArgv"],
      });
    }
  });
export const files = z.array(managedFileSchema);
export const managedIdentitySchema = managedFileSchema
  .extend({
    device: natural,
    inode: positive,
    mtimeNs: ns,
    ctimeNs: ns,
  })
  .strict();
const protectedIdentity = managedFileSchema
  .extend({ dev: natural, ino: positive, mode: natural })
  .strict();
export const packageSchema = z
  .object({
    id: z.string().min(1),
    name: z.literal("katana-render-runtime-cli"),
    manifestPath: z.string(),
  })
  .strict();
export const targetSchema = z
  .object({ name: z.literal("krr"), kind: z.tuple([z.literal("bin")]), srcPath: z.string() })
  .strict();
export const managedSourceSchema = z
  .object({
    schema: z.literal("krr-native-build-source-manifest-v1"),
    digest,
    files,
  })
  .strict();
const policy = z
  .object({
    metric: z.literal("unique-inode-allocated-bytes"),
    metric_version: z.literal(1),
    baseline_bytes: z.literal(2363047936),
    factor: z.literal(2),
    threshold_bytes: z.literal(4726095872),
    target: z.string(),
  })
  .strict();
export const managedCapacitySchema = z
  .object({
    state: managedFileSchema,
    protectedRegistry: managedFileSchema,
    refs: z.array(protectedIdentity).min(1),
    capacityPolicy: policy,
    environmentValue: z.string(),
    environmentSha256: digest,
  })
  .strict();
export const managedSnapshotSchema = z
  .object({
    source: files,
    tools: z
      .object({
        cargo: managedFileSchema,
        rustc: managedFileSchema,
        rtk: managedFileSchema,
        just: managedFileSchema,
        python3: managedFileSchema,
        git: managedFileSchema,
        bash: managedFileSchema,
        bun: managedFileSchema,
        lsof: managedFileSchema,
      })
      .strict(),
    toolLaunchers: z
      .object({ cargo: rustToolLauncherSchema, rustc: rustToolLauncherSchema })
      .strict(),
    control: files,
    environment,
    environmentSha256: digest,
    capacityAdmission: managedCapacitySchema,
    toolchainFiles: files,
    toolchainAbsent: strings,
  })
  .strict();

// Cargoの追加API項目は保持し、KRRが使う意味と型を狭める。
export const cargoMetadataSchema = z
  .object({
    version: z.literal(1),
    workspace_root: z.string(),
    target_directory: z.string(),
    packages: z.array(
      z
        .object({
          id: z.string(),
          name: z.string(),
          manifest_path: z.string(),
          features: z.record(z.string(), z.array(z.string())),
          targets: z.array(
            z
              .object({ name: z.string(), kind: z.array(z.string()), src_path: z.string() })
              .passthrough(),
          ),
        })
        .passthrough(),
    ),
  })
  .passthrough();
export const cargoArtifactSchema = z
  .object({
    executable: z.string(),
    fresh: z.boolean(),
    filenames: z.array(z.string()),
    features: z.array(z.string()),
    profile: z
      .object({
        opt_level: z.enum(["0", "1", "2", "3", "s", "z"]),
        debuginfo: z.union([
          z.null(),
          z.enum(["line-directives-only", "line-tables-only"]),
          natural.max(2),
        ]),
        debug_assertions: z.boolean(),
        overflow_checks: z.boolean(),
        test: z.literal(false),
      })
      .strict(),
    target: z
      .object({ name: z.literal("krr"), kind: z.tuple([z.literal("bin")]), src_path: z.string() })
      .passthrough(),
  })
  .passthrough();
