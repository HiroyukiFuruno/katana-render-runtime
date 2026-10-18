use super::layout::{DOM_ADVANCE_UNITS_PER_PX, node_text, semantic_node, stylesheet_font};
use super::*;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::roxmltree::Node as SvgNode;
use svgtypes::{Transform, ViewBox};
#[path = "js_runtime_tests_block_arrow_geometry.rs"]
mod geometry;
pub(super) use geometry::{svg_number_or, transform_bounds};

pub(super) fn assert_block_arrow(root: SvgNode<'_, '_>, svg: &str) -> Result<(), String> {
    let arrow = semantic_node(root, "-blockArrowId6")?;
    if node_text(arrow) != "&nbsp;&nbsp;&nbsp;"
        || svg.matches("&amp;nbsp;").count() != BLOCK_SERIALIZED_NBSP_TOKEN_COUNT
    {
        return Err("Block arrow must retain three serialized NBSP tokens".to_owned());
    }
    if arrow.attribute("data-look") != Some("classic") {
        return Err("Block arrow lost its classic rendering identity".to_owned());
    }
    let polygon = arrow_polygon(arrow)?;
    let bounds = arrow_polygon_bounds(polygon)?;
    assert_down_arrow_shape(bounds)?;
    let width = assert_block_arrow_width(root, bounds)?;
    assert_arrow_center(root, arrow, polygon, bounds, width)
}

fn arrow_polygon<'a, 'input>(arrow: SvgNode<'a, 'input>) -> Result<SvgNode<'a, 'input>, String> {
    let mut polygons = arrow
        .descendants()
        .filter(|item| item.is_element() && item.tag_name().name() == "polygon");
    let polygon = polygons
        .next()
        .ok_or_else(|| "Block arrow polygon is missing".to_owned())?;
    if polygons.next().is_some() {
        return Err("Block arrow has duplicate polygons".to_owned());
    }
    Ok(polygon)
}

fn arrow_polygon_bounds(
    polygon: SvgNode<'_, '_>,
) -> Result<[f64; BLOCK_BOUNDS_COMPONENT_COUNT], String> {
    let points = polygon
        .attribute("points")
        .ok_or_else(|| "Block arrow polygon has no points".to_owned())?;
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut count = 0;
    for pair in points.split_whitespace() {
        let (x, y) = finite_polygon_point(pair)?;
        bounds = [
            bounds[0].min(x),
            bounds[1].min(y),
            bounds[BLOCK_RIGHT_BOUND_INDEX].max(x),
            bounds[BLOCK_BOTTOM_BOUND_INDEX].max(y),
        ];
        count += 1;
    }
    if count < BLOCK_MIN_ARROW_POLYGON_VERTICES {
        return Err("Block arrow polygon needs at least three vertices".to_owned());
    }
    Ok(bounds)
}

fn finite_polygon_point(pair: &str) -> Result<(f64, f64), String> {
    let (x, y) = pair
        .split_once(',')
        .ok_or_else(|| "invalid Block arrow polygon point".to_owned())?;
    let x = x.parse::<f64>().map_err(|error| error.to_string())?;
    let y = y.parse::<f64>().map_err(|error| error.to_string())?;
    if !x.is_finite() || !y.is_finite() {
        return Err("non-finite Block arrow polygon point".to_owned());
    }
    Ok((x, y))
}

fn assert_down_arrow_shape(bounds: [f64; BLOCK_BOUNDS_COMPONENT_COUNT]) -> Result<(), String> {
    if bounds[1] >= bounds[BLOCK_BOTTOM_BOUND_INDEX] || bounds[BLOCK_BOTTOM_BOUND_INDEX] != 0.0 {
        return Err("Block arrow polygon no longer points down from its baseline".to_owned());
    }
    Ok(())
}

fn assert_block_arrow_width(
    root: SvgNode<'_, '_>,
    bounds: [f64; BLOCK_BOUNDS_COMPONENT_COUNT],
) -> Result<f64, String> {
    let expected = block_arrow_dom_width(root)?;
    let actual = bounds[BLOCK_RIGHT_BOUND_INDEX] - bounds[0];
    if actual != expected {
        return Err(format!(
            "Block arrow width {actual} differs from DOM text bounds plus default padding: {expected}"
        ));
    }
    Ok(expected)
}

