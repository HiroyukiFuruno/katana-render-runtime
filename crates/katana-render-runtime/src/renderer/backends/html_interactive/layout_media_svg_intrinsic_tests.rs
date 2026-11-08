use super::super::layout::HtmlLayoutRenderer;
use super::super::style::CssStyle;
use super::test_support::{TestResult, find_svg, svg_coordinate};
use super::{embedded_svg_size, intrinsic_image_width};
use crate::HtmlBrowserViewport;
use crate::renderer::backends::html_document::{HtmlDocument, HtmlDocumentNode};
use std::collections::HashMap;
use xmltree::Element;

const NATURAL_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="50" viewBox="0 0 80 50"><rect width="80" height="50" fill="#ff0000"/><circle cx="40" cy="25" r="15" fill="#0000ff"/></svg>"##;
const PERCENT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:600.40625px" viewBox="4 4 600.40625 380"><rect x="4" y="4" width="600.40625" height="380" fill="#ff0000"/></svg>"##;
const BACKGROUND_RGBA: [u8; 4] = [30, 30, 30, 255];
const CAPTURE_WIDTH: u32 = 625;
const CAPTURE_HEIGHT: u32 = 404;
const SVG_VIEWPORT_WIDTH: u32 = 601;
const SVG_VIEWPORT_HEIGHT: u32 = 380;
const PERCENT_AUTO_HEIGHT: f32 = 189.859_38;

#[test]
fn natural_svg_keeps_attribute_dimensions_in_body_and_centered_flex() -> TestResult {
    for flex in [false, true] {
        let html = capture_html(NATURAL_SVG, flex);
        assert_geometry(&html, 80.0, 50.0)?;
        let pixels = capture_pixels(&html)?;
        let changed = pixels
            .chunks_exact(4)
            .filter(|pixel| *pixel != BACKGROUND_RGBA)
            .count();
        assert!(changed >= 4_000, "natural SVG lost visible area: {changed}");
        assert!(
            pixels
                .chunks_exact(4)
                .any(|pixel| pixel == [255, 0, 0, 255])
        );
        assert!(
            pixels
                .chunks_exact(4)
                .any(|pixel| pixel == [0, 0, 255, 255])
        );
    }
    Ok(())
}

#[test]
fn percentage_svg_obeys_maximum_width_inside_centered_flex() -> TestResult {
    let html = capture_html(PERCENT_SVG, true);
    assert_geometry(&html, 600.406_25, 380.0)?;
    let pixels = capture_pixels(&html)?;
    let red = pixels
        .chunks_exact(4)
        .filter(|pixel| *pixel == [255, 0, 0, 255])
        .count();
    assert!(red > 220_000, "percentage SVG lost visible area: {red}");
    Ok(())
}

#[test]
fn percentage_svg_uses_default_intrinsic_width_in_nested_auto_flex() -> TestResult {
    let html = capture_auto_viewport_html(PERCENT_SVG);
    assert_geometry(&html, 300.0, PERCENT_AUTO_HEIGHT)
}

#[test]
fn embedded_svg_auto_margins_position_the_resolved_width() -> TestResult {
    let cases = [
        (320, "80", "margin:0 auto", 120.0, 80.0),
        (320, "80", "margin-left:auto;margin-right:0", 240.0, 80.0),
        (320, "80", "margin-left:0;margin-right:auto", 0.0, 80.0),
        (320, "80", "margin-left:12px;margin-right:20px", 12.0, 80.0),
        (320, "100%", "max-width:120px;margin:0 auto", 100.0, 120.0),
        (60, "80", "margin:0 auto", 0.0, 60.0),
    ];
    for (host_width, svg_width, style, expected_x, expected_width) in cases {
        let styled_svg = NATURAL_SVG.replacen(
            "width=\"80\" height=\"50\"",
            &format!("width=\"{svg_width}\" height=\"50\" style=\"{style}\""),
            1,
        );
        let html = format!(
            "<style>html,body{{margin:0}}</style><div style=\"width:{host_width}px\">{styled_svg}</div>"
        );
        assert_eq!(
            rendered_svg_x_and_width(&html)?,
            (expected_x, expected_width)
        );
    }
    Ok(())
}

