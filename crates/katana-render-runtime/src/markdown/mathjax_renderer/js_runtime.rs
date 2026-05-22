use super::js_runtime_scripts::MathJaxRuntimeScripts;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use serde::Deserialize;
use std::path::Path;

pub(super) struct MathJaxJsRuntimeOps;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MathJaxRuntimeResponse {
    Svg { svg: String },
    Error { message: String },
}

impl MathJaxJsRuntimeOps {
    pub(super) fn render(
        source: &str,
        mathjax_js: &Path,
        preset: &DiagramColorPreset,
        display: bool,
    ) -> Result<String, String> {
        if !mathjax_js.exists() {
            return Err(format!(
                "MathJax runtime asset is not installed: {}",
                mathjax_js.display()
            ));
        }
        let request = MathJaxRenderRequest::new(source, preset, display);
        let request_json = request.to_json_value().to_string();
        let scripts = MathJaxRuntimeScripts::build(&request_json);
        let output = DiagramV8Runtime::render(&scripts)?;
        match parse_response(&output)? {
            MathJaxRuntimeResponse::Svg { svg } => Ok(svg),
            MathJaxRuntimeResponse::Error { message } => Err(message),
        }
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
