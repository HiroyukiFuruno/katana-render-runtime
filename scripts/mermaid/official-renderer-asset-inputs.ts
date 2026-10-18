import path from "node:path";
import { getOptionValue } from "../shared/cli-options";

const DEFAULT_MERMAID_JS =
  "crates/katana-render-runtime/vendor/mermaid/12.2.0-krr.pr6823.1/mermaid.min.js";
const DEFAULT_MERMAID_ZENUML_JS =
  "crates/katana-render-runtime/vendor/mermaid-zenuml/1.0.1/mermaid-zenuml.min.js";

export interface OfficialRendererAssetInputs {
  mermaidJs: string;
  mermaidZenumlJs: string;
}

export function resolveOfficialRendererAssetInputs(
  argv: readonly string[],
  env: Readonly<Record<string, string | undefined>>,
  cwd: string,
  home: string,
): OfficialRendererAssetInputs {
  const mermaidJs = getOptionValue(argv, "--mermaid-js", env.MERMAID_JS || DEFAULT_MERMAID_JS);
  const mermaidZenumlJs = getOptionValue(
    argv,
    "--mermaid-zenuml-js",
    env.MERMAID_ZENUML_JS || DEFAULT_MERMAID_ZENUML_JS,
  );

  return {
    mermaidJs: path.resolve(cwd, expandHome(mermaidJs, home)),
    mermaidZenumlJs: path.resolve(cwd, expandHome(mermaidZenumlJs, home)),
  };
}

function expandHome(value: string, home: string): string {
  return value.startsWith("~/") ? path.join(home, value.slice(2)) : value;
}
