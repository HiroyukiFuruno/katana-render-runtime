import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { parse } from "@babel/parser";
import type {
  File,
  ObjectExpression,
  ObjectMethod,
  ObjectProperty,
  VariableDeclarator,
} from "@babel/types";
import {
  isArrayExpression,
  isIdentifier,
  isObjectExpression,
  isObjectMethod,
  isObjectProperty,
  isSpreadElement,
  isStringLiteral,
  isVariableDeclarator,
} from "@babel/types";

export type RuntimeAssetKind =
  | "mermaid"
  | "mermaid-zenuml"
  | "zenuml-core"
  | "drawio"
  | "mathjax"
  | "plantuml";

export interface RuntimeAssetDefinition {
  readonly kind: RuntimeAssetKind;
  readonly displayName: string;
  readonly version: string;
  readonly checksum: string;
  readonly fileName: string;
  readonly rustVersionConst: string;
  readonly rustChecksumConst: string;
  readonly rustDownloadConst: string;
  readonly latestUrl: string;
  readonly source?: {
    readonly kind: "patched-upstream";
    readonly upstreamVersion: string;
    readonly immutableUrl: string;
    readonly provenancePath: string;
  };
  releasePageUrl(version: string): string;
  downloadUrl(version: string): string;
}

const MERMAID_PATCHED_URL =
  "https://raw.githubusercontent.com/HiroyukiFuruno/katana-render-runtime/v0.4.24/crates/katana-render-runtime/vendor/mermaid/12.3.0-krr.pr6823.1/mermaid.min.js";

