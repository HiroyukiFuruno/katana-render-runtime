use super::super::super::super::font::font_source_generation;
use super::super::entry::{CacheKey, load_file_font};
use resvg::usvg::fontdb::{Database, ID};
use std::path::Path;

#[test]
fn atomic_font_replacement_rejects_started_generation_and_loads_current_font() {
    let path = super::temp_font_path("atomic-generation");
    assert!(std::fs::write(&path, super::FONT_BYTES).is_ok());
    assert!(super::file_database(&path).is_ok_and(|(database, face)| {
        replace_and_assert_generation(&database, face, &path);
        true
    }));
    assert!(std::fs::remove_file(path).is_ok());
}

fn replace_and_assert_generation(database: &Database, face: ID, path: &Path) {
    let old_generation = font_source_generation(database, face);
    let replacement = super::temp_font_path("atomic-replacement");
    assert!(std::fs::write(&replacement, [super::FONT_BYTES, &[0_u8][..]].concat()).is_ok());
    assert!(std::fs::rename(&replacement, path).is_ok());
    let current_generation = font_source_generation(database, face);
    assert_ne!(old_generation, current_generation);
    let old_key = CacheKey::File {
        path: path.to_path_buf(),
        generation: old_generation.clone(),
        face_index: 0,
    };
    assert!(std::fs::metadata(path).is_ok_and(|metadata| {
        let length = metadata.len();
        assert!(load_file_font(database, face, path, 0, old_key, old_generation, length).is_none());
        assert_current_font_shapes(database, face, path, length);
        true
    }));
}

fn assert_current_font_shapes(database: &Database, face: ID, path: &Path, length: u64) {
    let generation = font_source_generation(database, face);
    let key = CacheKey::File {
        path: path.to_path_buf(),
        generation: generation.clone(),
        face_index: 0,
    };
    assert!(
        load_file_font(database, face, path, 0, key, generation, length).is_some_and(|font| {
            super::shape_width(&font, super::VARIATION_SIZE, "office")
                .is_ok_and(|width| width.is_finite() && width > 0.0)
        })
    );
}
