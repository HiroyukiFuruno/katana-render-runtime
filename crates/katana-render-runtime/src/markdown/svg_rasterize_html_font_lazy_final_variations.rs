use crate::markdown::svg_rasterize::html_glyph_variations;
#[cfg(test)]
pub(super) use html_glyph_variations::{
    add_auto_opsz, applied_design_coordinates, normalized_coords,
};
use resvg::usvg::{
    FontVariation,
    fontdb::{Database, ID},
};

type FaceVariationObservation = (Vec<FontVariation>, Vec<FontVariation>, Vec<f32>);

pub(super) fn face_observation(
    fontdb: &Database,
    span: &resvg::usvg::layout::Span,
    face_id: ID,
    glyph: &resvg::usvg::layout::PositionedGlyph,
) -> Result<FaceVariationObservation, String> {
    let font_sizes = span
        .positioned_glyphs
        .iter()
        .filter(|candidate| candidate.font == face_id)
        .map(|candidate| candidate.font_size())
        .collect::<Vec<_>>();
    validate_observation_inputs(&span.variations, &font_sizes)
        .map_err(|error| format!("{error} for face {face_id:?}"))?;
    let location = html_glyph_variations::resolve(
        fontdb,
        face_id,
        &span.variations,
        span.font_optical_sizing,
        glyph.font_size(),
    )?;
    Ok(location)
}

pub(super) fn validate_observation_inputs(
    variations: &[FontVariation],
    font_sizes: &[f32],
) -> Result<(), &'static str> {
    if variations
        .iter()
        .any(|variation| !variation.value.is_finite())
    {
        return Err("non-finite variation value");
    }
    let Some(first) = font_sizes.first() else {
        return Err("missing glyph font size");
    };
    if !first.is_finite() || font_sizes.iter().any(|size| !size.is_finite()) {
        return Err("non-finite font size");
    }
    if font_sizes
        .iter()
        .any(|size| size.to_bits() != first.to_bits())
    {
        return Err("inconsistent font size");
    }
    Ok(())
}
