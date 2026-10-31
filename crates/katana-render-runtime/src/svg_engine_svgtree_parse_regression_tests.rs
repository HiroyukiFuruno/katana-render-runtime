use super::{AId, parse_document, raw_attribute};
use crate::svg_engine::usvg::{self, ImageRendering, Node};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const DPI: f32 = 96.0;
const PARENT_FONT_SIZE: f32 = 16.0;
const FLOAT_TOLERANCE: f32 = 0.0001;

const INITIAL_VALUES: [(AId, &str); 9] = [
    (AId::Fill, "black"),
    (AId::FillOpacity, "1"),
    (AId::FillRule, "nonzero"),
    (AId::Stroke, "none"),
    (AId::StrokeWidth, "1"),
    (AId::StrokeLinecap, "butt"),
    (AId::StrokeLinejoin, "miter"),
    (AId::StrokeMiterlimit, "4"),
    (AId::Visibility, "visible"),
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
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\"><g><rect id=\"target\" fill=\"inherit\" fill-opacity=\"inherit\" fill-rule=\"inherit\" stroke=\"inherit\" stroke-width=\"inherit\" stroke-linecap=\"inherit\" stroke-linejoin=\"inherit\" stroke-miterlimit=\"inherit\" visibility=\"inherit\"/></g></svg>"
    );
    let document = parse_document(&source, None)?;
    for (aid, expected) in INITIAL_VALUES {
        assert_eq!(raw_attribute(&document, "target", aid)?, expected);
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
