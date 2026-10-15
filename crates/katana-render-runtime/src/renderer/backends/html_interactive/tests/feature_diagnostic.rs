use super::super::HtmlInteractiveSession;
use super::support::{TestResult, click_element, to_string};
use crate::renderer::backends::html_browser::{HtmlBrowserSource, HtmlBrowserViewport};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[test]
fn delayed_timer_requires_host_time_advance() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<output id=state>waiting</output><script>setTimeout(() => document.querySelector('#state').textContent = 'done', 25)</script>",
        "https://example.test/timer.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    assert_state(&mut session, "waiting")?;
    session.advance_time(24.0).map_err(to_string)?;
    assert_state(&mut session, "waiting")?;
    session.advance_time(1.0).map_err(to_string)?;
    assert_state(&mut session, "done")?;
    Ok(())
}

#[test]
fn timed_out_event_rejects_later_timer_advance() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<button id=run style='margin:0' onclick=\"for (;;) {}\">Run</button>",
        "https://example.test/timeout.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    let timeout = click_element(&mut session, "run")
        .err()
        .ok_or("infinite event handler must time out")?;
    assert!(
        timeout.contains("timed out"),
        "unexpected timeout error: {timeout}"
    );
    let discarded = session
        .advance_time(1.0)
        .err()
        .ok_or("discarded runtime must reject timer advancement")?;
    assert!(
        discarded.to_string().contains("discarded"),
        "unexpected discarded runtime error: {discarded}"
    );
    Ok(())
}

