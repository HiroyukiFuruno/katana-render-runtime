use resvg::usvg::{
    self,
    fontdb::{self, ID},
};
use std::sync::Arc;

pub(in super::super) fn html_selector() -> usvg::FallbackSelectionFn<'static> {
    let stock = usvg::FontResolver::default_fallback_selector();
    Box::new(move |character, excluded_ids, database| {
        select_cached(character, excluded_ids, database)
            .unwrap_or_else(|_| stock(character, excluded_ids, database))
    })
}

fn select_cached(
    character: char,
    excluded_ids: &[ID],
    database: &Arc<fontdb::Database>,
) -> Result<Option<ID>, ()> {
    let base_face_id = excluded_ids[0];
    let Some(base_face) = database.face(base_face_id) else {
        return Ok(None);
    };
    for face in database.faces() {
        if excluded_ids.contains(&face.id) || !same_fallback_attributes(base_face, face) {
            continue;
        }
        if !super::has_char(database, face.id, character)? {
            continue;
        }
        log_fallback(base_face, face);
        return Ok(Some(face.id));
    }
    Ok(None)
}

fn same_fallback_attributes(base_face: &fontdb::FaceInfo, face: &fontdb::FaceInfo) -> bool {
    !(base_face.style != face.style
        && base_face.weight != face.weight
        && base_face.stretch != face.stretch)
}

fn log_fallback(base_face: &fontdb::FaceInfo, face: &fontdb::FaceInfo) {
    let base_family = preferred_family(base_face);
    let new_family = face
        .families
        .iter()
        .find(|family| family.1 == fontdb::Language::English_UnitedStates)
        .unwrap_or(&base_face.families[0]);
    log::warn!("Fallback from {} to {}.", base_family.0, new_family.0);
}

fn preferred_family(face: &fontdb::FaceInfo) -> &(String, fontdb::Language) {
    face.families
        .iter()
        .find(|family| family.1 == fontdb::Language::English_UnitedStates)
        .unwrap_or(&face.families[0])
}
