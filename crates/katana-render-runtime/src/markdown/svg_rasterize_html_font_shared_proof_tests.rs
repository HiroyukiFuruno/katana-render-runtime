use super::super::super::super::font_source_fixture::mapped_source;
use super::same_source;
use crate::markdown::svg_rasterize::font::html_font_db_for_text;
use resvg::usvg::fontdb::Source;

#[test]
fn sharedfile_proof_requires_identical_path_and_same_backing_arc() -> Result<(), String> {
    let database = html_font_db_for_text("Arial", "proof identity");
    let path = database
        .faces()
        .find_map(|face| match &face.source {
            Source::File(path) => Some(path),
            _ => None,
        })
        .ok_or("real File source missing")?;
    let first = mapped_source(path)?;
    assert!(same_source(&first, &first.clone()));
    assert!(!same_source(&first, &mapped_source(path)?));
    assert!(!same_source(&first, &Source::File(path.clone())));
    let Source::SharedFile(_, data) = &first else {
        return Err("not SharedFile".into());
    };
    let other_path = path.with_extension("different");
    assert!(!same_source(
        &first,
        &Source::SharedFile(other_path, std::sync::Arc::clone(data))
    ));
    Ok(())
}
