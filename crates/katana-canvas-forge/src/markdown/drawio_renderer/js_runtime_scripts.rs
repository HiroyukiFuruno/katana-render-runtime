use crate::markdown::diagram_js_runtime::DiagramRuntimeScript;

pub(super) struct DrawioRuntimeScripts;

impl DrawioRuntimeScripts {
    pub(super) fn build<'a>(bundle: &'a str, request_json: &str) -> Vec<DiagramRuntimeScript<'a>> {
        vec![
            DiagramRuntimeScript::owned("drawio-request.js", request_script(request_json)),
            DiagramRuntimeScript::borrowed("drawio-runtime.min.js", DRAWIO_RUNTIME),
            DiagramRuntimeScript::borrowed("drawio.min.js", bundle),
            DiagramRuntimeScript::borrowed("render-drawio.js", "katanaRunDrawioRuntime();"),
        ]
    }
}

fn request_script(request_json: &str) -> String {
    format!("globalThis.__katanaDrawioRequest = {request_json};")
}

const DRAWIO_RUNTIME: &str = include_str!("../diagram_runtime/generated/drawio-runtime.min.js");

#[cfg(test)]
mod tests {
    use super::DrawioRuntimeScripts;

    #[test]
    fn build_includes_bundle_and_render_script() {
        let scripts = DrawioRuntimeScripts::build("bundle", "{}");
        assert!(scripts.iter().any(|it| it.name == "drawio-request.js"));
        assert!(scripts.iter().any(|it| it.name == "drawio-runtime.min.js"));
        assert!(scripts.iter().any(|it| it.name == "drawio.min.js"));
        assert!(scripts.iter().any(|it| it.name == "render-drawio.js"));
    }
}
