use super::{custom, fixture, local, mixed_empty, response_reuse, unknown};
use crate::markdown::chrome_font_identity_probe_tests::test_support::{
    TempDirectory, probe_command, run_ignored_test,
};
use std::{path::Path, process::Output};

pub(in super::super) fn ignored_probe_cli_writes_strict_response_file_and_refuses_reuse()
-> Result<(), String> {
    let fixture = fixture::CliFixture::new()?;
    local::verify(&fixture)?;
    mixed_empty::verify(&fixture)?;
    response_reuse::verify(&fixture)?;
    unknown::verify(&fixture)?;
    custom::verify(&fixture)
}

#[test]
fn probe_child_reports_missing_environment_and_unreadable_request() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let request = temp.path().join("missing-request.json");
    let output = temp.path().join("response.json");
    let cases = [
        (None, None, "KRR_CHROME_FONT_IDENTITY_REQUEST is required"),
        (
            Some(request.as_path()),
            None,
            "KRR_CHROME_FONT_IDENTITY_OUTPUT is required",
        ),
        (
            Some(request.as_path()),
            Some(output.as_path()),
            "read request:",
        ),
    ];
    for (request, response, expected) in cases {
        assert_probe_failure(
            &run_probe_with_optional_paths(request, response)?,
            expected,
            &output,
        );
    }
    Ok(())
}

#[test]
fn probe_child_rejects_malformed_request_without_response() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let request = temp.path().join("malformed-request.json");
    let output = temp.path().join("response.json");
    std::fs::write(&request, b"{").map_err(|error| error.to_string())?;

    assert_probe_failure(
        &run_probe_with_optional_paths(Some(&request), Some(&output))?,
        "parse request:",
        &output,
    );
    Ok(())
}

#[test]
fn probe_child_rejects_invalid_json_contracts_without_response() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let cases: [(&[u8], &str); 3] = [
        (
            br#"{"schema":"krr-chrome-font-identity-request-v1","captures":[{"conditionId":"same","selectedFonts":[],"resourceFiles":[]},{"conditionId":"same","selectedFonts":[],"resourceFiles":[]}]}"#,
            "capture condition IDs must be non-empty and unique",
        ),
        (
            br#"{"schema":"krr-chrome-font-identity-request-v1","captures":[],"unexpected":true}"#,
            "parse request:",
        ),
        (
            br#"{"schema":"krr-chrome-font-identity-request-v1","captures":[{"conditionId":1,"selectedFonts":[],"resourceFiles":[]}]}"#,
            "parse request:",
        ),
    ];
    for (index, (bytes, expected)) in cases.into_iter().enumerate() {
        let request = temp.path().join(format!("request-{index}.json"));
        let output = temp.path().join(format!("response-{index}.json"));
        std::fs::write(&request, bytes).map_err(|error| error.to_string())?;
        let result = run_probe_with_optional_paths(Some(&request), Some(&output))?;
        assert_probe_failure(&result, expected, &output);
    }
    Ok(())
}

#[test]
fn probe_child_rejects_missing_response_parent() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let request = temp.path().join("request.json");
    let output = temp.path().join("missing-parent").join("response.json");
    std::fs::write(
        &request,
        br#"{"schema":"krr-chrome-font-identity-request-v1","captures":[{"conditionId":"response-parent","selectedFonts":[],"resourceFiles":[]}]}"#,
    )
    .map_err(|error| error.to_string())?;
    let result = run_ignored_test(&request, &output, super::PROBE_TEST)?;
    assert_probe_failure(
        &result,
        "create fresh response without overwriting:",
        &output,
    );
    Ok(())
}

fn run_probe_with_optional_paths(
    request: Option<&Path>,
    response: Option<&Path>,
) -> Result<Output, String> {
    let mut command = probe_command(super::PROBE_TEST)?;
    for (name, path) in [
        ("KRR_CHROME_FONT_IDENTITY_REQUEST", request),
        ("KRR_CHROME_FONT_IDENTITY_OUTPUT", response),
    ] {
        if let Some(path) = path {
            command.env(name, path);
        } else {
            command.env_remove(name);
        }
    }
    command.output().map_err(|error| error.to_string())
}

fn assert_probe_failure(result: &Output, expected: &str, output: &Path) {
    assert!(!result.status.success(), "{expected}");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(report.contains(expected), "{expected}: {report}");
    assert!(!output.exists());
}
