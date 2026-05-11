use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_assets::RuntimeAsset;
use crate::system::ProcessService;
use serde::Serialize;
use std::io::{Error as IoError, Write};
use std::path::Path;
use std::process::{Output, Stdio};

const RENDERER_SCRIPT: &str = include_str!("js_runtime/render_zenuml_browser.mjs");

pub(super) struct ZenumlBrowserRenderOps;

impl ZenumlBrowserRenderOps {
    pub(super) fn render(
        source: &str,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        svg_id: String,
    ) -> Result<String, String> {
        let zenuml_js = materialize_zenuml_asset()?;
        let request = ZenumlBrowserRequest::new(source, mermaid_js, &zenuml_js, preset, svg_id);
        let output =
            Self::run_renderer(serde_json::to_vec(&request).map_err(serialize_error_message)?)?;
        render_output(output)
    }

    fn run_renderer(input: Vec<u8>) -> Result<Output, String> {
        let mut child = ProcessService::create_command("node")
            .arg("--input-type=module")
            .arg("--eval")
            .arg(RENDERER_SCRIPT)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(renderer_start_error_message)?;
        let mut stdin = child.stdin.take().ok_or_else(stdin_unavailable_message)?;
        stdin.write_all(&input).map_err(io_error_message)?;
        drop(stdin);
        child.wait_with_output().map_err(io_error_message)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ZenumlBrowserRequest<'a> {
    source: &'a str,
    mermaid_js: String,
    zenuml_js: String,
    svg_id: String,
    theme: &'a str,
    background: &'a str,
    fill: &'a str,
    text: &'a str,
    stroke: &'a str,
    arrow: &'a str,
}

impl<'a> ZenumlBrowserRequest<'a> {
    fn new(
        source: &'a str,
        mermaid_js: &Path,
        zenuml_js: &Path,
        preset: &'a DiagramColorPreset,
        svg_id: String,
    ) -> Self {
        Self {
            source,
            mermaid_js: mermaid_js.to_string_lossy().into_owned(),
            zenuml_js: zenuml_js.to_string_lossy().into_owned(),
            svg_id,
            theme: preset.mermaid_theme,
            background: preset.background,
            fill: preset.fill,
            text: preset.text,
            stroke: preset.stroke,
            arrow: preset.arrow,
        }
    }
}

fn materialize_zenuml_asset() -> Result<std::path::PathBuf, String> {
    let asset = RuntimeAsset::mermaid_zenuml();
    asset.materialize_at(asset.materialized_path())
}

fn serialize_error_message(error: serde_json::Error) -> String {
    error.to_string()
}

fn renderer_start_error_message(error: IoError) -> String {
    format!("Failed to start ZenUML browser renderer: {error}")
}

fn stdin_unavailable_message() -> String {
    "ZenUML browser renderer stdin is unavailable".to_string()
}

fn io_error_message(error: IoError) -> String {
    error.to_string()
}

fn render_output(output: Output) -> Result<String, String> {
    if output.status.success() {
        return String::from_utf8(output.stdout).map_err(|it| it.to_string());
    }
    Err(process_error_message(output))
}

fn process_error_message(output: Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = [stderr.trim(), stdout.trim()]
        .into_iter()
        .find(|it| !it.is_empty())
        .unwrap_or("unknown ZenUML browser renderer failure");
    format!("ZenUML browser renderer failed: {detail}")
}

#[cfg(test)]
#[path = "zenuml_browser_runtime_tests.rs"]
mod tests;
