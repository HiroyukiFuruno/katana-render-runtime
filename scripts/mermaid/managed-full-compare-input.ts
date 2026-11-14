import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { FixtureRepository } from "./diagram_update_fixtures";
import { sha256 as digestSha256, fileSha256 } from "./mermaid-pair-manifest";
import { physicalFileIdentity } from "./raster-comparison-binding";
import { type RasterFileIdentity, sameRasterFileIdentity } from "./raster-production-binding";
import { RasterProductionSession } from "./raster-production-session";

const absolutePath = z
  .string()
  .refine((value) => path.isAbsolute(value) && path.resolve(value) === value);
const sha256 = z.string().regex(/^[a-f0-9]{64}$/);
const filePin = z.object({ path: absolutePath, sha256 }).strict();
const languageInput = z
  .object({
    fixtures: absolutePath,
    officialTemplate: filePin,
    officialReceipt: filePin,
    officialProcessEvidence: filePin,
    officialPairDirectory: absolutePath,
    nativeFrameEvidence: filePin.optional(),
    nativeFrameCapture: z
      .object({
        probeBinary: filePin,
        config: filePin,
        fontContext: filePin,
        buildProof: z
          .object({
            bundleManifest: filePin.refine((value) => path.basename(value.path) === "bundle.json"),
            artifactPath: z
              .string()
              .min(1)
              .refine(
                (value) =>
                  !path.isAbsolute(value) &&
                  value.split(/[\\/]/).every((part) => part !== ".." && part !== ""),
              ),
            artifactSha256: sha256,
          })
          .strict(),
      })
      .strict()
      .optional(),
  })
  .strict();

export const managedFullCompareRequestSchema = z
  .object({
    schema: z.literal("krr-mermaid-managed-full-run-request-v1"),
    repositoryRoot: absolutePath,
    outputRoot: absolutePath,
    sourceManifest: filePin,
    protectedRegistry: filePin,
    binary: absolutePath,
    runtime: filePin,
    tools: z
      .object({
        cargo: filePin,
        rustc: filePin,
        python: filePin,
        bun: filePin,
        mermaidJs: filePin,
        mermaidZenUmlJs: filePin,
      })
      .strict(),
    languages: z.object({ en: languageInput, ja: languageInput }).strict(),
  })
  .strict();

export type ManagedFullCompareRequest = z.infer<typeof managedFullCompareRequestSchema>;
export type LanguageName = keyof ManagedFullCompareRequest["languages"];

export interface PinnedInputs {
  files: Map<string, RasterFileIdentity>;
  directories: Map<string, DirectoryIdentity>;
  fixtureSources: Record<LanguageName, FixtureSourcePin[]>;
}

export interface FixtureSourcePin {
  slug: string;
  path: string;
  markdownSha256: string;
  originalFenceSourceSha256: string;
  canonicalSourceSha256: string;
  empty: boolean;
}

export interface LanguageFixtureInventory {
  slugs: string[];
  sources: FixtureSourcePin[];
}

export interface DirectoryIdentity {
  path: string;
  dev: string;
  ino: string;
  mode: number;
  mtimeNs: string;
  ctimeNs: string;
}

export function parseManagedFullCompareRequest(value: unknown): ManagedFullCompareRequest {
  return managedFullCompareRequestSchema.parse(value);
}

export function validatePinnedInputs(request: ManagedFullCompareRequest): PinnedInputs {
  assertCanonicalDirectory(request.repositoryRoot, "repository root");
  if (request.binary !== path.join(request.repositoryRoot, "target/debug/krr")) {
    throw new Error("Managed full-run binary must use the canonical workspace target path");
  }
  assertManagedOutputRoot(request.outputRoot, request.repositoryRoot);
  const filePins = allFilePins(request);
  const files = new Map<string, RasterFileIdentity>();
  for (const pin of filePins.values()) files.set(pin.path, assertFilePin(pin));
  for (const language of ["en", "ja"] as const) {
    const probe = request.languages[language].nativeFrameCapture?.probeBinary;
    if (probe !== undefined) assertExecutableProbeBinary(probe.path, language);
  }
  const ignoreFile = path.join(request.repositoryRoot, ".gitignore");
  const ignoreIdentity = assertFilePin({ path: ignoreFile, sha256: fileSha256(ignoreFile) });
  if (files.has(ignoreFile)) throw new Error("Repository ignore file conflicts with an input pin");
  files.set(ignoreFile, ignoreIdentity);
  const fixtureSources = {
    en: captureCanonicalFixtureSources(request.repositoryRoot, "en", files).sources,
    ja: captureCanonicalFixtureSources(request.repositoryRoot, "ja", files).sources,
  };
  const directories = allDirectories(request);
  return { files, directories, fixtureSources };
}

