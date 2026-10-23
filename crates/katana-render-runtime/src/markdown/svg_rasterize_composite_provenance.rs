use super::types::SvgRasterizeError;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct AuthoredClipInfo {
    pub(super) ids: HashSet<String>,
    pub(super) has_object_bounding_box: bool,
}

pub(super) fn authored_clip_ids(svg: &str) -> Result<AuthoredClipInfo, SvgRasterizeError> {
    let document = resvg::usvg::roxmltree::Document::parse(svg)
        .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
    let clips = document
        .descendants()
        .filter(|node| node.has_tag_name("clipPath"))
        .collect::<Vec<_>>();
    Ok(AuthoredClipInfo {
        ids: clips
            .iter()
            .filter_map(|node| node.attribute("id"))
            .map(str::to_owned)
            .collect(),
        has_object_bounding_box: clips
            .iter()
            .any(|node| node.attribute("clipPathUnits") == Some("objectBoundingBox")),
    })
}
