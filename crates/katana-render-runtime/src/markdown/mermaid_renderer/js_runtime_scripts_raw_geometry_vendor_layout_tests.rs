use super::geometry_structure::{
    assert_actual_vendor_class_structure, assert_empty_methods_group_present,
};
use super::geometry_tree::raw_geometry_tree;
use super::{
    RAW_TEST_SVG_ID, envelope_svg, fixture_mermaid_source, raw_geometry_request,
    raw_geometry_vendor_config, render_vendor_raw_and_processed,
    render_vendor_raw_and_processed_with_metric_labels,
};

const ORIGINAL_CLASS_SOURCE: &str = r#"classDiagram
    class PreviewPane {
        +full_render(source)
        +show_content(ui)
    }
    class RenderedSection {
        <<enumeration>>
        Markdown
        Image
        Error
    }
    PreviewPane --> RenderedSection"#;
const EMPTY_METHODS_CLASS_SOURCE: &str = r#"classDiagram
    class Packet {
        +String body
    }
    class Sink
    Packet --> Sink"#;
const ER_VENDOR_EN_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/05-01-er-diagram-simple.md");
const ER_VENDOR_JA_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/ja/05-01-er.md");
const ER_VENDOR_UNICODE_MARKDOWN: &str = r#"# ER Unicode compatibility

~~~mermaid
erDiagram
    顧客 ||--o{ 注文 : 作成
    顧客 {
        文字列 名前
    }
    注文 {
        整数 番号
    }
~~~
"#;
const TREEMAP_VENDOR_EN_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/23-01-treemap-flat.md");
const TREEMAP_VENDOR_JA_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/ja/23-01-treemap.md");

#[test]
fn actual_vendor_class_geometry_survives_production_postprocess() -> Result<(), String> {
    assert_actual_vendor_geometry(ORIGINAL_CLASS_SOURCE, &["PreviewPane", "RenderedSection"])
}

#[test]
fn actual_vendor_empty_methods_geometry_survives_postprocess() -> Result<(), String> {
    assert_actual_vendor_geometry(EMPTY_METHODS_CLASS_SOURCE, &["Packet", "Sink"])
}

#[test]
fn er_vendor_layout_fragments_survive_production_postprocess() -> Result<(), String> {
    let config = raw_geometry_vendor_config();
    for (fixture, markdown) in [
        ("en-er", ER_VENDOR_EN_MARKDOWN),
        ("ja-er", ER_VENDOR_JA_MARKDOWN),
        ("unicode-er", ER_VENDOR_UNICODE_MARKDOWN),
    ] {
        assert_er_vendor_fixture(fixture, markdown, &config)?;
    }
    Ok(())
}

fn assert_er_vendor_fixture(
    fixture: &str,
    markdown: &str,
    config: &serde_json::Value,
) -> Result<(), String> {
    let envelope = render_vendor_fixture(fixture, markdown, config)?;
    assert_er_vendor_output(fixture, &envelope)
}

fn assert_er_vendor_output(fixture: &str, envelope: &serde_json::Value) -> Result<(), String> {
    let raw = raw_geometry_tree(envelope_svg(envelope, "raw")?)?;
    let processed_svg = envelope_svg(envelope, "processed")?;
    let processed = raw_geometry_tree(processed_svg)?;
    super::er_attribute_tests::assert_er_attribute_layout(processed_svg)?;
    if raw != processed {
        return Err(format!(
            "ER vendor XML changed for {fixture}: {}",
            super::first_geometry_difference(&raw, &processed)
        ));
    }
    Ok(())
}

#[test]
fn treemap_value_typography_survives_production_postprocess() -> Result<(), String> {
    let config = raw_geometry_vendor_config();
    for (fixture, markdown) in [
        ("en-treemap", TREEMAP_VENDOR_EN_MARKDOWN),
        ("ja-treemap", TREEMAP_VENDOR_JA_MARKDOWN),
    ] {
        assert_treemap_fixture(fixture, markdown, &config)?;
    }
    Ok(())
}

fn assert_treemap_fixture(
    fixture: &str,
    markdown: &str,
    config: &serde_json::Value,
) -> Result<(), String> {
    let envelope = render_vendor_fixture(fixture, markdown, config)?;
    let raw = raw_geometry_tree(envelope_svg(&envelope, "raw")?)?;
    let mut processed = raw_geometry_tree(envelope_svg(&envelope, "processed")?)?;
    super::normalize_permitted_changes(&raw, &mut processed)?;
    if raw != processed {
        return Err(format!(
            "Treemap value typography or position changed for {fixture}: {}",
            super::first_geometry_difference(&raw, &processed)
        ));
    }
    Ok(())
}

fn render_vendor_fixture(
    fixture: &str,
    markdown: &str,
    config: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let source = fixture_mermaid_source(markdown)?;
    let request = raw_geometry_request(&source);
    let envelope = render_vendor_raw_and_processed_with_metric_labels(
        &request,
        config,
        &["DOCUMENT", "SECTION", "DIAGRAM"],
    )?;
    if envelope["renderInput"] != source {
        return Err(format!(
            "fixture source changed before vendor render: {fixture}"
        ));
    }
    Ok(envelope)
}

fn assert_actual_vendor_geometry(source: &str, labels: &[&str; 2]) -> Result<(), String> {
    let request = raw_geometry_request(source);
    let config = raw_geometry_vendor_config();
    let envelope = render_vendor_raw_and_processed(&request, &config)?;
    assert_eq!(envelope["renderCalls"], 1);
    assert_eq!(envelope["renderInput"], source);
    assert_eq!(envelope["svgId"], RAW_TEST_SVG_ID);
    assert_eq!(envelope["initializeInput"], config);
    let raw = envelope_svg(&envelope, "raw")?;
    let processed = envelope_svg(&envelope, "processed")?;
    let expected = raw_geometry_tree(raw)?;
    let mut actual = raw_geometry_tree(processed)?;
    if source == EMPTY_METHODS_CLASS_SOURCE {
        assert_empty_methods_group_present(raw)?;
    }
    super::normalize_permitted_changes(&expected, &mut actual)?;
    assert_eq!(expected, actual);
    assert_actual_vendor_class_structure(raw, labels)
}