const DEFINITIONS: RuntimeAssetDefinition[] = [
  {
    kind: "mermaid",
    displayName: "Mermaid.js",
    version: "12.3.0-krr.pr6823.1",
    checksum: "2c1ab92dfbd9149c5eec9c1f9c18999063b1721ad7868163ea3f4fe4e9231eaf",
    fileName: "mermaid.min.js",
    rustVersionConst: "MERMAID_JS_VERSION",
    rustChecksumConst: "MERMAID_JS_CHECKSUM",
    rustDownloadConst: "MERMAID_DOWNLOAD_URL",
    latestUrl: "https://registry.npmjs.org/mermaid/latest",
    source: {
      kind: "patched-upstream",
      upstreamVersion: "12.3.0",
      immutableUrl: MERMAID_PATCHED_URL,
      provenancePath: "scripts/runtime-assets/mermaid-12.3.0-krr.pr6823.1.provenance.json",
    },
    releasePageUrl: () => MERMAID_PATCHED_URL,
    downloadUrl: () => MERMAID_PATCHED_URL,
  },
  {
    kind: "mermaid-zenuml",
    displayName: "Mermaid ZenUML",
    version: "1.0.1",
    checksum: "f75662869d22aac58870e52370fe4c46b89b9710bf4b03df93b3227b5e18e730",
    fileName: "mermaid-zenuml.min.js",
    rustVersionConst: "MERMAID_ZENUML_JS_VERSION",
    rustChecksumConst: "MERMAID_ZENUML_JS_CHECKSUM",
    rustDownloadConst: "MERMAID_ZENUML_DOWNLOAD_URL",
    latestUrl: "https://registry.npmjs.org/@mermaid-js/mermaid-zenuml/latest",
    releasePageUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid-zenuml@${version}/dist/mermaid-zenuml.min.js`,
    downloadUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid-zenuml@${version}/dist/mermaid-zenuml.min.js`,
  },
  {
    kind: "zenuml-core",
    displayName: "ZenUML Core",
    version: "4.12.2",
    checksum: "49527ba7d088f7cd7c67172580a271e48aea461da4071713c6b031907b8911e2",
    fileName: "zenuml.js",
    rustVersionConst: "ZENUML_CORE_JS_VERSION",
    rustChecksumConst: "ZENUML_CORE_JS_CHECKSUM",
    rustDownloadConst: "ZENUML_CORE_DOWNLOAD_URL",
    latestUrl: "https://registry.npmjs.org/@zenuml/core/latest",
    releasePageUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/@zenuml/core@${version}/dist/zenuml.js`,
    downloadUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/@zenuml/core@${version}/dist/zenuml.js`,
  },
  {
    kind: "drawio",
    displayName: "Draw.io",
    version: "32.6.9",
    checksum: "d0d0ee121adcf6994612e4943f81fb17f66c463fcba017dca310731e18f88775",
    fileName: "drawio.min.js",
    rustVersionConst: "DRAWIO_JS_VERSION",
    rustChecksumConst: "DRAWIO_JS_CHECKSUM",
    rustDownloadConst: "DRAWIO_DOWNLOAD_URL",
    latestUrl: "https://api.github.com/repos/jgraph/drawio/releases/latest",
    releasePageUrl: (version: string) =>
      `https://github.com/jgraph/drawio/releases/tag/v${version}`,
    downloadUrl: (version: string) =>
      `https://github.com/jgraph/drawio/releases/download/v${version}/draw.war`,
  },
  {
    kind: "mathjax",
    displayName: "MathJax",
    version: "4.1.3",
    checksum: "23c036deccc0f2374834a47e4032e452419f3ac027bf17e17c104e2746b19f4c",
    fileName: "tex-svg.js",
    rustVersionConst: "MATHJAX_JS_VERSION",
    rustChecksumConst: "MATHJAX_JS_CHECKSUM",
    rustDownloadConst: "MATHJAX_DOWNLOAD_URL",
    latestUrl: "https://registry.npmjs.org/mathjax/latest",
    releasePageUrl: (version: string) =>
      `https://cdn.jsdelivr.net/npm/mathjax@${version}/tex-svg.js`,
    downloadUrl: (version: string) => `https://cdn.jsdelivr.net/npm/mathjax@${version}/tex-svg.js`,
  },
  {
    kind: "plantuml",
    displayName: "PlantUML JAR",
    version: "1.2026.9",
    checksum: "1dbb847646f0f440595fecd5f991e902aae47dc3391a01f64bab548089241a46",
    fileName: "plantuml.jar",
    rustVersionConst: "PLANTUML_JAR_VERSION",
    rustChecksumConst: "PLANTUML_JAR_CHECKSUM",
    rustDownloadConst: "PLANTUML_DOWNLOAD_URL",
    latestUrl:
      "https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/maven-metadata.xml",
    releasePageUrl: (version: string) =>
      `https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/${version}/plantuml-lgpl-${version}.jar`,
    downloadUrl: (version: string) =>
      `https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/${version}/plantuml-lgpl-${version}.jar`,
  },
];

export const RuntimeAssetCatalog = {
  all(): RuntimeAssetDefinition[] {
    return DEFINITIONS;
  },

  byKind(kind: string): RuntimeAssetDefinition {
    const definition = DEFINITIONS.find((it) => it.kind === kind);
    if (definition === undefined) {
      throw new Error(`Unknown runtime asset: ${kind}`);
    }
    return definition;
  },
};

export function runtimeAssetUpstreamVersion(definition: RuntimeAssetDefinition): string {
  return definition.source?.upstreamVersion ?? definition.version;
}

export function isPatchedRuntimeAsset(definition: RuntimeAssetDefinition): boolean {
  return definition.source?.kind === "patched-upstream";
}

export function runtimeAssetFreshnessEntries(
  source: string,
): readonly { kind: string; version: string; latestUrl: string }[] {
  const file = parse(source, {
    sourceType: "module",
    plugins: ["typescript"],
    errorRecovery: false,
  });
  const definitions = findDefinitions(file);
  const declaration = definitions[0];
  if (definitions.length !== 1 || declaration === undefined) {
    throw new Error("Runtime asset DEFINITIONS declaration must be unique");
  }
  return readDefinitions(declaration);
}

