use super::{
    CombinedMeasurement, FAMILY, FONT_BYTES, FontShape, SAMPLE_LETTER_SPACING, combined_measurement,
};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

pub(super) fn verify_case(
    database: &Arc<Database>,
    text: &str,
    size: f32,
    weight: u16,
    features: Option<&str>,
) -> Result<(), String> {
    let shape = FontShape::new(size, weight, false);
    let actual = combined_measurement(
        database,
        FAMILY,
        text,
        shape,
        SAMPLE_LETTER_SPACING,
        features,
    )
    .ok_or("combined measurement failed")?;
    let base = super::super::super::matching_font_face(database, FAMILY, weight, false)
        .ok_or("bundled face missing")?;
    let width =
        super::super::super::shaped_runs_advance(database, base, FAMILY, text, shape, features)
            .ok_or("existing width path failed")?;
    assert_combined_width(&actual, width, text);
    let oracle = rustybuzz::ttf_parser::Face::parse(FONT_BYTES, 0)
        .map_err(|error| format!("font oracle: {error:?}"))?;
    let scale = shape.size / f32::from(oracle.units_per_em());
    assert_oracle_metrics(&actual, base, &oracle, scale);
    Ok(())
}

fn assert_combined_width(actual: &CombinedMeasurement, width: f32, text: &str) {
    assert_eq!(
        actual.advance.to_bits(),
        (width + text.chars().count().saturating_sub(1) as f32 * SAMPLE_LETTER_SPACING).to_bits()
    );
}

fn assert_oracle_metrics(
    actual: &CombinedMeasurement,
    base: ID,
    oracle: &rustybuzz::ttf_parser::Face<'_>,
    scale: f32,
) {
    assert_eq!(actual.run_cells.len(), 1);
    let cell = &actual.run_cells[0];
    assert_eq!(cell.face_id, base);
    assert!((cell.above_baseline - f32::from(oracle.ascender()) * scale).abs() < 0.0001);
    assert!((cell.below_baseline + f32::from(oracle.descender()) * scale).abs() < 0.0001);
    assert!((cell.line_gap - f32::from(oracle.line_gap()) * scale).abs() < 0.0001);
}
