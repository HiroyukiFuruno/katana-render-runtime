#[cfg(target_os = "macos")]
use super::reference_score_tests;
#[cfg(target_os = "macos")]
use crate::markdown::color_preset::DiagramColorPreset;

#[cfg(target_os = "macos")]
const ISHIKAWA_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/13-01-ishikawa-diagram-3-categories.md");
#[cfg(target_os = "macos")]
const ISHIKAWA_RAW_VIEW_BOX: [f64; 4] = [
    -303.554_901_123_046_9,
    -40.568_523_406_982_42,
    395.554_901_123_046_9,
    580.134_094_238_281_2,
];
/* WHY: R977の独立raw出力をviewBox oracleに使い、更新済goldenへの追随を避ける。 */
#[cfg(target_os = "macos")]
const ISHIKAWA_RAW_SOURCE_SHA256: &str =
    "115ea08f204882d14872f92a13d4c75e556796c19771979cb68ecafd48cef881";

#[cfg(target_os = "macos")]
#[test]
fn real_ishikawa_middle_baseline_matches_independent_raw_view_box() -> Result<(), String> {
    let source = reference_score_tests::mermaid_fixture_source(ISHIKAWA_MARKDOWN)?;
    let (actual, observations) = reference_score_tests::render_with_native_font_observations(
        &source,
        DiagramColorPreset::dark(),
    )?;
    let result = reference_score_tests::assert_view_box_matches(
        &actual,
        ISHIKAWA_RAW_VIEW_BOX,
        ISHIKAWA_RAW_SOURCE_SHA256,
    );
    result.map_err(|error| format!("{error}; native font observations: {observations}"))
}
