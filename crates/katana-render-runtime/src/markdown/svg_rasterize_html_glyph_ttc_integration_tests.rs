use super::{CachedFace, HtmlGlyphRasterizer};
use resvg::usvg::fontdb::{Database, ID, Weight};

const FAMILY: &str = "Helvetica";
const SAMPLE: &str = "AV";

#[test]
fn valid_helvetica_ttc_faces_repack_and_preserve_metrics() -> Result<(), String> {
    let database = super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    let mut repacked = false;
    for weight in [Weight::NORMAL, Weight::BOLD] {
        repacked |= verify_helvetica_face(&database, &rasterizer, weight)?;
    }
    if !repacked {
        return Err("Helvetica TTC index-one regression face was not discovered".to_string());
    }
    Ok(())
}

fn verify_helvetica_face(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    weight: Weight,
) -> Result<bool, String> {
    let id = find_family_face(database, weight)?;
    let Some((_, index)) = rasterizer.face_file(id)? else {
        return Err("Helvetica face is not file-backed".to_string());
    };
    let face = required_face(rasterizer, id, weight)?;
    if index == 0 {
        return Ok(false);
    }
    let typeface = face.typeface.as_skia();
    let Some((_, actual_index)) = typeface.to_existing_font_data() else {
        return Err("repacked Helvetica data is unavailable".to_string());
    };
    if actual_index != 0 {
        return Err("repacked Helvetica face did not become SFNT index zero".to_string());
    }
    let upem = rasterizer.face_metadata(id)?.upem;
    if typeface.units_per_em().map(|value| value as f32) != Some(upem)
        || typeface.count_glyphs() == 0
    {
        return Err("repacked Helvetica metrics changed".to_string());
    }
    Ok(true)
}

fn required_face(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    weight: Weight,
) -> Result<CachedFace, String> {
    rasterizer
        .ensure_face(id)?
        .ok_or_else(|| format!("Helvetica weight {} could not be rasterized", weight.0))
}

fn find_family_face(database: &Database, weight: Weight) -> Result<ID, String> {
    database
        .faces()
        .find(|face| face.weight == weight && face.families.iter().any(|(name, _)| name == FAMILY))
        .map(|face| face.id)
        .ok_or_else(|| format!("{FAMILY} weight {} missing", weight.0))
}
