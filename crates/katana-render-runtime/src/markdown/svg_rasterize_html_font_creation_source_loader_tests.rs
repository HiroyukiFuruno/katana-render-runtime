use super::{
    Arc, CreationProof, Database, FileStamp, ID, SharedBytes, Source, retain_verified_source,
};
use crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT;
use std::path::Path;

#[test]
fn moved_font_after_discovery_is_not_retained_as_shared() -> std::io::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("font.ttf");
    let moved = directory.path().join("moved.ttf");
    std::fs::write(&path, BUNDLED_SANS_SERIF_FONT)?;
    let mut proof = CreationProof::default();
    assert!(!proof.record_missing(&path));
    let already_loaded = proof.mark_loaded_identity(&path);
    assert!(!already_loaded);
    std::fs::rename(&path, &moved)?;
    let before = super::stamp_path_uncached(&path).ok();
    assert!(before.is_none());

    let mut database = Database::new();
    let mut sources = super::SharedSources::default();
    let mut remaining = BUNDLED_SANS_SERIF_FONT.len();
    super::load_one_source(
        &mut proof,
        &mut database,
        &path,
        before,
        already_loaded,
        Some((&mut sources, &mut remaining)),
    );
    assert_eq!(database.faces().count(), 0);
    assert_eq!(remaining, BUNDLED_SANS_SERIF_FONT.len());
    assert!(!proof.current(&database));
    assert_eq!(std::fs::read(&moved)?, BUNDLED_SANS_SERIF_FONT);
    Ok(())
}

#[test]
fn shared_font_proof_rejects_removed_faces_and_unshared_file_sources() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("font.ttf");
    std::fs::write(&path, BUNDLED_SANS_SERIF_FONT).map_err(|error| error.to_string())?;
    let stamp = super::stamp_path_uncached(&path).map_err(|()| "fixture font stamp unavailable")?;
    let data: SharedBytes = Arc::new(BUNDLED_SANS_SERIF_FONT.to_vec());
    let mut database = Database::new();
    let shared_ids = database
        .load_font_source(Source::SharedFile(path.clone(), Arc::clone(&data)))
        .into_iter()
        .collect::<Vec<_>>();
    assert!(!shared_ids.is_empty());

    let is_reusable =
        |database: &Database, ids| source_is_reusable(database, &path, &stamp, &data, ids);
    assert!(is_reusable(&database, shared_ids.clone()));
    for id in &shared_ids {
        database.remove_face(*id);
    }
    assert!(!is_reusable(&database, shared_ids));

    let file_ids = database.load_font_source(Source::File(path.clone()));
    assert!(!file_ids.is_empty());
    assert!(!is_reusable(&database, file_ids.into_iter().collect()));
    Ok(())
}

fn source_is_reusable(
    database: &Database,
    path: &Path,
    stamp: &FileStamp,
    data: &SharedBytes,
    ids: Vec<ID>,
) -> bool {
    let mut proof = CreationProof::default();
    let loaded = (Arc::clone(data), data.as_ref().as_ref().len(), ids, false);
    retain_verified_source(
        &mut proof,
        database,
        path,
        Some(stamp.clone()),
        Some(stamp.clone()),
        Some(loaded),
        None,
    );
    proof.current(database)
}
