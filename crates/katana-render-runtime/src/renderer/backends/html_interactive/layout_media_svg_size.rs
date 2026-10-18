use super::super::constants::{CONTROL_HEIGHT, MIN_LAYOUT_WIDTH};
use super::super::document::attribute;
use super::super::style::CssStyle;
use super::DEFAULT_IMAGE_MAX_HEIGHT;

const VIEW_BOX_VALUE_COUNT: usize = 4;
const VIEW_BOX_WIDTH_INDEX: usize = 2;
const VIEW_BOX_HEIGHT_INDEX: usize = 3;
type ViewBox = (f32, f32, f32, f32);

pub(super) fn intrinsic_svg_width(
    attributes: &[(String, String)],
    available_width: f32,
    style: &CssStyle,
) -> Option<f32> {
    let explicit_width =
        attribute(attributes, "width").and_then(|value| parse_svg_length(value, available_width));
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
    let width = used_svg_width(natural_width, available_width, style);
    /* WHY: SVGの明示height属性は、widthのCSS指定やmax-widthではautoへ変更されない。 */
    let height = style
        .height
        .or_else(|| {
            attribute(attributes, "height")
                .and_then(|value| parse_svg_length(value, available_width))
        })
        .unwrap_or_else(|| auto_svg_height(width, natural_width, view_box))
        .max(CONTROL_HEIGHT);
    (width, height)
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
