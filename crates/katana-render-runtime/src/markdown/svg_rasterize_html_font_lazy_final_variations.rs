use resvg::usvg::{
    FontOpticalSizing, FontVariation,
    fontdb::{Database, ID},
};
use skrifa::{FontRef, MetadataProvider, Tag};

#[path = "svg_rasterize_html_font_lazy_final_variation_design.rs"]
mod design;
pub(super) use design::applied_design_coordinates;

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
    let variations = effective_variations(fontdb, span, glyph)?;
    let applied = applied_variations(fontdb, face_id, &variations)?;
    let design = applied_design_coordinates(fontdb, face_id, &applied)?;
    let normalized = normalized_coords(fontdb, face_id, &design)?;
    Ok((applied, design, normalized))
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

fn effective_variations(
    fontdb: &Database,
    span: &resvg::usvg::layout::Span,
    glyph: &resvg::usvg::layout::PositionedGlyph,
) -> Result<Vec<FontVariation>, String> {
    let mut variations = span.variations.clone();
    if span.font_optical_sizing == FontOpticalSizing::Auto
        && !variations.iter().any(|variation| variation.tag == *b"opsz")
    {
        let has_opsz = fontdb
            .with_face_data(glyph.font, |data, index| {
                let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
                Ok::<_, String>(
                    font.axes()
                        .get_by_tag(Tag::from_be_bytes(*b"opsz"))
                        .is_some(),
                )
            })
            .ok_or_else(|| format!("font face data unavailable for {:#?}", glyph.font))??;
        variations = add_auto_opsz(
            variations,
            span.font_optical_sizing,
            glyph.font_size(),
            has_opsz,
        );
    }
    Ok(variations)
}

pub(super) fn add_auto_opsz(
    mut variations: Vec<FontVariation>,
    optical_sizing: FontOpticalSizing,
    font_size: f32,
    has_opsz_axis: bool,
) -> Vec<FontVariation> {
    if optical_sizing == FontOpticalSizing::Auto
        && has_opsz_axis
        && !variations.iter().any(|variation| variation.tag == *b"opsz")
    {
        variations.push(FontVariation::new(*b"opsz", font_size));
    }
    variations
}

pub(super) fn normalized_coords(
    fontdb: &Database,
    face_id: ID,
    variations: &[FontVariation],
) -> Result<Vec<f32>, String> {
    fontdb
        .with_face_data(face_id, |data, index| {
            let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
            validate_variations(face_id, variations)?;
            let location = font.axes().location(
                variations
                    .iter()
                    .map(|variation| (Tag::from_be_bytes(variation.tag), variation.value)),
            );
            let coords = location
                .coords()
                .iter()
                .map(|coord| coord.to_f32())
                .collect::<Vec<_>>();
            coords
                .iter()
                .all(|coord| coord.is_finite())
                .then_some(coords)
                .ok_or_else(|| format!("non-finite normalized coordinates for face {face_id:?}"))
        })
        .ok_or_else(|| format!("font face data unavailable for {face_id:?}"))?
}

fn validate_variations(face_id: ID, variations: &[FontVariation]) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for variation in variations {
        if !variation.value.is_finite() {
            return Err(format!("non-finite variation value for face {face_id:?}"));
        }
        let tag = Tag::from_be_bytes(variation.tag);
        if !seen.insert(tag.to_be_bytes()) {
            return Err(format!(
                "duplicate variation axis {tag:?} for face {face_id:?}"
            ));
        }
    }
    Ok(())
}

fn applied_variations(
    fontdb: &Database,
    face_id: ID,
    variations: &[FontVariation],
) -> Result<Vec<FontVariation>, String> {
    fontdb
        .with_face_data(face_id, |data, index| {
            let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
            let mut seen = std::collections::BTreeSet::new();
            let mut applied = Vec::new();
            for variation in variations {
                let tag = Tag::from_be_bytes(variation.tag);
                if font.axes().get_by_tag(tag).is_some() {
                    if !seen.insert(variation.tag) {
                        return Err(format!(
                            "duplicate variation axis {tag:?} for face {face_id:?}"
                        ));
                    }
                    applied.push(*variation);
                }
            }
            Ok(applied)
        })
        .ok_or_else(|| format!("font face data unavailable for {face_id:?}"))?
}
