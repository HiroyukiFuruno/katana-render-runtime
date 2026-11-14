use super::{StaticHtmlRuntime, html_hash, test_error};
use crate::renderer::backends::html_browser::HtmlBrowserSource;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "session_feature_diagnostic_child_helpers.rs"]
mod helpers;
#[path = "session_feature_diagnostic_live_children_tests.rs"]
mod live_children_tests;
use helpers::{
    TEST_NAME, assert_child_failure, assert_valid_child, assert_valid_control, child_diagnostic,
    current_binary_sha, expected_canonicalize_error, expected_read_error, expected_write_error,
    require_child_success, run_child, run_child_without_env, run_without_input, valid_fixture,
    workspace,
};

#[test]
fn child_process_writes_real_dom_script_state_and_provenance() -> Result<(), String> {
    let root = workspace()?;
    let input = root.join("fixture.html");
    let output = root.join("feature.json");
    let html = "<div id=state>before</div><script>document.querySelector('#state').textContent='after'</script>";
    std::fs::write(&input, html).map_err(test_error)?;
    let binary_sha = current_binary_sha()?;
    run_child(&input, &output, &binary_sha)
        .and_then(|child| require_child_success(child, "static feature child"))?;
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output).map_err(test_error)?).map_err(test_error)?;
    assert_eq!(report["htmlSha256"], json!(html_hash(html)));
    assert_eq!(report["executionNonce"], "static-child-nonce");
    assert_eq!(report["binarySha256"], binary_sha);
    assert_eq!(report["state"]["value"], "after");
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn diagnostic_entrypoint_skips_when_input_is_not_requested() -> Result<(), String> {
    require_child_success(run_without_input(None)?, "diagnostic no-op child")?;
    Ok(())
}

