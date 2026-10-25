use super::CreationProof;
use crate::markdown::svg_rasterize::{SvgRasterizeOps, font::HtmlFontRequest};
use resvg::usvg::fontdb::{Database, FaceInfo, Source};

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
