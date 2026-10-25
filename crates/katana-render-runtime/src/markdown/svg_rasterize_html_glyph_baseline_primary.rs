use super::{Policy, metrics_shift, uniform_policy};
use crate::markdown::svg_rasterize::{
    html_glyph::HtmlGlyphRasterizer, text_shaping::loaded_html_font,
};
use resvg::usvg;

pub(in super::super) fn resolved_policy(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
) -> Result<Option<Policy>, String> {
    let Some(mut policy) = uniform_policy(text) else {
        return Ok(None);
    };
    let mut spans = text.chunks().iter().flat_map(|chunk| chunk.spans());
    let Some(first) = spans.next() else {
        return Ok(Some(policy));
    };
    if spans.any(|span| span.font() != first.font()) {
        return Ok(Some(policy));
    }
    let Some(id) = loaded_html_font(first.font(), rasterizer.database) else {
        return Ok(Some(policy));
    };
    let metadata = rasterizer.face_metadata(id)?;
    if !metadata.unsupported {
        /* WHY: usvgのspan基準線は主font由来であり、fallback runごとには変えない。 */
        policy.primary_shift = Some(metrics_shift(rasterizer.database, id, &metadata, policy));
    }
    Ok(Some(policy))
}
