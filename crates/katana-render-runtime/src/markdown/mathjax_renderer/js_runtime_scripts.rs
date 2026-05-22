use crate::markdown::diagram_js_runtime::DiagramRuntimeScript;

pub(super) struct MathJaxRuntimeScripts;

impl MathJaxRuntimeScripts {
    pub(super) fn build(
        runtime_source: String,
        request_json: &str,
    ) -> Vec<DiagramRuntimeScript<'static>> {
        vec![
            DiagramRuntimeScript::owned("mathjax-runtime.min.js", runtime_source),
            DiagramRuntimeScript::owned(
                "render-mathjax.js",
                format!("katanaRunMathJaxRuntime({request_json});"),
            ),
        ]
    }

    pub(super) fn runtime_source() -> &'static str {
        MATHJAX_RUNTIME
    }
}

const MATHJAX_RUNTIME: &str = include_str!("../diagram_runtime/generated/mathjax-runtime.min.js");

#[cfg(test)]
mod tests {
    use super::MathJaxRuntimeScripts;

    #[test]
    fn build_loads_setup_before_mathjax_bundle() {
        let scripts = MathJaxRuntimeScripts::build("runtime".to_string(), "{}");

        assert_eq!(scripts[0].name, "mathjax-runtime.min.js");
        assert_eq!(scripts[1].name, "render-mathjax.js");
    }
}
