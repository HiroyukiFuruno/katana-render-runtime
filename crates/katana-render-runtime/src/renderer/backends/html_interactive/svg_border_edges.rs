use super::super::super::style::CssStyle;

const BORDER_EDGE_COUNT: usize = 4;

pub(super) struct BorderEdge<'a> {
    pub(super) start: (f32, f32),
    pub(super) end: (f32, f32),
    pub(super) color: Option<&'a str>,
    pub(super) width: f32,
    pub(super) dasharray: Option<String>,
    pub(super) dotted: bool,
}

pub(super) fn border_edges<'a>(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &'a CssStyle,
) -> [BorderEdge<'a>; BORDER_EDGE_COUNT] {
    [
        top_edge(x, y, width, style),
        right_edge(x, y, width, height, style),
        bottom_edge(x, y, width, height, style),
        left_edge(x, y, height, style),
    ]
}

fn top_edge<'a>(x: f32, y: f32, width: f32, style: &'a CssStyle) -> BorderEdge<'a> {
    horizontal_edge(
        x,
        y + style.border_top_width() / 2.0,
        width,
        style.border_top_color(),
        style.border_top_width(),
        style.border_top_dasharray(),
        style.border_top_dotted(),
    )
}

fn right_edge<'a>(x: f32, y: f32, width: f32, height: f32, style: &'a CssStyle) -> BorderEdge<'a> {
    vertical_edge(
        x + width - style.border_right_width() / 2.0,
        y,
        height,
        style.border_right_color(),
        style.border_right_width(),
        style.border_right_dasharray(),
        style.border_right_dotted(),
    )
}

fn bottom_edge<'a>(x: f32, y: f32, width: f32, height: f32, style: &'a CssStyle) -> BorderEdge<'a> {
    horizontal_edge(
        x,
        y + height - style.border_bottom_width() / 2.0,
        width,
        style.border_bottom_color(),
        style.border_bottom_width(),
        style.border_bottom_dasharray(),
        style.border_bottom_dotted(),
    )
}

fn left_edge<'a>(x: f32, y: f32, height: f32, style: &'a CssStyle) -> BorderEdge<'a> {
    vertical_edge(
        x + style.border_left_width() / 2.0,
        y,
        height,
        style.border_left_color(),
        style.border_left_width(),
        style.border_left_dasharray(),
        style.border_left_dotted(),
    )
}

fn horizontal_edge<'a>(
    x: f32,
    y: f32,
    length: f32,
    color: Option<&'a str>,
    width: f32,
    dasharray: Option<String>,
    dotted: bool,
) -> BorderEdge<'a> {
    BorderEdge {
        start: (x, y),
        end: (x + length, y),
        color,
        width,
        dasharray,
        dotted,
    }
}

fn vertical_edge<'a>(
    x: f32,
    y: f32,
    length: f32,
    color: Option<&'a str>,
    width: f32,
    dasharray: Option<String>,
    dotted: bool,
) -> BorderEdge<'a> {
    BorderEdge {
        start: (x, y),
        end: (x, y + length),
        color,
        width,
        dasharray,
        dotted,
    }
}