fn assert_state(session: &mut HtmlInteractiveSession, expected: &str) -> TestResult {
    let actual = session
        .runtime
        .observe_feature_state("document.querySelector('#state')?.textContent")
        .map_err(to_string)?;
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn writes_fixture_feature_state_interactive() -> TestResult {
    let Some(request) = FeatureRequest::read()? else {
        return Ok(());
    };
    let (mut session, html) = request.session()?;
    apply_case_operation(&mut session, &request.case_id)?;
    write_feature_report(&request, &html, &mut session)
}

struct FeatureRequest {
    input: PathBuf,
    expression: String,
    case_id: String,
    nonce: String,
    binary_sha: String,
    output: PathBuf,
    width: u32,
    height: u32,
}

impl FeatureRequest {
    fn read() -> Result<Option<Self>, String> {
        let Some(input) = std::env::var_os("KRR_HTML_FEATURE_INPUT") else {
            return Ok(None);
        };
        Ok(Some(Self {
            input: PathBuf::from(input).canonicalize().map_err(to_string)?,
            expression: std::env::var("KRR_HTML_FEATURE_EXPRESSION").map_err(to_string)?,
            case_id: std::env::var("KRR_HTML_FEATURE_CASE").map_err(to_string)?,
            nonce: std::env::var("KRR_HTML_FEATURE_NONCE").map_err(to_string)?,
            binary_sha: std::env::var("KRR_HTML_FEATURE_BINARY_SHA256").map_err(to_string)?,
            output: PathBuf::from(
                std::env::var_os("KRR_HTML_FEATURE_OUTPUT").ok_or("missing output")?,
            ),
            width: std::env::var("KRR_HTML_FEATURE_WIDTH")
                .unwrap_or_else(|_| "1280".to_owned())
                .parse()
                .map_err(to_string)?,
            height: std::env::var("KRR_HTML_FEATURE_HEIGHT")
                .unwrap_or_else(|_| "900".to_owned())
                .parse()
                .map_err(to_string)?,
        }))
    }

    fn session(&self) -> TestResult<(HtmlInteractiveSession, String)> {
        let html = fs::read_to_string(&self.input).map_err(to_string)?;
        let origin = url::Url::from_file_path(&self.input).map_err(|()| "invalid origin")?;
        let source = HtmlBrowserSource::new(html.clone(), origin.as_str()).map_err(to_string)?;
        let viewport = HtmlBrowserViewport::new(self.width, self.height, 1.0).map_err(to_string)?;
        let session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
        Ok((session, html))
    }
}

fn write_feature_report(
    request: &FeatureRequest,
    html: &str,
    session: &mut HtmlInteractiveSession,
) -> TestResult {
    let state = session
        .runtime
        .observe_feature_state(&request.expression)
        .map_err(to_string)?;
    let frame = session
        .latest_frame()
        .ok_or("missing post-operation frame")?;
    let capture = request.output.with_extension("png");
    let capture_sha256 = save_feature_capture(request, frame, &capture)?;
    let report = feature_report(
        request,
        html,
        state,
        capture,
        capture_sha256,
        &session.hit_targets,
    );
    fs::write(
        &request.output,
        serde_json::to_vec_pretty(&report).map_err(to_string)?,
    )
    .map_err(to_string)
}

fn save_feature_capture(
    request: &FeatureRequest,
    frame: &crate::renderer::backends::html_browser::HtmlBrowserFrame,
    capture: &PathBuf,
) -> TestResult<String> {
    image::save_buffer_with_format(
        capture,
        &frame.pixels,
        request.width,
        request.height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(to_string)?;
    Ok(Sha256::digest(fs::read(capture).map_err(to_string)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn feature_report(
    request: &FeatureRequest,
    html: &str,
    state: serde_json::Value,
    capture: PathBuf,
    capture_sha256: String,
    targets: &[super::super::types::HitTarget],
) -> serde_json::Value {
    let geometry = targets
        .iter()
        .map(|target| {
            json!({
                "nodeId": target.node_id,
                "x": target.x,
                "y": target.y,
                "width": target.width,
                "height": target.height,
            })
        })
        .collect::<Vec<_>>();
    json!({"htmlSha256": html_hash(html), "executionNonce": request.nonce, "binarySha256": request.binary_sha, "caseId": request.case_id, "state": state, "capturePath": capture, "captureSha256": capture_sha256, "viewport": {"width": request.width, "height": request.height, "scale": 1}, "geometry": geometry})
}

fn html_hash(html: &str) -> String {
    Sha256::digest(html.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn apply_case_operation(session: &mut HtmlInteractiveSession, case_id: &str) -> TestResult {
    if case_id == "action-pointer" {
        return click_element(session, "action");
    }
    if case_id == "events" {
        click_element(session, "field")?;
        return apply_events_operation(session);
    }
    let Some(operation) = case_id.strip_prefix("input-state:") else {
        return Ok(());
    };
    apply_input_state_operation(session, operation)
}

fn apply_input_state_operation(
    session: &mut HtmlInteractiveSession,
    operation: &str,
) -> TestResult {
    match operation {
        "empty" | "disabled" => Ok(()),
        "filled" => {
            click_element(session, "name")?;
            session
                .dispatch_input(
                    crate::renderer::backends::html_browser::HtmlBrowserInput::Text {
                        text: "架空の名称".to_owned(),
                    },
                )
                .map_err(to_string)?;
            click_element(session, "save")
        }
        "selected" => click_element(session, "tab-advanced"),
        "expanded" => click_element(session, "accordion"),
        "focus-visible" => session
            .dispatch_input(
                crate::renderer::backends::html_browser::HtmlBrowserInput::KeyDown {
                    key: "Tab".to_owned(),
                },
            )
            .map_err(to_string),
        _ => Err(format!("unknown input-state operation: {operation}")),
    }
}

fn apply_events_operation(session: &mut HtmlInteractiveSession) -> TestResult {
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::Text {
                text: "入力".to_owned(),
            },
        )
        .map_err(to_string)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::KeyDown {
                key: "Shift".to_owned(),
            },
        )
        .map_err(to_string)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::KeyUp {
                key: "Shift".to_owned(),
            },
        )
        .map_err(to_string)?;
    let target = submit_target(session)?;
    let x = target.x + 1.0;
    let y = target.y + 1.0 - session.scroll_y;
    dispatch_pointer_pair(session, x, y)
}

fn submit_target(
    session: &mut HtmlInteractiveSession,
) -> TestResult<super::super::types::HitTarget> {
    let button = session
        .runtime
        .query_feature_node("button")
        .ok_or("missing submit button node")?;
    session
        .hit_targets
        .iter()
        .find(|target| target.node_id == button.0)
        .cloned()
        .ok_or_else(|| "missing submit button target".to_string())
}

fn dispatch_pointer_pair(session: &mut HtmlInteractiveSession, x: f32, y: f32) -> TestResult {
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::PointerDown {
                x,
                y,
                button: 0,
            },
        )
        .map_err(to_string)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::PointerUp {
                x,
                y,
                button: 0,
            },
        )
        .map_err(to_string)
}

#[cfg(test)]
mod tests {
    use super::{FeatureRequest, feature_report};
    use std::path::PathBuf;

    const ACTION_POINTER_VIEWPORTS: [(u32, u32); 3] = [(1280, 900), (960, 720), (620, 900)];
    const ACTION_POINTER_FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/html/interaction-fidelity.html"
    ));

    fn assert_action_state(
        session: &mut super::HtmlInteractiveSession,
        expanded: &str,
        result: &str,
    ) -> super::TestResult {
        let actual = session.runtime.observe_feature_state(
            "({expanded:document.querySelector('#action').getAttribute('aria-expanded'),result:document.querySelector('#result').textContent})",
        ).map_err(super::to_string)?;
        assert_eq!(
            actual,
            serde_json::json!({"expanded": expanded, "result": result})
        );
        Ok(())
    }

    #[test]
    fn action_pointer_case_clicks_authored_fixture_at_all_performance_viewports()
    -> super::TestResult {
        for (width, height) in ACTION_POINTER_VIEWPORTS {
            let mut session =
                super::super::support::start_with_viewport(ACTION_POINTER_FIXTURE, width, height)?;
            assert_action_state(&mut session, "false", "")?;
            super::apply_case_operation(&mut session, "action-pointer")?;
            assert_action_state(&mut session, "true", "opened")?;
            super::apply_case_operation(&mut session, "action-pointer")?;
            assert_action_state(&mut session, "false", "closed")?;
        }
        Ok(())
    }

    #[test]
    fn feature_report_preserves_state_provenance_and_empty_geometry() {
        let request = FeatureRequest {
            input: PathBuf::from("fixture.html"),
            expression: "true".to_owned(),
            case_id: "input-state:empty".to_owned(),
            nonce: "nonce".to_owned(),
            binary_sha: "binary".to_owned(),
            output: PathBuf::from("report.json"),
            width: 320,
            height: 240,
        };
        let report = feature_report(
            &request,
            "<html></html>",
            serde_json::json!({"state": "ok"}),
            PathBuf::from("capture.png"),
            "capture-sha".to_owned(),
            &[],
        );
        assert_eq!(report["caseId"], "input-state:empty");
        assert_eq!(report["viewport"]["width"], 320);
        assert_eq!(report["captureSha256"], "capture-sha");
        assert!(
            report["geometry"]
                .as_array()
                .is_some_and(|items| items.is_empty())
        );
    }
}

