use super::super::layout::HtmlLayoutRenderer;
use super::{
    embedded_svg_size, image_box_size, parse_svg_length, parse_view_box, position_embedded_svg,
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
