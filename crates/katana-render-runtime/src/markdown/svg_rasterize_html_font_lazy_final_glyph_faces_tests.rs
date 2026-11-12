use super::*;
use resvg::usvg::{
    Options, Tree,
    fontdb::{Database, Source},
};
use std::{collections::BTreeMap, sync::Arc};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

#[test]
fn unresolved_face_id_records_the_glyph_count_error() -> Result<(), String> {
    let tree = Tree::from_str(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#,
        &Options::default(),
    )
    .map_err(|error| error.to_string())?;
    let id = ID::dummy();
    let mut face_counts = BTreeMap::new();
    face_counts.insert(id, 7);

    let resolved = super::resolve(&tree, "root", 3, &face_counts);

    assert!(resolved.faces.is_empty());
    assert!(resolved.owners.is_empty());
    assert_eq!(resolved.errors.len(), 1);
    assert_eq!(resolved.errors[0].face_id, Some(id.to_string()));
    assert_eq!(resolved.errors[0].reason, "fontdb face ID is unresolved");
    Ok(())
}

#[test]
fn binary_font_source_is_measure_only_and_never_physical_verified() -> Result<(), String> {
    let (tree, id) = font_tree(Source::Binary(Arc::new(FONT_BYTES.to_vec())))?;
    let mut face_counts = BTreeMap::new();
    face_counts.insert(id, 2);

    let resolved = super::resolve(&tree, "root", 4, &face_counts);

    assert_eq!(resolved.faces.len(), 1);
    assert_eq!(resolved.faces[0].glyph_count, 2);
    assert_eq!(resolved.faces[0].identity, "unknown");
    assert!(resolved.faces[0].source.is_some());
    assert_eq!(resolved.owners.len(), 0);
    assert_eq!(resolved.errors.len(), 1);
    assert_eq!(
        resolved.errors[0].reason,
        "font source lacks stable physical file identity"
    );
    Ok(())
}

#[test]
fn shared_font_audit_records_physical_identity_without_retaining_bytes() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("NotoSans-Regular.ttf");
    std::fs::write(&path, FONT_BYTES).map_err(|error| error.to_string())?;
    let source = Source::SharedFile(path.clone(), Arc::new(FONT_BYTES.to_vec()));
    let (tree, id) = font_tree(source)?;
    let counts = actual_glyph_counts(&tree)?;
    assert_eq!(counts, BTreeMap::from([(id, 2)]));
    let resolved = super::resolve(&tree, "root", 4, &counts);
    assert!(resolved.errors.is_empty());
    let [face] = resolved.faces.as_slice() else {
        return Err("one audited face required".into());
    };
    assert_eq!(face.identity, "physical_verified");
    assert_eq!(face.glyph_count, 2);
    let source = face.source.as_ref().ok_or("audited source missing")?;
    assert_eq!(source.path.as_ref(), Some(&path));
    assert!(source.stamp.is_some() && source.bytes_sha256.is_some());
    let [owner] = resolved.owners.as_slice() else {
        return Err("one shared owner required".into());
    };
    assert!(owner.upgrade().is_some());
    drop(tree);
    assert!(owner.upgrade().is_none());
    Ok(())
}

fn actual_glyph_counts(tree: &Tree) -> Result<BTreeMap<ID, u64>, String> {
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("audit-text") else {
        return Err("audit SVG text missing".into());
    };
    let mut counts = BTreeMap::new();
    for glyph in text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
    {
        *counts.entry(glyph.font).or_default() += 1;
    }
    Ok(counts)
}

fn font_tree(source: Source) -> Result<(Tree, ID), String> {
    let mut database = Database::new();
    let id = *database
        .load_font_source(source)
        .first()
        .ok_or("fixture font face missing")?;
    let options = Options {
        fontdb: Arc::new(database),
        ..Options::default()
    };
    let tree = Tree::from_str(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><text id="audit-text" x="2" y="24" font-family="Noto Sans" font-size="16">AV</text></svg>"#,
        &options,
    )
    .map_err(|error| error.to_string())?;
    Ok((tree, id))
}
