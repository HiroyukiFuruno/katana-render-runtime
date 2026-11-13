use super::super::style::CssStyle;
use super::escape_xml;

#[path = "svg_border_edges.rs"]
mod edges;
#[path = "svg_border_rounded.rs"]
mod rounded;
#[path = "svg_border_uniform.rs"]
mod uniform;

use uniform::{append_uniform_border, uniform_edge_style};

#[cfg(test)]
#[path = "svg_border_tests.rs"]
mod uniform_tests;

pub(super) fn append_border(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) {
    if style.has_border_edge_overrides() {
        if let Some(uniform) = uniform_edge_style(style) {
            append_uniform_border(svg, x, y, width, height, &uniform);
            return;
        }
        append_border_edges(svg, x, y, width, height, style);
        return;
    }
    append_uniform_border(svg, x, y, width, height, style);
}

fn append_border_edges(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) {
    if style.resolved_border_radius(width, height) != (0.0, 0.0)
        && rounded::rounded_edge_colors_present(style)
        && !style.border_has_dash_style()
    {
        rounded::append_rounded_border_edges(svg, x, y, width, height, style);
        return;
    }
    for edge in edges::border_edges(x, y, width, height, style) {
        append_border_edge(
            svg,
            edge.start,
            edge.end,
            edge.color,
            edge.width,
            edge.dasharray,
            edge.dotted,
        );
    }
}

fn edge_dash_attribute(dasharray: Option<String>, dotted: bool) -> String {
    dasharray.map_or_else(String::new, |value| {
        format!(
            r#" stroke-dasharray="{value}"{}"#,
            if dotted {
                r#" stroke-linecap="round""#
            } else {
                ""
            }
        )
    })
}

fn append_border_edge(
    svg: &mut String,
    start: (f32, f32),
    end: (f32, f32),
    color: Option<&str>,
    width: f32,
    dasharray: Option<String>,
    dotted: bool,
) {
    let Some(color) = color.filter(|_| width > 0.0) else {
        return;
    };
    let dash = edge_dash_attribute(dasharray, dotted);
    svg.push_str(&format!(
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{width}"{dash}/>"#,
        start.0,
        start.1,
        end.0,
        end.1,
        escape_xml(color)
    ));
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, append_border};

    #[test]
    fn border_edge_without_color_or_positive_width_is_not_painted() {
        let svg = small_border_markup("border-top-width:2px");
        assert!(svg.is_empty());
    }

    #[test]
    fn colored_zero_width_edge_is_omitted_beside_visible_edge() {
        let svg =
            small_border_markup("border-top:0px solid #a01234;border-right:2px solid #203456");
        assert_eq!(svg.matches("<line").count(), 1);
        assert!(!svg.contains("#a01234"));
        assert!(svg.contains("stroke=\"#203456\""));
    }

    fn small_border_markup(css: &str) -> String {
        let attributes = [("style".to_string(), css.to_string())];
        let style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        let mut svg = String::new();
        append_border(&mut svg, 0.0, 0.0, 20.0, 10.0, &style);
        svg
    }

    #[test]
    fn unequal_same_color_borders_keep_rounded_corners() {
        let attributes = [(
            "style".to_string(),
            "border:2px solid #334455;border-left-width:8px;border-radius:12px".to_string(),
        )];
        let style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        let mut svg = String::new();

        append_border(&mut svg, 0.0, 0.0, 120.0, 48.0, &style);

        assert!(svg.contains("<path"));
        assert_eq!(svg.matches("<path").count(), 4);
        assert!(svg.contains(" A "));
        assert!(!svg.contains("<line"));
    }

    #[test]
    fn unequal_colored_rounded_border_paths_parse_as_svg() {
        let attributes = [(
            "style".to_string(),
            "border-top:1px solid #e2e8f0;border-right:1px solid #e2e8f0;border-bottom:1px solid #e2e8f0;border-left:4px solid #1e40af;border-radius:8px".to_string(),
        )];
        let style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        let mut paths = String::new();
        append_border(&mut paths, 16.0, 16.0, 588.0, 190.0, &style);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 240">{paths}</svg>"#
        );
        let parsed = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default());
        assert!(parsed.is_ok(), "invalid rounded border SVG: {parsed:?}");
    }

    #[test]
    fn dashed_rounded_border_keeps_dash_pattern_on_uniform_outline() {
        let attributes = [(
            "style".to_string(),
            "border:2px dashed #334455;border-radius:12px".to_string(),
        )];
        let style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        let mut svg = String::new();

        append_border(&mut svg, 0.0, 0.0, 120.0, 48.0, &style);

        assert_eq!(svg.matches("<rect").count(), 1);
        assert!(!svg.contains("<path"));
        assert!(svg.contains("stroke-dasharray=\"6 6\""));
        assert!(svg.contains("rx=\"11\""));
    }

    #[test]
    fn dashed_and_dotted_edges_emit_svg_dash_patterns() {
        let attributes = [(
            "style".to_string(),
            "border:2px dashed #334455;border-bottom-style:dotted".to_string(),
        )];
        let style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        let mut svg = String::new();

        append_border(&mut svg, 0.0, 0.0, 120.0, 48.0, &style);

        assert!(svg.contains("stroke-dasharray=\"6 6\""));
        assert!(svg.contains("stroke-dasharray=\"2 2\""));
        assert!(svg.contains("stroke-linecap=\"round\""));
    }
}
