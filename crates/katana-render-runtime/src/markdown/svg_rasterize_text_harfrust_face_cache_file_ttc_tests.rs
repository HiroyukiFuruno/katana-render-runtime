use super::*;

#[cfg(target_os = "macos")]
#[test]
fn real_ttc_faces_match_owned_glyphs_when_available() -> Result<(), String> {
    let path = Path::new("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc");
    if !path.exists() {
        return Err("expected Hiragino W3 TTC path".to_string());
    }
    let mut database = Database::new();
    database.load_font_source(Source::File(path.to_path_buf()));
    let faces = ttc_faces(&database);
    if !faces.iter().any(|(_, index)| *index == 0) || !faces.iter().any(|(_, index)| *index == 2) {
        return Err("expected Hiragino W3 TTC faces 0 and 2".to_string());
    }
    assert_ttc_faces(&database, faces)?;
    Ok(())
}
fn ttc_faces(database: &Database) -> Vec<(resvg::usvg::fontdb::ID, u32)> {
    database
        .faces()
        .filter_map(|face| {
            database.face_source(face.id).and_then(|(source, index)| {
                matches!(source, Source::File(_)).then_some((face.id, index))
            })
        })
        .filter(|(_, index)| *index == 0 || *index == 2)
        .collect()
}
fn assert_ttc_faces(
    database: &Database,
    faces: Vec<(resvg::usvg::fontdb::ID, u32)>,
) -> Result<(), String> {
    let _scope = FontFaceCacheScope::enter();
    for (face_id, face_index) in faces {
        let lazy = font_for_face(database, face_id).ok_or("lazy TTC face missing")?;
        let owned = database
            .with_face_data(face_id, |data, index| {
                harfrust::font::Font::new(data.to_vec(), index)
            })
            .flatten()
            .ok_or("owned TTC face missing")?;
        assert_lazy_matches_owned(&lazy, &owned, face_index)?;
        assert!(super::super::commit_file_font(database, face_id, &lazy));
    }
    Ok(())
}
pub(super) fn assert_lazy_matches_owned(
    lazy: &harfrust::font::Font,
    owned: &harfrust::font::Font,
    face_index: u32,
) -> Result<(), String> {
    let shape = FontShape::new(TTC_SIZE, TTC_WEIGHT, true);
    let lazy_buffer =
        shape_font(lazy, TTC_TEXT, shape, Some("kern")).ok_or("lazy TTC shaping failed")?;
    let owned_buffer =
        shape_font(owned, TTC_TEXT, shape, Some("kern")).ok_or("owned TTC shaping failed")?;
    assert_eq!(
        glyph_ids(&lazy_buffer),
        glyph_ids(&owned_buffer),
        "TTC face {face_index} glyph IDs differ"
    );
    assert_eq!(
        glyph_positions(&lazy_buffer),
        glyph_positions(&owned_buffer),
        "TTC face {face_index} glyph positions differ"
    );
    Ok(())
}
fn glyph_ids(buffer: &harfrust::Buffer) -> Vec<u32> {
    buffer
        .glyph_infos()
        .iter()
        .map(|info| info.glyph_id)
        .collect()
}
fn glyph_positions(buffer: &harfrust::Buffer) -> Vec<(i32, i32, i32, i32)> {
    buffer
        .glyph_positions()
        .iter()
        .map(|position| {
            (
                position.x_advance,
                position.y_advance,
                position.x_offset,
                position.y_offset,
            )
        })
        .collect()
}
