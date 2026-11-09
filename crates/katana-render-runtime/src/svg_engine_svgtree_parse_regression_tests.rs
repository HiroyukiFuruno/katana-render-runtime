use super::{AId, parse_document, raw_attribute};
use crate::svg_engine::usvg::{self, ImageRendering, Node};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const DPI: f32 = 96.0;
const PARENT_FONT_SIZE: f32 = 16.0;
const FLOAT_TOLERANCE: f32 = 0.0001;
const OVER_LIMIT_SVG_CHILD_COUNT: usize = 1_000_001;

#[test]
fn svg_node_limit_rejects_wide_documents() -> Result<(), String> {
    let children = "<g/>".repeat(OVER_LIMIT_SVG_CHILD_COUNT);
    let source = format!(r#"<svg xmlns="{SVG_NS}">{children}</svg>"#);
    let xml = roxmltree::Document::parse(&source).map_err(|error| error.to_string())?;
    assert!(matches!(
        super::Document::parse_tree(&xml, None),
        Err(roxmltree::Error::NodesLimitReached)
    ));
    Ok(())
}

#[test]
fn recursive_use_children_do_not_expand_or_remove_valid_siblings() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg">
      <defs>
        <g id="parent"><use href="#parent"/><rect width="9" height="9"/></g>
        <g id="indirect"><use href="#indirect-instance"/><rect width="9" height="9"/></g>
        <rect id="target" width="4" height="4"/>
      </defs>
      <use id="parent-instance" href="#parent"/>
      <use id="indirect-instance" href="#indirect"/>
      <use id="valid-instance" href="#target"/>
      <rect id="after" width="8" height="8" fill="green"/>
      </svg>"##;
    let document = parse_document(source, None)?;
    for id in ["parent-instance", "indirect-instance"] {
        let node = document.element_by_id(id).ok_or("use element missing")?;
        assert!(node.children().next().is_none(), "{id}");
    }
    let valid = document
        .element_by_id("valid-instance")
        .and_then(|node| node.children().next())
        .ok_or("valid use did not expand")?;
    assert_eq!(valid.attribute::<&str>(AId::Width), Some("4"));
    assert_eq!(raw_attribute(&document, "after", AId::Fill)?, "green");
    Ok(())
}

const INITIAL_VALUES: [(AId, &str); 18] = [
    (AId::Fill, "black"),
    (AId::FillOpacity, "1"),
    (AId::FillRule, "nonzero"),
    (AId::Stroke, "none"),
    (AId::StrokeWidth, "1"),
    (AId::StrokeLinecap, "butt"),
    (AId::StrokeLinejoin, "miter"),
    (AId::StrokeMiterlimit, "4"),
    (AId::StrokeDashoffset, "0"),
    (AId::Visibility, "visible"),
    (AId::FontSize, "medium"),
    (AId::BaselineShift, "baseline"),
    (AId::ColorInterpolationFilters, "linearRGB"),
    (AId::Direction, "ltr"),
    (AId::Display, "inline"),
    (AId::Overflow, "visible"),
    (AId::TextAnchor, "start"),
    (AId::WritingMode, "lr-tb"),
];

const IMAGE_RENDERING_VALUES: [(&str, Option<ImageRendering>); 8] = [
    ("auto", Some(ImageRendering::OptimizeQuality)),
    ("optimizeSpeed", Some(ImageRendering::OptimizeSpeed)),
    ("optimizeQuality", Some(ImageRendering::OptimizeQuality)),
    ("smooth", Some(ImageRendering::Smooth)),
    ("high-quality", Some(ImageRendering::HighQuality)),
    ("crisp-edges", Some(ImageRendering::CrispEdges)),
    ("pixelated", Some(ImageRendering::Pixelated)),
    ("unsupported", None),
];

#[test]
fn explicit_inherit_without_authored_ancestor_uses_svg_initial_values() -> Result<(), String> {
    let inherit_attributes = INITIAL_VALUES
        .iter()
        .map(|(aid, _)| format!(" {}=\"inherit\"", aid.to_str()))
        .collect::<String>();
    let source =
        format!("<svg xmlns=\"{SVG_NS}\"><g><rect id=\"target\"{inherit_attributes}/></g></svg>");
    let document = parse_document(&source, None)?;
    for (aid, expected) in INITIAL_VALUES {
        assert_eq!(raw_attribute(&document, "target", aid)?, expected);
    }
    Ok(())
}

