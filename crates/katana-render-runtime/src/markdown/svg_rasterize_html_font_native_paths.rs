use resvg::usvg::fontdb::{Database, FaceInfo, Source};
use std::{collections::BTreeSet, path::PathBuf};

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(super) fn native_system_font_paths(families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
    /* WHY: 一時DBはinstalled face metadataだけを走査し、選択したFile sourceだけを呼出元DBへ残す。 */
    let mut database = Database::new();
    database.load_system_fonts();
    named_font_paths(&database, families)
}

pub(super) fn named_font_paths(
    database: &Database,
    requested_families: &BTreeSet<&str>,
) -> BTreeSet<PathBuf> {
    database
        .faces()
        .filter(|face| face_has_requested_family(face, requested_families))
        .filter_map(|face| match &face.source {
            Source::File(path) => Some(path.clone()),
            _ => None,
        })
        .collect()
}

fn face_has_requested_family(face: &FaceInfo, requested_families: &BTreeSet<&str>) -> bool {
    face.families.iter().any(|(family, _)| {
        requested_families
            .iter()
            .any(|requested| family.eq_ignore_ascii_case(requested))
    })
}
