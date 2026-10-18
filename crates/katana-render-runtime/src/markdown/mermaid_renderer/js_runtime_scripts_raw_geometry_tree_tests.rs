use xml::reader::{ParserConfig, XmlEvent};
#[path = "js_runtime_scripts_raw_geometry_schema_tests.rs"]
mod schema;
use schema::validate_supported_geometry_events;

#[derive(Debug, PartialEq)]
pub(super) struct RawGeometryNode(Vec<XmlEvent>);

/* WHY: local名だけに縮めるとnamespaceやPIの描画差が消えるので、完全なXMLイベントを保持する。 */
pub(super) fn raw_geometry_tree(svg: &str) -> Result<RawGeometryNode, String> {
    let mut config = ParserConfig::new()
        .ignore_comments(false)
        .coalesce_characters(false)
        .ignore_root_level_whitespace(false);
    config.allow_multiple_root_elements = false;
    let events = config
        .create_reader(svg.as_bytes())
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    validate_supported_geometry_events(&events)?;
    Ok(RawGeometryNode(events))
}

/* WHY: 色属性の存在を保持し、色token同士の値変更と一般row追加以外は同値化しない。 */
pub(super) fn normalize_permitted_changes(
    raw: &RawGeometryNode,
    processed: &mut RawGeometryNode,
) -> Result<(), String> {
    for (expected, actual) in raw.0.iter().zip(processed.0.iter_mut()) {
        if let (
            XmlEvent::StartElement {
                attributes: left, ..
            },
            XmlEvent::StartElement {
                attributes: right, ..
            },
        ) = (expected, actual)
        {
            normalize_permitted_attributes(left, right)?;
        }
    }
    Ok(())
}

fn normalize_permitted_attributes(
    expected: &[xml::attribute::OwnedAttribute],
    actual: &mut [xml::attribute::OwnedAttribute],
) -> Result<(), String> {
    for actual in actual {
        if let Some(expected) = expected.iter().find(|attr| attr.name == actual.name) {
            normalize_permitted_attribute(expected, actual)?;
        }
    }
    Ok(())
}

fn normalize_permitted_attribute(
    expected: &xml::attribute::OwnedAttribute,
    actual: &mut xml::attribute::OwnedAttribute,
) -> Result<(), String> {
    if expected.name.namespace.is_some() || expected.name.prefix.is_some() {
        return Ok(());
    }
    let name = expected.name.local_name.as_str();
    if name == "class"
        && expected.value == "text-outer-tspan"
        && actual.value == "text-outer-tspan row"
    {
        actual.value.clone_from(&expected.value);
    } else if matches!(name, "fill" | "stroke" | "color") && expected.value != actual.value {
        if !limited_color_token(&expected.value) || !limited_color_token(&actual.value) {
            return Err(format!("unsupported paint change: {name}"));
        }
        actual.value.clone_from(&expected.value);
    }
    Ok(())
}

const SHORT_RGB_HEX_DIGITS: usize = 3;
const RGB_HEX_DIGITS: usize = 6;

fn limited_color_token(value: &str) -> bool {
    if let Some(hex) = value.strip_prefix('#') {
        return matches!(hex.len(), SHORT_RGB_HEX_DIGITS | RGB_HEX_DIGITS)
            && hex.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    matches!(
        value,
        "black" | "white" | "red" | "green" | "blue" | "gray" | "grey"
    )
}

#[test]
fn geometry_comparison_preserves_namespaces_bindings_and_processing_instructions()
-> Result<(), String> {
    let raw = r##"<?xml-stylesheet href="a.css"?><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><path xlink:href="#a" d="M1 2L3 4"/></svg>"##;
    let expected = raw_geometry_tree(raw)?;
    for changed in [
        raw.replace("a.css", "b.css"),
        raw.replace("xlink:", "other:")
            .replace("xmlns:xlink", "xmlns:other"),
        raw.replace("#a", "#b"),
    ] {
        assert_ne!(expected, raw_geometry_tree(&changed)?);
    }
    assert!(
        raw_geometry_tree(&raw.replace("http://www.w3.org/1999/xlink", "urn:unknown")).is_err()
    );
    Ok(())
}

#[test]
fn only_forward_row_and_limited_color_changes_are_equivalent() -> Result<(), String> {
    let raw = r##"<svg xmlns="http://www.w3.org/2000/svg"><text class="text-outer-tspan" fill="#fff">A</text></svg>"##;
    let expected = raw_geometry_tree(raw)?;
    let added = raw
        .replace("text-outer-tspan", "text-outer-tspan row")
        .replace("#fff", "#111");
    let mut actual = raw_geometry_tree(&added)?;
    normalize_permitted_changes(&expected, &mut actual)?;
    assert_eq!(expected, actual);
    for value in ["none", "url(#invisible)", "transparent", "unrecognized"] {
        let mut actual = raw_geometry_tree(&raw.replace("#fff", value))?;
        assert!(normalize_permitted_changes(&expected, &mut actual).is_err());
    }
    let mut dropped = raw_geometry_tree(&raw.replace(" fill=\"#fff\"", ""))?;
    normalize_permitted_changes(&expected, &mut dropped)?;
    assert_ne!(expected, dropped);
    let mut backwards = raw_geometry_tree(raw)?;
    normalize_permitted_changes(&raw_geometry_tree(&added)?, &mut backwards)?;
    assert_ne!(raw_geometry_tree(&added)?, backwards);
    Ok(())
}

#[test]
fn raw_geometry_comparison_detects_attribute_drop_and_node_changes() -> Result<(), String> {
    let raw = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="4 4 90 70"><g id="node-a" transform="translate(13,17)"><path d="M1 2L3 4"/></g></svg>"#;
    let expected = raw_geometry_tree(raw)?;
    for changed in [
        raw.replace("M1 2L3 4", "M5 6L7 8"),
        raw.replace(" transform=\"translate(13,17)\"", ""),
        raw.replace("node-a", "node-b"),
    ] {
        assert_ne!(expected, raw_geometry_tree(&changed)?);
    }
    Ok(())
}

#[test]
fn vendor_filter_and_gradient_geometry_changes_remain_observable() -> Result<(), String> {
    let raw = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><filter id="shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="#FFFFFF"/></filter></defs><linearGradient id="gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="#cccccc" stop-opacity="1"/><stop offset="100%" stop-color="#111111" stop-opacity="1"/></linearGradient></svg>"##;
    let expected = raw_geometry_tree(raw)?;
    for changed in [
        raw.replace("stdDeviation=\"0\"", "stdDeviation=\"1\""),
        raw.replace("dx=\"4\"", "dx=\"5\""),
        raw.replace("flood-opacity=\"0.06\"", "flood-opacity=\"0.07\""),
        raw.replace("<feDropShadow dx=\"4\" dy=\"4\" stdDeviation=\"0\" flood-opacity=\"0.06\" flood-color=\"#FFFFFF\"/>", ""),
        raw.replace("gradientUnits=\"objectBoundingBox\"", "gradientUnits=\"userSpaceOnUse\""),
        raw.replace("offset=\"100%\"", "offset=\"90%\""),
        raw.replace("stop-opacity=\"1\"", "stop-opacity=\"0.5\""),
    ] {
        let mut actual = raw_geometry_tree(&changed)?;
        normalize_permitted_changes(&expected, &mut actual)?;
        assert_ne!(expected, actual);
    }
    Ok(())
}
