use resvg::usvg::{
    FontVariation,
    fontdb::{Database, ID},
};
use skrifa::{FontRef, MetadataProvider, Tag};

pub(in crate::markdown::svg_rasterize) fn applied_design_coordinates(
    fontdb: &Database,
    face_id: ID,
    variations: &[FontVariation],
) -> Result<Vec<FontVariation>, String> {
    fontdb
        .with_face_data(face_id, |data, index| {
            let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
            super::validate_variations(face_id, variations)?;
            collect_coordinates(&font, face_id, variations)
        })
        .ok_or_else(|| format!("font face data unavailable for {face_id:?}"))?
}

fn collect_coordinates(
    font: &FontRef<'_>,
    face_id: ID,
    variations: &[FontVariation],
) -> Result<Vec<FontVariation>, String> {
    let axes = font.axes();
    if axes.is_empty() {
        return Ok(Vec::new());
    }
    let filtered = axes.filter(
        variations
            .iter()
            .map(|variation| (Tag::from_be_bytes(variation.tag), variation.value)),
    );
    let filtered = filtered.collect::<Vec<_>>();
    reject_unknown_axes(&axes, face_id, variations)?;
    Ok(axes
        .iter()
        .map(|axis| coordinate_for(&axis, &filtered))
        .collect())
}

fn reject_unknown_axes(
    axes: &skrifa::AxisCollection<'_>,
    face_id: ID,
    variations: &[FontVariation],
) -> Result<(), String> {
    let known = axes
        .iter()
        .map(|axis| axis.tag())
        .collect::<std::collections::BTreeSet<_>>();
    if variations
        .iter()
        .any(|variation| !known.contains(&Tag::from_be_bytes(variation.tag)))
    {
        return Err(format!("unknown variation axis for face {face_id:?}"));
    }
    Ok(())
}

fn coordinate_for(
    axis: &skrifa::Axis,
    filtered: &[skrifa::setting::VariationSetting],
) -> FontVariation {
    let value = filtered
        .iter()
        .find(|setting| setting.selector == axis.tag())
        .map(|setting| setting.value)
        .unwrap_or_else(|| axis.default_value());
    FontVariation::new(axis.tag().to_be_bytes(), value)
}
