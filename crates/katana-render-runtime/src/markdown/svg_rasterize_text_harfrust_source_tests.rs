use super::{FontShape, shape_run_character_metrics, shape_text_with_face};
use resvg::usvg::fontdb::{Database, Source};
use std::sync::Arc;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const TEST_FONT_SIZE: f32 = 16.0;
const TEST_FONT_WEIGHT: u16 = 400;

fn verify_source(source: Source) -> Result<(), String> {
    let mut parsed = Database::new();
    parsed.load_font_data(FONT_BYTES.to_vec());
    let mut info = parsed.faces().next().ok_or("font face missing")?.clone();
    info.source = source;
    let mut database = Database::new();
    database.push_face_info(info);
    let face = database.faces().next().ok_or("font face missing")?.id;
    let font = FontShape::new(TEST_FONT_SIZE, TEST_FONT_WEIGHT, false);
    let metrics = shape_run_character_metrics(&database, face, "AV", font, None)
        .ok_or("character shaping failed")?;
    let width = shape_text_with_face(&database, face, "AV", font, None)
        .ok_or("whole run shaping failed")?;
    assert_eq!(metrics.len(), 2);
    assert!((metrics.iter().map(|value| value.0).sum::<f32>() - width).abs() < 0.001);
    assert!(width > 0.0);
    assert!(shape_run_character_metrics(&database, face, "אב", font, None).is_none());
    Ok(())
}

#[test]
fn owned_font_character_advances_equal_the_shaped_run() -> Result<(), String> {
    verify_source(Source::Binary(Arc::new(FONT_BYTES.to_vec())))
}

#[test]
fn shared_font_character_advances_equal_the_shaped_run() -> Result<(), String> {
    verify_source(Source::SharedFile(
        "shared-regression.ttf".into(),
        Arc::new(FONT_BYTES.to_vec()),
    ))
}

#[test]
fn file_font_character_advances_equal_the_shaped_run() -> Result<(), String> {
    verify_source(Source::File(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf"),
    ))
}

#[test]
fn direct_font_character_metrics_preserve_latin_advances() -> Result<(), String> {
    let font = FontShape::new(TEST_FONT_SIZE, TEST_FONT_WEIGHT, false);
    let metrics = super::shape_face_character_metrics(FONT_BYTES, 0, "AV", font, None)
        .ok_or("Latin metrics must shape")?;
    assert_eq!(metrics.len(), 2);
    assert!(metrics.iter().all(|metric| metric.0 > 0.0));
    assert!(super::shape_face_character_metrics(b"invalid font", 0, "AV", font, None).is_none());
    Ok(())
}

#[test]
fn glyph_accumulator_preserves_utf8_clusters_and_first_offset() {
    let calls = std::cell::Cell::new(0);
    let metrics = super::collect_glyph_character_metrics("éx", 0.5, || {
        calls.set(calls.get() + 1);
        [(0, 4, 6), (0, 8, 99), (2, 10, -4)].into_iter()
    });
    assert_eq!(calls.get(), 1);
    assert_eq!(metrics, Some(vec![(6.0, 3.0), (5.0, -2.0)]));
}

#[test]
fn glyph_accumulator_rejects_interior_and_unfilled_clusters() {
    assert!(
        super::collect_glyph_character_metrics("éx", 1.0, || {
            [(1, 4, 0), (2, 8, 0)].into_iter()
        })
        .is_none()
    );
    assert!(
        super::collect_glyph_character_metrics("éx", 1.0, || { [(0, 4, 0)].into_iter() }).is_none()
    );
}

#[test]
fn glyph_accumulator_preserves_sequential_float_addition() {
    let metrics = super::collect_glyph_character_metrics("a", 1.0, || {
        [(0, 16_777_216, 2), (0, 1, 5), (0, -16_777_216, 8)].into_iter()
    });
    assert_eq!(metrics, Some(vec![(0.0, 2.0)]));
}
