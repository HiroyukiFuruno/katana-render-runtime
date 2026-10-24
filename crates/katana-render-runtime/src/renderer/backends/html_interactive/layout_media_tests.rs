use super::super::layout::HtmlLayoutRenderer;
use super::{
    embedded_svg_size, image_box_size, inherit_svg_text_style, parse_svg_length, parse_view_box,
    position_embedded_svg,
};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_document::HtmlDocumentNode;
use crate::renderer::backends::html_document::{
    EMBEDDED_SVG_HEIGHT_PLACEHOLDER, EMBEDDED_SVG_WIDTH_PLACEHOLDER, EMBEDDED_SVG_X_PLACEHOLDER,
    EMBEDDED_SVG_Y_PLACEHOLDER,
};
use std::collections::HashMap;

type TestResult<T = ()> = Result<T, String>;
const TEST_VIEWPORT_WIDTH: u32 = 320;
const TEST_VIEWPORT_HEIGHT: u32 = 240;
const TEXT_SVG_WIDTH: u32 = 160;
const TEXT_SVG_HEIGHT: u32 = 48;

#[test]
fn image_without_a_source_does_not_emit_svg_image_content() -> TestResult {
    let viewport = HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    let nodes = vec![HtmlDocumentNode::Element {
        node_id: 1,
        tag: "img".to_string(),
        attributes: Vec::new(),
        children: Vec::new(),
    }];
    let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;

    assert!(!layout.svg.contains("<image"));
    Ok(())
}