function findDefinitions(file: File): VariableDeclarator[] {
  const found: VariableDeclarator[] = [];
  for (const statement of file.program.body) {
    const declaration =
      statement.type === "VariableDeclaration"
        ? statement
        : statement.type === "ExportNamedDeclaration" &&
            statement.declaration?.type === "VariableDeclaration"
          ? statement.declaration
          : undefined;
    if (declaration === undefined) continue;
    for (const candidate of declaration.declarations) {
      if (
        isVariableDeclarator(candidate) &&
        isIdentifier(candidate.id) &&
        candidate.id.name === "DEFINITIONS"
      ) {
        found.push(candidate);
      }
    }
  }
  return found;
}

function readDefinitions(
  declaration: VariableDeclarator,
): readonly { kind: string; version: string; latestUrl: string }[] {
  if (
    declaration.init === null ||
    declaration.init === undefined ||
    !isArrayExpression(declaration.init)
  ) {
    throw new Error("Runtime asset DEFINITIONS must be an array literal");
  }
  const rows: { kind: string; version: string; latestUrl: string }[] = [];
  const kinds = new Set<string>();
  for (const element of declaration.init.elements) {
    if (element === null || element.type === "SpreadElement" || !isObjectExpression(element)) {
      throw new Error("Runtime asset definitions must be direct object literals");
    }
    const properties = objectProperties(element);
    const kind = stringProperty(properties, "kind");
    if (kinds.has(kind)) throw new Error(`Duplicate runtime asset kind: ${kind}`);
    kinds.add(kind);
    const version = assetVersion(properties);
    rows.push({ kind, version, latestUrl: stringProperty(properties, "latestUrl") });
  }
  if (rows.length === 0) throw new Error("Runtime asset DEFINITIONS must not be empty");
  return rows;
}

function objectProperties(object: ObjectExpression): Map<string, ObjectProperty | ObjectMethod> {
  const properties = new Map<string, ObjectProperty | ObjectMethod>();
  for (const member of object.properties) {
    if (isSpreadElement(member)) throw new Error("Runtime asset object spreads are not supported");
    if (!isObjectProperty(member) && !isObjectMethod(member))
      throw new Error("Unsupported runtime asset member");
    if (member.computed || (!isIdentifier(member.key) && !isStringLiteral(member.key))) {
      throw new Error("Runtime asset property names must be literal identifiers or strings");
    }
    const key = member.key.type === "Identifier" ? member.key.name : member.key.value;
    if (properties.has(key)) throw new Error(`Duplicate runtime asset key: ${key}`);
    properties.set(key, member);
  }
  return properties;
}

function stringProperty(
  properties: Map<string, ObjectProperty | ObjectMethod>,
  key: string,
): string {
  const property = properties.get(key);
  if (property === undefined || !isObjectProperty(property) || !isStringLiteral(property.value)) {
    throw new Error(`Runtime asset ${key} must be a direct string literal`);
  }
  if (property.value.value === "") throw new Error(`Runtime asset ${key} must not be empty`);
  return property.value.value;
}

function assetVersion(properties: Map<string, ObjectProperty | ObjectMethod>): string {
  const artifactVersion = stringProperty(properties, "version");
  const source = properties.get("source");
  if (source === undefined) return artifactVersion;
  if (!isObjectProperty(source) || !isObjectExpression(source.value)) {
    throw new Error("Runtime asset source must be an object literal");
  }
  const sourceProperties = objectProperties(source.value);
  if (stringProperty(sourceProperties, "kind") !== "patched-upstream") {
    throw new Error("Unsupported runtime asset source kind");
  }
  return stringProperty(sourceProperties, "upstreamVersion");
}

function runFreshnessCatalogCli(argv: readonly string[]): void {
  const [flag, catalogArgument] = argv;
  if (argv.length !== 2 || flag !== "--freshness-catalog" || catalogArgument === undefined) {
    throw new Error("Expected exactly --freshness-catalog <catalog.ts path>");
  }
  const catalogPath = path.resolve(catalogArgument);
  const source = fs.readFileSync(catalogPath, "utf8");
  process.stdout.write(`${JSON.stringify(runtimeAssetFreshnessEntries(source))}\n`);
}

