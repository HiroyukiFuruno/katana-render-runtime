use super::{
    FONT_BYTES, FontFaceCacheScope, MAX_SOURCE_BYTES, font_for_face, font_source_generation,
    shape_and_commit,
};
use resvg::usvg::fontdb::{Database, Source};
use std::sync::Arc;

#[test]
fn shared_file_keeps_warm_file_metrics_reusable() -> Result<(), String> {
    let fixture = shared_font_file_fixture("shared-preserves-file")?;
    check_shared_file_preserves_cache(&fixture.database, fixture.face_id)
}

#[test]
fn malformed_shared_file_does_not_prevent_normal_file_shaping() -> Result<(), String> {
    let fixture = shared_font_file_fixture("shared-malformed-recovery")?;
    let mut malformed = FONT_BYTES.to_vec();
    malformed[0] = !malformed[0];
    let (shared_db, shared_id) = shared_database(Arc::new(malformed))?;
    let _scope = FontFaceCacheScope::enter();
    assert!(font_for_face(&shared_db, shared_id).is_none());

    let file = font_for_face(&fixture.database, fixture.face_id)
        .ok_or("normal file font missing after malformed shared source")?;
    let baseline = harfrust::font::Font::new(FONT_BYTES.to_vec(), 0)
        .ok_or("independent baseline font missing")?;
    assert_eq!(
        super::shape_width(&file, super::VARIATION_SIZE, "AVoffice")?,
        super::shape_width(&baseline, super::VARIATION_SIZE, "AVoffice")?
    );
    Ok(())
}

fn check_shared_file_preserves_cache(
    file_db: &Database,
    file_id: resvg::usvg::fontdb::ID,
) -> Result<(), String> {
    let _scope = FontFaceCacheScope::enter();
    let file_font = font_for_face(file_db, file_id).ok_or("file font missing")?;
    shape_and_commit(file_db, file_id, &file_font, "file")?;
    let durable = font_source_generation(file_db, file_id).durable_reusable();
    let file_key = active_file_key();
    assert_file_cache_state(durable, &file_key);
    let (shared_db, shared_id) = shared_database(Arc::new(FONT_BYTES.to_vec()))?;
    let shared = font_for_face(&shared_db, shared_id).ok_or("shared font missing")?;
    assert_eq!(shared.retained_source_bytes, FONT_BYTES.len());
    assert!(shared.key.is_none());
    assert_file_cache_state(durable, &file_key);
    let reused = font_for_face(file_db, file_id).ok_or("reused file font missing")?;
    if durable {
        assert!(std::ptr::eq(file_font.metrics(), reused.metrics()));
    }
    assert_file_cache_state(durable, &file_key);
    Ok(())
}

#[test]
fn oversized_shared_file_evicts_cached_faces_without_being_retained() -> Result<(), String> {
    let fixture = shared_font_file_fixture("shared-oversized")?;
    check_oversized_shared_file(&fixture.database, fixture.face_id)
}

fn check_oversized_shared_file(
    file_db: &Database,
    file_id: resvg::usvg::fontdb::ID,
) -> Result<(), String> {
    let mut oversized = FONT_BYTES.to_vec();
    oversized.resize(MAX_SOURCE_BYTES + 1, 0);
    let (shared_db, shared_id) = shared_database(Arc::new(oversized))?;
    let _scope = FontFaceCacheScope::enter();
    let file_font = font_for_face(file_db, file_id).ok_or("file font missing")?;
    shape_and_commit(file_db, file_id, &file_font, "file")?;
    let durable = font_source_generation(file_db, file_id).durable_reusable();
    let file_key = active_file_key();
    assert_file_cache_state(durable, &file_key);
    let shared = font_for_face(&shared_db, shared_id).ok_or("oversized shared font missing")?;
    assert!(shared.key.is_none());
    assert_eq!(shared.retained_source_bytes, MAX_SOURCE_BYTES + 1);
    assert_cache_empty();
    Ok(())
}

fn shared_database(bytes: Arc<Vec<u8>>) -> Result<(Database, resvg::usvg::fontdb::ID), String> {
    let mut source = Database::new();
    source.load_font_data(FONT_BYTES.to_vec());
    let mut face = source
        .faces()
        .next()
        .ok_or("shared source face missing")?
        .clone();
    face.source = Source::SharedFile("shared-cache-regression.ttf".into(), bytes);
    let mut database = Database::new();
    let face_id = database.push_face_info(face);
    Ok((database, face_id))
}

fn active_file_key() -> Option<super::super::CacheKey> {
    super::super::ACTIVE_SCOPE
        .with(|cache| cache.borrow().entry.as_ref().map(|entry| entry.key.clone()))
}

fn assert_cache_empty() {
    super::super::ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert!(cache.entry.is_none() && cache.spare.is_none());
    });
}

fn assert_file_cache_state(durable: bool, expected_key: &Option<super::super::CacheKey>) {
    if !durable {
        assert_cache_empty();
        return;
    }
    assert!(expected_key.is_some());
    assert_eq!(active_file_key(), expected_key.clone());
    super::super::ACTIVE_SCOPE.with(|cache| assert!(cache.borrow().spare.is_none()));
}

fn shared_font_file_fixture(
    label: &str,
) -> Result<crate::markdown::svg_rasterize::font_file_fixture::FontFileFixture, String> {
    let path = super::temp_font_path(label);
    crate::markdown::svg_rasterize::font_file_fixture::create(path, FONT_BYTES)
        .map_err(|error| error.to_string())
}
