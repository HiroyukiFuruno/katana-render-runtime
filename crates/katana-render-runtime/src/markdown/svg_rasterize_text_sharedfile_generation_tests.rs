use super::super::super::super::font_source_fixture::mapped_source;
use super::{file_source_stamp, font_source_generation_uncached, stamp_path_uncached};
use crate::markdown::svg_rasterize::font::html_font_db_for_text;
use resvg::usvg::fontdb::{Database, ID, Source};

#[test]
fn sharedfile_source_preserves_file_generation_and_uncached_stamp() -> Result<(), String> {
    let original = html_font_db_for_text("Arial", "generation");
    let face = original
        .faces()
        .find(|face| matches!(face.source, Source::File(_)))
        .ok_or("real File face missing")?;
    let Source::File(path) = &face.source else {
        return Err("not File".into());
    };
    let mut shared = Database::new();
    let mut fresh = face.clone();
    fresh.id = ID::dummy();
    fresh.source = mapped_source(path)?;
    let fresh = shared.push_face_info(fresh);
    let (is_file, stamp, generation) = file_source_stamp(&shared, fresh);
    assert!(is_file && generation.is_file());
    assert_eq!(
        stamp.as_ref(),
        Some(&stamp_path_uncached(path).map_err(|_| "stamp failed")?)
    );
    assert_eq!(generation, font_source_generation_uncached(&shared, fresh));
    Ok(())
}