export const RuntimeAssetCatalogSource = {
  updatePinnedAsset(
    source: string,
    kind: RuntimeAssetKind,
    version: string,
    checksum: string,
  ): string {
    let updated = RuntimeAssetCatalogSource.replaceString(source, kind, "version", version);
    updated = RuntimeAssetCatalogSource.replaceString(updated, kind, "checksum", checksum);
    return updated;
  },

  replaceString(
    source: string,
    kind: RuntimeAssetKind,
    propertyName: "version" | "checksum",
    value: string,
  ): string {
    const blockPattern = new RegExp(`(\\{\\n\\s+kind: "${kind}",[\\s\\S]*?\\n\\s+\\},)`);
    const block = source.match(blockPattern)?.[1];
    if (block === undefined) {
      throw new Error(`Runtime asset catalog entry not found: ${kind}`);
    }
    const propertyPattern = new RegExp(`(\\s+${propertyName}: )"[^"]+"`);
    if (!propertyPattern.test(block)) {
      throw new Error(`Runtime asset catalog property not found: ${kind}.${propertyName}`);
    }
    return source.replace(block, block.replace(propertyPattern, `$1"${value}"`));
  },
};

export const RuntimeAssetPaths = {
  vendorDir(definition: RuntimeAssetDefinition, version = definition.version): string {
    return path.join("crates", "katana-render-runtime", "vendor", definition.kind, version);
  },

  assetFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return path.join(RuntimeAssetPaths.vendorDir(definition, version), definition.fileName);
  },

  checksumFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    return `${RuntimeAssetPaths.assetFile(definition, version)}.sha256`;
  },

  compressedAssetFile(definition: RuntimeAssetDefinition, version = definition.version): string {
    if (definition.kind === "drawio") {
      return path.join("crates", "katana-render-runtime-assets", "assets", "drawio.min.js.br");
    }
    return `${RuntimeAssetPaths.assetFile(definition, version)}.br`;
  },

  justVersionVariable(definition: RuntimeAssetDefinition): string {
    if (definition.kind === "plantuml") {
      return "PLANTUML_JAR_VERSION";
    }
    return `${definition.kind.toUpperCase().replaceAll("-", "_")}_JS_VERSION`;
  },

  runtimeAssetsRust(): string {
    return path.join("crates", "katana-render-runtime", "src", "markdown", "runtime_assets.rs");
  },

  plantumlAssetRust(): string {
    return path.join(
      "crates",
      "katana-render-runtime",
      "src",
      "markdown",
      "plantuml_renderer",
      "asset.rs",
    );
  },

  rendererCargoToml(): string {
    return path.join("crates", "katana-render-runtime", "Cargo.toml");
  },

  mermaidDiagramUpdateScript(): string {
    return path.join("scripts", "mermaid", "diagram-update.ts");
  },

  justfile(): string {
    return "Justfile";
  },

  runtimeAssetCommon(): string {
    return path.join("scripts", "runtime-assets", "runtime-asset-common.ts");
  },
};

export const RuntimeAssetChecksum = {
  digestFile(filePath: string): string {
    const content = fs.readFileSync(filePath);
    return crypto.createHash("sha256").update(content).digest("hex");
  },

  readChecksumFile(filePath: string): string {
    const content = fs.readFileSync(filePath, "utf8").trim();
    const checksum = content.split(/\s+/).at(0);
    if (checksum === undefined) {
      throw new Error(`Runtime asset checksum file is empty: ${filePath}`);
    }
    return checksum;
  },

  writeChecksumFile(definition: RuntimeAssetDefinition, version: string): string {
    const assetFile = RuntimeAssetPaths.assetFile(definition, version);
    const checksum = RuntimeAssetChecksum.digestFile(assetFile);
    const checksumFile = RuntimeAssetPaths.checksumFile(definition, version);
    fs.writeFileSync(checksumFile, `${checksum}  ${definition.fileName}\n`, "utf8");
    return checksum;
  },
};

if (import.meta.main) {
  try {
    runFreshnessCatalogCli(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