fn block_arrow_dom_width(root: SvgNode<'_, '_>) -> Result<f64, String> {
    let (family, size) = stylesheet_font(root)?;
    let metrics = SvgRasterizeOps::measure_svg_text_metrics(
        "&nbsp;&nbsp;&nbsp;",
        family,
        size,
        BLOCK_NORMAL_FONT_WEIGHT,
        false,
    )
    .map_err(|error| error.to_string())?;
    /* WHY: 独立raw JSと同じDOM配置幅・行領域・字形のはみ出しへ、既定paddingを加える。 */
    let [left, top, right, bottom] = metrics
        .ink_bounds
        .unwrap_or([0.0; BLOCK_BOUNDS_COMPONENT_COUNT]);
    let advance =
        (f64::from(metrics.advance) * DOM_ADVANCE_UNITS_PER_PX).ceil() / DOM_ADVANCE_UNITS_PER_PX;
    let width = right.max(advance) - left.min(0.0);
    let height = bottom.max(f64::from(metrics.descent).round())
        - top.min(-f64::from(metrics.ascent).round());
    Ok(width + height + f64::from(BLOCK_ARROW_TOTAL_PADDING_PX))
}

fn assert_arrow_center(
    root: SvgNode<'_, '_>,
    arrow: SvgNode<'_, '_>,
    polygon: SvgNode<'_, '_>,
    bounds: [f64; BLOCK_BOUNDS_COMPONENT_COUNT],
    width: f64,
) -> Result<(), String> {
    assert_arrow_centered_in_diagram(root, arrow)?;
    assert_arrow_centered_on_label(polygon, bounds, width)
}

fn assert_arrow_centered_in_diagram(
    root: SvgNode<'_, '_>,
    arrow: SvgNode<'_, '_>,
) -> Result<(), String> {
    let view_box: ViewBox = root
        .attribute("viewBox")
        .ok_or_else(|| "missing SVG viewBox".to_owned())?
        .parse::<ViewBox>()
        .map_err(|error| error.to_string())?;
    let transform = node_transform(arrow, "Block arrow")?;
    if transform.e != view_box.x + view_box.w / 2.0 {
        return Err("Block arrow is not centered on the diagram".to_owned());
    }
    Ok(())
}

fn assert_arrow_centered_on_label(
    polygon: SvgNode<'_, '_>,
    bounds: [f64; BLOCK_BOUNDS_COMPONENT_COUNT],
    width: f64,
) -> Result<(), String> {
    let transform = node_transform(polygon, "Block arrow polygon")?;
    if transform.e != -(bounds[0] + bounds[BLOCK_RIGHT_BOUND_INDEX]) / 2.0
        || bounds[BLOCK_RIGHT_BOUND_INDEX] - bounds[0] != width
    {
        return Err("Block arrow polygon is not centered on its measured label".to_owned());
    }
    Ok(())
}

fn node_transform(node: SvgNode<'_, '_>, name: &str) -> Result<Transform, String> {
    node.attribute("transform")
        .ok_or_else(|| format!("missing {name} transform"))?
        .parse()
        .map_err(|error: svgtypes::Error| error.to_string())
}

pub(super) fn assert_connected_block_edges(root: SvgNode<'_, '_>) -> Result<(), String> {
    let arrow = semantic_node(root, "-blockArrowId6")?;
    if node_text(arrow) != "&nbsp;&nbsp;&nbsp;" {
        return Err("Block NBSP arrow label changed".to_owned());
    }
    for suffix in ["ID-D", "C-D"] {
        let connected = root.descendants().any(|item| {
            item.is_element()
                && item.tag_name().name() == "path"
                && item.attribute("data-edge") == Some("true")
                && item
                    .attribute("data-id")
                    .is_some_and(|id| id.ends_with(suffix))
                && item.attribute("marker-end").is_some()
        });
        if !connected {
            return Err(format!("missing connected Block edge {suffix:?}"));
        }
    }
    Ok(())
}
