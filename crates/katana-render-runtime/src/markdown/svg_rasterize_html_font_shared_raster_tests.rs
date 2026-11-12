use super::super::{admission, proof::CreationProof, request::HtmlFontRequest, sources};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};
use resvg::usvg::fontdb::{Database, FaceInfo, Source};
use std::sync::Arc;

const TEXT: &str = "Open linked page";
const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="260" height="60"><rect width="260" height="60" fill="white"/><text x="4" y="34" font-family="Arial" font-size="16">Open linked page</text></svg>"#;

fn original() -> (Arc<Database>, CreationProof) {
    let request = HtmlFontRequest::from_text("Arial", TEXT);
    let (database, proof) = sources::build_known_html_font_db(&request);
    (Arc::new(database), proof)
}

#[test]
fn transient_shared_arial_preserves_file_prefix_and_four_styles() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let (original, proof) = original();
        let (shared, shared_proof) = proof.transient(&original).ok_or("transient proof failed")?;
        assert!(!Arc::ptr_eq(&original, &shared));
        assert_eq!(original.len(), shared.len());
        for (weight, italic) in [(400, false), (700, false), (400, true), (700, true)] {
            assert_style(&original, &proof, &shared, &shared_proof, weight, italic)?;
        }
        assert!(proof.current(&original) && shared_proof.current(&shared));
        Ok(())
    })
}

fn assert_style(
    original: &Arc<Database>,
    proof: &CreationProof,
    shared: &Arc<Database>,
    shared_proof: &CreationProof,
    weight: u16,
    italic: bool,
) -> Result<(), String> {
    let old = admission::admit(original, proof, "Arial", TEXT, weight, italic)
        .ok_or("original File admission failed")?;
    let fresh = admission::admit(shared, shared_proof, "Arial", TEXT, weight, italic)
        .ok_or("SharedFile admission failed")?;
    let left = original.face(old).ok_or("original face missing")?;
    let right = shared.face(fresh).ok_or("shared face missing")?;
    assert_same_metadata_and_source(left, right)?;
    assert!(proof.contains(old) && shared_proof.contains(fresh));
    Ok(())
}

fn assert_same_metadata_and_source(left: &FaceInfo, right: &FaceInfo) -> Result<(), String> {
    let (Source::File(old), Source::SharedFile(fresh, _)) = (&left.source, &right.source) else {
        return Err("expected actual File origin and SharedFile target".into());
    };
    assert_eq!(old, fresh);
    assert_eq!(
        (
            left.index,
            &left.families,
            &left.post_script_name,
            left.style,
            left.weight,
            left.stretch,
            left.monospaced
        ),
        (
            right.index,
            &right.families,
            &right.post_script_name,
            right.style,
            right.weight,
            right.stretch,
            right.monospaced
        )
    );
    Ok(())
}

#[test]
fn transient_mapping_is_shared_once_per_path_and_drops_with_database() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let (original, proof) = original();
        let (database, proof) = proof.transient(&original).ok_or("transient failed")?;
        let mut paths = std::collections::BTreeMap::new();
        for face in database.faces() {
            if let Source::SharedFile(path, data) = &face.source
                && let Some(previous) = paths.insert(path.clone(), Arc::clone(data))
            {
                assert!(Arc::ptr_eq(&previous, data));
            }
        }
        assert!(!paths.is_empty());
        let weak = paths.values().map(Arc::downgrade).collect::<Vec<_>>();
        drop(paths);
        drop(proof);
        drop(database);
        assert!(weak.iter().all(|data| data.upgrade().is_none()));
        Ok(())
    })
}

#[test]
fn transient_sharedfile_raster_matches_eager_all_pixels() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let (original, proof) = original();
        let (database, _) = proof.transient(&original).ok_or("transient failed")?;
        let mut shared_options = super::super::super::rasterizer_options_with_font_db(database);
        text_shaping::install_html_font_resolver(&mut shared_options);
        let request = HtmlFontRequest::from_text("Arial", TEXT);
        let eager = Arc::new(sources::build_html_font_db(&request));
        let mut eager_options = super::super::super::rasterizer_options_with_font_db(eager);
        text_shaping::install_html_font_resolver(&mut eager_options);
        let shared = SvgRasterizeOps::rasterize_with_options(SVG, 1.0, &shared_options)
            .map_err(|error| error.to_string())?;
        let expected = SvgRasterizeOps::rasterize_with_options(SVG, 1.0, &eager_options)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            (shared.width, shared.height),
            (expected.width, expected.height)
        );
        assert_eq!(shared.rgba, expected.rgba);
        Ok(())
    })
}

#[test]
fn transient_prefix_cannot_admit_removed_face_or_expand_original_prefix() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let (original, proof) = original();
        let (database, shared_proof) = proof.transient(&original).ok_or("transient failed")?;
        let selected = admission::admit(&database, &shared_proof, "Arial", TEXT, 400, false)
            .ok_or("SharedFile admission failed")?;
        let mut changed = (*database).clone();
        changed.remove_face(selected);
        assert!(!shared_proof.current(&changed));
        assert!(proof.current(&original));
        assert!(admission::admit(&original, &proof, "missing, Arial", TEXT, 400, false).is_none());
        assert!(
            admission::admit(&database, &shared_proof, "missing, Arial", TEXT, 400, false)
                .is_none()
        );
        Ok(())
    })
}

