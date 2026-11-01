use super::{FIRST_TEXT, FONT_BYTES, key, with_file_database, with_width};

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
