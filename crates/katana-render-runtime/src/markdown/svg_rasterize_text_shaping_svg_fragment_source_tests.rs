use super::{Database, FONT, SvgRasterizeError, layout_for_face, svg_font_shape};
use resvg::usvg::fontdb::{ID, Source};

fn layout(
    database: &Database,
    base: ID,
    face: ID,
) -> Result<Option<super::SvgTextFragmentLayout>, SvgRasterizeError> {
    layout_for_face(
        database,
        base,
        face,
        "A",
        svg_font_shape(),
        &["A".to_owned()],
    )
}

fn assert_valid_layout(database: &Database, base: ID, face: ID) -> Result<(), String> {
    let result = layout(database, base, face).map_err(|error| format!("{error:?}"))?;
    let layout = result.ok_or("valid font fragment layout rejected")?;
    assert_eq!(layout.advances.len(), 1);
    assert!(layout.metrics.advance > 0.0);
    Ok(())
}

#[test]
fn removed_face_rejects_fragment_shaping_after_valid_control() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT.to_vec());
    let face = database.faces().next().ok_or("Noto face missing")?.id;
    assert_valid_layout(&database, face, face)?;
    database.remove_face(face);
    assert!(database.face(face).is_none());
    assert!(matches!(layout(&database, face, face),
        Err(SvgRasterizeError::RasterizeFailed(message)) if message == "SVG font face unavailable"));
    Ok(())
}

#[test]
fn rtl_fragment_is_rejected_after_a_real_font_control() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT.to_vec());
    let face = database.faces().next().ok_or("Noto face missing")?.id;
    assert_valid_layout(&database, face, face)?;
    let rtl = "אבג";
    let result = layout_for_face(
        &database,
        face,
        face,
        rtl,
        svg_font_shape(),
        &[rtl.to_owned()],
    )
    .map_err(|error| format!("{error:?}"))?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn removed_primary_font_rejects_fallback_fragment_metrics() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("primary.ttf");
    std::fs::write(&path, FONT).map_err(|error| error.to_string())?;
    let mut database = Database::new();
    let file_faces = database.load_font_source(Source::File(path.clone()));
    let binary_faces =
        database.load_font_source(Source::Binary(std::sync::Arc::new(FONT.to_vec())));
    let ([base], [fallback]) = (file_faces.as_slice(), binary_faces.as_slice()) else {
        return Err("expected one face from each real font source".to_owned());
    };
    assert_ne!(base, fallback);
    assert_valid_layout(&database, *base, *fallback)?;
    std::fs::remove_file(&path).map_err(|error| error.to_string())?;
    assert_valid_layout(&database, *fallback, *fallback)?;
    assert!(matches!(layout(&database, *base, *fallback),
        Err(SvgRasterizeError::RasterizeFailed(message)) if message == "SVG primary font metrics unavailable"));
    Ok(())
}