#[test]
fn inherited_dash_offset_matches_public_stroke_values() -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\" width=\"40\" height=\"20\"><rect id=\"default\" width=\"10\" height=\"10\" stroke=\"black\" stroke-dasharray=\"2 3\" stroke-dashoffset=\"inherit\"/><g stroke-dashoffset=\"3\"><rect id=\"inherited\" x=\"10\" width=\"10\" height=\"10\" stroke=\"black\" stroke-dasharray=\"2 3\" stroke-dashoffset=\"inherit\"/></g><rect id=\"explicit\" x=\"20\" width=\"10\" height=\"10\" stroke=\"black\" stroke-dasharray=\"2 3\" stroke-dashoffset=\"3\"/></svg>"
    );
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    for (id, expected) in [("default", 0.0), ("inherited", 3.0), ("explicit", 3.0)] {
        let Some(Node::Path(path)) = tree.node_by_id(id) else {
            return Err(format!("{id} rectangle missing"));
        };
        let stroke = path.stroke().ok_or("stroke missing")?;
        assert_eq!(stroke.dashoffset(), expected, "{id}");
        assert_eq!(stroke.dasharray(), Some(&[2.0, 3.0][..]), "{id}");
    }
    Ok(())
}

#[test]
fn image_rendering_values_agree_between_presentation_attributes_and_css() -> Result<(), String> {
    for (value, expected) in IMAGE_RENDERING_VALUES {
        assert_image_rendering_equivalence(value, expected)?;
    }
    Ok(())
}

fn assert_image_rendering_equivalence(
    value: &str,
    expected: Option<ImageRendering>,
) -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\"><image id=\"attribute\" image-rendering=\"{value}\"/><image id=\"style\" style=\"image-rendering: {value}\"/></svg>"
    );
    let document = parse_document(&source, None)?;
    let attribute = document
        .element_by_id("attribute")
        .ok_or_else(|| "presentation image missing".to_owned())?;
    let styled = document
        .element_by_id("style")
        .ok_or_else(|| "styled image missing".to_owned())?;
    assert_eq!(
        attribute.try_attribute::<ImageRendering>(AId::ImageRendering),
        expected
    );
    assert_eq!(
        styled.try_attribute::<ImageRendering>(AId::ImageRendering),
        expected
    );
    Ok(())
}

#[test]
fn invalid_font_shorthand_does_not_replace_a_valid_longhand() -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\"><g id=\"target\" style=\"font-size: 18px; font: invalid\"/></svg>"
    );
    let document = parse_document(&source, None)?;
    assert_eq!(raw_attribute(&document, "target", AId::FontSize)?, "18px");
    assert!(
        document
            .element_by_id("target")
            .ok_or_else(|| "text node missing".to_owned())?
            .try_attribute::<&str>(AId::FontFamily)
            .is_none()
    );
    Ok(())
}

fn parsed_rect_width(width: &str, child_font_size: Option<&str>) -> Result<f32, String> {
    let font_attribute = match child_font_size {
        Some(size) => format!(" font-size=\"{size}\""),
        None => String::new(),
    };
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\" width=\"300\" height=\"200\"><g font-size=\"16px\"><rect id=\"sample\"{font_attribute} width=\"{width}\" height=\"1px\"/></g></svg>"
    );
    let options = usvg::Options {
        dpi: DPI,
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&source, &options).map_err(|error| error.to_string())?;
    match tree.node_by_id("sample") {
        Some(Node::Path(path)) => Ok(path.abs_bounding_box().width()),
        _ => Err("sample rectangle was not converted to a path".to_owned()),
    }
}

fn assert_width(width: &str, font_size: Option<&str>, expected: f32) -> Result<(), String> {
    let actual = parsed_rect_width(width, font_size)?;
    if (actual - expected).abs() > FLOAT_TOLERANCE {
        return Err(format!(
            "width {width} resolved to {actual}, expected {expected}"
        ));
    }
    Ok(())
}

#[test]
fn physical_lengths_use_the_dpi_conversion_factors() -> Result<(), String> {
    for (length, expected) in [
        ("1in", DPI),
        ("1cm", DPI / 2.54),
        ("1mm", DPI / 25.4),
        ("1pt", DPI / 72.0),
        ("1pc", DPI / 6.0),
    ] {
        assert_width(length, None, expected)?;
    }
    Ok(())
}

#[test]
fn em_lengths_resolve_inherited_and_named_font_sizes() -> Result<(), String> {
    assert_width("2em", None, PARENT_FONT_SIZE * 2.0)?;
    assert_width("2em", Some("larger"), PARENT_FONT_SIZE * 1.2 * 2.0)?;
    Ok(())
}

#[test]
fn invalid_named_font_size_keeps_the_parent_size_for_em_conversion() -> Result<(), String> {
    assert_width("2em", Some("not-a-font-size"), PARENT_FONT_SIZE * 2.0)
}