#[test]
fn explicit_css_width_takes_priority_over_svg_width_attribute() -> TestResult {
    let svg = NATURAL_SVG.replacen(
        "width=\"80\" height=\"50\"",
        "width=\"80\" height=\"50\" style=\"width:160px\"",
        1,
    );
    /* WHY: SVG2 presentation属性のheightは、widthだけのCSS指定ではautoにならない。 */
    assert_geometry(&capture_html(&svg, true), 160.0, 50.0)
}

#[test]
fn explicit_css_height_takes_priority_over_svg_height_attribute() -> TestResult {
    let svg = NATURAL_SVG.replacen(
        "width=\"80\" height=\"50\"",
        "width=\"80\" height=\"50\" style=\"width:160px;height:30px\"",
        1,
    );
    assert_geometry(&capture_html(&svg, true), 160.0, 30.0)
}

#[test]
fn css_auto_height_uses_svg_view_box_ratio_instead_of_height_attribute() {
    let attributes = [
        ("width".to_owned(), "1144".to_owned()),
        ("height".to_owned(), "404".to_owned()),
        ("viewbox".to_owned(), "0 0 1144 404".to_owned()),
    ];
    let auto_style = CssStyle::from_attributes(
        &[("style".to_owned(), "width:1120px;height:auto".to_owned())],
        &CssStyle::browser_default(),
    );
    let auto_size = embedded_svg_size(&attributes, 1120.0, &auto_style);
    assert_eq!(auto_size.1, 395.515_63);

    let width_only_style = CssStyle::from_attributes(
        &[("style".to_owned(), "width:1120px".to_owned())],
        &CssStyle::browser_default(),
    );
    assert_eq!(
        embedded_svg_size(&attributes, 1120.0, &width_only_style),
        (1120.0, 404.0)
    );
}

#[test]
fn percentage_svg_auto_height_uses_view_box_aspect_ratio() {
    let attributes = vec![
        ("width".to_owned(), "100%".to_owned()),
        ("viewbox".to_owned(), "4 4 600.40625 380".to_owned()),
    ];
    let style = CssStyle::from_attributes(
        &[("style".to_owned(), "max-width:600.40625px".to_owned())],
        &CssStyle::browser_default(),
    );
    /* WHY: SVG2 intrinsic比率は百分率の解決幅601ではなくviewBoxから得る。 */
    assert_eq!(
        embedded_svg_size(&attributes, 601.0, &style),
        (600.406_25, 380.0)
    );
}

#[test]
fn maximum_width_does_not_expand_svg_natural_dimensions() -> TestResult {
    let svg = NATURAL_SVG.replacen(
        "width=\"80\" height=\"50\"",
        "width=\"80\" height=\"50\" style=\"max-width:160px\"",
        1,
    );
    assert_geometry(&capture_html(&svg, false), 80.0, 50.0)
}

#[test]
fn minimum_width_wins_over_maximum_with_explicit_css_width() -> TestResult {
    let svg = NATURAL_SVG.replacen(
        "width=\"80\" height=\"50\"",
        "width=\"80\" height=\"50\" style=\"width:80px;min-width:120px;max-width:100px\"",
        1,
    );
    for flex in [false, true] {
        assert_geometry(&capture_html(&svg, flex), 120.0, 50.0)?;
    }
    Ok(())
}

#[test]
fn minimum_width_wins_over_maximum_with_only_svg_width_attribute() -> TestResult {
    let svg = NATURAL_SVG.replacen(
        "width=\"80\" height=\"50\"",
        "width=\"80\" height=\"50\" style=\"min-width:120px;max-width:100px\"",
        1,
    );
    for flex in [false, true] {
        assert_geometry(&capture_html(&svg, flex), 120.0, 50.0)?;
    }
    Ok(())
}

