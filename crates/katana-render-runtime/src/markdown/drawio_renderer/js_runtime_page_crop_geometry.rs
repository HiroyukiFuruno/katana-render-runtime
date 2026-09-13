use super::super::DrawioJsRuntimeOps;
use super::super::page_crop_test_support::temp_runtime_path;
use crate::markdown::color_preset::DiagramColorPreset;

const SKETCH_BOARD_WIDTH: i32 = 540;
const SKETCH_BOARD_HEIGHT: i32 = 440;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SvgBox {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

impl SvgBox {
    pub(super) fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x: f64::from(x),
            y: f64::from(y),
            width: f64::from(width),
            height: f64::from(height),
        }
    }

    pub(super) fn right(self) -> f64 {
        self.x + self.width
    }

    pub(super) fn bottom(self) -> f64 {
        self.y + self.height
    }

    pub(super) fn contains(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }
}

pub(super) fn svg_canvas_box(svg: &str) -> Option<SvgBox> {
    let root = svg.split_once("<svg")?.1.split_once('>')?.0;
    let values = svg_attribute(root, "viewBox")
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<Vec<f64>, _>>()
        .ok()?;
    let [x, y, width, height] = values.as_slice() else {
        return None;
    };
    Some(SvgBox {
        x: *x,
        y: *y,
        width: *width,
        height: *height,
    })
}

pub(super) fn svg_canvas_contains_cells<const N: usize>(svg: &str, cell_ids: [&str; N]) -> bool {
    let Some(canvas) = svg_canvas_box(svg) else {
        return false;
    };
    cell_ids
        .into_iter()
        .all(|cell_id| svg_cell_rect_box(svg, cell_id).is_some_and(|box_| canvas.contains(box_)))
}

pub(super) fn svg_cell_rect_box(svg: &str, cell_id: &str) -> Option<SvgBox> {
    let cell_marker = format!(r#"<g data-cell-id="{cell_id}">"#);
    let group = svg.split_once(&cell_marker)?.1.split_once("</g>")?.0;
    let rect = group.split_once("<rect")?.1.split_once('>')?.0;
    let (translate_x, translate_y) = svg_group_translate(group);
    Some(SvgBox {
        x: svg_attribute(rect, "x").parse::<f64>().ok()? + translate_x,
        y: svg_attribute(rect, "y").parse::<f64>().ok()? + translate_y,
        width: svg_attribute(rect, "width").parse().ok()?,
        height: svg_attribute(rect, "height").parse().ok()?,
    })
}

pub(super) fn svg_cell_stroked_path_box(svg: &str, cell_id: &str) -> Option<SvgBox> {
    let cell_marker = format!(r#"<g data-cell-id="{cell_id}">"#);
    let group = svg.split_once(&cell_marker)?.1.split_once("</g>")?.0;
    let path = group
        .split("<path")
        .skip(1)
        .filter_map(|path| path.split_once('>').map(|(path, _)| path))
        .find(|path| !["", "none"].contains(&svg_attribute(path, "stroke")))?;
    let values = svg_attribute(path, "d")
        .split_whitespace()
        .collect::<Vec<_>>();
    let ["M", x, y, "H", right, "V", bottom, ..] = values.as_slice() else {
        return None;
    };
    let (x, y, right, bottom) = (
        x.parse::<f64>().ok()?,
        y.parse::<f64>().ok()?,
        right.parse::<f64>().ok()?,
        bottom.parse::<f64>().ok()?,
    );
    Some(SvgBox {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

pub(super) fn svg_group_translate(group: &str) -> (f64, f64) {
    let transform = svg_attribute(group, "transform");
    let Some(value) = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
    else {
        return (0.0, 0.0);
    };
    let values = value
        .split([',', ' '])
        .filter(|value| !value.is_empty())
        .map(str::parse)
        .collect::<Result<Vec<f64>, _>>();
    let Some((x, y)) = values
        .ok()
        .and_then(|values| values.first().zip(values.get(1)).map(|(x, y)| (*x, *y)))
    else {
        return (0.0, 0.0);
    };
    (x, y)
}

pub(super) fn svg_attribute<'a>(element: &'a str, name: &str) -> &'a str {
    let Some((_, value)) = element.split_once(&format!(r#"{name}=""#)) else {
        return "";
    };
    let Some((value, _)) = value.split_once('"') else {
        return "";
    };
    value
}

pub(super) fn sketch_stack_layout_crop_matches(
    svg: &str,
    width: i32,
    height: i32,
    origin: i32,
) -> bool {
    let Some(canvas) = svg_canvas_box(svg) else {
        return false;
    };
    let Some(board) = svg_cell_rect_box(svg, "board") else {
        return false;
    };
    canvas.width == f64::from(width)
        && canvas.height == f64::from(height)
        && board == SvgBox::new(1, 1, SKETCH_BOARD_WIDTH, SKETCH_BOARD_HEIGHT)
        && canvas.contains(board)
        && svg_canvas_contains_cells(svg, ["column-a", "column-b", "column-c"])
        && canvas.x <= f64::from(origin)
        && canvas.y <= f64::from(origin)
}

pub(super) fn normal_crop_is_smaller_than_special(
    svg: &str,
    special_canvas: Option<SvgBox>,
) -> bool {
    let Some(canvas) = svg_canvas_box(svg) else {
        return false;
    };
    let Some(special_canvas) = special_canvas else {
        return false;
    };
    svg_canvas_contains_cells(svg, ["board", "column-a", "column-b", "column-c"])
        && canvas.width < special_canvas.width
        && canvas.height <= special_canvas.height
}

pub(super) fn render_sketch_bundle_with_normal_crop_control(
    source: &str,
    bundle: &str,
    control_source: &str,
    control_bundle: &str,
    prefix: &str,
) -> (Result<String, String>, Result<String, String>) {
    let path = temp_runtime_path(&format!("{prefix}-rendered"));
    let control_path = temp_runtime_path(&format!("{prefix}-normal-control"));
    assert!(std::fs::write(&path, bundle).is_ok());
    assert!(std::fs::write(&control_path, control_bundle).is_ok());
    let preset = DiagramColorPreset::dark();
    let rendered = DrawioJsRuntimeOps::render(source, &path, preset);
    let control = DrawioJsRuntimeOps::render(control_source, &control_path, preset);
    (rendered, control)
}
