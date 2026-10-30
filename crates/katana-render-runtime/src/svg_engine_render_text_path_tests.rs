use super::{Mode, assert_reference};
use rstest::rstest;
use std::path::{Path, PathBuf};

const UPSTREAM_TEST_FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests"
);

#[rstest]
fn upstream_text_path_cases_match_reference(
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/text/textPath/*.svg")]
    fixture: PathBuf,
) -> Result<(), String> {
    assert_reference(&fixture, Mode::Normal)
}

#[rstest]
#[case::complex_text_clip_path("masking/clipPath/clipping-with-complex-text-and-clip-rule.svg")]
fn upstream_complex_text_clip_case_matches_reference(#[case] fixture: &str) -> Result<(), String> {
    assert_reference(
        &Path::new(UPSTREAM_TEST_FIXTURES).join(fixture),
        Mode::Normal,
    )
}
