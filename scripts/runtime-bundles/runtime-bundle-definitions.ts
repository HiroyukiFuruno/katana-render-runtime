import { DrawioRuntimeBundleDefinition } from "#drawio/runtime_definition";
import { MermaidRuntimeBundleDefinition } from "#mermaid/runtime_definition";
import type { RuntimeBundleDefinition } from "#shared/runtime_bundle";
import { ZenumlRuntimeBundleDefinition } from "#zenuml/runtime_definition";
import { MathJaxRuntimeBundleDefinition } from "../../crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition";

// 製品生成と検証で同じ入力集合を参照するため、定義の入口を共有する。
export function runtimeBundleDefinitions(): RuntimeBundleDefinition[] {
  return [
    MermaidRuntimeBundleDefinition.create(),
    DrawioRuntimeBundleDefinition.create(),
    ZenumlRuntimeBundleDefinition.create(),
    MathJaxRuntimeBundleDefinition.create(),
  ];
}
