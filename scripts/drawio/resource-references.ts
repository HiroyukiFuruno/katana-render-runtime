import fs from "node:fs";
import { type DrawioResourceReference, DrawioSourceKind } from "./resource-common";

const IMAGE_EXTENSIONS = [".gif", ".jpeg", ".jpg", ".png", ".svg", ".webp"];
const RESOURCE_GROUP_BY_PREFIX = new Map([
  ["arrows2", "arrows"],
  ["ios", "ios7"],
  ["ios7", "ios7"],
  ["ios7ui", "ios7"],
  ["pid2misc", "pid2"],
  ["pid2valves", "pid2"],
  ["rackgeneral", "rack"],
  ["veeam2", "veeam"],
]);

export class DrawioResourceReferenceExtractor {
  references(fixturePath: string): DrawioResourceReference[] {
    const source = fs.readFileSync(fixturePath, "utf8");
    return [
      ...this.imageReferences(fixturePath, source),
      ...this.stencilReferences(fixturePath, source),
    ].sort((left, right) => left.logicalPath.localeCompare(right.logicalPath));
  }

  private imageReferences(fixturePath: string, source: string): DrawioResourceReference[] {
    return Array.from(source.matchAll(/image=([^;"&<]+)/g))
      .map((match) => DrawioResourceValue.normalize(DrawioResourceMatch.capture(match)))
      .filter(DrawioResourceValue.isAuditableImage)
      .map((rawValue) => DrawioResourceValue.logicalImagePath(rawValue))
      .filter((logicalPath) => logicalPath.length > 0)
      .map((logicalPath) => ({
        fixturePath,
        logicalPath,
        sourceKind: DrawioSourceKind.fromLogicalPath(logicalPath),
        rawValue: logicalPath,
      }));
  }

  private stencilReferences(fixturePath: string, source: string): DrawioResourceReference[] {
    return Array.from(source.matchAll(/shape=mxgraph\.([^;"&<]+)/g))
      .map((match) => DrawioResourceValue.stencilGroup(DrawioResourceMatch.capture(match)))
      .flatMap((group) => DrawioResourceValue.stencilLogicalPaths(group))
      .map((logicalPath) => ({
        fixturePath,
        logicalPath,
        sourceKind: "drawio-stencil",
        rawValue: logicalPath,
      }));
  }
}

const DrawioResourceMatch = {
  capture(match: RegExpMatchArray): string {
    const value = match.at(1);
    if (value === undefined) {
      throw new Error("Draw.io resource reference was not captured");
    }
    return value;
  },
};

const DrawioResourceValue = {
  normalize(value: string): string {
    return DrawioResourceValue.decode(value).trim().replace(/^\/+/, "");
  },

  isAuditableImage(value: string): boolean {
    if (value.startsWith("data:image/")) {
      return false;
    }
    const pathname = DrawioResourceValue.pathname(value);
    return IMAGE_EXTENSIONS.some((extension) => pathname.toLowerCase().endsWith(extension));
  },

  logicalImagePath(value: string): string {
    return DrawioResourceValue.pathname(value).replace(/^\/+/, "");
  },

  stencilGroup(value: string): string {
    const prefix = value.split(/[.;"'& ]/)[0] || "";
    return DrawioResourceValue.resourceGroup(prefix.toLowerCase());
  },

  stencilLogicalPaths(group: string): string[] {
    return [`stencils/${group}`];
  },

  resourceGroup(prefix: string): string {
    return RESOURCE_GROUP_BY_PREFIX.get(prefix) || prefix;
  },

  decode(value: string): string {
    try {
      return decodeURIComponent(value);
    } catch {
      return value;
    }
  },

  pathname(value: string): string {
    return new URL(value, "https://katana.local").pathname;
  },
};
