use super::super::super::super::proof::CreationProof;
use super::{CopiedFont, SharedSources, stamp_path_uncached};
use resvg::usvg::fontdb::{Database, Source};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[test]
fn failed_directory_map_has_no_blob_and_retries_no_stale_mapping() -> Result<(), String> {
    let copied = CopiedFont::new()?;
    let path = unmappable_directory(&copied)?;
    let stamp = stamp_path_uncached(&path).map_err(|_| "directory stamp failed")?;
    assert!(stamp.durable_reusable());
    let mut sources = SharedSources::default();
    assert!(sources.get(&path, &stamp).is_none());
    assert_eq!(sources.paths.len(), 1);
    assert!(sources.get(&path, &stamp).is_none());
    assert_eq!(sources.paths.len(), 1);
    Ok(())
}

#[test]
fn failed_map_replacement_requires_fresh_source_id_and_new_stamp() -> Result<(), String> {
    let copied = CopiedFont::new()?;
    let path = unmappable_directory(&copied)?;
    let before = stamp_path_uncached(&path).map_err(|_| "directory stamp failed")?;
    let mut old_sources = SharedSources::default();
    assert!(old_sources.get(&path, &before).is_none());
    assert_eq!(old_sources.paths.len(), 1);
    std::fs::rename(&path, copied.directory.join("failed-directory"))
        .map_err(|error| error.to_string())?;
    std::fs::copy(&copied.path, &path).map_err(|error| error.to_string())?;
    let after = stamp_path_uncached(&path).map_err(|_| "replacement stamp failed")?;
    assert_ne!(before, after);
    assert!(old_sources.get(&path, &before).is_none());
    assert!(old_sources.get(&path, &after).is_none());
    let data = SharedSources::default()
        .get(&path, &after)
        .ok_or("fresh map failed")?;
    assert_eq!(
        data.as_ref().as_ref(),
        std::fs::read(&copied.path)
            .map_err(|error| error.to_string())?
            .as_slice()
    );
    Ok(())
}

fn unmappable_directory(copied: &CopiedFont) -> Result<PathBuf, String> {
    let path = copied.directory.join("unmappable");
    std::fs::create_dir(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

#[test]
fn original_and_shared_proof_reject_atomic_font_path_replacement() -> Result<(), String> {
    let copied = CopiedFont::new()?;
    let (original, proof) = copied_proof(&copied.path)?;
    let (shared, shared_proof) = proof.transient(&original).ok_or("transient failed")?;
    let mapped = shared
        .faces()
        .find_map(|face| match &face.source {
            Source::SharedFile(_, data) => Some(Arc::downgrade(data)),
            _ => None,
        })
        .ok_or("SharedFile map missing")?;
    let before = stamp_path_uncached(&copied.path).map_err(|_| "old stamp failed")?;
    replace_font_inode(&copied)?;
    let after = stamp_path_uncached(&copied.path).map_err(|_| "new stamp failed")?;
    assert_ne!(before, after);
    assert_stale_proofs(&original, &proof, &shared, &shared_proof, &copied.path)?;
    drop(shared_proof);
    drop(shared);
    assert!(mapped.upgrade().is_none());
    Ok(())
}

fn assert_stale_proofs(
    original: &Arc<Database>,
    proof: &CreationProof,
    shared: &Arc<Database>,
    shared_proof: &CreationProof,
    path: &Path,
) -> Result<(), String> {
    assert!(!proof.current(original));
    assert!(!shared_proof.current(shared));
    assert!(proof.transient(original).is_none());
    assert!(shared_proof.transient(shared).is_none());
    let (fresh, fresh_proof) = copied_proof(path)?;
    assert!(fresh_proof.current(&fresh));
    assert!(fresh_proof.transient(&fresh).is_some());
    Ok(())
}

fn copied_proof(path: &Path) -> Result<(Arc<Database>, CreationProof), String> {
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    proof.load(
        &mut database,
        &[path.to_str().ok_or("nonUTF8 scratch path")?],
    );
    proof.capture_prefix(&database);
    assert!(!database.is_empty());
    assert!(proof.current(&database));
    Ok((Arc::new(database), proof))
}

fn replace_font_inode(copied: &CopiedFont) -> Result<(), String> {
    let replacement = copied.directory.join("replacement.ttf");
    std::fs::copy(&copied.path, &replacement).map_err(|error| error.to_string())?;
    std::fs::rename(replacement, &copied.path).map_err(|error| error.to_string())?;
    Ok(())
}
