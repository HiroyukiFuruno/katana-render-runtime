use resvg::usvg;

pub(super) fn probe_font_has_char(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    character: char,
) -> Option<bool> {
    with_layout_font_face_data(database, face_id, |data, face_index| {
        /* WHY: glyphの有無だけを調べるため、不要なGSUB/GPOS coverage構築を省く。 */
        rustybuzz::ttf_parser::Face::parse(data, face_index)
            .ok()
            .map(|face| face.glyph_index(character).is_some())
    })
    .flatten()
}

pub(in super::super) fn with_layout_font_face_data<T>(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    operation: impl FnOnce(&[u8], u32) -> T,
) -> Option<T> {
    #[cfg(windows)]
    if let Some((usvg::fontdb::Source::File(path), face_index)) = database.face_source(face_id)
        && let Some(bytes) = super::windows_font_snapshot(path.as_path())
    {
        return Some(operation(bytes.as_slice(), face_index));
    }
    database.with_face_data(face_id, operation)
}
