use super::super::{test_error, with_success};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const TEST_NAME: &str = "renderer::backends::html_runtime::session::session_feature_diagnostic::writes_fixture_feature_state_when_requested";
static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn workspace() -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(test_error)?
        .as_nanos();
    let counter = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "krr-static-feature-{stamp}-{}-{counter}",
        std::process::id()
    ));
    std::fs::create_dir(&path).map_err(test_error)?;
    Ok(path)
}

pub(super) fn current_binary_sha() -> Result<String, String> {
    let bytes = std::fs::read(std::env::current_exe().map_err(test_error)?).map_err(test_error)?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(super) fn run_child(input: &Path, output: &Path, binary_sha: &str) -> Result<Output, String> {
    crate::system::ProcessService::create_command(std::env::current_exe().map_err(test_error)?)
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env("KRR_HTML_FEATURE_INPUT", input)
        .env("KRR_HTML_FEATURE_OUTPUT", output)
        .env(
            "KRR_HTML_FEATURE_EXPRESSION",
            "({value:document.querySelector('#state').textContent})",
        )
        .env("KRR_HTML_FEATURE_NONCE", "static-child-nonce")
        .env("KRR_HTML_FEATURE_BINARY_SHA256", binary_sha)
        .output()
        .map_err(test_error)
}

pub(super) fn run_child_without_env(
    input: &Path,
    output: &Path,
    binary_sha: &str,
    omitted_env: &str,
) -> Result<Output, String> {
    let mut command =
        crate::system::ProcessService::create_command(std::env::current_exe().map_err(test_error)?);
    command
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env("KRR_HTML_FEATURE_INPUT", input)
        .env("KRR_HTML_FEATURE_OUTPUT", output)
        .env(
            "KRR_HTML_FEATURE_EXPRESSION",
            "({value:document.querySelector('#state').textContent})",
        )
        .env("KRR_HTML_FEATURE_NONCE", "static-child-nonce")
        .env("KRR_HTML_FEATURE_BINARY_SHA256", binary_sha)
        .env_remove(omitted_env);
    command.output().map_err(test_error)
}

pub(super) fn valid_fixture(root: &Path) -> Result<(PathBuf, PathBuf, String), String> {
    let input = root.join("fixture.html");
    let output = root.join("feature.json");
    let html = "<div id=state>before</div>".to_string();
    std::fs::write(&input, &html).map_err(test_error)?;
    Ok((input, output, html))
}

pub(super) fn assert_valid_control(input: &Path, output: &Path, binary_sha: &str) {
    with_success(
        run_child(input, output, binary_sha),
        "valid diagnostic control process",
        |child| {
            assert_valid_child(&child);
        },
    );
    with_success(
        std::fs::read(output),
        "valid control report file",
        |bytes| {
            with_success(
                serde_json::from_slice::<serde_json::Value>(&bytes),
                "valid control report JSON",
                |report| {
                    assert_eq!(report["executionNonce"], "static-child-nonce");
                    assert_eq!(report["state"]["value"], "before");
                },
            );
        },
    );
}

pub(super) fn require_child_success(child: Output, context: &str) -> Result<Output, String> {
    if child.status.success() {
        return Ok(child);
    }
    Err(format!("{context} failed: {}", child_diagnostic(&child)))
}

pub(super) fn child_diagnostic(child: &Output) -> String {
    format!(
        "stdout={} stderr={}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    )
}

pub(super) fn assert_child_failure(
    child: Output,
    context: &str,
    expected: &[&str],
) -> Result<(), String> {
    let diagnostic = match require_child_success(child, context) {
        Ok(_) => return Err(format!("{context} unexpectedly succeeded")),
        Err(error) => error,
    };
    let prefix = format!("{context} failed: stdout=");
    assert!(
        diagnostic.starts_with(&prefix) && diagnostic.contains(" stderr="),
        "{context} diagnostic framing changed: {diagnostic}"
    );
    for fragment in expected {
        assert!(
            diagnostic.contains(fragment),
            "{context} diagnostic omitted {fragment:?}: {diagnostic}"
        );
    }
    Ok(())
}

pub(super) fn run_without_input(input: Option<&Path>) -> Result<Output, String> {
    let mut command =
        crate::system::ProcessService::create_command(std::env::current_exe().map_err(test_error)?);
    command.args(["--exact", TEST_NAME, "--nocapture"]);
    if let Some(input) = input {
        command.env("KRR_HTML_FEATURE_INPUT", input);
    } else {
        command.env_remove("KRR_HTML_FEATURE_INPUT");
    }
    command
        .env_remove("KRR_HTML_FEATURE_OUTPUT")
        .env_remove("KRR_HTML_FEATURE_EXPRESSION")
        .env_remove("KRR_HTML_FEATURE_NONCE")
        .env_remove("KRR_HTML_FEATURE_BINARY_SHA256")
        .output()
        .map_err(test_error)
}

pub(super) fn assert_valid_child(child: &Output) {
    assert!(
        child.status.success(),
        "valid diagnostic control failed: {}",
        child_diagnostic(child)
    );
}

pub(super) fn expected_canonicalize_error(path: &Path) -> Result<String, String> {
    match std::fs::canonicalize(path) {
        Ok(_) => Err("missing path unexpectedly exists".to_string()),
        Err(error) => Ok(test_error(error)),
    }
}

pub(super) fn expected_read_error(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(_) => Err("directory input unexpectedly readable as a file".to_string()),
        Err(error) => Ok(test_error(error)),
    }
}

pub(super) fn expected_write_error(path: &Path) -> Result<String, String> {
    match std::fs::write(path, b"probe") {
        Ok(_) => Err("directory output unexpectedly accepted".to_string()),
        Err(error) => Ok(test_error(error)),
    }
}
