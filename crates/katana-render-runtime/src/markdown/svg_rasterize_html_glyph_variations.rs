use resvg::usvg::{
    FontOpticalSizing, FontVariation,
    fontdb::{Database, ID},
};
use skrifa::{FontRef, MetadataProvider, Tag};

#[path = "svg_rasterize_html_font_lazy_final_variation_design.rs"]
mod design;
pub(super) use design::applied_design_coordinates;

#[cfg(test)]
pub(super) type ResolvedLocation = (Vec<FontVariation>, Vec<FontVariation>, Vec<f32>);
pub(super) type ResolvedDesign = (Vec<FontVariation>, Vec<FontVariation>);

#[cfg(test)]
pub(super) fn resolve(
    fontdb: &Database,
    face_id: ID,
    requested: &[FontVariation],
    optical_sizing: FontOpticalSizing,
    font_size: f32,
) -> Result<ResolvedLocation, String> {
    let (effective, design) =
        resolve_design(fontdb, face_id, requested, optical_sizing, font_size)?;
    let normalized = normalized_coords(fontdb, face_id, &design)?;
    Ok((effective, design, normalized))
}

pub(super) fn resolve_design(
    fontdb: &Database,
    face_id: ID,
    requested: &[FontVariation],
    optical_sizing: FontOpticalSizing,
    font_size: f32,
) -> Result<ResolvedDesign, String> {
    validate_variations(face_id, requested)?;
    if !font_size.is_finite() || font_size <= 0.0 {
        return Err(format!("invalid variation font size for face {face_id:?}"));
    }
    let has_opsz = fontdb
        .with_face_data(face_id, |data, index| {
            let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
            Ok::<_, String>(
                font.axes()
                    .get_by_tag(Tag::from_be_bytes(*b"opsz"))
                    .is_some(),
            )
        })
        .ok_or_else(|| format!("font face data unavailable for {face_id:?}"))??;
    let variations = add_auto_opsz(requested.to_vec(), optical_sizing, font_size, has_opsz);
    let effective = applied_variations(fontdb, face_id, &variations)?;
    let design = applied_design_coordinates(fontdb, face_id, &effective)?;
    Ok((effective, design))
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

#[cfg(test)]
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
            Ok(location
                .coords()
                .iter()
                .map(|coord| coord.to_f32())
                .collect())
        })
        .ok_or_else(|| format!("font face data unavailable for {face_id:?}"))?
}

pub(super) fn validate_variations(face_id: ID, variations: &[FontVariation]) -> Result<(), String> {
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
            Ok(variations
                .iter()
                .filter(|variation| {
                    font.axes()
                        .get_by_tag(Tag::from_be_bytes(variation.tag))
                        .is_some()
                })
                .copied()
                .collect())
        })
        .ok_or_else(|| format!("font face data unavailable for {face_id:?}"))?
}

#[cfg(test)]
mod tests {
    use super::{applied_variations, normalized_coords};
    use crate::markdown::svg_rasterize::font_file_fixture::{FONT_BYTES, create};

    #[test]
    fn variation_queries_distinguish_damaged_and_missing_registered_font()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let fixture = create(directory.path().join("variation-input.ttf"), FONT_BYTES)?;
        let applied = || applied_variations(&fixture.database, fixture.face_id, &[]);
        let normalized = || normalized_coords(&fixture.database, fixture.face_id, &[]);
        assert!(applied()?.is_empty());
        assert!(normalized()?.is_empty());

        let truncated = &FONT_BYTES[..std::mem::size_of::<u32>()];
        std::fs::write(&fixture.path, truncated)?;
        let parse_error = skrifa::FontRef::from_index(truncated, 0)
            .err()
            .ok_or("truncated fixture must fail font parsing")?;
        assert_eq!(applied().err(), Some(parse_error.to_string()));
        assert_eq!(normalized().err(), Some(parse_error.to_string()));

        std::fs::remove_file(&fixture.path)?;
        let missing = format!("font face data unavailable for {:?}", fixture.face_id);
        assert_eq!(applied().err().as_deref(), Some(missing.as_str()));
        assert_eq!(normalized().err().as_deref(), Some(missing.as_str()));
        Ok(())
    }
}
