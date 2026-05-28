import type { RuntimeBundleName } from "#shared/runtime_bundle";

export function runtimeEntryName(name: RuntimeBundleName): string {
  if (name === "mermaid") {
    return "katanaRunMermaidRuntime";
  }
  if (name === "drawio") {
    return "katanaRunDrawioRuntime";
  }
  if (name === "zenuml") {
    return "katanaRunZenumlRuntime";
  }
  return "katanaRunMathJaxRuntime";
}

export function terserReservedRuntimeEntryNames(): string[] {
  return [
    "katanaRunMermaidRuntime",
    "katanaRunDrawioRuntime",
    "katanaRunZenumlRuntime",
    "katanaRunMathJaxRuntime",
  ];
}
