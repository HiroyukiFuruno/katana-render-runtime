use super::{Policy, metrics_shift, middle, policy::uniform_policy_and_primary_span};
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
    let Some((mut policy, primary_span)) =
        uniform_policy_and_primary_span(text, nested_text_context, additional_text_context)
    else {
        return Ok(None);
    };
    if let Some(middle_policy) = middle::MiddlePolicy::from_policy(policy) {
        return resolve_middle_policy(rasterizer, text, primary_span, policy, middle_policy);
    }
    if policy.kind == super::PolicyKind::Hanging {
        return resolve_hanging_policy(rasterizer, text, primary_span, policy);
    }
    let Some((id, metadata, _)) = primary_face_metadata(rasterizer, text, primary_span)? else {
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
    primary_span: &usvg::TextSpan,
    mut policy: Policy,
) -> Result<Option<Policy>, String> {
    let Some((id, metadata, _)) = primary_face_metadata(rasterizer, text, primary_span)? else {
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
    primary_span: &usvg::TextSpan,
    mut policy: Policy,
    middle_policy: middle::MiddlePolicy,
) -> Result<Option<Policy>, String> {
    let Some((id, metadata, variable_face)) =
        primary_face_metadata(rasterizer, text, primary_span)?
    else {
        return Ok(None);
    };
    if metadata.unsupported {
        return Ok(None);
    }
    let Some(shift) = middle::shift(
        rasterizer,
        id,
        &metadata,
        middle_policy,
        variable_face.as_ref(),
    )?
    else {
        return Ok(None);
    };
    policy.primary_shift = Some(shift);
    Ok(Some(policy))
}

type PrimaryFace = (
    usvg::fontdb::ID,
    super::FaceMetadata,
    Option<super::CachedFace>,
);

fn primary_face_metadata(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
    primary_span: &usvg::TextSpan,
) -> Result<Option<PrimaryFace>, String> {
    let font = primary_span.font();
    let mut other_spans = text.chunks().iter().flat_map(|chunk| chunk.spans()).skip(1);
    if other_spans.any(|span| span.font() != font) {
        return Ok(None);
    }
    let Some(id) = loaded_html_font(font, rasterizer.database) else {
        return Ok(None);
    };
    let metadata = rasterizer.face_metadata(id)?;
    if metadata.unsupported
        && let Some(face) = rasterizer.primary_variable_face(text, id)?
    {
        return Ok(Some((id, face.metadata(), Some(face))));
    }
    Ok(Some((id, metadata, None)))
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_baseline_primary_tests.rs"]
mod tests;
