use super::super::super::{HtmlFontRequest, sources};
use super::assert_same_metadata_and_source;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::fontdb::{Database, FaceInfo, Source};
use std::{collections::BTreeMap, sync::Arc};

const NORMAL_WEIGHT: u16 = 400;
const BOLD_WEIGHT: u16 = 700;

#[test]
fn transient_ttc_preserves_every_original_index_and_same_path_arc() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let request = HtmlFontRequest::from_text("Helvetica", "TTC face indices");
        let (database, proof) = sources::build_known_html_font_db(&request);
        let original = Arc::new(database);
        let faces = original
            .faces()
            .filter(|face| {
                proof.contains(face.id)
                    && matches!(&face.source, Source::File(path)
                if path.extension().is_some_and(|extension| extension == "ttc"))
            })
            .collect::<Vec<_>>();
        assert!(faces.len() > 1);
        let (shared, shared_proof) = proof.transient(&original).ok_or("TTC transient failed")?;
        assert_ttc_queries(&original, &shared)?;
        let mut paths = BTreeMap::new();
        let mut indices = BTreeMap::new();
        for old in faces {
            assert_ttc_face(old, &shared, &shared_proof, &mut paths, &mut indices)?;
        }
        assert!(indices.values().any(|indices| indices.len() > 1));
        assert!(proof.current(&original) && shared_proof.current(&shared));
        Ok(())
    })
}

type MappedPaths = BTreeMap<std::path::PathBuf, Arc<dyn AsRef<[u8]> + Send + Sync>>;
type FaceIndices = BTreeMap<std::path::PathBuf, std::collections::BTreeSet<u32>>;

fn assert_ttc_face(
    old: &FaceInfo,
    shared: &Database,
    proof: &super::super::super::proof::CreationProof,
    paths: &mut MappedPaths,
    indices: &mut FaceIndices,
) -> Result<(), String> {
    let fresh = remapped_face(old, shared, proof)?;
    assert_same_metadata_and_source(old, fresh)?;
    let Source::SharedFile(path, data) = &fresh.source else {
        return Err("not SharedFile".into());
    };
    if let Some(previous) = paths.insert(path.clone(), Arc::clone(data)) {
        assert!(Arc::ptr_eq(&previous, data));
    }
    indices.entry(path.clone()).or_default().insert(fresh.index);
    assert_eq!(
        shared.with_face_data(fresh.id, |bytes, index| {
            assert_eq!(index, old.index);
            rustybuzz::ttf_parser::Face::parse(bytes, index).is_ok()
        }),
        Some(true)
    );
    Ok(())
}

fn remapped_face<'a>(
    old: &FaceInfo,
    shared: &'a Database,
    proof: &super::super::super::proof::CreationProof,
) -> Result<&'a FaceInfo, String> {
    shared.faces().find(|face| {
        proof.contains(face.id) && face.index == old.index
            && face.post_script_name == old.post_script_name
            && matches!((&old.source, &face.source), (Source::File(left), Source::SharedFile(right, _)) if left == right)
    }).ok_or_else(|| "original TTC face missing after remap".into())
}

fn assert_ttc_queries(original: &Database, shared: &Database) -> Result<(), String> {
    use resvg::usvg::fontdb::{Family, Query, Style, Weight};
    for (weight, style) in [
        (NORMAL_WEIGHT, Style::Normal),
        (BOLD_WEIGHT, Style::Normal),
        (NORMAL_WEIGHT, Style::Italic),
        (BOLD_WEIGHT, Style::Italic),
    ] {
        let query = Query {
            families: &[Family::Name("Helvetica")],
            weight: Weight(weight),
            style,
            ..Query::default()
        };
        let before = original.query(&query).ok_or("original TTC query failed")?;
        let after = shared.query(&query).ok_or("shared TTC query failed")?;
        assert_same_metadata_and_source(
            original.face(before).ok_or("original face missing")?,
            shared.face(after).ok_or("shared face missing")?,
        )?;
    }
    Ok(())
}
