use super::{FontShape, shape_svg_run_metrics_with_face, svg_run_endpoints_with_face};
use resvg::usvg::fontdb::{Database, Source};
use std::sync::Arc;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const RUN_CASES: &[(&str, f32, u16)] = &[("AV", 16.0, 400), ("ffi", 18.25, 700), ("DB", 14.0, 400)];
const SOURCE_KIND_COUNT: usize = 3;

#[test]
fn real_font_sources_preserve_width_bits_and_raw_metric_endpoints() -> Result<(), String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    let oracle = rustybuzz::ttf_parser::Face::parse(FONT_BYTES, 0)
        .map_err(|error| format!("real font oracle failed: {error:?}"))?;
    assert_real_source_metrics(path, &oracle)?;
    Ok(())
}

fn assert_real_source_metrics(
    path: std::path::PathBuf,
    oracle: &rustybuzz::ttf_parser::Face<'_>,
) -> Result<(), String> {
    for source in real_font_sources(path) {
        assert_metrics_for_source(source, oracle)?;
    }
    Ok(())
}

fn real_font_sources(path: std::path::PathBuf) -> [Source; SOURCE_KIND_COUNT] {
    [
        Source::Binary(Arc::new(FONT_BYTES.to_vec())),
        Source::SharedFile(path.clone(), Arc::new(FONT_BYTES.to_vec())),
        Source::File(path),
    ]
}

fn assert_metrics_for_source(
    source: Source,
    oracle: &rustybuzz::ttf_parser::Face<'_>,
) -> Result<(), String> {
    let database = database_with_source(source)?;
    let id = database
        .faces()
        .next()
        .ok_or("installed test face missing")?
        .id;
    for &(text, size, weight) in RUN_CASES {
        assert_one_run_matches_oracle(&database, id, oracle, text, size, weight)?;
    }
    Ok(())
}

fn database_with_source(source: Source) -> Result<Database, String> {
    let mut parsed = Database::new();
    parsed.load_font_data(FONT_BYTES.to_vec());
    let mut face = parsed
        .faces()
        .next()
        .ok_or("real font face missing")?
        .clone();
    face.source = source;
    let mut database = Database::new();
    database.push_face_info(face);
    Ok(database)
}

fn assert_one_run_matches_oracle(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
    oracle: &rustybuzz::ttf_parser::Face<'_>,
    text: &str,
    size: f32,
    weight: u16,
) -> Result<(), String> {
    let font = FontShape::new(size, weight, false);
    let actual = shape_svg_run_metrics_with_face(database, id, text, font, false)
        .ok_or("raw SVG run failed")?
        .0;
    let width = super::super::shape_text_with_face(database, id, text, font, None)
        .ok_or("existing width failed")?;
    assert_eq!(actual.advance.to_bits(), width.to_bits());
    assert_raw_endpoints_match_oracle(actual, oracle, font.size);
    Ok(())
}

fn assert_raw_endpoints_match_oracle(
    actual: super::SvgTextRunMetrics,
    oracle: &rustybuzz::ttf_parser::Face<'_>,
    size: f32,
) {
    let scale = size / f32::from(oracle.units_per_em());
    assert!((actual.ascent - f32::from(oracle.ascender()) * scale).abs() < 0.0001);
    assert!((actual.descent + f32::from(oracle.descender()) * scale).abs() < 0.0001);
}

#[test]
fn subunit_integer_shape_scale_is_rejected_for_a_usable_real_font() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let face = database.faces().next().ok_or("real font face missing")?.id;
    assert!(
        shape_svg_run_metrics_with_face(
            &database,
            face,
            "DB",
            FontShape::new(16.0, 400, false),
            false
        )
        .is_some()
    );
    let tiny = FontShape::new(1.0e-9, 400, false);
    assert!(shape_svg_run_metrics_with_face(&database, face, "DB", tiny, false).is_none());
    assert!(svg_run_endpoints_with_face(&database, face, tiny).is_none());
    Ok(())
}
