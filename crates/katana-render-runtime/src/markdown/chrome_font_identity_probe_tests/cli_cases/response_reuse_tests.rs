use super::super::{PROBE_TEST, fixture::CliFixture};
use crate::markdown::chrome_font_identity_probe_tests::test_support::run_ignored_test;
use std::fs;

pub(in super::super) fn verify(fixture: &CliFixture) -> Result<(), String> {
    let response_path = fixture.temp.path().join("local-response.json");
    let request_path = response_path.with_extension("request.json");
    let old_response = fs::read(&response_path).map_err(|error| error.to_string())?;
    let output = run_ignored_test(&request_path, &response_path, PROBE_TEST)?;
    if output.status.success() {
        return Err("reused response path unexpectedly succeeded".to_string());
    }
    let preserved = fs::read(&response_path).map_err(|error| error.to_string())?;
    assert_eq!(preserved, old_response);
    Ok(())
}
