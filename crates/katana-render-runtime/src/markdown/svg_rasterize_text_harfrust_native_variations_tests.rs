use super::FontShape;
use super::native_variations::{
    shape_run_character_metrics_with_variations_with_trim,
    shape_text_with_face_variations_with_trim,
};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::{
    FontVariation,
    fontdb::{Database, Source},
};
use std::path::PathBuf;

const VARIABLE_FONT: &str =
    "../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf";
const TEXT: &str = "A";
const FONT_SIZE: f32 = 18.0;
const FONT_WEIGHT: u16 = 400;

#[test]
fn native_variations_shape_the_loaded_variable_face_for_width_and_character_metrics()
-> Result<(), String> {
    let (database, face) = variable_face()?;
    let variations = [FontVariation::new(*b"wdth", 75.0)];
    let (width, metrics) = native_variation_measurements(&database, face, &variations)?;
    assert_eq!(metrics.len(), 1);
    assert_eq!(
        metrics.iter().map(|(advance, _)| advance).sum::<f32>(),
        width
    );
    Ok(())
}

#[test]
fn native_variations_reject_duplicate_axes_before_shaping() -> Result<(), String> {
    let (database, face) = variable_face()?;
    let duplicate = [
        FontVariation::new(*b"wdth", 75.0),
        FontVariation::new(*b"wdth", 100.0),
    ];
    let shaped = SvgRasterizeOps::with_html_font_render_scope(|| {
        shape_text_with_face_variations_with_trim(
            &database,
            face,
            TEXT,
            FontShape::new(FONT_SIZE, FONT_WEIGHT, false),
            None,
            &duplicate,
        )
    });
    assert_eq!(shaped, None);
    Ok(())
}

fn native_variation_measurements(
    database: &Database,
    face: resvg::usvg::fontdb::ID,
    variations: &[FontVariation],
) -> Result<(f32, Vec<(f32, f32)>), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let shape = FontShape::new(FONT_SIZE, FONT_WEIGHT, false);
        let width = shape_text_with_face_variations_with_trim(
            database,
            face,
            TEXT,
            shape,
            Some("kern"),
            variations,
        )
        .ok_or("native variation width unavailable")?;
        let metrics = shape_run_character_metrics_with_variations_with_trim(
            database,
            face,
            TEXT,
            shape,
            Some("kern"),
            variations,
        )
        .ok_or("native variation character metrics unavailable")?;
        Ok((width, metrics))
    })
}

fn variable_face() -> Result<(Database, resvg::usvg::fontdb::ID), String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(VARIABLE_FONT);
    let mut database = Database::new();
    let face = *database
        .load_font_source(Source::File(path))
        .first()
        .ok_or("variable fixture face missing")?;
    Ok((database, face))
}
