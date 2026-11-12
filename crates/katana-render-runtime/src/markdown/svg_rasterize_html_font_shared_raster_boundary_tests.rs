use super::super::{build, shared_sources::SharedBytes};
use crate::markdown::svg_rasterize::font_file_fixture::{FONT_BYTES, create};
use resvg::usvg::fontdb::{Database, Source};
use std::{collections::BTreeMap, sync::Arc};

#[test]
fn html_render_reports_malformed_svg_parse_error() -> Result<(), String> {
    let svg = "<svg";
    let options = crate::markdown::svg_rasterize::font::html::html_rasterizer_options(svg);
    let compatible =
        crate::markdown::svg_rasterize::SvgRasterizeOps::preprocess_for_rasterizer(svg);
    let expected = match resvg::usvg::Tree::from_str(&compatible, &options) {
        Ok(_) => return Err("malformed SVG unexpectedly parsed".to_string()),
        Err(error) => error.to_string(),
    };
    let actual = match super::super::render_html_with_options(svg, &options) {
        Ok(_) => return Err("malformed SVG unexpectedly rendered".to_string()),
        Err(error) => error,
    };
    assert!(!expected.is_empty());
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn transient_build_rejects_missing_stamp_and_unknown_prefix_face() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let fixture =
        create(directory.path().join("font.ttf"), FONT_BYTES).map_err(|error| error.to_string())?;
    let prefix: Vec<_> = fixture.database.faces().cloned().collect();
    let empty = BTreeMap::new();
    let owners = BTreeMap::new();
    assert!(build(&fixture.database, &[], &empty, &owners).is_some());
    assert!(build(&fixture.database, &prefix, &empty, &owners).is_none());

    let mut removed = fixture.database.clone();
    removed.remove_face(fixture.face_id);
    assert!(build(&removed, &prefix, &empty, &owners).is_none());

    let stamp = crate::markdown::svg_rasterize::text_shaping::stamp_path_uncached(&fixture.path)
        .map_err(|_| "font fixture stamp failed")?;
    assert!(stamp.durable_reusable());
    let stamps = BTreeMap::from([(fixture.path.clone(), stamp)]);
    assert!(build(&fixture.database, &prefix, &stamps, &owners).is_some());
    std::fs::remove_file(&fixture.path).map_err(|error| error.to_string())?;
    assert!(build(&fixture.database, &prefix, &stamps, &owners).is_none());
    Ok(())
}

#[test]
fn transient_build_preserves_binary_and_requires_owned_shared_mapping() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("font.ttf");
    std::fs::write(&path, FONT_BYTES).map_err(|error| error.to_string())?;
    let data: SharedBytes = Arc::new(FONT_BYTES.to_vec());
    let mut original = Database::new();
    original.load_font_source(Source::Binary(Arc::clone(&data)));
    original.load_font_source(Source::SharedFile(path.clone(), Arc::clone(&data)));
    assert_eq!(original.len(), 2);
    let prefix: Vec<_> = original.faces().cloned().collect();
    assert!(build(&original, &prefix, &BTreeMap::new(), &BTreeMap::new()).is_none());
    let identity = Arc::as_ptr(&data) as *const () as usize;
    let owned = BTreeMap::from([(path, identity)]);
    let (fresh, remapped) = build(&original, &prefix, &BTreeMap::new(), &owned)
        .ok_or("owned shared source rejected")?;
    assert_eq!(remapped.len(), prefix.len());
    assert!(
        fresh
            .faces()
            .any(|face| matches!(face.source, Source::Binary(_)))
    );
    assert!(fresh.faces().any(
        |face| matches!(&face.source, Source::SharedFile(_, bytes) if Arc::ptr_eq(bytes, &data))
    ));
    Ok(())
}
