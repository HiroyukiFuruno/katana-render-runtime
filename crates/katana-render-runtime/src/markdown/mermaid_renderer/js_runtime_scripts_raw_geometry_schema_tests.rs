use xml::name::OwnedName;
use xml::reader::XmlEvent;

/* WHY: 未対応の属性やDTDを同一出力という理由で黙認せず、既知SVG/XML名だけ比較する。 */
pub(super) fn validate_supported_geometry_events(events: &[XmlEvent]) -> Result<(), String> {
    let mut has_root = false;
    for event in events {
        if let XmlEvent::StartElement {
            name, attributes, ..
        } = event
        {
            if !has_root && name.local_name != "svg" {
                return Err("SVG root missing".to_string());
            }
            has_root = true;
            validate_element_name(name)?;
            validate_attributes(attributes)?;
        } else if matches!(event, XmlEvent::Doctype { .. }) {
            return Err("unsupported XML doctype".to_string());
        }
    }
    if !has_root {
        return Err("SVG root missing".to_string());
    }
    Ok(())
}

fn validate_attributes(attributes: &[xml::attribute::OwnedAttribute]) -> Result<(), String> {
    for (index, attribute) in attributes.iter().enumerate() {
        validate_attribute_name(&attribute.name)?;
        if attributes[..index].iter().any(|other| {
            other.name.namespace == attribute.name.namespace
                && other.name.local_name == attribute.name.local_name
        }) {
            return Err("duplicate XML attribute".to_string());
        }
    }
    Ok(())
}

fn validate_element_name(name: &OwnedName) -> Result<(), String> {
    if name.namespace.as_deref() != Some("http://www.w3.org/2000/svg")
        || !SUPPORTED_GEOMETRY_ELEMENTS.contains(&name.local_name.as_str())
    {
        return Err(format!("unsupported geometry element: {name}"));
    }
    Ok(())
}

fn validate_attribute_name(name: &OwnedName) -> Result<(), String> {
    let supported = match name.namespace.as_deref() {
        None => {
            name.prefix.is_none()
                && SUPPORTED_GEOMETRY_ATTRIBUTES.contains(&name.local_name.as_str())
        }
        Some("http://www.w3.org/1999/xlink") => name.local_name == "href",
        Some("http://www.w3.org/XML/1998/namespace") => {
            matches!(name.local_name.as_str(), "space" | "lang")
        }
        _ => false,
    };
    if !supported {
        return Err(format!("unsupported geometry attribute: {name}"));
    }
    Ok(())
}

const SUPPORTED_GEOMETRY_ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "defs",
    "path",
    "rect",
    "line",
    "polyline",
    "polygon",
    "circle",
    "ellipse",
    "text",
    "tspan",
    "marker",
    "style",
    "title",
    "desc",
    "clipPath",
    "filter",
    "feDropShadow",
    "linearGradient",
    "stop",
];
const SUPPORTED_GEOMETRY_ATTRIBUTES: &[&str] = &[
    "flood-color",
    "flood-opacity",
    "gradientUnits",
    "offset",
    "stdDeviation",
    "stop-color",
    "stop-opacity",
    "id",
    "class",
    "style",
    "role",
    "aria-roledescription",
    "aria-labelledby",
    "aria-describedby",
    "width",
    "height",
    "viewBox",
    "preserveAspectRatio",
    "transform",
    "d",
    "points",
    "x",
    "y",
    "dx",
    "dy",
    "x1",
    "y1",
    "x2",
    "y2",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "fill",
    "stroke",
    "color",
    "fill-opacity",
    "stroke-opacity",
    "stroke-width",
    "opacity",
    "fill-rule",
    "clip-rule",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-dasharray",
    "stroke-dashoffset",
    "marker-start",
    "marker-mid",
    "marker-end",
    "markerUnits",
    "markerWidth",
    "markerHeight",
    "refX",
    "refY",
    "orient",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "text-anchor",
    "textLength",
    "lengthAdjust",
    "dominant-baseline",
    "alignment-baseline",
    "letter-spacing",
    "word-spacing",
    "shape-rendering",
    "text-rendering",
    "clip-path",
    "clipPathUnits",
    "data-id",
    "data-look",
    "data-edge",
    "data-node",
    "data-et",
    "data-source",
    "data-target",
    "data-path",
    "data-points",
];

#[test]
fn unsupported_geometry_attribute_is_rejected_even_if_both_outputs_match() {
    let raw = r#"<svg xmlns="http://www.w3.org/2000/svg" unsupported-geometry="9"/>"#;
    assert!(super::raw_geometry_tree(raw).is_err());
}

#[test]
fn unsupported_filter_element_is_rejected_even_if_both_outputs_match() {
    let raw = r#"<svg xmlns="http://www.w3.org/2000/svg"><filter><feGaussianBlur stdDeviation="3"/></filter></svg>"#;
    assert!(super::raw_geometry_tree(raw).is_err());
}
