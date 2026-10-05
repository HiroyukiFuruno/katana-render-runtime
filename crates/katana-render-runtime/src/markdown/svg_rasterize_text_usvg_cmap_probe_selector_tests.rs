use super::*;

fn assert_probe_reuse_matches_file_durability(
    database: &Arc<Database>,
    face_id: ID,
    font_path: &Path,
) -> Result<(), String> {
    let durable = super::super::super::super::font::file_stamp_durable_reusable(font_path);
    let (first_probe, subsequent_probe) = with_validated_scope(|| {
        (
            super::super::has_char(database, face_id, 'A'),
            super::super::has_char(database, face_id, 'A'),
        )
    });
    if durable {
        assert_eq!(first_probe, Ok(true));
        assert_eq!(subsequent_probe, Ok(true));
    } else {
        assert_eq!(first_probe, Err(()));
        assert_eq!(subsequent_probe, Err(()));
        let generation =
            super::super::super::super::font::font_source_generation(database, face_id);
        assert!(
            super::super::super::storage::lookup(database, face_id, 0, &generation,)
                .map_err(|_| "cmap cache lock poisoned")?
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn selector_falls_back_to_stock_when_cached_probe_is_unavailable() -> Result<(), String> {
    let (_lock, _reset) = super::cache_test_guard()?;
    let first_font = super::TempFont::write(super::FONT_BYTES)?;
    let second_font = super::TempFont::write(super::FONT_BYTES)?;
    let (database, base_face, fallback_face) =
        super::database_with_two_file_fonts(&first_font, &second_font)?;
    assert_eq!(
        super::super::has_char(&database, fallback_face, 'A'),
        Err(())
    );
    assert_probe_reuse_matches_file_durability(&database, fallback_face, &second_font.0)?;

    let excluded = [base_face];
    let stock = resvg::usvg::FontResolver::default_fallback_selector();
    let cached = super::super::super::selector::html_selector();
    let (stock_result, cached_result) = super::with_validated_scope(|| {
        (
            stock('A', &excluded, &mut Arc::clone(&database)),
            cached('A', &excluded, &mut Arc::clone(&database)),
        )
    });
    assert_eq!(stock_result, Some(fallback_face));
    assert_eq!(cached_result, stock_result);
    Ok(())
}
