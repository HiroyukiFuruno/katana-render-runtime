use super::super::super::super::text_shaping::{FileStamp, stamp_path_uncached};
use super::super::shared_raster::shared_sources::{SharedBytes, SharedSources};
use super::CreationProof;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::{path::Path, sync::Arc};

#[path = "svg_rasterize_html_font_creation_shared_source.rs"]
mod mapping;
use mapping::shared_source;

type Sharing<'a> = (&'a mut SharedSources, &'a mut usize);
type LoadedShared = (SharedBytes, usize, Vec<ID>, bool);

struct LoadAttempt {
    before: Option<FileStamp>,
    after: Option<FileStamp>,
    already_loaded: bool,
    failed: bool,
    shared: Option<LoadedShared>,
}

pub(super) fn load_one(
    proof: &mut CreationProof,
    database: &mut Database,
    path: &Path,
    sharing: Option<Sharing<'_>>,
) {
    if proof.record_missing(path) {
        return;
    }
    let already_loaded = proof.mark_loaded_identity(path);
    let before = stamp_path_uncached(path)
        .ok()
        .filter(FileStamp::durable_reusable);
    load_one_source(proof, database, path, before, already_loaded, sharing);
}

fn load_one_source(
    proof: &mut CreationProof,
    database: &mut Database,
    path: &Path,
    before: Option<FileStamp>,
    already_loaded: bool,
    mut sharing: Option<Sharing<'_>>,
) {
    let mut loaded = None;
    let load_failed = load_source(
        database,
        path,
        &before,
        already_loaded,
        sharing.as_mut(),
        &mut loaded,
    );
    let after = stamp_path_uncached(path).ok();
    let attempt = LoadAttempt {
        before,
        after,
        already_loaded,
        failed: load_failed,
        shared: loaded,
    };
    finish_load(proof, database, path, attempt, sharing.as_mut());
}

fn finish_load(
    proof: &mut CreationProof,
    database: &Database,
    path: &Path,
    attempt: LoadAttempt,
    sharing: Option<&mut Sharing<'_>>,
) {
    proof.record_load(
        path,
        attempt.before.clone(),
        attempt.after.clone(),
        attempt.already_loaded,
        attempt.failed,
    );
    retain_verified_source(
        proof,
        database,
        path,
        attempt.before,
        attempt.after,
        attempt.shared,
        sharing,
    );
}

fn load_source(
    database: &mut Database,
    path: &Path,
    stamp: &Option<FileStamp>,
    already_loaded: bool,
    sharing: Option<&mut Sharing<'_>>,
    loaded: &mut Option<LoadedShared>,
) -> bool {
    if already_loaded {
        return false;
    }
    let Some((sources, remaining)) = sharing else {
        return load_file(database, path);
    };
    load_with_shared(database, path, stamp, loaded, sources, remaining)
}

fn load_with_shared(
    database: &mut Database,
    path: &Path,
    stamp: &Option<FileStamp>,
    loaded: &mut Option<LoadedShared>,
    sources: &mut SharedSources,
    remaining: &mut usize,
) -> bool {
    match shared_source(path, stamp.as_ref(), sources, *remaining) {
        Ok(Some(source)) => {
            let ids = database.load_font_source(Source::SharedFile(
                path.to_path_buf(),
                Arc::clone(&source.data),
            ));
            let failed = ids.is_empty();
            *loaded = Some((
                source.data,
                source.length,
                ids.into_iter().collect(),
                source.newly_mapped,
            ));
            failed
        }
        Ok(None) => load_file(database, path),
        Err(()) => true,
    }
}

fn load_file(database: &mut Database, path: &Path) -> bool {
    database
        .load_font_source(Source::File(path.to_path_buf()))
        .is_empty()
}

fn retain_verified_source(
    proof: &mut CreationProof,
    database: &Database,
    path: &Path,
    before: Option<FileStamp>,
    after: Option<FileStamp>,
    loaded: Option<LoadedShared>,
    sharing: Option<&mut Sharing<'_>>,
) {
    let Some((data, length, ids, newly_mapped)) = loaded else {
        return;
    };
    if before != after || !loaded_shared_faces_match(database, path, &data, &ids) {
        proof.failed = true;
        return;
    }
    proof
        .immutable_shared
        .insert(path.to_path_buf(), Arc::as_ptr(&data) as *const () as usize);
    if newly_mapped && let Some((_, remaining)) = sharing {
        **remaining -= length;
    }
}

fn loaded_shared_faces_match(
    database: &Database,
    path: &Path,
    data: &SharedBytes,
    ids: &[ID],
) -> bool {
    !ids.is_empty()
        && ids
            .iter()
            .all(|id| loaded_face_matches(database, *id, path, data))
}

fn loaded_face_matches(database: &Database, id: ID, path: &Path, data: &SharedBytes) -> bool {
    let Some(face) = database.face(id) else {
        return false;
    };
    let Some((Source::SharedFile(source, bytes), index)) = database.face_source(id) else {
        return false;
    };
    source == path && index == face.index && Arc::ptr_eq(&bytes, data)
}
