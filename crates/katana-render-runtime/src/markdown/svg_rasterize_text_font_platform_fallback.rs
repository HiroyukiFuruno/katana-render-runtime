use super::FallbackAttributes;
use resvg::usvg::fontdb::{Database, ID};

const FALLBACK_FONT_SIZE: f64 = 16.0;

#[path = "svg_rasterize_text_coretext_actual_fallback.rs"]
pub(in crate::markdown::svg_rasterize::text_shaping) mod actual_fallback;
#[cfg(test)]
#[path = "svg_rasterize_text_coretext_fallback_diagnostic_tests.rs"]
mod coretext_diagnostic_tests;
#[path = "svg_rasterize_text_coretext_font_identity.rs"]
mod font_identity;
#[cfg(test)]
#[path = "svg_rasterize_text_font_platform_fallback_tests.rs"]
mod tests;

fn fallback_family(database: &Database, base_id: ID, character: char) -> Option<String> {
    let fallback =
        actual_fallback::default_fallback_font(database, base_id, character, FALLBACK_FONT_SIZE)?;
    Some(unsafe { fallback.family_name() }.to_string())
}

pub(super) fn select(
    database: &Database,
    base: ID,
    character: char,
    attributes: FallbackAttributes,
    probe: &mut impl FnMut(ID, char) -> Option<bool>,
) -> (Option<ID>, bool) {
    if database
        .face(base)
        .is_some_and(super::super::super::font::is_system_ui_primary)
    {
        return (None, true);
    }
    let Some(family) = fallback_family(database, base, character) else {
        return (None, true);
    };
    select_family(database, base, character, attributes, &family, probe)
}

fn select_family(
    database: &Database,
    base: ID,
    character: char,
    attributes: FallbackAttributes,
    family: &str,
    probe: &mut impl FnMut(ID, char) -> Option<bool>,
) -> (Option<ID>, bool) {
    super::scan_fallback_faces(database, base, character, attributes, |id, glyph| {
        if !database.face(id).is_some_and(|face| {
            face.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
        }) {
            return Some(false);
        }
        probe(id, glyph)
    })
}
