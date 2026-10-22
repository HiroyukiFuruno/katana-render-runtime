#[cfg(target_os = "macos")]
use super::reference_score_tests;
#[cfg(target_os = "macos")]
use crate::markdown::color_preset::DiagramColorPreset;
#[cfg(target_os = "macos")]
use resvg::usvg::roxmltree::{Document, Node as SvgNode};

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
const ARCHITECTURE_SIMPLE_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/09-01-architecture-diagram-simple.md");
#[cfg(target_os = "macos")]
const ARCHITECTURE_SIMPLE_RAW_VIEW_BOX: [f64; RAW_VIEW_BOX_COMPONENT_COUNT] =
    [-282.686_523_437_5, -65.5, 647.373_046_875, 262.0];
/* WHY: R1063の独立official raw SVGを固定し、候補SVGのviewBoxへ追随しない。 */
#[cfg(target_os = "macos")]
const ARCHITECTURE_SIMPLE_RAW_SOURCE_SHA256: &str =
    "e0a287f2137560d35ec1b076fc33597983e64f723e49950eb3e0681885d192b1";

#[cfg(target_os = "macos")]
const ISHIKAWA_FOUR_CATEGORIES_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/13-02-ishikawa-diagram-4-categories.md");
#[cfg(target_os = "macos")]
const ISHIKAWA_FOUR_CATEGORIES_RAW_VIEW_BOX: [f64; RAW_VIEW_BOX_COMPONENT_COUNT] = [
    -379.764_556_884_765_6,
    -39.989_240_646_362_305,
    471.764_556_884_765_6,
    580.134_033_203_125,
];
/* WHY: R1063の独立official raw SVGを固定し、候補SVGのviewBoxへ追随しない。 */
#[cfg(target_os = "macos")]
const ISHIKAWA_FOUR_CATEGORIES_RAW_SOURCE_SHA256: &str =
    "3315470739379bd41e1126e4d419e1dea14de1a8d3cb3f81c2645c09c0dd68dd";

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
const ER_SIMPLE_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/05-01-er-diagram-simple.md");
#[cfg(target_os = "macos")]
const ER_SIMPLE_RAW_VIEW_BOX: [f64; RAW_VIEW_BOX_COMPONENT_COUNT] = [10.25, 4.0, 169.03125, 527.5];
/* WHY: R1063の独立official raw SVGを使い、ERのhidden g計測をvendor配置へ反映する。 */
#[cfg(target_os = "macos")]
const ER_SIMPLE_RAW_SOURCE_SHA256: &str =
    "c8300bd6b9dce85216876a0a9f1202e99f3445264e62573e909b9306ece47878";

#[cfg(target_os = "macos")]
const ER_MULTI_ENTITY_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/05-02-er-diagram-multi-entity.md");
#[cfg(target_os = "macos")]
const ER_MULTI_ENTITY_RAW_VIEW_BOX: [f64; RAW_VIEW_BOX_COMPONENT_COUNT] =
    [10.25, 4.0, 372.859375, 637.25];
/* WHY: R1063の独立official raw SVGを使い、複数entityでもhidden g計測を固定する。 */
#[cfg(target_os = "macos")]
const ER_MULTI_ENTITY_RAW_SOURCE_SHA256: &str =
    "64c349a228acb5eb73f84edbb62eaa643666a5d74fa54382e06d019d1403f9e2";

#[cfg(target_os = "macos")]
const C4_COMPONENT_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/08-04-c4-component.md");
#[cfg(target_os = "macos")]
const C4_DYNAMIC_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/08-05-c4-dynamic.md");
#[cfg(target_os = "macos")]
const C4_DEPLOYMENT_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/08-06-c4-deployment.md");

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
fn real_simple_architecture_matches_independent_raw_view_box() -> Result<(), String> {
    assert_fixture_matches_raw_view_box(
        ARCHITECTURE_SIMPLE_MARKDOWN,
        ARCHITECTURE_SIMPLE_RAW_VIEW_BOX,
        ARCHITECTURE_SIMPLE_RAW_SOURCE_SHA256,
    )
}

