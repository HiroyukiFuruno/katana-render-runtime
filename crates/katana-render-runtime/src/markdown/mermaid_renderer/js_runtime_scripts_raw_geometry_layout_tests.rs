use super::render_vendor_raw_and_processed_with_metric_labels;
use super::{
    envelope_svg, fixture_mermaid_source, raw_geometry_request, raw_geometry_vendor_config,
};
use super::{normalize_permitted_changes, raw_geometry_tree};

const KANBAN_FIXTURE: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/14-01-kanban-simple.md");
const ISHIKAWA_FIXTURE: &str = include_str!("../../../../../tests/fixtures/mermaid/ja/13-02-4.md");

#[test]
fn actual_kanban_fixture_preserves_vendor_geometry_through_production_postprocess()
-> Result<(), String> {
    assert_public_fixture_geometry(KANBAN_FIXTURE, &["Todo"])
}

#[test]
fn actual_ishikawa_fixture_preserves_vendor_geometry_through_production_postprocess()
-> Result<(), String> {
    assert_public_fixture_geometry(ISHIKAWA_FIXTURE, &["ぼやけた写真"])
}

fn assert_public_fixture_geometry(markdown: &str, metric_labels: &[&str]) -> Result<(), String> {
    let source = fixture_mermaid_source(markdown)?;
    let request = raw_geometry_request(&source);
    let config = raw_geometry_vendor_config();
    let envelope =
        render_vendor_raw_and_processed_with_metric_labels(&request, &config, metric_labels)?;
    assert_eq!(envelope["renderInput"].as_str(), Some(source.as_str()));
    let expected = raw_geometry_tree(envelope_svg(&envelope, "raw")?)?;
    let mut actual = raw_geometry_tree(envelope_svg(&envelope, "processed")?)?;
    normalize_permitted_changes(&expected, &mut actual)?;
    assert_eq!(expected, actual);
    Ok(())
}
