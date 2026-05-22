import {
  RuntimeBundleDefinition,
} from "../shared/runtime_bundle";

const MATHJAX_JS_RUNTIME =
  "crates/katana-render-runtime/src/markdown/mathjax_renderer/js_runtime";

export const MathJaxRuntimeBundleDefinition = {
  create(): RuntimeBundleDefinition {
    return new RuntimeBundleDefinition(
      "mathjax",
      "mathjax-runtime.min.js",
      [],
      MATHJAX_RUNTIME_ENTRYPOINT,
      `${MATHJAX_JS_RUNTIME}/render_mathjax.js`,
    );
  },
};

const MATHJAX_RUNTIME_ENTRYPOINT = `
`;
