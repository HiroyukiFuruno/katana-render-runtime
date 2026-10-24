use super::super::constants::{CONTROL_HEIGHT, MIN_LAYOUT_WIDTH};
use super::super::document::attribute;
use super::super::style::CssStyle;
use super::DEFAULT_IMAGE_MAX_HEIGHT;

const VIEW_BOX_VALUE_COUNT: usize = 4;
const VIEW_BOX_WIDTH_INDEX: usize = 2;
const VIEW_BOX_HEIGHT_INDEX: usize = 3;
const DEFAULT_INTRINSIC_SVG_WIDTH: f32 = 300.0;
const CSS_LAYOUT_UNITS_PER_PX: f32 = 64.0;
type ViewBox = (f32, f32, f32, f32);

pub(super) fn quantize_css_layout(value: f32) -> f32 {
    (value * CSS_LAYOUT_UNITS_PER_PX).floor() / CSS_LAYOUT_UNITS_PER_PX
}

pub(super) fn intrinsic_svg_width(
    attributes: &[(String, String)],
    available_width: f32,
    style: &CssStyle,
) -> Option<f32> {
    let explicit_width =
        attribute(attributes, "width").and_then(|value| parse_svg_length(value, available_width));
    /* WHY: 百分率はintrinsic幅を持たず、auto flexの測定時に親幅へ展開すると循環する。 */
    if style.width.is_none()
        && explicit_width.is_some()
        && attribute(attributes, "width").is_some_and(|value| value.trim().ends_with('%'))
    {
        return Some(quantize_css_layout(used_svg_width(
            DEFAULT_INTRINSIC_SVG_WIDTH,
            available_width,
            style,
        )));
    }
    /* WHY: 投影済みSVGは子が空でも寸法を持つ。viewBoxだけの既存自然幅方針は変えない。 */
    (style.width.is_some() || explicit_width.is_some())
        .then(|| embedded_svg_size(attributes, available_width, style).0)
}

pub(super) fn embedded_svg_size(
    attributes: &[(String, String)],
    available_width: f32,
    style: &CssStyle,
) -> (f32, f32) {
    let view_box = attribute(attributes, "viewbox").and_then(parse_view_box);
    let natural_width = natural_svg_width(attributes, available_width, view_box);
    let width = quantize_css_layout(used_svg_width(natural_width, available_width, style));
    /* WHY: CSSのheight:autoはSVGの自然比率を使い、height属性より優先する。 */
    let height = if let Some(height) = style.height {
        height
    } else if style.explicit_height_auto {
        auto_svg_height(width, natural_width, view_box)
    } else {
        attribute(attributes, "height")
            .and_then(|value| parse_svg_length(value, available_width))
            .unwrap_or_else(|| auto_svg_height(width, natural_width, view_box))
    }
    .max(CONTROL_HEIGHT);
    let height = quantize_css_layout(apply_svg_height_constraints(height, style));
    (width, height)
}

fn apply_svg_height_constraints(height: f32, style: &CssStyle) -> f32 {
    let height = height.max(CONTROL_HEIGHT);
    let height = style
        .max_height
        .map_or(height, |maximum| height.min(style.outer_height(maximum)));
    height.max(style.minimum_outer_height())
}

fn natural_svg_width(
    attributes: &[(String, String)],
    available_width: f32,
    view_box: Option<ViewBox>,
) -> f32 {
    attribute(attributes, "width")
        .and_then(|value| parse_svg_length(value, available_width))
        .or_else(|| view_box.map(|(_, _, width, _)| width))
        .unwrap_or(available_width)
        .max(MIN_LAYOUT_WIDTH)
}

fn used_svg_width(natural_width: f32, available_width: f32, style: &CssStyle) -> f32 {
    let width = style
        .explicit_width(available_width)
        .unwrap_or(natural_width);
    /* WHY: max-widthは上限であり、属性が指定した自然幅を広げる値ではない。 */
    let width = style.max_width.map_or(width, |maximum| {
        width.min(style.outer_width(maximum.resolve(available_width)))
    });
    /* WHY: CSSの幅制約が競合した場合は、自然幅でもmax-widthよりmin-widthを優先する。 */
    let width = style
        .minimum_outer_width(available_width)
        .map_or(width, |minimum| width.max(minimum));
    width.min(available_width).max(MIN_LAYOUT_WIDTH)
}

fn auto_svg_height(width: f32, natural_width: f32, view_box: Option<ViewBox>) -> f32 {
    /* WHY: 百分率を解決した幅には自然比率がないため、auto高さの比率はviewBoxから得る。 */
    view_box.map_or(
        DEFAULT_IMAGE_MAX_HEIGHT * width / natural_width,
        |(_, _, vb_width, vb_height)| width * vb_height / vb_width,
    )
}

pub(super) fn parse_svg_length(value: &str, available_width: f32) -> Option<f32> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|percent| percent.is_finite() && *percent > 0.0)
            .map(|percent| available_width * percent / 100.0);
    }
    value
        .strip_suffix("px")
        .unwrap_or(value)
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(super) fn parse_view_box(value: &str) -> Option<ViewBox> {
    let values = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == VIEW_BOX_VALUE_COUNT
        && values.iter().all(|value| value.is_finite())
        && values[VIEW_BOX_WIDTH_INDEX] > 0.0
        && values[VIEW_BOX_HEIGHT_INDEX] > 0.0)
        .then(|| {
            (
                values[0],
                values[1],
                values[VIEW_BOX_WIDTH_INDEX],
                values[VIEW_BOX_HEIGHT_INDEX],
            )
        })
}

