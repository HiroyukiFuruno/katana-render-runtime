use super::{
    Arc, CreationProof, Database, FileStamp, ID, SharedBytes, Source, retain_verified_source,
};
use crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT;
use std::path::Path;

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
