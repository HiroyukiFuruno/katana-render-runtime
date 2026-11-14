use super::super::{
    DiagramColorPreset, MermaidJsRuntimeOps, RuntimeAsset, assert_block_nbsp_arrow_svg,
    block_nbsp_arrow_source, layout,
};
use resvg::usvg::roxmltree::Document;

const BLOCK: &str = r#"<svg viewBox="0 0 300 100"><g class="node" id="id-ID"><rect class="label-container" x="0" y="0" width="250" height="80"/></g><g class="node" id="id-A"><rect class="label-container" x="10" y="10" width="30" height="20"/></g><g class="node" id="id-B"><rect class="label-container" x="100" y="10" width="30" height="20"/></g><g class="node" id="id-C"><rect class="label-container" x="190" y="10" width="30" height="20"/></g></svg>"#;

#[test]
fn rendered_block_rejects_invalid_geometry_and_xml() -> Result<(), String> {
    let asset = RuntimeAsset::mermaid();
    let path = asset.materialize_at(asset.materialized_path())?;
    let svg =
        MermaidJsRuntimeOps::render(block_nbsp_arrow_source(), &path, DiagramColorPreset::dark())?;
    assert_block_nbsp_arrow_svg(&svg)?;
    let document = Document::parse(&svg).map_err(|error| error.to_string())?;
    let block = layout::semantic_node(document.root_element(), "-ID")?;
    let rect = layout::label_rectangle(block)?;
    for name in ["x", "transform"] {
        let mut changed = svg.clone();
        if let Some(attribute) = rect.attribute_node(name) {
            changed.replace_range(attribute.range_value(), "invalid");
        } else {
            changed.insert_str(
                rect.range().start + "<rect".len(),
                &format!(" {name}=\"invalid\""),
            );
        }
        assert!(
            assert_block_nbsp_arrow_svg(&changed).is_err(),
            "accepted {name}"
        );
    }
    let changed = svg.replacen("</svg>", "</sv", 1);
    assert_ne!(changed, svg);
    assert!(assert_block_nbsp_arrow_svg(&changed).is_err());
    assert_missing_view_box(&svg)?;
    Ok(())
}

fn assert_missing_view_box(svg: &str) -> Result<(), String> {
    let document = Document::parse(svg).map_err(|error| error.to_string())?;
    let attribute = document
        .root_element()
        .attribute_node("viewBox")
        .ok_or("rendered Block viewBox missing")?;
    let mut changed = svg.to_owned();
    changed.replace_range(attribute.range(), "");
    assert_bounds_error(&changed, "missing SVG viewBox")
}

fn assert_bounds_error(svg: &str, expected: &str) -> Result<(), String> {
    let document = Document::parse(svg).map_err(|error| error.to_string())?;
    let root = document.root_element();
    let block = layout::semantic_node(root, "-ID")?;
    assert_eq!(
        layout::assert_block_containment(root, block).err(),
        Some(expected.to_owned())
    );
    Ok(())
}

#[test]
fn block_geometry_helper_reports_missing_semantic_node() -> Result<(), String> {
    let document =
        Document::parse(r#"<svg viewBox="0 0 300 100"/>"#).map_err(|error| error.to_string())?;
    assert_eq!(
        layout::semantic_node(document.root_element(), "-A").err(),
        Some("missing Block node \"-A\"".to_owned())
    );
    Ok(())
}

#[test]
fn block_geometry_helper_reports_duplicate_semantic_node() -> Result<(), String> {
    let document = Document::parse(
        r#"<svg viewBox="0 0 300 100"><g class="node" id="id-A"/><g class="node" id="other-A"/></svg>"#,
    )
    .map_err(|error| error.to_string())?;
    assert_eq!(
        layout::semantic_node(document.root_element(), "-A").err(),
        Some("duplicate Block node \"-A\"".to_owned())
    );
    Ok(())
}

#[test]
fn block_geometry_helper_reports_missing_label_rectangle() -> Result<(), String> {
    let document =
        Document::parse(r#"<svg><g><rect/></g></svg>"#).map_err(|error| error.to_string())?;
    let group = document
        .root_element()
        .descendants()
        .find(|n| n.has_tag_name("g"))
        .ok_or_else(|| "fixture SVG must contain a group".to_owned())?;
    assert_eq!(
        layout::label_rectangle(group).err(),
        Some("missing label-container rectangle".to_owned())
    );
    Ok(())
}

#[test]
fn block_geometry_helper_reports_duplicate_label_rectangles() -> Result<(), String> {
    let document = Document::parse(
        r#"<svg><g><rect class="label-container"/><rect class="label-container"/></g></svg>"#,
    )
    .map_err(|error| error.to_string())?;
    let group = document
        .root_element()
        .descendants()
        .find(|n| n.has_tag_name("g"))
        .ok_or_else(|| "fixture SVG must contain a group".to_owned())?;
    assert_eq!(
        layout::label_rectangle(group).err(),
        Some("duplicate label-container rectangle".to_owned())
    );
    Ok(())
}

#[test]
fn block_bounds_validator_reports_invalid_numeric_geometry() -> Result<(), String> {
    assert_bounds_error(
        &BLOCK.replace(r#"width="30""#, r#"width="NaN""#),
        "invalid SVG label geometry",
    )?;
    assert_bounds_error(
        &BLOCK.replace(r#"viewBox="0 0 300 100""#, r#"viewBox="0 0 0 100""#),
        "viewBox has a negative or zero size",
    )?;
    Ok(())
}

#[test]
fn block_bounds_validator_reports_overlapping_children() -> Result<(), String> {
    assert_bounds_error(
        &BLOCK.replace(r#"x="100""#, r#"x="25""#),
        "Block child labels are not ordered and separated",
    )?;
    Ok(())
}

#[test]
fn block_bounds_validator_reports_child_outside_parent() -> Result<(), String> {
    assert_bounds_error(
        &BLOCK.replace(r#"x="190""#, r#"x="260""#),
        "Block composite does not enclose each child label",
    )?;
    Ok(())
}

#[test]
fn block_bounds_validator_reports_parent_outside_viewbox() -> Result<(), String> {
    assert_bounds_error(
        &BLOCK.replace(r#"viewBox="0 0 300 100""#, r#"viewBox="0 0 200 100""#),
        "Block composite extends beyond the SVG viewBox",
    )?;
    Ok(())
}

#[path = "js_runtime_block_style_boundary_tests.rs"]
mod style_boundaries;
