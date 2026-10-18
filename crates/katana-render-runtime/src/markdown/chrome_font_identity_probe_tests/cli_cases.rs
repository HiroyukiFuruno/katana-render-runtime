mod custom;
mod fixture;
mod local;
mod mixed_empty;
mod response_reuse;
mod unknown;

pub(super) use super::test_support::PROBE_TEST;

pub(super) fn ignored_probe_cli_writes_strict_response_file_and_refuses_reuse() -> Result<(), String>
{
    let fixture = fixture::CliFixture::new()?;
    local::verify(&fixture)?;
    mixed_empty::verify(&fixture)?;
    response_reuse::verify(&fixture)?;
    unknown::verify(&fixture)?;
    custom::verify(&fixture)
}
