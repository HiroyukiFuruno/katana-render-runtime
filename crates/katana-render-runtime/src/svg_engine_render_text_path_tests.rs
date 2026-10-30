use super::{Mode, assert_reference};
use rstest::rstest;
use std::path::Path;

const TEXT_PATH_FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/text/textPath"
);
const UPSTREAM_TEST_FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests"
);

#[rstest]
#[case::simple_case("simple-case.svg")]
#[case::relative_tspan_position("tspan-with-relative-position.svg")]
#[case::absolute_tspan_position("tspan-with-absolute-position.svg")]
#[case::baseline_shift("with-baseline-shift.svg")]
#[case::baseline_shift_and_rotate("with-baseline-shift-and-rotate.svg")]
#[case::arabic_with_dy("arabic-with-dy.svg")]
fn upstream_text_path_cases_match_reference(#[case] fixture: &str) -> Result<(), String> {
    assert_reference(&Path::new(TEXT_PATH_FIXTURES).join(fixture), Mode::Normal)
}

#[rstest]
#[case::complex_text_clip_path("masking/clipPath/clipping-with-complex-text-and-clip-rule.svg")]
fn upstream_complex_text_clip_case_matches_reference(#[case] fixture: &str) -> Result<(), String> {
    assert_reference(
        &Path::new(UPSTREAM_TEST_FIXTURES).join(fixture),
        Mode::Normal,
    )
}