#[cfg(target_os = "macos")]
#[test]
fn real_ishikawa_four_categories_matches_independent_raw_view_box() -> Result<(), String> {
    assert_fixture_matches_raw_view_box(
        ISHIKAWA_FOUR_CATEGORIES_MARKDOWN,
        ISHIKAWA_FOUR_CATEGORIES_RAW_VIEW_BOX,
        ISHIKAWA_FOUR_CATEGORIES_RAW_SOURCE_SHA256,
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
#[test]
fn real_simple_er_hidden_group_matches_independent_raw_view_box() -> Result<(), String> {
    assert_fixture_matches_raw_view_box(
        ER_SIMPLE_MARKDOWN,
        ER_SIMPLE_RAW_VIEW_BOX,
        ER_SIMPLE_RAW_SOURCE_SHA256,
    )
}

#[cfg(target_os = "macos")]
#[test]
fn real_multi_entity_er_hidden_group_matches_independent_raw_view_box() -> Result<(), String> {
    assert_fixture_matches_raw_view_box(
        ER_MULTI_ENTITY_MARKDOWN,
        ER_MULTI_ENTITY_RAW_VIEW_BOX,
        ER_MULTI_ENTITY_RAW_SOURCE_SHA256,
    )
}

#[cfg(target_os = "macos")]
fn rendered_label_text<'a, 'input>(
    document: &'a Document<'input>,
    label: &str,
) -> Result<SvgNode<'a, 'input>, String> {
    document
        .descendants()
        .find(|node| {
            node.tag_name().name() == "text"
                && node
                    .descendants()
                    .filter(|descendant| descendant.is_text())
                    .filter_map(|descendant| descendant.text())
                    .collect::<String>()
                    == label
        })
        .ok_or_else(|| format!("label {label:?} missing from rendered SVG"))
}

#[cfg(target_os = "macos")]
fn rendered_label_parent_translate_x(text: SvgNode<'_, '_>, label: &str) -> Result<f64, String> {
    let mut ancestor = text.parent();
    while let Some(node) = ancestor {
        let is_label_group = node
            .attribute("class")
            .is_some_and(|class| class.split_ascii_whitespace().any(|token| token == "label"));
        if is_label_group {
            let transform = node
                .attribute("transform")
                .ok_or("C4 label group transform missing")?;
            let value = transform
                .strip_prefix("translate(")
                .and_then(|value| value.split([',', ' ', ')']).next())
                .ok_or_else(|| format!("invalid parent transform {transform:?}"))?;
            return value.parse::<f64>().map_err(|error| error.to_string());
        }
        ancestor = node.parent();
    }
    Err(format!("translated parent for {label:?} missing"))
}

#[cfg(target_os = "macos")]
fn rendered_parent_translate_x(svg: &str, label: &str) -> Result<f64, String> {
    let document = Document::parse(svg).map_err(|error| error.to_string())?;
    rendered_label_parent_translate_x(rendered_label_text(&document, label)?, label)
}

#[cfg(target_os = "macos")]
#[test]
fn real_c4_parent_translate_x_matches_r1063_raw_reference() -> Result<(), String> {
    /* WHY: R1063公式rawを固定し、更新済み候補や別取得rawへの追随を防ぐ。
    SHA-256: 08-04 dbec3c2ea1976d954f7e4b0eeac74c9dfc52455d46c6ce02d8d8d4bffc7b3d0f,
    08-05 634f06f1e77e880142a7db2a3fd9b3819b08873727cc24fdd740839a50070a1c,
    08-06 59ddcda705b2aa58f65ed34062533e4566abb0392c4727531f3281e7103bd26b. */
    let cases = [
        (
            C4_COMPONENT_MARKDOWN,
            "Single Page Application",
            -79.4921875,
        ),
        (C4_DYNAMIC_MARKDOWN, "Single Page Application", -79.4921875),
        (C4_DEPLOYMENT_MARKDOWN, "Mobile App", -48.15625),
    ];
    for (fixture, label, expected) in cases {
        let source = reference_score_tests::mermaid_fixture_source(fixture)?;
        let (rendered, observations) = reference_score_tests::render_with_native_font_observations(
            &source,
            DiagramColorPreset::dark(),
        )?;
        let actual = rendered_parent_translate_x(&rendered, label)?;
        assert!(
            (actual - expected).abs()
                <= 1.0 / reference_score_tests::SVG_DOM_PLACEMENT_SCALE as f64,
            "{label:?}: {actual} != {expected}; observations: {observations}"
        );
    }
    Ok(())
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