#[test]
fn view_box_only_svg_keeps_existing_intrinsic_width_policy() {
    let node = HtmlDocumentNode::Element {
        node_id: 1,
        tag: "svg".to_owned(),
        attributes: vec![("viewbox".to_owned(), "0 0 80 50".to_owned())],
        children: Vec::new(),
    };
    let style = CssStyle::browser_default();
    assert_eq!(intrinsic_image_width(&node, 601.0, &style), None);
}

fn capture_html(svg: &str, flex: bool) -> String {
    let content = if flex {
        format!("<main class=capture><div class=viewport>{svg}</div></main>")
    } else {
        svg.to_owned()
    };
    capture_document(
        content,
        format!(
            "width:{SVG_VIEWPORT_WIDTH}px;height:{SVG_VIEWPORT_HEIGHT}px;display:flex;align-items:center;justify-content:center"
        ),
        format!("max-width:{SVG_VIEWPORT_WIDTH}px;max-height:{SVG_VIEWPORT_HEIGHT}px"),
    )
}

fn capture_auto_viewport_html(svg: &str) -> String {
    capture_document(
        format!("<main class=capture><div class=viewport>{svg}</div></main>"),
        "display:flex;align-items:center;justify-content:center".to_owned(),
        String::new(),
    )
}

fn capture_document(content: String, viewport_style: String, svg_style: String) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;width:{CAPTURE_WIDTH}px;height:{CAPTURE_HEIGHT}px;overflow:hidden;background:#1e1e1e}}.capture{{width:{CAPTURE_WIDTH}px;height:{CAPTURE_HEIGHT}px;display:flex;align-items:center;justify-content:center}}.viewport{{{viewport_style}}}.viewport>svg{{{svg_style}}}</style></head><body>{content}</body></html>"
    )
}

fn capture_pixels(html: &str) -> TestResult<Vec<u8>> {
    super::test_support::capture_frame_pixels(
        html,
        "https://example.test/natural-svg.html",
        CAPTURE_WIDTH,
        CAPTURE_HEIGHT,
    )
}

fn assert_geometry(html: &str, width: f32, height: f32) -> TestResult {
    let root = rendered_svg_document(html)?;
    let svg = find_svg(&root).ok_or("missing embedded SVG")?;
    for (name, expected) in [("width", width), ("height", height)] {
        let actual = svg
            .attributes
            .get(name)
            .ok_or("missing SVG dimension")?
            .parse::<f32>()?;
        assert!(
            (actual - expected).abs() < 0.001,
            "SVG {name}={actual}, expected {expected}"
        );
    }
    Ok(())
}

fn rendered_svg_x_and_width(html: &str) -> TestResult<(f32, f32)> {
    let root = rendered_svg_document(html)?;
    let svg = find_svg(&root).ok_or("missing embedded SVG")?;
    let x = svg_coordinate(svg, "x")?;
    let width = svg_coordinate(svg, "width")?;
    Ok((x, width))
}

#[test]
fn svg_coordinate_reports_missing_and_invalid_values() -> TestResult {
    let missing = Element::parse("<svg/>".as_bytes())?;
    let missing_error = svg_coordinate(&missing, "width")
        .err()
        .ok_or("missing width was accepted")?;
    assert_eq!(missing_error.to_string(), "missing SVG width");

    let invalid = Element::parse("<svg width=\"wide\"/>".as_bytes())?;
    let actual_error = svg_coordinate(&invalid, "width")
        .err()
        .ok_or("invalid width was accepted")?;
    let expected_error = "wide"
        .parse::<f32>()
        .err()
        .ok_or("invalid numeric fixture was accepted")?;
    assert!(
        actual_error
            .downcast_ref::<std::num::ParseFloatError>()
            .is_some()
    );
    assert_eq!(actual_error.to_string(), expected_error.to_string());
    Ok(())
}

fn rendered_svg_document(html: &str) -> TestResult<Element> {
    let document = HtmlDocument::parse(html);
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let viewport = HtmlBrowserViewport::new(CAPTURE_WIDTH, CAPTURE_HEIGHT, 1.0)?;
    let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;
    Ok(Element::parse(layout.svg.as_bytes())?)
}