export function validateFreshInputState(
  request: ManagedFullCompareRequest,
  pins: PinnedInputs,
): void {
  for (const [filePath, expected] of pins.files) {
    if (!sameRasterFileIdentity(expected, physicalFileIdentity(filePath))) {
      throw new Error(`Input file identity changed: ${filePath}`);
    }
  }
  for (const [directoryPath, expected] of pins.directories) {
    if (!sameDirectoryIdentity(expected, directoryIdentity(directoryPath))) {
      throw new Error(`Input directory changed: ${directoryPath}`);
    }
  }
  if (fileSha256(request.sourceManifest.path) !== request.sourceManifest.sha256) {
    throw new Error("Expected source manifest changed during full comparison");
  }
}

export function validateLanguageInventory(
  request: ManagedFullCompareRequest,
  language: LanguageName,
  expectedSources: FixtureSourcePin[],
): LanguageFixtureInventory {
  const input = request.languages[language];
  const sourceRoot = path.join(request.repositoryRoot, "tests/fixtures/mermaid", language);
  if (input.fixtures !== sourceRoot) {
    throw new Error(`${language} fixture path must be the canonical repository source`);
  }
  assertCanonicalDirectory(input.fixtures, `${language} fixtures`);
  assertCanonicalDirectory(input.officialPairDirectory, `${language} official pairs`);
  const inventory = captureCanonicalFixtureSources(request.repositoryRoot, language);
  if (JSON.stringify(inventory.sources) !== JSON.stringify(expectedSources)) {
    throw new Error(`${language} fixture source hashes changed after initial pinning`);
  }
  const slugs = inventory.slugs;
  const official = RasterProductionSession.fromPinnedConfig(
    input.officialTemplate.path,
    input.officialTemplate.sha256,
  );
  if (
    official.options.role !== "official" ||
    JSON.stringify(official.options.inventory) !== JSON.stringify(slugs)
  ) {
    throw new Error(`${language} official template does not bind the exact fixture inventory`);
  }
  return inventory;
}

export function captureCanonicalFixtureSources(
  repositoryRoot: string,
  language: LanguageName,
  identities?: Map<string, RasterFileIdentity>,
): LanguageFixtureInventory {
  const directory = path.join(repositoryRoot, "tests/fixtures/mermaid", language);
  assertCanonicalDirectory(directory, `${language} fixtures`);
  const markdownNames = fs
    .readdirSync(directory)
    .filter((name) => name.endsWith(".md"))
    .sort();
  const expectedCount = language === "en" ? 59 : 58;
  if (markdownNames.length !== expectedCount + 1) {
    throw new Error(`${language} canonical fixture directory has an unexpected Markdown inventory`);
  }
  const fixtures = new FixtureRepository(directory).list();
  const nonempty = fixtures.filter((fixture) => (fixture.originalFenceSource ?? "").trim() !== "");
  const empty = fixtures.filter((fixture) => (fixture.originalFenceSource ?? "").trim() === "");
  if (
    fixtures.length !== expectedCount + 1 ||
    nonempty.length !== expectedCount ||
    empty.length !== 1 ||
    new Set(fixtures.map((fixture) => fixture.slug)).size !== fixtures.length
  ) {
    throw new Error(
      `${language} canonical inventory must contain ${expectedCount} nonempty fixtures and one empty fixture`,
    );
  }
  const sources = fixtures.map((fixture) => {
    const stat = fs.lstatSync(fixture.filePath);
    if (
      stat.isSymbolicLink() ||
      !stat.isFile() ||
      fs.realpathSync(fixture.filePath) !== fixture.filePath
    ) {
      throw new Error(`${language} fixture must be a canonical regular file: ${fixture.filePath}`);
    }
    const markdown = fs.readFileSync(fixture.filePath);
    const markdownSha256 = digestSha256(markdown);
    const identity = physicalFileIdentity(fixture.filePath);
    if (identity.sha256 !== markdownSha256) {
      throw new Error(`${language} fixture changed while being pinned: ${fixture.filePath}`);
    }
    identities?.set(fixture.filePath, identity);
    const originalFenceSource = fixture.originalFenceSource ?? "";
    const canonicalSource = fixture.canonicalSource;
    if (canonicalSource === undefined) {
      throw new Error(`${language} fixture has no canonical source: ${fixture.slug}`);
    }
    return {
      slug: fixture.slug,
      path: fixture.filePath,
      markdownSha256,
      originalFenceSourceSha256: digestSha256(Buffer.from(originalFenceSource, "utf8")),
      canonicalSourceSha256: digestSha256(Buffer.from(canonicalSource, "utf8")),
      empty: originalFenceSource.trim() === "",
    };
  });
  return { slugs: nonempty.map((fixture) => fixture.slug).sort(), sources };
}