#[path = "svg_rasterize_html_font_shared_tree_tests.rs"]
mod tree_tests;
#[path = "svg_rasterize_html_font_shared_ttc_tests.rs"]
mod ttc_tests;
#[path = "svg_rasterize_html_font_shared_raster_boundary_tests.rs"]
mod boundary_tests;

type ProvenShared = (Arc<Database>, Arc<CreationProof>);
type MappingWeak = std::sync::Weak<dyn AsRef<[u8]> + Send + Sync>;
const REGULAR_FONT_WEIGHT: u16 = 400;

fn positive_no_route_shared(
    original: &Arc<Database>,
    proof: CreationProof,
) -> Result<ProvenShared, String> {
    assert!(!super::super::lazy_diagnostic::active());
    let proof = Arc::new(proof);
    let (shared, shared_proof) =
        super::super::lazy_diagnostic::select_database(original, Arc::clone(&proof))
            .ok_or("no-route actual File transient failed")?;
    assert!(!Arc::ptr_eq(original, &shared));
    assert_eq!(shared.len(), original.len());
    assert_style(
        original,
        &proof,
        &shared,
        &shared_proof,
        REGULAR_FONT_WEIGHT,
        false,
    )?;
    for (left, right) in original.faces().zip(shared.faces()) {
        assert_transient_source_metadata(left, right)?;
    }
    assert_generic_families_preserved(original, &shared);
    assert!(shared_proof.selected_current(&shared));
    Ok((shared, shared_proof))
}

fn assert_transient_source_metadata(left: &FaceInfo, right: &FaceInfo) -> Result<(), String> {
    match &left.source {
        Source::File(_) => assert_same_metadata_and_source(left, right),
        Source::Binary(_) => {
            let mut expected = left.clone();
            expected.id = right.id;
            assert!(super::super::proof::same_face(&expected, right));
            Ok(())
        }
        Source::SharedFile(_, _) => Err("original positive source is already SharedFile".into()),
    }
}

fn assert_generic_families_preserved(original: &Database, shared: &Database) {
    use resvg::usvg::fontdb::Family;
    for family in [
        Family::Serif,
        Family::SansSerif,
        Family::Cursive,
        Family::Fantasy,
        Family::Monospace,
    ] {
        assert_eq!(original.family_name(&family), shared.family_name(&family));
    }
}

fn shared_mapping_owners(database: &Database) -> Result<Vec<MappingWeak>, String> {
    let owners: Vec<_> = database
        .faces()
        .filter_map(|face| match &face.source {
            Source::SharedFile(_, data) => Some(Arc::downgrade(data)),
            _ => None,
        })
        .collect();
    if owners.is_empty() {
        return Err("positive actual SharedFile mapping missing".into());
    }
    assert!(owners.iter().all(|owner| owner.upgrade().is_some()));
    Ok(owners)
}

fn assert_second_shared_transient_rejected(shared: &Arc<Database>, proof: &Arc<CreationProof>) {
    let before: Vec<_> = shared.faces().cloned().collect();
    let namespace = Arc::downgrade(shared);
    assert!(proof.selected_current(shared));
    assert!(!super::super::lazy_diagnostic::active());
    assert!(super::super::lazy_diagnostic::select_database(shared, Arc::clone(proof)).is_none());
    assert!(proof.selected_current(shared));
    assert!(std::sync::Weak::ptr_eq(&namespace, &Arc::downgrade(shared)));
    assert_eq!(before.len(), shared.len());
    assert!(
        before
            .iter()
            .zip(shared.faces())
            .all(|(left, right)| { super::super::proof::same_face(left, right) })
    );
}

fn release_accepted_shared_mapping(
    shared: Arc<Database>,
    proof: Arc<CreationProof>,
) -> Result<(), String> {
    let owners = shared_mapping_owners(&shared)?;
    let database_owner = Arc::downgrade(&shared);
    drop(shared);
    assert!(database_owner.upgrade().is_none());
    /* WHY: accepted proofのprefixが最後のmapping ownerなのでDB解放だけでは失効しない。 */
    assert!(owners.iter().all(|owner| owner.upgrade().is_some()));
    drop(proof);
    assert!(owners.iter().all(|owner| owner.upgrade().is_none()));
    Ok(())
}

#[test]
fn transient_rejects_sharedfile_origin_without_leaking_existing_library_mapping()
-> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| -> Result<(), String> {
        let (original, proof) = original();
        let original_owner = Arc::downgrade(&original);
        let (shared, shared_proof) = positive_no_route_shared(&original, proof)?;
        assert_second_shared_transient_rejected(&shared, &shared_proof);
        release_accepted_shared_mapping(shared, shared_proof)?;
        drop(original);
        assert!(original_owner.upgrade().is_none());
        assert!(!super::super::lazy_diagnostic::active());
        Ok(())
    })
}
