use super::{custom, fixture, local, mixed_empty, response_reuse, unknown};

pub(in super::super) fn ignored_probe_cli_writes_strict_response_file_and_refuses_reuse()
-> Result<(), String> {
    let fixture = fixture::CliFixture::new()?;
    local::verify(&fixture)?;
    mixed_empty::verify(&fixture)?;
    response_reuse::verify(&fixture)?;
    unknown::verify(&fixture)?;
    custom::verify(&fixture)
}
