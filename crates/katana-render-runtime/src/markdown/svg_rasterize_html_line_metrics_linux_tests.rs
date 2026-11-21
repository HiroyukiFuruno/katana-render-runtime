use super::FaceMetricScope;
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use resvg::usvg::fontdb::Database;

const FONT_SIZE: f32 = 16.0;

#[test]
fn linux_invalid_face_metric_is_recorded_without_legacy_fallback() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let id = database.faces().next().ok_or("fixture face missing")?.id;
    let detail = FaceMetricScope::new(true);

    assert_eq!(
        super::resolve_face_line_metrics(Err(()), &database, id, FONT_SIZE, Some(&detail)),
        None
    );
    Ok(())
}