#[test]
fn data_png_uses_natural_dimensions_instead_of_the_default_image_box() -> TestResult {
    let viewport = HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    let nodes = vec![HtmlDocumentNode::Element {
        node_id: 1,
        tag: "img".to_string(),
        attributes: vec![(
            "src".to_string(),
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_string(),
        )],
        children: Vec::new(),
    }];
    let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;

    assert!(
        layout.svg.contains(r#"width="1" height="1""#),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn data_png_preserves_natural_ratio_under_width_and_height_constraints() {
    let browser_default =
        crate::renderer::backends::html_interactive::style::CssStyle::browser_default();
    let constrained = crate::renderer::backends::html_interactive::style::CssStyle::from_attributes(
        &[(
            "style".to_string(),
            "max-width: 100%; max-height: 400px".to_string(),
        )],
        &browser_default,
    );

    let (width, height) = image_box_size(
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAACBAAAAfd",
        1_200.0,
        &constrained,
    );

    assert!((width - 410.134_12).abs() < 0.001, "{width}");
    assert_eq!(height, 400.0);
    assert!((width / height - 2_064.0 / 2_013.0).abs() < 0.0001);
}

#[test]
fn embedded_svg_without_serialized_markup_does_not_emit_svg_content() -> TestResult {
    let viewport = HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    let nodes = vec![HtmlDocumentNode::Element {
        node_id: 1,
        tag: "svg".to_string(),
        attributes: Vec::new(),
        children: Vec::new(),
    }];
    let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;

    assert_eq!(layout.svg.matches("<svg").count(), 1);
    Ok(())
}

#[test]
fn embedded_svg_sizing_keeps_explicit_height_when_width_is_constrained() {
    let attributes = vec![
        ("width".to_string(), "400".to_string()),
        ("height".to_string(), "200".to_string()),
        ("viewbox".to_string(), "0 0 400 200".to_string()),
    ];
    let unconstrained =
        crate::renderer::backends::html_interactive::style::CssStyle::browser_default();
    let constrained = crate::renderer::backends::html_interactive::style::CssStyle::from_attributes(
        &[("style".to_string(), "max-width: 120px".to_string())],
        &unconstrained,
    );

    assert_eq!(
        embedded_svg_size(&attributes, 300.0, &unconstrained),
        (300.0, 200.0)
    );
    /* WHY: SVG2のheight属性はpresentation hintであり、max-widthでは変更されない。 */
    assert_eq!(
        embedded_svg_size(&attributes, 300.0, &constrained),
        (120.0, 200.0)
    );
}

#[test]
fn embedded_svg_auto_height_preserves_view_box_ratio_and_css_constraints() {
    let unconstrained =
        crate::renderer::backends::html_interactive::style::CssStyle::browser_default();
    let constrained = crate::renderer::backends::html_interactive::style::CssStyle::from_attributes(
        &[("style".to_string(), "max-width: 120px".to_string())],
        &unconstrained,
    );
    let attributes = vec![
        ("width".to_string(), "400".to_string()),
        ("viewbox".to_string(), "0 0 400 200".to_string()),
    ];
    assert_eq!(
        embedded_svg_size(&attributes, 300.0, &unconstrained),
        (300.0, 150.0)
    );
    assert_eq!(
        embedded_svg_size(
            &[("viewbox".to_string(), "0 0 400 200".to_string())],
            300.0,
            &unconstrained,
        ),
        (300.0, 150.0)
    );
    let constrained_size = embedded_svg_size(&attributes, 300.0, &constrained);
    assert_eq!(constrained_size.0, 120.0);
    assert!((constrained_size.1 - 60.0).abs() < 0.0001);
}

#[test]
fn embedded_svg_parsers_and_positioning_reject_invalid_dimensions() {
    assert_eq!(parse_svg_length("50%", 200.0), Some(100.0));
    assert_eq!(parse_svg_length("20px", 200.0), Some(20.0));
    assert_eq!(parse_svg_length("-1", 200.0), None);
    assert_eq!(parse_svg_length("NaN", 200.0), None);
    assert_eq!(parse_view_box("0 0 120 80"), Some((0.0, 0.0, 120.0, 80.0)));
    assert_eq!(parse_view_box("0 0 0 80"), None);
    assert_eq!(parse_view_box("invalid"), None);

    let markup = format!(
        "<svg x=\"{EMBEDDED_SVG_X_PLACEHOLDER}\" y=\"{EMBEDDED_SVG_Y_PLACEHOLDER}\" width=\"{EMBEDDED_SVG_WIDTH_PLACEHOLDER}\" height=\"{EMBEDDED_SVG_HEIGHT_PLACEHOLDER}\"></svg>"
    );
    assert_eq!(
        position_embedded_svg(&markup, 1.0, 2.0, 30.0, 40.0),
        "<svg x=\"1\" y=\"2\" width=\"30\" height=\"40\"></svg>"
    );
}

#[test]
fn embedded_svg_inherits_parent_font_without_rewriting_explicit_child_font() -> TestResult {
    let mut style = crate::renderer::backends::html_interactive::style::CssStyle::browser_default();
    style.font_family = "Noto Sans, sans-serif".to_string();
    style.font_size = 18.0;
    style.font_weight = 700;
    style.italic = true;
    let markup = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{TEXT_SVG_WIDTH}" height="{TEXT_SVG_HEIGHT}"><text x="4" y="24">Parent</text><text x="4" y="44" font-family="monospace">Child</text></svg>"#
    );

    let wrapped = inherit_svg_text_style(&markup, &style);
    assert_inherited_font_attributes(&wrapped)?;
    assert!(wrapped.contains(markup.as_str()));
    assert!(wrapped.contains(r#"font-family="monospace""#));
    let document = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{TEXT_SVG_WIDTH}" height="{TEXT_SVG_HEIGHT}">{wrapped}</svg>"#
    );
    let raster =
        crate::markdown::svg_rasterize::SvgRasterizeOps::rasterize_html_svg(&document, 1.0)
            .map_err(|error| error.to_string())?;
    assert!(raster.rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
    Ok(())
}

fn assert_inherited_font_attributes(markup: &str) -> TestResult {
    let wrapper = xmltree::Element::parse(markup.as_bytes()).map_err(|error| error.to_string())?;
    assert_eq!(wrapper.name, "g");
    for (attribute, expected) in [
        ("font-family", "Noto Sans, sans-serif"),
        ("font-size", "18"),
        ("font-weight", "700"),
        ("font-style", "italic"),
    ] {
        assert_eq!(
            wrapper.attributes.get(attribute).map(String::as_str),
            Some(expected)
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn embedded_svg_layout_uses_inherited_and_explicit_faces() -> TestResult {
    let mut style = crate::renderer::backends::html_interactive::style::CssStyle::browser_default();
    style.font_family = "Helvetica".to_string();
    style.font_size = 18.0;
    style.font_weight = 700;
    style.italic = true;
    let markup = r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="48"><text id="parent" x="4" y="24">Parent</text><text id="child" x="4" y="44" font-family="Times New Roman" font-size="12" font-weight="400" font-style="normal">Child</text></svg>"#;
    let wrapped = inherit_svg_text_style(markup, &style);
    let document = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="48">{wrapped}</svg>"#
    );
    let database = system_font_database();
    let options = resvg::usvg::Options {
        fontdb: std::sync::Arc::new(database),
        ..resvg::usvg::Options::default()
    };
    let tree =
        resvg::usvg::Tree::from_str(&document, &options).map_err(|error| error.to_string())?;
    let parent = positioned_face(tree.node_by_id("parent"), &tree)?;
    let child = positioned_face(tree.node_by_id("child"), &tree)?;
    assert_eq!(parent.font_size, 18.0);
    assert_eq!(parent.weight, 700);
    assert_eq!(parent.style, resvg::usvg::FontStyle::Italic);
    assert_eq!(child.font_size, 12.0);
    assert_eq!(child.weight, 400);
    assert_eq!(child.style, resvg::usvg::FontStyle::Normal);
    assert_ne!(parent.post_script_name, child.post_script_name);
    Ok(())
}

#[cfg(target_os = "macos")]
fn system_font_database() -> resvg::usvg::fontdb::Database {
    let mut database = resvg::usvg::fontdb::Database::new();
    database.load_system_fonts();
    database
}

#[cfg(target_os = "macos")]
struct PositionedFace {
    post_script_name: String,
    font_size: f32,
    weight: u16,
    style: resvg::usvg::FontStyle,
}

#[cfg(target_os = "macos")]
fn positioned_face(
    node: Option<&resvg::usvg::Node>,
    tree: &resvg::usvg::Tree,
) -> TestResult<PositionedFace> {
    let Some(resvg::usvg::Node::Text(text)) = node else {
        return Err("expected text node".to_string());
    };
    let glyph = text
        .layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .next()
        .ok_or("expected positioned glyph")?;
    let face = tree
        .fontdb()
        .face(glyph.font)
        .ok_or("expected selected font face")?;
    Ok(PositionedFace {
        post_script_name: face.post_script_name.clone(),
        font_size: glyph.font_size(),
        weight: face.weight.0,
        style: face.style.into(),
    })
}
