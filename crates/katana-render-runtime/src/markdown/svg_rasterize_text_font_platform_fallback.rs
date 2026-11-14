use super::FallbackAttributes;
use objc2_core_foundation::{CFRange, CFString};
use objc2_core_text::CTFont;
use resvg::usvg::fontdb::{Database, ID};

const FALLBACK_FONT_SIZE: f64 = 16.0;

#[cfg(test)]
#[path = "svg_rasterize_text_coretext_fallback_diagnostic_tests.rs"]
mod coretext_diagnostic_tests;
#[path = "svg_rasterize_text_coretext_font_identity.rs"]
mod font_identity;
#[cfg(test)]
#[path = "svg_rasterize_text_font_platform_fallback_tests.rs"]
mod tests;

fn fallback_family(database: &Database, base_id: ID, character: char) -> Option<String> {
    let base_name = &database.face(base_id)?.post_script_name;
    if base_name.is_empty() {
        return None;
    }
    let name = CFString::from_str(base_name);
    /* WHY: CoreText が別の base へ置換した場合は誤った cascade を使わない。 */
    let base = unsafe { CTFont::with_name(&name, FALLBACK_FONT_SIZE, std::ptr::null()) };
    if unsafe { base.post_script_name() }.to_string() != base_name.as_str() {
        return None;
    }
    if !font_identity::matches(database, base_id, &base) {
        return None;
    }
    let text = CFString::from_str(&character.to_string());
    let range = CFRange {
        location: 0,
        length: text.length(),
    };
    /* WHY: CFString の UTF-16 長を使い、補助平面文字も範囲に収める。 */
    let fallback = unsafe { base.for_string(&text, range) };
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