#[cfg(test)]
mod tests {
    use super::super::super::layout::HtmlLayoutRenderer;
    use super::super::super::style::CssStyle;
    use super::super::test_support::{find_svg, svg_coordinate};
    use super::{embedded_svg_size, quantize_css_layout};
    use crate::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocument;
    use std::collections::HashMap;
    use xmltree::Element;

    const FRACTIONAL_VIEW_BOX_WIDTH: f32 = 235.179_69;
    const FRACTIONAL_VIEW_BOX_HEIGHT: f32 = 367.0;
    const FRACTIONAL_USED_WIDTH: f32 = 235.171_88;
    const FRACTIONAL_USED_HEIGHT: f32 = 366.984_38;
    const FRACTIONAL_AVAILABLE_WIDTH: f32 = 400.0;
    const JOURNEY_HOST_WIDTH: f32 = 924.0;
    const JOURNEY_HOST_HEIGHT: f32 = 540.0;
    const JOURNEY_SVG_WIDTH_VALUE: f32 = 900.0;
    const JOURNEY_MIN_HEIGHT: f32 = 600.0;
    const JOURNEY_CAPTURE_WIDTH: u32 = 924;
    const JOURNEY_CAPTURE_HEIGHT: u32 = 564;
    const JOURNEY_CENTERED_OFFSET: f32 = 12.0;
    const JOURNEY_SVG_WIDTH: &str = "900";
    const JOURNEY_SVG_HEIGHT: &str = "565";
    const JOURNEY_VIEW_BOX: &str = "0 -25 900 540";

    #[test]
    fn svg_dimensions_follow_css_layout_unit_floor() {
        let style = CssStyle::browser_default();
        let attributes = [(
            "viewbox".to_owned(),
            format!("0 0 {FRACTIONAL_VIEW_BOX_WIDTH} {FRACTIONAL_VIEW_BOX_HEIGHT}"),
        )];
        assert_eq!(
            quantize_css_layout(FRACTIONAL_VIEW_BOX_WIDTH),
            FRACTIONAL_USED_WIDTH
        );
        assert_eq!(
            embedded_svg_size(&attributes, FRACTIONAL_AVAILABLE_WIDTH, &style),
            (FRACTIONAL_USED_WIDTH, FRACTIONAL_USED_HEIGHT)
        );
    }

    #[test]
    fn svg_max_height_uses_the_containing_block_without_scaling_width() {
        let mut parent = CssStyle::browser_default();
        parent.percentage_height_basis = Some(JOURNEY_HOST_HEIGHT);
        let style = CssStyle::from_attributes(
            &[("style".to_owned(), "max-height:100%".to_owned())],
            &parent,
        );
        let attributes = journey_svg_attributes();

        assert_eq!(
            embedded_svg_size(&attributes, JOURNEY_HOST_WIDTH, &style),
            (JOURNEY_SVG_WIDTH_VALUE, JOURNEY_HOST_HEIGHT)
        );
    }

    #[test]
    fn svg_min_height_wins_when_max_height_is_smaller() {
        let style = CssStyle::from_attributes(
            &[(
                "style".to_owned(),
                format!("min-height:{JOURNEY_MIN_HEIGHT}px;max-height:{JOURNEY_HOST_HEIGHT}px"),
            )],
            &CssStyle::browser_default(),
        );
        let attributes = journey_svg_attributes();

        assert_eq!(
            embedded_svg_size(&attributes, JOURNEY_HOST_WIDTH, &style),
            (JOURNEY_SVG_WIDTH_VALUE, JOURNEY_MIN_HEIGHT)
        );
    }

    #[test]
    fn svg_max_height_keeps_centered_parent_origin() -> Result<(), String> {
        let svg = rendered_journey_svg()?;
        for (name, expected) in [
            ("x", JOURNEY_CENTERED_OFFSET),
            ("y", JOURNEY_CENTERED_OFFSET),
            ("width", JOURNEY_SVG_WIDTH_VALUE),
            ("height", JOURNEY_HOST_HEIGHT),
        ] {
            let actual = svg_coordinate(&svg, name)?;
            assert_eq!(actual, expected, "SVG {name}={actual}, expected {expected}");
        }
        Ok(())
    }

    fn journey_svg_attributes() -> [(String, String); 3] {
        [
            ("width".to_owned(), JOURNEY_SVG_WIDTH.to_owned()),
            ("height".to_owned(), JOURNEY_SVG_HEIGHT.to_owned()),
            ("viewbox".to_owned(), JOURNEY_VIEW_BOX.to_owned()),
        ]
    }

    fn rendered_journey_svg() -> Result<Element, String> {
        let html = format!(
            "<style>html,body{{margin:0}}</style><main style=\"width:{JOURNEY_CAPTURE_WIDTH}px;height:{JOURNEY_CAPTURE_HEIGHT}px;display:flex;align-items:center;justify-content:center\"><div style=\"width:{JOURNEY_HOST_WIDTH}px;height:{JOURNEY_HOST_HEIGHT}px;display:flex;align-items:center;justify-content:center\"><svg width=\"{JOURNEY_SVG_WIDTH}\" height=\"{JOURNEY_SVG_HEIGHT}\" viewBox=\"{JOURNEY_VIEW_BOX}\" style=\"max-width:{JOURNEY_SVG_WIDTH}px;max-height:100%\"></svg></div></main>"
        );
        let document = HtmlDocument::parse(&html);
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let viewport = HtmlBrowserViewport::new(JOURNEY_CAPTURE_WIDTH, JOURNEY_CAPTURE_HEIGHT, 1.0)
            .map_err(|error| error.to_string())?;
        let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;
        let root = Element::parse(layout.svg.as_bytes()).map_err(|error| error.to_string())?;
        find_svg(&root)
            .cloned()
            .ok_or_else(|| "missing journey SVG".to_owned())
    }
}
