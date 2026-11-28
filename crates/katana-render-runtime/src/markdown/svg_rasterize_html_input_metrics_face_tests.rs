use super::input_character_metrics_for_face;
use crate::markdown::svg_rasterize::font_file_fixture::{self, FONT_BYTES};

#[test]
fn input_metrics_reject_a_missing_selected_font_source() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture = font_file_fixture::create(directory.path().join("font.ttf"), FONT_BYTES)
        .map_err(|error| error.to_string())?;
    let measure =
        || input_character_metrics_for_face(&fixture.database, fixture.face_id, 16.0, 400, false);
    assert!(measure().is_some());
    std::fs::remove_file(&fixture.path).map_err(|error| error.to_string())?;
    assert!(measure().is_none());
    Ok(())
}
