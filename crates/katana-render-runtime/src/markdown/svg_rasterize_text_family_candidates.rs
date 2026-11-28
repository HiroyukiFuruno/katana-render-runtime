use super::super::font::matching_font_face;
use resvg::usvg::fontdb::{Database, ID};

pub(super) fn family_candidates(
    database: &Database,
    family: &str,
    weight: u16,
    italic: bool,
) -> Vec<ID> {
    let mut candidates = specified_family_faces(database, family, weight, italic);
    if family.split(',').any(is_system_family) {
        /* WHY: 日本語のsystem fallbackは、明示されたauthor familyをすべて試した後に使う。 */
        candidates.extend(matching_font_face(
            database,
            ".Hiragino Kaku Gothic Interface",
            weight,
            italic,
        ));
    }
    candidates
}

pub(in crate::markdown::svg_rasterize::text_shaping) fn specified_family_faces(
    database: &Database,
    family: &str,
    weight: u16,
    italic: bool,
) -> Vec<ID> {
    family
        .split(',')
        .filter_map(|name| matching_font_face(database, name.trim(), weight, italic))
        .collect()
}

fn is_system_family(name: &str) -> bool {
    matches!(
        name.trim()
            .trim_matches(['\'', '"'])
            .to_ascii_lowercase()
            .as_str(),
        "system-ui" | "-apple-system" | "blinkmacsystemfont"
    )
}
