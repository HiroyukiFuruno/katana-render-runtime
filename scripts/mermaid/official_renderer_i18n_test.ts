import { expect, test } from "bun:test";
import path from "node:path";
import { MermaidRuntimeScripts } from "./official-renderer-i18n";

const runtimeScriptNames = MermaidRuntimeScripts.paths().map((it) => path.basename(it));

test("公式参照生成は Mermaid normalizer の依存ファイルを利用側より先に読む", () => {
  expectLoadedBefore("svg_class_layout_fixes.js", "svg_class_fixes.js");
  expectLoadedBefore("svg_mindmap_layout_fixes.js", "svg_diagram_normalizers.js");
  expectLoadedBefore("svg_requirement_localized_fixes.js", "svg_visual_fixes.js");
  expectLoadedBefore("svg_sequence_activation_fixes.js", "svg_normalize.js");
});

function expectLoadedBefore(dependency: string, consumer: string) {
  expect(runtimeScriptNames).toContain(dependency);
  expect(runtimeScriptNames).toContain(consumer);
  expect(runtimeScriptNames.indexOf(dependency)).toBeLessThan(runtimeScriptNames.indexOf(consumer));
}
