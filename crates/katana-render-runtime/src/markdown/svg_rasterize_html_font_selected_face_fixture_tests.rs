use super::{
    CreationProof, Database, Fixture, admitted_ttc_face, file_face, fixture_database,
    write_real_fonts,
};
use std::sync::Arc;

#[test]
fn selected_face_rejects_an_unshared_file_source() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let original = fixture.database();
    let error = fixture.assert_face(&original, fixture.a, fixture.a).err();
    assert_eq!(error.as_deref(), Some("positive branch is not SharedFile"));
    Ok(())
}

#[test]
fn selected_face_requires_an_active_attempt_for_real_font() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let mut database = fixture.database();
    crate::markdown::svg_rasterize::SvgRasterizeOps::with_html_font_render_scope(|| {
        assert_eq!(
            fixture.select(&mut database, fixture.a).err().as_deref(),
            Some("selected mapping rejected")
        );
    });
    assert_eq!(
        database.faces().count(),
        fixture.context.original.faces().count()
    );
    Ok(())
}

#[test]
fn missing_font_and_ttc_faces_report_their_specific_errors() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let missing = directory.path().join("missing.ttf");
    let database = Arc::new(Database::new());
    let proof = Arc::new(CreationProof::default());
    assert_eq!(
        file_face(&database, &missing, "fixture face missing")
            .err()
            .as_deref(),
        Some("fixture face missing")
    );
    assert_eq!(
        admitted_ttc_face(&database, &proof, &missing)
            .err()
            .as_deref(),
        Some("actual TTC nonzero face did not pass existing admission")
    );
    Ok(())
}

#[test]
fn absent_destination_preserves_the_file_write_error() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let missing_root = directory.path().join("missing");
    let expected = std::fs::write(missing_root.join("noto.ttf"), b"")
        .err()
        .ok_or("unexpected write success")?
        .to_string();
    assert_eq!(
        write_real_fonts(&missing_root).err().as_deref(),
        Some(expected.as_str())
    );
    assert!(!missing_root.exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn non_utf8_font_path_is_rejected_before_loading() -> Result<(), String> {
    use std::os::unix::ffi::OsStringExt;

    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let invalid = directory
        .path()
        .join(std::ffi::OsString::from_vec(vec![0xff]));
    let valid = directory.path().join("valid.ttf");
    let error = fixture_database(directory.path(), [&invalid, &valid, &valid]).err();
    assert_eq!(error.as_deref(), Some("fixture path not UTF-8"));
    Ok(())
}
