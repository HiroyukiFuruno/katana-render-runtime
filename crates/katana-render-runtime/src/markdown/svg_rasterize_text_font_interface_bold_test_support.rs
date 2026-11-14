use resvg::usvg::fontdb::{Database, FaceInfo, ID, Source};
use std::sync::Arc;

const SYSTEM_UI_REGULAR_WEIGHT: u16 = resvg::usvg::fontdb::Weight::NORMAL.0;

pub(super) fn shared_file_alias_fixture() -> Result<(Database, ID, ID), String> {
    let cached = crate::markdown::svg_rasterize::font::html_font_db_for_text("system-ui", "日本");
    let mut database = (*cached).clone();
    let alias = super::super::super::super::font::bold_alias_id(&database)
        .ok_or("registered interface alias missing")?;
    let mut face = database
        .face(alias)
        .ok_or("interface face missing")?
        .clone();
    face.source = share_registered_file(&database, &face)?;
    database.remove_face(alias);
    let alias = database.push_face_info(face);
    let base =
        super::super::matching_font_face(&database, "system-ui", SYSTEM_UI_REGULAR_WEIGHT, false)
            .ok_or("system-ui primary face missing")?;
    Ok((database, base, alias))
}

fn share_registered_file(database: &Database, face: &FaceInfo) -> Result<Source, String> {
    let Source::File(path) = &face.source else {
        return Err("registered face is not initially File-backed".into());
    };
    let bytes = database
        .with_face_data(face.id, |data, _| data.to_vec())
        .ok_or("read registered interface font data")?;
    Ok(Source::SharedFile(path.clone(), Arc::new(bytes)))
}

#[test]
fn shared_interface_fixture_rejects_binary_and_missing_file_faces()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::markdown::svg_rasterize::font_file_fixture::{self, FONT_BYTES};

    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let binary = database.faces().next().ok_or("binary font missing")?;
    assert_eq!(
        share_registered_file(&database, binary).err().as_deref(),
        Some("registered face is not initially File-backed")
    );

    let directory = tempfile::tempdir()?;
    let fixture = font_file_fixture::create(directory.path().join("NotoSans.ttf"), FONT_BYTES)?;
    let face = fixture
        .database
        .face(fixture.face_id)
        .ok_or("file font missing")?;
    let shared = share_registered_file(&fixture.database, face)?;
    assert!(matches!(shared, Source::SharedFile(path, bytes)
        if path == fixture.path && bytes.as_ref().as_ref() == FONT_BYTES));
    std::fs::remove_file(&fixture.path)?;
    assert_eq!(
        share_registered_file(&fixture.database, face)
            .err()
            .as_deref(),
        Some("read registered interface font data")
    );
    Ok(())
}
