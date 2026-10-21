use resvg::usvg::fontdb::{Database, ID, Source};
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
    let Source::File(path) = &face.source else {
        return Err("registered face is not initially File-backed".into());
    };
    let bytes = std::fs::read(path).map_err(|error| format!("read interface font: {error}"))?;
    face.source = Source::SharedFile(path.clone(), Arc::new(bytes));
    database.remove_face(alias);
    let alias = database.push_face_info(face);
    let base =
        super::super::matching_font_face(&database, "system-ui", SYSTEM_UI_REGULAR_WEIGHT, false)
            .ok_or("system-ui primary face missing")?;
    Ok((database, base, alias))
}
