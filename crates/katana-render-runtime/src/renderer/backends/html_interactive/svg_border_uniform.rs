use super::super::super::style::CssStyle;
use super::super::escape_xml;

fn uniform_edges_match(style: &CssStyle, width: f32, color: &str) -> bool {
    let top_style = style.border_top_style.unwrap_or(style.border_style);
    let widths = [
        style.border_right_width(),
        style.border_bottom_width(),
        style.border_left_width(),
    ];
    let colors = [
        style.border_right_color(),
        style.border_bottom_color(),
        style.border_left_color(),
    ];
    let side_styles = [
        style.border_right_style.unwrap_or(style.border_style),
        style.border_bottom_style.unwrap_or(style.border_style),
        style.border_left_style.unwrap_or(style.border_style),
    ];
    width > 0.0
        && widths.iter().all(|edge| *edge == width)
        && colors.iter().all(|edge| *edge == Some(color))
        && side_styles.iter().all(|edge| *edge == top_style)
        && style.border_top_dasharray() == style.border_right_dasharray()
        && style.border_top_dasharray() == style.border_bottom_dasharray()
        && style.border_top_dasharray() == style.border_left_dasharray()
        && style.border_top_dotted() == style.border_right_dotted()
        && style.border_top_dotted() == style.border_bottom_dotted()
        && style.border_top_dotted() == style.border_left_dotted()
}

pub(super) fn uniform_edge_style(style: &CssStyle) -> Option<CssStyle> {
    let width = style.border_top_width();
    let color = style.border_top_color()?;
    let top_style = style.border_top_style.unwrap_or(style.border_style);
    if !uniform_edges_match(style, width, color) {
        return None;
    }
    let mut uniform = style.clone();
    uniform.border = Some(color.to_string());
    uniform.border_style = top_style;
    uniform.border_width = width;
    Some(uniform)
}

fn border_dash_attribute(style: &CssStyle) -> String {
    style.border_dasharray().map_or_else(String::new, |value| {
        format!(
            r#" stroke-dasharray="{value}"{}"#,
            if style.border_style_is_dotted() {
                r#" stroke-linecap="round""#
            } else {
                ""
            }
        )
    })
}

struct UniformBorderGeometry<'a> {
    border: &'a str,
    origin_inset: f32,
    painted_width: f32,
    painted_height: f32,
    radius: (f32, f32),
    border_width: f32,
}

fn uniform_geometry(
    style: &CssStyle,
    width: f32,
    height: f32,
) -> Option<UniformBorderGeometry<'_>> {
    let border = style.border.as_deref()?;
    if style.border_width <= 0.0 || style.border_is_none() {
        return None;
    }
    let inset = style.border_width / 2.0;
    let painted_width = (width - style.border_width).max(0.0);
    let painted_height = (height - style.border_width).max(0.0);
    let radius = style.resolved_border_radius(width, height);
    let radius = ((radius.0 - inset).max(0.0), (radius.1 - inset).max(0.0));
    Some(UniformBorderGeometry {
        border,
        origin_inset: inset,
        painted_width,
        painted_height,
        radius,
        border_width: style.border_width,
    })
}

pub(super) fn append_uniform_border(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) {
    append_uniform_border_inner(svg, (x, y), (width, height), style);
}

fn append_uniform_border_inner(
    svg: &mut String,
    origin: (f32, f32),
    size: (f32, f32),
    style: &CssStyle,
) {
    let Some(geometry) = uniform_geometry(style, size.0, size.1) else {
        return;
    };
    let dash = border_dash_attribute(style);
    append_uniform_rect(svg, origin, geometry, &dash);
}

fn append_uniform_rect(
    svg: &mut String,
    origin: (f32, f32),
    geometry: UniformBorderGeometry<'_>,
    dash: &str,
) {
    let inset = geometry.origin_inset;
    svg.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" fill="none" stroke="{}" stroke-width="{}"{dash}/>"#,
        origin.0 + inset,
        origin.1 + inset,
        geometry.painted_width,
        geometry.painted_height,
        geometry.radius.0,
        geometry.radius.1,
        escape_xml(geometry.border),
        geometry.border_width,
    ));
}

#[cfg(test)]
mod tests {
    use super::append_uniform_border;
    use crate::renderer::backends::html_interactive::style::CssStyle;

    #[test]
    fn dotted_uniform_border_emits_round_dash_caps() {
        let style = CssStyle::from_attributes(
            &[("style".to_string(), "border: 2px dotted red".to_string())],
            &CssStyle::browser_default(),
        );
        let mut svg = String::new();

        append_uniform_border(&mut svg, 0.0, 0.0, 100.0, 40.0, &style);

        assert!(svg.contains("stroke-dasharray="));
        assert!(svg.contains("stroke-linecap=\"round\""));
    }

    #[test]
    fn none_or_zero_width_uniform_border_emits_nothing() {
        for declaration in ["border: none", "border: 0 solid red"] {
            let style = CssStyle::from_attributes(
                &[("style".to_string(), declaration.to_string())],
                &CssStyle::browser_default(),
            );
            let mut svg = String::new();
            append_uniform_border(&mut svg, 0.0, 0.0, 100.0, 40.0, &style);
            assert!(svg.is_empty(), "declaration {declaration}");
        }
    }
}
