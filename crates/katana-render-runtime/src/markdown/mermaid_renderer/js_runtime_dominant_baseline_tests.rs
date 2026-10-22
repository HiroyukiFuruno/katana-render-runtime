#[cfg(target_os = "macos")]
use super::reference_score_tests;
#[cfg(target_os = "macos")]
use crate::markdown::color_preset::DiagramColorPreset;

#[cfg(target_os = "macos")]
const RAW_VIEW_BOX_COMPONENT_COUNT: usize = 4;

#[cfg(target_os = "macos")]
const ISHIKAWA_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/13-01-ishikawa-diagram-3-categories.md");
#[cfg(target_os = "macos")]
const ISHIKAWA_RAW_VIEW_BOX: [f64; RAW_VIEW_BOX_COMPONENT_COUNT] = [
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
const MINDMAP_SIMPLE_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/07-01-mindmap-simple.md");
#[cfg(target_os = "macos")]
const MINDMAP_SIMPLE_RAW_VIEW_BOX: [f64; RAW_VIEW_BOX_COMPONENT_COUNT] =
    [5.0, 5.0, 528.906_372_070_312_5, 403.066_314_697_265_6];
/* WHY: R1063の独立raw描画を使い、空のedge labelがviewBoxへ混入しないことを検証する。 */
#[cfg(target_os = "macos")]
const MINDMAP_SIMPLE_RAW_SOURCE_SHA256: &str =
    "2b2dfb633e46be1093b257c0e79708d99bad199ec94d55ee811f910b360b5367";

#[cfg(target_os = "macos")]
const TREEMAP_NESTED_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/23-02-treemap-beta-nested.md");
#[cfg(target_os = "macos")]
const TREEMAP_NESTED_RAW_VIEW_BOX: [f64; RAW_VIEW_BOX_COMPONENT_COUNT] = [2.0, 27.0, 996.0, 371.0];
/* WHY: R1063の独立raw描画を使い、非表示文字列がtreemap範囲を広げないことを検証する。 */
#[cfg(target_os = "macos")]
const TREEMAP_NESTED_RAW_SOURCE_SHA256: &str =
    "6507795be33df8f922509fea808984a8cb18f518d0a680496f1f9e337f1aadc8";

#[cfg(target_os = "macos")]
#[test]
fn real_ishikawa_middle_baseline_matches_independent_raw_view_box() -> Result<(), String> {
    assert_fixture_matches_raw_view_box(
        ISHIKAWA_MARKDOWN,
        ISHIKAWA_RAW_VIEW_BOX,
        ISHIKAWA_RAW_SOURCE_SHA256,
    )
}

#[cfg(target_os = "macos")]
#[test]
fn real_simple_mindmap_empty_edge_labels_match_independent_raw_view_box() -> Result<(), String> {
    assert_fixture_matches_raw_view_box(
        MINDMAP_SIMPLE_MARKDOWN,
        MINDMAP_SIMPLE_RAW_VIEW_BOX,
        MINDMAP_SIMPLE_RAW_SOURCE_SHA256,
    )
}

#[cfg(target_os = "macos")]
#[test]
fn real_nested_treemap_hidden_labels_match_independent_raw_view_box() -> Result<(), String> {
    assert_fixture_matches_raw_view_box(
        TREEMAP_NESTED_MARKDOWN,
        TREEMAP_NESTED_RAW_VIEW_BOX,
        TREEMAP_NESTED_RAW_SOURCE_SHA256,
    )
}

#[cfg(target_os = "macos")]
fn assert_fixture_matches_raw_view_box(
    markdown: &str,
    expected: [f64; RAW_VIEW_BOX_COMPONENT_COUNT],
    source_sha256: &str,
) -> Result<(), String> {
    let source = reference_score_tests::mermaid_fixture_source(markdown)?;
    let (actual, observations) = reference_score_tests::render_with_native_font_observations(
        &source,
        DiagramColorPreset::dark(),
    )?;
    let result = reference_score_tests::assert_view_box_matches(&actual, expected, source_sha256);
    result.map_err(|error| format!("{error}; native font observations: {observations}"))
}
