use super::CreationProof;
use crate::markdown::svg_rasterize::font::html::shared_raster::shared_sources::{
    SharedSourceSeed, SharedSources,
};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, font::HtmlFontRequest};
use resvg::usvg::fontdb::{Database, FaceInfo, Source};
use std::sync::Arc;

const SHARED_BUDGET_BYTES: usize = 64 * 1024 * 1024;

#[test]
fn preloaded_sharedfile_keeps_selected_system_face_metadata() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let request = HtmlFontRequest::from_text("Helvetica", "preload source");
        let (file_database, _) = super::super::sources::build_known_html_font_db(&request);
        let (shared_database, proof) =
            super::super::sources::build_known_html_font_db_with_shared_budget(
                &request,
                SHARED_BUDGET_BYTES,
            );
        if !proof.current(&shared_database) {
            return Err("preloaded database proof is stale".into());
        }
        compare_shared_faces(&file_database, &shared_database, &proof)
    })
}

fn compare_shared_faces(
    file_database: &Database,
    shared_database: &Database,
    proof: &CreationProof,
) -> Result<(), String> {
    let mut count = 0;
    for shared in shared_database
        .faces()
        .filter(|face| matches!(face.source, Source::SharedFile(_, _)))
    {
        compare_shared_face(file_database, shared_database, proof, shared)?;
        count += 1;
    }
    (count > 0)
        .then_some(())
        .ok_or_else(|| "eligible system face was not preloaded".into())
}

fn compare_shared_face(
    file_database: &Database,
    shared_database: &Database,
    proof: &CreationProof,
    shared: &FaceInfo,
) -> Result<(), String> {
    let Source::SharedFile(path, data) = &shared.source else {
        return Err("preloaded face is not SharedFile".into());
    };
    let file = file_database
        .faces()
        .find(|face| same_file_identity(face, shared, path))
        .ok_or("matching File face missing")?;
    assert_same_face_metadata(file, shared);
    assert!(proof.owns_immutable_source(path, data));
    assert!(proof.contains(shared.id));
    let bytes_match = shared_database.with_face_data(shared.id, |bytes, index| {
        index == file.index && !bytes.is_empty()
    });
    assert_eq!(bytes_match, Some(true));
    Ok(())
}

fn same_file_identity(file: &FaceInfo, shared: &FaceInfo, path: &std::path::Path) -> bool {
    file.index == shared.index
        && file.post_script_name == shared.post_script_name
        && matches!(&file.source, Source::File(file_path) if file_path == path)
}

fn assert_same_face_metadata(file: &FaceInfo, shared: &FaceInfo) {
    assert_eq!(file.index, shared.index);
    assert_eq!(file.families, shared.families);
    assert_eq!(file.post_script_name, shared.post_script_name);
    assert_eq!(file.style, shared.style);
    assert_eq!(file.weight, shared.weight);
    assert_eq!(file.stretch, shared.stretch);
    assert_eq!(file.monospaced, shared.monospaced);
}

#[test]
fn shared_font_seed_with_wrong_length_fails_closed() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let request = HtmlFontRequest::from_text("Helvetica", "preload source");
        let (original, original_proof) =
            super::super::sources::build_known_html_font_db_with_shared_budget(
                &request,
                SHARED_BUDGET_BYTES,
            );
        let seeds = original_proof.reusable_shared_sources(&original);
        let seed = seeds.first().ok_or("system font seed missing")?;
        let (path, stamp, data) = seed;
        let (valid_proof, valid_database) = load_seed_without_budget(seed)?;
        assert!(!valid_proof.failed);
        assert!(
            valid_database
                .faces()
                .any(|face| matches!(face.source, Source::SharedFile(_, _)))
        );
        let (_, shortened) = data.as_ref().as_ref().split_last().ok_or("empty seed")?;
        let bad_seed: SharedSourceSeed =
            (path.clone(), stamp.clone(), Arc::new(shortened.to_vec()));
        let (proof, database) = load_seed_without_budget(&bad_seed)?;
        assert!(proof.failed);
        assert!(database.is_empty());
        Ok(())
    })
}

#[test]
fn shared_seed_reuse_ignores_a_removed_fallback_face() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let request = HtmlFontRequest::from_text("Helvetica, Arial", "shared fallback");
        let (original, proof) = super::super::sources::build_known_html_font_db_with_shared_budget(
            &request,
            SHARED_BUDGET_BYTES,
        );
        let seeds = proof.reusable_shared_sources(&original);
        let [primary, fallback, ..] = seeds.as_slice() else {
            return Err("two physical shared font seeds are required".to_owned());
        };
        let (mut proof, mut database) = load_seed_without_budget(primary)?;
        proof.capture_prefix(&database);
        load_seed(&mut proof, &mut database, fallback)?;
        assert_eq!(proof.reusable_shared_sources(&database).len(), 2);

        remove_fallback_seed_faces(&proof, &mut database, fallback);

        assert!(proof.current(&database));
        let remaining = proof.reusable_shared_sources(&database);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, primary.0);
        Ok(())
    })
}

fn remove_fallback_seed_faces(
    proof: &CreationProof,
    database: &mut Database,
    seed: &SharedSourceSeed,
) {
    let ids = database
        .faces()
        .filter(|face| matches!(&face.source, Source::SharedFile(path, _) if path == &seed.0))
        .map(|face| face.id)
        .collect::<Vec<_>>();
    assert!(!ids.is_empty());
    for id in ids {
        assert!(!proof.contains(id));
        database.remove_face(id);
    }
}

fn load_seed_without_budget(seed: &SharedSourceSeed) -> Result<(CreationProof, Database), String> {
    let mut proof = CreationProof::default();
    let mut database = Database::new();
    load_seed(&mut proof, &mut database, seed)?;
    Ok((proof, database))
}

fn load_seed(
    proof: &mut CreationProof,
    database: &mut Database,
    seed: &SharedSourceSeed,
) -> Result<(), String> {
    let mut sources = SharedSources::with_seeds(std::slice::from_ref(seed));
    let mut remaining = 0;
    proof.load_shared(
        database,
        &[seed.0.to_str().ok_or("system font path is not UTF-8")?],
        &mut sources,
        &mut remaining,
    );
    assert_eq!(remaining, 0);
    Ok(())
}
