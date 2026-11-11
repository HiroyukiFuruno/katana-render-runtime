use super::super::types::{ElementBox, OverflowClip};

pub(super) fn intersection_metadata(element: &ElementBox) -> String {
    let (positioning, containing_block, viewport_escape) =
        super::positioning::intersection_positioning(element);
    let positioning_origin = element
        .positioning_origin_node_id
        .map_or_else(|| "null".to_string(), |node_id| node_id.to_string());
    let clips = intersection_clips(element);
    let clips_overflow = element.clips_overflow();
    let padding_edge = intersection_clip(&element.padding_edge);
    let fragments = intersection_fragments(element);
    format!(
        "{{\"positioning\":\"{positioning}\",\"containingBlock\":{containing_block},\"positioningOrigin\":{positioning_origin},\"viewportEscape\":{viewport_escape},\"paddingEdge\":{padding_edge},\"clipsOverflow\":{clips_overflow},\"clips\":[{clips}],\"fragments\":[{fragments}]}}"
    )
}

fn intersection_clips(element: &ElementBox) -> String {
    element
        .overflow_clips
        .iter()
        .map(intersection_clip)
        .collect::<Vec<_>>()
        .join(",")
}

fn intersection_clip(clip: &OverflowClip) -> String {
    let (radius_x, radius_y) = clip.maximum_radii();
    let corners = clip
        .transformed_corners
        .iter()
        .map(|(x, y)| format!("[{x},{y}]"))
        .collect::<Vec<_>>()
        .join(",");
    let radii = clip
        .corner_radii
        .iter()
        .map(|(radius_x, radius_y)| format!("[{radius_x},{radius_y}]"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"owner\":{},\"activeAxes\":[{},{}],\"x\":{},\"y\":{},\"width\":{},\"height\":{},\"corners\":[{corners}],\"radiusX\":{},\"radiusY\":{},\"radii\":[{radii}]}}",
        clip.owner_node_id,
        clip.active_axes.0,
        clip.active_axes.1,
        clip.x,
        clip.y,
        clip.width,
        clip.height,
        radius_x,
        radius_y,
        radii = radii
    )
}

fn intersection_fragments(element: &ElementBox) -> String {
    element
        .inline_fragments
        .iter()
        .map(|fragment| {
            let (x, y, width, height) = fragment.transformed_axis_aligned();
            format!("{{\"x\":{x},\"y\":{y},\"width\":{width},\"height\":{height}}}")
        })
        .collect::<Vec<_>>()
        .join(",")
}
