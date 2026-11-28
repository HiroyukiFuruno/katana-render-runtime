use super::{FIRST_TEXT, FONT_BYTES, key, with_file_database, with_width};
use resvg::usvg::fontdb::{Database, ID};
use std::{path::Path, sync::Arc};

#[test]
fn unavailable_generation_rejects_cache_write_and_old_record_reuse() {
    with_file_database(|database, face, path| {
        let before = super::super::font_source_generation(database, face);
        assert!(before.reusable());
        with_width(database, face, FIRST_TEXT, |width| {
            let key = key(face, FIRST_TEXT);
            assert!(
                super::super::cached_shaped_run_record_for_generation(database, &key, &before)
                    .is_some()
            );
            assert!(std::fs::remove_file(path).is_ok());
            let unavailable = super::super::font_source_generation(database, face);
            assert!(!unavailable.reusable());
            super::super::cache_run_after_generation(
                database,
                key.clone(),
                width + 1.0,
                None,
                unavailable,
            );
            assert!(
                super::super::cached_shaped_run_record_for_generation(database, &key, &before)
                    .is_some()
            );
            assert!(super::super::cached_shaped_run_record(database, &key).is_none());
            assert!(std::fs::write(path, FONT_BYTES).is_ok());
        });
    });
}

#[test]
fn changed_font_source_rejects_metrics_then_recovers() -> Result<(), String> {
    for remove in [false, true] {
        let mut result = Ok(());
        with_file_database(|database, face, path| {
            result = check_changed_source(database, face, path, remove);
        });
        result?;
    }
    Ok(())
}

fn check_changed_source(
    database: &Arc<Database>,
    face: ID,
    path: &Path,
    remove: bool,
) -> Result<(), String> {
    const TEXT: &str = "Generation AV";
    let before = super::super::font_source_generation(database, face);
    let metrics = super::super::super::harfrust::shape_svg_run_metrics_with_face(
        database,
        face,
        TEXT,
        super::font(),
        false,
    )
    .ok_or("real font could not shape text")?
    .0;
    assert!(metrics.advance.is_finite() && metrics.advance > 0.0);
    if remove {
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
    } else {
        std::fs::write(path, [FONT_BYTES, &[0_u8][..]].concat())
            .map_err(|error| error.to_string())?;
    }
    let stale =
        super::super::validate_and_cache_svg_run(database, face, key(face, TEXT), before, metrics);
    std::fs::write(path, FONT_BYTES).map_err(|error| error.to_string())?;
    assert!(stale.is_none());
    assert_restored_metrics(database, face, TEXT)
}

fn assert_restored_metrics(database: &Arc<Database>, face: ID, text: &str) -> Result<(), String> {
    let fresh = super::super::cached_svg_run(database, face, text, super::font())
        .ok_or("restored real font could not shape text")?;
    assert!(fresh.advance.is_finite() && fresh.advance > 0.0);
    Ok(())
}