#[cfg(test)]
mod child_process_tests {
    use super::html_hash;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};

    const FEATURE_TEST_NAME: &str = "renderer::backends::html_interactive::tests::feature_diagnostic::writes_fixture_feature_state_interactive";

    fn child_workspace() -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "krr-feature-diagnostic-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).map_err(|error| error.to_string())?;
        Ok(path)
    }

    fn current_binary_sha() -> Result<String, String> {
        let bytes = std::fs::read(std::env::current_exe().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        Ok(Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    fn run_feature_child(
        input: &Path,
        output: &Path,
        expression: &str,
        binary_sha: &str,
    ) -> Result<Output, String> {
        Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
            .args(["--exact", FEATURE_TEST_NAME, "--nocapture"])
            .env("KRR_HTML_FEATURE_INPUT", input)
            .env("KRR_HTML_FEATURE_OUTPUT", output)
            .env("KRR_HTML_FEATURE_EXPRESSION", expression)
            .env("KRR_HTML_FEATURE_CASE", "input-state:filled")
            .env("KRR_HTML_FEATURE_NONCE", "child-test-nonce")
            .env("KRR_HTML_FEATURE_BINARY_SHA256", binary_sha)
            .env("KRR_HTML_FEATURE_WIDTH", "320")
            .env("KRR_HTML_FEATURE_HEIGHT", "240")
            .output()
            .map_err(|error| error.to_string())
    }

    fn assert_feature_output(output: &Path, html: &str, binary_sha: &str) -> Result<(), String> {
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        let capture = PathBuf::from(
            report["capturePath"]
                .as_str()
                .ok_or("capture path missing")?,
        );
        let png = std::fs::read(capture).map_err(|error| error.to_string())?;
        let png_sha = Sha256::digest(&png);
        let decoded = image::load_from_memory(&png).map_err(|error| error.to_string())?;
        assert_eq!((decoded.width(), decoded.height()), (320, 240));
        assert_eq!(report["htmlSha256"], json!(html_hash(html)));
        assert_eq!(report["executionNonce"], "child-test-nonce");
        assert_eq!(report["binarySha256"], binary_sha);
        assert_eq!(report["caseId"], "input-state:filled");
        assert_eq!(report["state"]["value"], "架空の名称");
        let expected_sha = png_sha
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(report["captureSha256"], expected_sha);
        assert!(!png_sha.is_empty());
        Ok(())
    }

    #[test]
    fn child_process_writes_state_png_and_provenance_for_post_operation() -> Result<(), String> {
        let workspace = child_workspace()?;
        let input = workspace.join("fixture.html");
        let output = workspace.join("feature.json");
        let html = "<input id=name><button id=save>Save</button>";
        std::fs::write(&input, html).map_err(|error| error.to_string())?;
        let binary_sha = current_binary_sha()?;
        let child = run_feature_child(
            &input,
            &output,
            "({value:document.querySelector('#name').value})",
            &binary_sha,
        )?;
        if !child.status.success() {
            return Err(format!(
                "feature child failed: {}",
                String::from_utf8_lossy(&child.stderr)
            ));
        }
        assert_feature_output(&output, html, &binary_sha)?;
        std::fs::remove_dir_all(workspace).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn child_process_reports_invalid_input_and_state_without_output() -> Result<(), String> {
        let workspace = child_workspace()?;
        let missing = workspace.join("missing.html");
        let output = workspace.join("invalid.json");
        let binary_sha = current_binary_sha()?;
        let child = run_feature_child(&missing, &output, "true", &binary_sha)?;
        assert!(!child.status.success());
        assert!(!output.exists());

        let input = workspace.join("fixture.html");
        std::fs::write(&input, "<p>valid</p>").map_err(|error| error.to_string())?;
        let child = run_feature_child(&input, &output, "missingVariable.value", &binary_sha)?;
        assert!(!child.status.success());
        assert!(!output.exists());
        std::fs::remove_dir_all(workspace).map_err(|error| error.to_string())?;
        Ok(())
    }
}
