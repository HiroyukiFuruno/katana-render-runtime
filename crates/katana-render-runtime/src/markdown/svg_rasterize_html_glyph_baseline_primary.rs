use super::{Policy, metrics_shift, middle, policy::uniform_policy_with_context};
use crate::markdown::svg_rasterize::{
    html_glyph::HtmlGlyphRasterizer, text_shaping::loaded_html_font,
};
use resvg::usvg;
use skrifa::{FontRef, Tag};

const BASE_TABLE_TAG: Tag = Tag::from_be_bytes(*b"BASE");

pub(in super::super) fn resolved_policy(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
) -> Result<Option<Policy>, String> {
    let nested_text_context = rasterizer.has_authored_nested_text_context(text);
    let additional_text_context = rasterizer.has_authored_additional_text_context(text);
    let Some(mut policy) =
        uniform_policy_with_context(text, nested_text_context, additional_text_context)
    else {
        return Ok(None);
    };
    if super::is_middle(policy.kind) {
        return resolve_middle_policy(rasterizer, text, policy);
    }
    if policy.kind == super::PolicyKind::Hanging {
        return resolve_hanging_policy(rasterizer, text, policy);
    }
    let Some((id, metadata)) = primary_face_metadata(rasterizer, text)? else {
        return Ok(Some(policy));
    };
    if !metadata.unsupported {
        /* WHY: usvgのspan基準線は主font由来であり、fallback runごとには変えない。 */
        policy.primary_shift = Some(metrics_shift(rasterizer.database, id, &metadata, policy));
    }
    Ok(Some(policy))
}

fn resolve_hanging_policy(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
    mut policy: Policy,
) -> Result<Option<Policy>, String> {
    let Some((id, metadata)) = primary_face_metadata(rasterizer, text)? else {
        return Ok(None);
    };
    if metadata.unsupported || !has_known_missing_base_table(rasterizer.database, id) {
        return Ok(None);
    }
    /* WHY: usvgのspan baselineは主font由来なので、fallback glyphにも同じCSS補正を使う。 */
    policy.primary_shift = Some(metrics_shift(rasterizer.database, id, &metadata, policy));
    Ok(Some(policy))
}

fn has_known_missing_base_table(database: &usvg::fontdb::Database, id: usvg::fontdb::ID) -> bool {
    database
        .with_face_data(id, |bytes, index| {
            FontRef::from_index(bytes, index)
                .ok()
                .map(|font| font.table_data(BASE_TABLE_TAG).is_none())
        })
        .flatten()
        .unwrap_or(false)
}

fn resolve_middle_policy(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
    mut policy: Policy,
) -> Result<Option<Policy>, String> {
    let Some((id, metadata)) = primary_face_metadata(rasterizer, text)? else {
        return Ok(None);
    };
    if metadata.unsupported {
        return Ok(None);
    }
    let Some(shift) = middle::shift(rasterizer, id, &metadata, policy)? else {
        return Ok(None);
    };
    policy.primary_shift = Some(shift);
    Ok(Some(policy))
}

fn primary_face_metadata(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
) -> Result<Option<(usvg::fontdb::ID, super::FaceMetadata)>, String> {
    let mut spans = text.chunks().iter().flat_map(|chunk| chunk.spans());
    let Some(first) = spans.next() else {
        return Ok(None);
    };
    if spans.any(|span| span.font() != first.font()) {
        return Ok(None);
    }
    let Some(id) = loaded_html_font(first.font(), rasterizer.database) else {
        return Ok(None);
    };
    Ok(Some((id, rasterizer.face_metadata(id)?)))
}
