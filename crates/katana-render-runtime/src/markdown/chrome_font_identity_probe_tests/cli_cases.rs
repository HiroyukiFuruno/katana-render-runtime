mod custom;
mod fixture;
mod local;
mod mixed_empty;
mod response_reuse;
mod unknown;

#[path = "cli_cases_tests.rs"]
mod test_cases;

pub(super) use super::test_support::PROBE_TEST;
pub(super) use test_cases::ignored_probe_cli_writes_strict_response_file_and_refuses_reuse;
