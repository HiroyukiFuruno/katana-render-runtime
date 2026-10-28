use resvg::usvg::FontVariation;
use skia_safe::{
    FontArguments, Typeface,
    font_arguments::{VariationPosition, variation_position::Coordinate},
};

pub(super) fn instantiate(
    base: &Typeface,
    design: &[FontVariation],
) -> Result<(Typeface, Vec<FontVariation>), String> {
    let coordinates = design
        .iter()
        .map(|axis| Coordinate {
            axis: u32::from_be_bytes(axis.tag).into(),
            value: axis.value,
        })
        .collect::<Vec<_>>();
    let mut arguments = FontArguments::new();
    arguments.set_collection_index(physical_collection_index(base)?);
    let arguments = arguments.set_variation_design_position(VariationPosition {
        coordinates: &coordinates,
    });
    let typeface = base
        .clone_with_arguments(&arguments)
        .ok_or("Skia variable typeface instantiation failed")?;
    let applied = applied_coordinates(&typeface)?;
    Ok((typeface, applied))
}

fn physical_collection_index(base: &Typeface) -> Result<usize, String> {
    let (_, index) = base
        .to_existing_font_data()
        .ok_or("Skia variable base physical font data unavailable")?;
    /* WHY: Skia setter内部のsigned index変換がpanicしない範囲を先に確認する。 */
    let index = i32::try_from(index).map_err(|_| "Skia collection index exceeds signed range")?;
    usize::try_from(index).map_err(|_| "Skia collection index exceeds host range".to_owned())
}

fn applied_coordinates(typeface: &Typeface) -> Result<Vec<FontVariation>, String> {
    Ok(typeface
        .variation_design_position()
        .ok_or("Skia variable typeface coordinates unavailable")?
        .into_iter()
        .map(|axis| FontVariation::new((*axis.axis).to_be_bytes(), axis.value))
        .collect())
}
