import {
  EXCLUDED,
  FEATURES,
  LICENSE_FILE_COUNT,
  type Provenance,
  SOURCE_COMMIT,
  SOURCE_FILE_COUNT,
  SOURCE_REPOSITORY,
  UPSTREAM_REPOSITORY,
  UPSTREAM_TEST_COUNT,
} from "./svg-engine-provenance-schema";

type JsonObject = Record<string, unknown>;

function fail(message: string): never {
  throw new Error(`SVG engine provenance: ${message}`);
}

function exactKeys(value: JsonObject, expected: readonly string[], label: string): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${label} has unknown or missing fields`);
  }
}

function number(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    fail(`${label} must be a non-negative integer`);
  }
  return value;
}

function object(value: unknown, label: string): JsonObject {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value as JsonObject;
}

function validateAdaptations(value: JsonObject, index: number): void {
  const allowed = [
    "cratePaths",
    "enabledFeaturePredicates",
    "harfrustAliasPaths",
    "internalUsvgAlias",
    "usvgPaths",
  ];
  if (Object.keys(value).some((key) => !allowed.includes(key))) {
    fail(`files[${index}] has unknown adaptation`);
  }
  for (const [key, entry] of Object.entries(value)) {
    if (key !== "enabledFeaturePredicates") {
      number(entry, `files[${index}].adaptations.${key}`);
      continue;
    }
    const predicates = object(entry, `files[${index}].adaptations.${key}`);
    for (const feature of Object.keys(predicates)) {
      if (!FEATURES.includes(feature as (typeof FEATURES)[number])) {
        fail(`files[${index}] has unknown feature adaptation`);
      }
      number(predicates[feature], `adaptation ${feature}`);
    }
  }
}

function validateProvenanceIdentity(provenance: Provenance): void {
  if (
    provenance.schema !== "krr-svg-engine-source-provenance-v1" ||
    provenance.sourceRepository !== SOURCE_REPOSITORY ||
    provenance.upstreamRepository !== UPSTREAM_REPOSITORY ||
    provenance.sourceCommit !== SOURCE_COMMIT ||
    provenance.licenseExpression !== "Apache-2.0 OR MIT" ||
    provenance.sourceFileCount !== SOURCE_FILE_COUNT ||
    provenance.files.length !== SOURCE_FILE_COUNT ||
    provenance.upstreamTestCount !== UPSTREAM_TEST_COUNT ||
    provenance.licenses.length !== LICENSE_FILE_COUNT
  ) {
    fail("approved source identity or inventory count differs");
  }
  exactKeys(
    provenance.adaptationPolicy,
    [
      "allowAttributes",
      "cratePaths",
      "enabledFeaturePredicates",
      "harfrustAlias",
      "resvgUsvgPaths",
      "sourceSemantics",
    ],
    "adaptationPolicy",
  );
  if (
    JSON.stringify([...provenance.excludedSourcePaths].sort()) !==
      JSON.stringify([...EXCLUDED].sort()) ||
    JSON.stringify([...provenance.enabledFeatures].sort()) !== JSON.stringify([...FEATURES].sort())
  ) {
    fail("excluded paths or enabled features differ");
  }
}

function validateManifestEntry(
  entry: Provenance["files"][number],
  index: number,
  destinations: Set<string>,
): void {
  const source = entry.source.replaceAll("\\", "/");
  const destination = entry.destination.replaceAll("\\", "/");
  if (EXCLUDED.includes(source as (typeof EXCLUDED)[number])) {
    fail(`files[${index}] includes an excluded source path`);
  }
  const prefix = source.startsWith("crates/usvg/src/")
    ? "usvg/"
    : source.startsWith("crates/resvg/src/")
      ? "resvg/"
      : undefined;
  const sourceSuffix = source.slice(source.indexOf("/src/") + 5);
  if (
    prefix === undefined ||
    destination !== `${prefix}${sourceSuffix}` ||
    destination.includes("..") ||
    !destination.endsWith(".rs") ||
    destinations.has(destination)
  ) {
    fail(`files[${index}] has invalid or duplicate path mapping`);
  }
  if (
    !/^[0-9a-f]{64}$/.test(entry.sourceSha256) ||
    !/^[0-9a-f]{64}$/.test(entry.transplantedSha256)
  ) {
    fail(`files[${index}] has invalid SHA256`);
  }
  validateAdaptations(entry.adaptations, index);
  destinations.add(destination);
}

export function validateManifest(provenance: Provenance): Set<string> {
  validateProvenanceIdentity(provenance);
  const destinations = new Set<string>();
  for (const [index, entry] of provenance.files.entries()) {
    validateManifestEntry(entry, index, destinations);
  }
  return destinations;
}
