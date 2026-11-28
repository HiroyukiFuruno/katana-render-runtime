use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

pub(in crate::markdown::svg_rasterize) fn names(family: &str) -> Vec<String> {
    super::font::css_font_family_names(family)
}

pub(in crate::markdown::svg_rasterize) fn matching(
    database: &Database,
    family: &str,
    weight: u16,
    italic: bool,
) -> Option<ID> {
    super::font::matching_font_face(database, family, weight, italic)
}

pub(in crate::markdown::svg_rasterize) fn usable() -> bool {
    super::font::memo_usable()
}

pub(in crate::markdown::svg_rasterize) fn observe_all(database: &Database) -> bool {
    database
        .faces()
        .all(|face| super::font::font_source_generation(database, face.id).durable_reusable())
}

pub(in crate::markdown::svg_rasterize) fn supports(
    database: &Arc<Database>,
    id: ID,
    character: char,
) -> bool {
    super::font::cached_font_has_char_with_generation(database, id, character).0 == Some(true)
}
