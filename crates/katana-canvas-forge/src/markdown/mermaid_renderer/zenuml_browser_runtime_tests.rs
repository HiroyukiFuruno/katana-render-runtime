use super::{
    ZenumlBrowserRenderOps, io_error_message, render_output, renderer_start_error_message,
    serialize_error_message, stdin_unavailable_message,
};
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_assets::RuntimeAsset;
use std::io::{Error as IoError, ErrorKind};
use std::process::{ExitStatus, Output};

#[cfg(unix)]
const UNIX_EXIT_STATUS_SHIFT: i32 = 8;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;

#[test]
fn renders_zenuml_svg_through_browser_runtime() {
    let mermaid = RuntimeAsset::mermaid();
    let rendered = mermaid
        .materialize_at(mermaid.materialized_path())
        .and_then(|mermaid_js| {
            ZenumlBrowserRenderOps::render(
                "zenuml\ntitle Unit Test\nA.method()",
                &mermaid_js,
                DiagramColorPreset::dark(),
                "unit-zenuml".to_string(),
            )
        });

    assert!(
        rendered.as_ref().is_ok_and(|svg| [
            "<foreignObject",
            "Unit Test",
            "viewBox=",
            "width=\"1520\"",
        ]
        .iter()
        .all(|expected| svg.contains(expected))),
        "{rendered:?}"
    );
}

#[test]
fn render_output_maps_success_stdout() {
    let success = render_output(output(0, "<svg></svg>", ""));

    assert!(matches!(success, Ok(svg) if svg == "<svg></svg>"));
}

#[test]
fn render_output_maps_stderr_failure_detail() {
    let failure = render_output(output(1, "", "boom"));

    assert!(matches!(failure, Err(error) if error.contains("boom")));
}

#[test]
fn render_output_maps_stdout_failure_detail() {
    let stdout_failure = render_output(output(1, "stdout boom", ""));

    assert!(matches!(stdout_failure, Err(error) if error.contains("stdout boom")));
}

#[test]
fn render_output_maps_unknown_failure_detail() {
    let unknown = render_output(output(1, "", ""));

    assert!(matches!(unknown, Err(error) if error.contains("unknown")));
}

#[test]
fn render_output_rejects_invalid_utf8_stdout() {
    let utf8_error = render_output(raw_output(0, vec![0xff], Vec::new()));

    assert!(utf8_error.is_err());
}

#[test]
fn error_message_helpers_preserve_source_details() {
    let start_error =
        renderer_start_error_message(IoError::new(ErrorKind::NotFound, "node missing"));
    let write_error = io_error_message(IoError::new(ErrorKind::BrokenPipe, "pipe closed"));
    let json_error = serde_json::from_str::<serde_json::Value>("{")
        .map(|_| String::new())
        .map_err(serialize_error_message);

    assert!(start_error.contains("node missing"));
    assert!(write_error.contains("pipe closed"));
    assert!(stdin_unavailable_message().contains("stdin"));
    assert!(matches!(json_error, Err(error) if error.contains("EOF")));
}

fn output(code: i32, stdout: &str, stderr: &str) -> Output {
    raw_output(code, stdout.as_bytes().to_vec(), stderr.as_bytes().to_vec())
}

fn raw_output(code: i32, stdout: Vec<u8>, stderr: Vec<u8>) -> Output {
    Output {
        status: exit_status(code),
        stdout,
        stderr,
    }
}

#[cfg(unix)]
fn exit_status(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << UNIX_EXIT_STATUS_SHIFT)
}

#[cfg(windows)]
fn exit_status(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code as u32)
}