#[test]
fn diagnostic_entrypoint_rejects_missing_input_path() -> Result<(), String> {
    let root = workspace()?;
    let missing = root.join("missing-fixture.html");
    let expected_error = expected_canonicalize_error(&missing)?;
    let child = run_without_input(Some(&missing))?;
    let context = format!("{TEST_NAME}: missing input path");
    assert_child_failure(child, &context, &[&expected_error])?;
    assert!(!root.join("feature.json").exists());
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn diagnostic_entrypoint_rejects_directory_input() -> Result<(), String> {
    let root = workspace()?;
    let output = root.join("feature.json");
    let binary_sha = current_binary_sha()?;
    let expected_error = expected_read_error(&root)?;
    let child = run_child(&root, &output, &binary_sha)?;
    let context = format!("{TEST_NAME}: directory input");
    assert_child_failure(child, &context, &[&expected_error])?;
    assert!(!output.exists());
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn diagnostic_entrypoint_rejects_directory_output() -> Result<(), String> {
    let root = workspace()?;
    let (input, output, _) = valid_fixture(&root)?;
    let binary_sha = current_binary_sha()?;
    assert_valid_control(&input, &output, &binary_sha);
    assert!(output.is_file());
    std::fs::remove_file(&output).map_err(test_error)?;
    std::fs::create_dir(&output).map_err(test_error)?;
    let expected_error = expected_write_error(&output)?;
    let child = run_child(&input, &output, &binary_sha)?;
    let context = format!("{TEST_NAME}: directory output");
    assert_child_failure(child, &context, &[&expected_error])?;
    assert!(output.is_dir());
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn diagnostic_entrypoint_rejects_script_compile_error() -> Result<(), String> {
    let root = workspace()?;
    let input = root.join("invalid-script.html");
    let output = root.join("feature.json");
    let html = "<script>if (</script>";
    std::fs::write(&input, html).map_err(test_error)?;
    let binary_sha = current_binary_sha()?;
    let child = run_child(&input, &output, &binary_sha)?;
    let context = format!("{TEST_NAME}: script compile error");
    assert_child_failure(child, &context, &["JavaScript compile error"])?;
    assert!(!output.exists());
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn diagnostic_entrypoint_rejects_each_missing_required_environment() -> Result<(), String> {
    let root = workspace()?;
    let (input, output, _) = valid_fixture(&root)?;
    let binary_sha = current_binary_sha()?;
    assert_valid_control(&input, &output, &binary_sha);
    assert!(output.is_file());
    std::fs::remove_file(&output).map_err(test_error)?;
    for omitted_env in [
        "KRR_HTML_FEATURE_EXPRESSION",
        "KRR_HTML_FEATURE_NONCE",
        "KRR_HTML_FEATURE_BINARY_SHA256",
        "KRR_HTML_FEATURE_OUTPUT",
    ] {
        let child = run_child_without_env(&input, &output, &binary_sha, omitted_env)?;
        let expected = if omitted_env == "KRR_HTML_FEATURE_OUTPUT" {
            "missing output"
        } else {
            "environment variable not found"
        };
        let context = format!("{TEST_NAME}: {omitted_env}");
        assert_child_failure(child, &context, &[expected])?;
        assert!(
            !output.exists(),
            "{omitted_env} must fail before report write"
        );
    }
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn feature_node_query_and_missing_runtime_slots_fail_closed() -> Result<(), String> {
    let source =
        HtmlBrowserSource::new("<p id=state>ready</p>", "https://example.test/feature.html")
            .map_err(test_error)?;
    let mut session = StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(test_error)?;
    assert!(session.query_feature_node("#state").is_some());
    assert!(session.query_feature_node("#missing").is_none());
    assert_eq!(
        session.observe_feature_state("document.querySelector('#state').textContent")?,
        json!("ready")
    );
    assert_missing_runtime_slots(&mut session);
    Ok(())
}

fn assert_missing_runtime_slots(session: &mut super::StaticHtmlRuntimeSession) {
    session.context = None;
    assert_eq!(
        session.observe_feature_state("true"),
        Err("missing runtime context".into())
    );

    let mut discarded = super::StaticHtmlRuntimeSession {
        context: None,
        isolate: None,
        external_stylesheets: std::collections::HashMap::new(),
    };
    assert!(discarded.query_feature_node("#state").is_none());
    assert_eq!(
        discarded.observe_feature_state("true"),
        Err("missing runtime isolate".into())
    );
}

#[test]
fn diagnostic_fs_guards_reject_actual_successful_operations() -> Result<(), String> {
    let root = workspace()?;
    let (input, output, _) = valid_fixture(&root)?;
    assert_eq!(
        expected_canonicalize_error(&input),
        Err("missing path unexpectedly exists".into())
    );
    assert_eq!(
        expected_read_error(&input),
        Err("directory input unexpectedly readable as a file".into())
    );
    assert_eq!(
        expected_write_error(&output),
        Err("directory output unexpectedly accepted".into())
    );
    assert_eq!(std::fs::read(&output).map_err(test_error)?, b"probe");
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn diagnostic_failure_guard_rejects_a_real_successful_child() -> Result<(), String> {
    let child = run_without_input(None)?;
    assert!(child.status.success());
    assert_eq!(
        assert_child_failure(child, "real success", &[]),
        Err("real success unexpectedly succeeded".into())
    );
    Ok(())
}

#[test]
fn diagnostic_control_panic_preserves_a_real_child_failure() -> Result<(), String> {
    let root = workspace()?;
    let missing = root.join("missing.html");
    let child = run_without_input(Some(&missing))?;
    assert!(!child.status.success());
    let diagnostic = child_diagnostic(&child);
    let panic = std::panic::catch_unwind(|| assert_valid_child(&child))
        .err()
        .ok_or("failed child must fail its positive control")?;
    assert_eq!(
        panic.downcast_ref::<String>(),
        Some(&format!("valid diagnostic control failed: {diagnostic}"))
    );
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn diagnostic_error_text_preserves_real_filesystem_json_and_time_errors() -> Result<(), String> {
    let root = workspace()?;
    let error = std::fs::read(root.join("missing.html"))
        .err()
        .ok_or("missing file must fail")?;
    let expected = error.to_string();
    assert_eq!(test_error(error), expected);
    let error = serde_json::from_str::<serde_json::Value>("{")
        .err()
        .ok_or("invalid JSON must fail")?;
    let expected = error.to_string();
    assert_eq!(test_error(error), expected);
    let error = UNIX_EPOCH
        .duration_since(SystemTime::now())
        .err()
        .ok_or("reverse time must fail")?;
    let expected = error.to_string();
    assert_eq!(test_error(error), expected);
    std::fs::remove_dir_all(root).map_err(test_error)?;
    Ok(())
}

#[test]
fn diagnostic_error_text_preserves_real_source_and_runtime_errors() -> Result<(), String> {
    let error = HtmlBrowserSource::new("<p>ready</p>", "not-a-url")
        .err()
        .ok_or("invalid source origin must fail")?;
    let expected = error.to_string();
    assert_eq!(test_error(error), expected);
    let error = StaticHtmlRuntime
        .start("<script>if (</script>")
        .err()
        .ok_or("invalid script must fail")?;
    let expected = error.to_string();
    assert_eq!(test_error(error), expected);
    Ok(())
}
