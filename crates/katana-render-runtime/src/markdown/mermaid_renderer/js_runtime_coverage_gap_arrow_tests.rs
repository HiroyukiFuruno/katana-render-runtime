use super::super::arrows;
use resvg::usvg::roxmltree::Document;

const LABEL: &str = "&amp;nbsp;&amp;nbsp;&amp;nbsp;";
const STYLE: &str = "<style>font-family: sans-serif; font-size: 12px;</style>";

fn arrow(polygon: &str, look: &str, label: &str) -> String {
    format!(
        r#"<svg viewBox="0 0 100 100">{STYLE}<g class="node" id="id-blockArrowId6" data-look="{look}"><text>{label}</text>{polygon}</g></svg>"#
    )
}

fn validate(svg: &str) -> Result<(), String> {
    let document = Document::parse(svg).map_err(|error| error.to_string())?;
    arrows::assert_block_arrow(document.root_element(), svg)
}

fn assert_error(svg: &str, expected: &str) {
    assert_eq!(
        validate(svg).err(),
        Some(expected.to_owned()),
        "invalid arrow must be rejected"
    );
}

#[test]
fn block_arrow_validator_reports_missing_semantic_node() -> Result<(), String> {
    let document =
        Document::parse(r#"<svg viewBox="0 0 100 100"/>"#).map_err(|error| error.to_string())?;
    assert_eq!(
        arrows::assert_block_arrow(document.root_element(), "<svg/>").err(),
        Some("missing Block node \"-blockArrowId6\"".to_owned())
    );
    Ok(())
}

#[test]
fn block_arrow_validator_reports_duplicate_polygons() {
    let polygon = r#"<polygon points="0,-10 10,-10 10,0 0,0"/>"#;
    assert_error(
        &arrow(&format!("{polygon}{polygon}"), "classic", LABEL),
        "Block arrow has duplicate polygons",
    );
}

#[test]
fn block_arrow_validator_reports_too_few_vertices() {
    assert_error(
        &arrow(r#"<polygon points="0,-10 10,0"/>"#, "classic", LABEL),
        "Block arrow polygon needs at least three vertices",
    );
}

#[test]
fn block_arrow_validator_reports_nonfinite_polygon_coordinates() {
    assert_error(
        &arrow(
            r#"<polygon points="NaN,-10 10,-10 10,0"/>"#,
            "classic",
            LABEL,
        ),
        "non-finite Block arrow polygon point",
    );
}

#[test]
fn block_arrow_validator_reports_wrong_orientation() {
    assert_error(
        &arrow(
            r#"<polygon points="0,10 10,10 10,0 0,0"/>"#,
            "classic",
            LABEL,
        ),
        "Block arrow polygon no longer points down from its baseline",
    );
}

#[test]
fn block_arrow_validator_reports_wrong_identity_and_missing_points() {
    let polygon = r#"<polygon points="0,-10 10,-10 10,0 0,0"/>"#;
    assert_error(
        &arrow(polygon, "other", LABEL),
        "Block arrow lost its classic rendering identity",
    );
    assert_error(
        &arrow(polygon, "classic", "arrow"),
        "Block arrow must retain three serialized NBSP tokens",
    );
    assert_error(
        &arrow("<polygon/>", "classic", LABEL),
        "Block arrow polygon has no points",
    );
}

#[test]
fn connected_block_edge_validator_reports_missing_edge_and_changed_label() -> Result<(), String> {
    let svg = format!(
        r#"<svg viewBox="0 0 100 100"><g class="node" id="id-blockArrowId6"><text>{LABEL}</text></g></svg>"#
    );
    let document = Document::parse(&svg).map_err(|error| error.to_string())?;
    assert_eq!(
        arrows::assert_connected_block_edges(document.root_element()).err(),
        Some("missing connected Block edge \"ID-D\"".to_owned())
    );
    let changed = svg.replace(LABEL, "arrow");
    let document = Document::parse(&changed).map_err(|error| error.to_string())?;
    assert_eq!(
        arrows::assert_connected_block_edges(document.root_element()).err(),
        Some("Block NBSP arrow label changed".to_owned())
    );
    Ok(())
}