function allFilePins(
  request: ManagedFullCompareRequest,
): Map<string, { path: string; sha256: string }> {
  const pins = new Map<string, { path: string; sha256: string }>();
  const add = (pin: { path: string; sha256: string }) => {
    const previous = pins.get(pin.path);
    if (previous !== undefined && previous.sha256 !== pin.sha256) {
      throw new Error(`Conflicting pinned SHA values for ${pin.path}`);
    }
    pins.set(pin.path, pin);
  };
  add(request.sourceManifest);
  add(request.protectedRegistry);
  add(request.runtime);
  for (const pin of Object.values(request.tools)) add(pin);
  for (const language of [request.languages.en, request.languages.ja]) {
    add(language.officialTemplate);
    add(language.officialReceipt);
    add(language.officialProcessEvidence);
    if (language.nativeFrameEvidence !== undefined) add(language.nativeFrameEvidence);
    if (language.nativeFrameCapture !== undefined) {
      add(language.nativeFrameCapture.probeBinary);
      add(language.nativeFrameCapture.config);
      add(language.nativeFrameCapture.fontContext);
      add(language.nativeFrameCapture.buildProof.bundleManifest);
    }
  }
  return pins;
}

function allDirectories(request: ManagedFullCompareRequest): Map<string, DirectoryIdentity> {
  const directories = new Map<string, DirectoryIdentity>();
  for (const directory of [
    request.languages.en.fixtures,
    request.languages.en.officialPairDirectory,
    request.languages.ja.fixtures,
    request.languages.ja.officialPairDirectory,
  ]) {
    directories.set(directory, directoryIdentity(directory));
  }
  return directories;
}

function assertFilePin(pin: { path: string; sha256: string }): RasterFileIdentity {
  const stat = fs.lstatSync(pin.path);
  if (stat.isSymbolicLink() || !stat.isFile() || fs.realpathSync(pin.path) !== pin.path) {
    throw new Error(`Pinned input must be a canonical regular file: ${pin.path}`);
  }
  if (fileSha256(pin.path) !== pin.sha256)
    throw new Error(`Pinned input SHA mismatch: ${pin.path}`);
  const identity = physicalFileIdentity(pin.path);
  if (identity.sha256 !== pin.sha256)
    throw new Error(`Pinned input changed while reading: ${pin.path}`);
  return identity;
}

function assertExecutableProbeBinary(filePath: string, language: LanguageName): void {
  try {
    fs.accessSync(filePath, fs.constants.X_OK);
  } catch {
    throw new Error(`${language} native frame probe binary is not executable: ${filePath}`);
  }
}

export function assertManagedOutputRoot(outputRoot: string, repositoryRoot: string): void {
  const tmpRoot = path.join(repositoryRoot, "tmp");
  assertCanonicalDirectory(tmpRoot, "ignored tmp output root");
  const ignoreRules = fs.readFileSync(path.join(repositoryRoot, ".gitignore"), "utf8");
  if (
    !ignoreRules.split(/\r?\n/).includes("tmp") ||
    fs.existsSync(path.join(tmpRoot, ".gitignore"))
  ) {
    throw new Error("Repository tmp output must remain ignored by the root .gitignore");
  }
  const relativeToTmp = path.relative(tmpRoot, outputRoot);
  const parent = path.dirname(outputRoot);
  if (fs.realpathSync(parent) !== parent || fs.existsSync(outputRoot)) {
    throw new Error("Full comparison output root must be new under a canonical parent");
  }
  if (
    relativeToTmp === "" ||
    relativeToTmp === ".." ||
    relativeToTmp.startsWith(`..${path.sep}`) ||
    path.isAbsolute(relativeToTmp)
  ) {
    throw new Error(
      "Full comparison output root must be inside the ignored repository tmp directory",
    );
  }
}

function assertCanonicalDirectory(directory: string, label: string): void {
  const stat = fs.lstatSync(directory);
  if (stat.isSymbolicLink() || !stat.isDirectory() || fs.realpathSync(directory) !== directory) {
    throw new Error(`${label} must be a canonical directory: ${directory}`);
  }
}

function directoryIdentity(directory: string): DirectoryIdentity {
  assertCanonicalDirectory(directory, "input directory");
  const stat = fs.statSync(directory, { bigint: true });
  return {
    path: directory,
    dev: stat.dev.toString(),
    ino: stat.ino.toString(),
    mode: Number(stat.mode),
    mtimeNs: stat.mtimeNs.toString(),
    ctimeNs: stat.ctimeNs.toString(),
  };
}

function sameDirectoryIdentity(left: DirectoryIdentity, right: DirectoryIdentity): boolean {
  return (
    left.path === right.path &&
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.mode === right.mode &&
    left.mtimeNs === right.mtimeNs &&
    left.ctimeNs === right.ctimeNs
  );
}
