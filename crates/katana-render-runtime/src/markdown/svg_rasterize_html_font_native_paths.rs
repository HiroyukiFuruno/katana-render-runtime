use resvg::usvg::fontdb::{Database, FaceInfo, Source};
use std::{collections::BTreeSet, path::PathBuf};

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_coretext_paths.rs"]
mod coretext;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_native_inventory.rs"]
mod inventory;
#[cfg(target_os = "macos")]
pub(super) use inventory::FontInventoryScope;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(super) fn native_system_font_paths(families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        coretext::resolve_paths(families, scan_system_font_paths)
    }
    #[cfg(target_os = "ios")]
    scan_system_font_paths(families)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn scan_system_font_paths(families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
    #[cfg(target_os = "macos")]
    if let Some(database) = inventory::frame_font_inventory(load_system_font_inventory) {
        return named_font_paths(&database, families);
    }
    named_font_paths(&load_system_font_inventory(), families)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn load_system_font_inventory() -> Database {
    /* WHY: 一時DBはinstalled face metadataだけを走査し、選択したFile sourceだけを呼出元DBへ残す。 */
    let mut database = Database::new();
    database.load_system_fonts();
    database
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
