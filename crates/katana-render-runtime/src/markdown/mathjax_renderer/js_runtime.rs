use super::js_runtime_scripts::MathJaxRuntimeScripts;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
#[cfg(test)]
use crate::markdown::runtime_resource_loader::{
    BundledResource, RuntimeResourceLoader as runtime_resource_loader,
};
use serde::Deserialize;
#[cfg(test)]
use std::path::Path;

pub(super) struct MathJaxJsRuntimeOps;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MathJaxRuntimeResponse {
    Svg { svg: String },
    Error { message: String },
}

impl MathJaxJsRuntimeOps {
    #[cfg(test)]
    pub(super) fn render(
        source: &str,
        mathjax_js: &Path,
        preset: &DiagramColorPreset,
        display: bool,
    ) -> Result<String, String> {
        Self::render_with_origin(source, mathjax_js, preset, display, false)
    }

    #[cfg(test)]
    pub(super) fn render_with_origin(
        source: &str,
        mathjax_js: &Path,
        preset: &DiagramColorPreset,
        display: bool,
        bundled: bool,
    ) -> Result<String, String> {
        let mathjax_source = if bundled {
            runtime_resource_loader::read(mathjax_js, Some(BundledResource::MathJax))
                .map_err(|error| error.to_string())?
        } else {
            runtime_source_from(mathjax_js)?.into()
        };
        Self::render_bundle(source, &mathjax_source, preset, display)
    }

    pub(super) fn render_bundle(
        source: &str,
        mathjax_source: &str,
        preset: &DiagramColorPreset,
        display: bool,
    ) -> Result<String, String> {
        let request = MathJaxRenderRequest::new(source, preset, display);
        let request_json = request.to_json_value().to_string();
        let scripts = MathJaxRuntimeScripts::build(mathjax_source, &request_json);
        let output = DiagramV8Runtime::render(&scripts)?;
        match parse_response(&output)? {
            MathJaxRuntimeResponse::Svg { svg } => Ok(svg),
            MathJaxRuntimeResponse::Error { message } => Err(message),
        }
    }
}

#[cfg(test)]
fn runtime_source_from(mathjax_js: &Path) -> Result<String, String> {
    match runtime_resource_loader::read_file(mathjax_js) {
        Ok(source) => Ok(source),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "MathJax runtime asset is not installed: {}",
            mathjax_js.display()
        )),
        Err(error) => Err(format!(
            "MathJax runtime asset could not be read: {}: {error}",
            mathjax_js.display()
        )),
    }
}

struct MathJaxRenderRequest<'a> {
    source: &'a str,
    display: bool,
    text: &'a str,
    dark_mode: bool,
}

impl<'a> MathJaxRenderRequest<'a> {
    fn new(source: &'a str, preset: &'a DiagramColorPreset, display: bool) -> Self {
        Self {
            source,
            display,
            text: preset.text.as_ref(),
            dark_mode: preset.dark_mode,
        }
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "source": self.source,
            "display": self.display,
            "text": self.text,
            "darkMode": self.dark_mode,
        })
    }
}

fn parse_response(output: &str) -> Result<MathJaxRuntimeResponse, String> {
    serde_json::from_str(output).map_err(|err| format!("Invalid MathJax runtime response: {err}"))
}

#[cfg(test)]
mod tests {
    use super::MathJaxJsRuntimeOps;
    use super::{MathJaxRuntimeResponse, parse_response, runtime_source_from};
    use crate::markdown::color_preset::DiagramColorPreset;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn runtime_source_reports_non_file_read_errors() {
        let path = std::env::temp_dir().join(format!(
            "krr-mathjax-runtime-directory-{}",
            std::process::id()
        ));
        assert!(std::fs::create_dir_all(&path).is_ok());

        let result = runtime_source_from(&path);

        assert!(matches!(result, Err(error) if error.contains("could not be read")));
    }

    #[test]
    fn runtime_source_reports_missing_assets() {
        let path = std::env::temp_dir().join(format!(
            "krr-mathjax-runtime-missing-{}",
            std::process::id()
        ));

        let result = runtime_source_from(&path);

        assert!(matches!(result, Err(error) if error.contains("not installed")));
    }

    #[test]
    fn parses_svg_error_and_invalid_runtime_responses() {
        let svg = parse_response(r#"{"kind":"svg","svg":"<svg/>"}"#);
        let error = parse_response(r#"{"kind":"error","message":"invalid TeX"}"#);
        let invalid = parse_response("not-json");

        assert!(matches!(svg, Ok(MathJaxRuntimeResponse::Svg { svg }) if svg == "<svg/>"));
        assert!(
            matches!(error, Ok(MathJaxRuntimeResponse::Error { message }) if message == "invalid TeX")
        );
        assert!(
            matches!(invalid, Err(message) if message.contains("Invalid MathJax runtime response"))
        );
    }

    #[test]
    fn render_propagates_runtime_and_response_contract_errors() {
        let path = runtime_path("contract-errors.js");
        let preset = DiagramColorPreset::current();

        assert!(
            std::fs::write(
                &path,
                "function katanaRunMathJaxRuntime() { return 'not-json'; }",
            )
            .is_ok()
        );
        let invalid_response = MathJaxJsRuntimeOps::render("x", &path, preset, false);

        assert!(
            std::fs::write(
                &path,
                "function katanaRunMathJaxRuntime() { throw new Error('runtime failure'); }",
            )
            .is_ok()
        );
        let runtime_failure = MathJaxJsRuntimeOps::render("x", &path, preset, false);

        assert!(
            matches!(invalid_response, Err(message) if message.contains("Invalid MathJax runtime response"))
        );
        assert!(matches!(runtime_failure, Err(message) if message.contains("runtime failure")));
    }

    #[test]
    fn render_returns_svg_and_runtime_reported_errors() {
        let path = runtime_path("runtime-responses.js");
        let preset = DiagramColorPreset::current();

        assert!(
            std::fs::write(
                &path,
                r#"function katanaRunMathJaxRuntime() { return '{"kind":"svg","svg":"<svg/>"}'; }"#,
            )
            .is_ok()
        );
        let svg = MathJaxJsRuntimeOps::render("x", &path, preset, false);

        assert!(std::fs::write(
            &path,
            r#"function katanaRunMathJaxRuntime() { return '{"kind":"error","message":"bad math"}'; }"#,
        )
        .is_ok());
        let error = MathJaxJsRuntimeOps::render("x", &path, preset, false);

        assert!(matches!(svg, Ok(value) if value == "<svg/>"));
        assert!(matches!(error, Err(message) if message == "bad math"));
    }

    fn runtime_path(name: &str) -> std::path::PathBuf {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "krr-mathjax-runtime-{name}-{}-{id}",
            std::process::id()
        ))
    }
}
