use resvg::usvg::{
    self,
    fontdb::{self, ID},
};
use std::sync::Arc;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_text_usvg_cmap_requested_weight_tests.rs"]
mod requested_weight_tests;

#[path = "svg_rasterize_text_usvg_cmap_selector_styled.rs"]
mod styled;
pub(in super::super) use styled::html_styled_selector;

#[path = "svg_rasterize_text_usvg_cmap_selector_loaded.rs"]
mod loaded;
pub(in crate::markdown::svg_rasterize) use loaded::loaded_html_font;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use loaded::requests_system_font;

#[cfg(target_os = "macos")]
const INTERFACE_BOLD_FONT_WEIGHT: u16 = 700;

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[path = "svg_rasterize_text_usvg_system_selector.rs"]
mod system_selector;

pub(in super::super) fn html_selector() -> usvg::FallbackSelectionFn<'static> {
    let stock = usvg::FontResolver::default_fallback_selector();
    Box::new(move |character, excluded_ids, database| {
        select_cached(character, excluded_ids, database)
            .unwrap_or_else(|_| stock(character, excluded_ids, database))
    })
}

pub(in super::super) fn html_font_selector() -> usvg::FontSelectionFn<'static> {
    let stock = usvg::FontResolver::default_font_selector();
    Box::new(move |font, database| {
        #[cfg(target_os = "macos")]
        if let Some(selection) = select_macos_html_font_override(font, database) {
            return selection;
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if requests_system_font(font) {
            return system_selector::select(font, database);
        }
        select_font_case_insensitively(font, database).or_else(|| stock(font, database))
    })
}

#[cfg(target_os = "macos")]
fn select_macos_html_font_override(
    font: &usvg::Font,
    database: &fontdb::Database,
) -> Option<Option<ID>> {
    if font.families().iter().any(|family| {
        matches!(
            family,
            usvg::FontFamily::Named(name)
                if super::super::super::font::is_internal_family(name)
        )
    }) {
        return Some(None);
    }
    if is_interface_bold_request(font) {
        return super::super::super::font::bold_alias_id(database).map(Some);
    }
    None
}

#[cfg(target_os = "macos")]
fn is_interface_bold_request(font: &usvg::Font) -> bool {
    font.weight() == INTERFACE_BOLD_FONT_WEIGHT
        && matches!(font.style(), usvg::FontStyle::Normal)
        && font.families().first().is_some_and(|family| {
            matches!(family, usvg::FontFamily::Named(name)
                if matches!(name.trim().to_ascii_lowercase().as_str(),
                    "hiragino kaku gothic interface" | ".hiragino kaku gothic interface"))
        })
}

fn select_font_case_insensitively(font: &usvg::Font, database: &fontdb::Database) -> Option<ID> {
    for family in font.families() {
        let selected = match family {
            usvg::FontFamily::Named(requested) => {
                let exact = fontdb::Family::Name(requested);
                query_family(font, database, &exact).or_else(|| {
                    let folded_name = database
                        .faces()
                        .flat_map(|face| face.families.iter())
                        .find(|(name, _)| name.eq_ignore_ascii_case(requested))
                        .map(|(name, _)| name.as_str())?;
                    query_family(font, database, &fontdb::Family::Name(folded_name))
                })
            }
            usvg::FontFamily::Serif => query_family(font, database, &fontdb::Family::Serif),
            usvg::FontFamily::SansSerif => query_family(font, database, &fontdb::Family::SansSerif),
            usvg::FontFamily::Cursive => query_family(font, database, &fontdb::Family::Cursive),
            usvg::FontFamily::Fantasy => query_family(font, database, &fontdb::Family::Fantasy),
            usvg::FontFamily::Monospace => query_family(font, database, &fontdb::Family::Monospace),
        };
        if selected.is_some() {
            return selected;
        }
    }
    query_family(font, database, &fontdb::Family::Serif)
}

fn query_family(
    font: &usvg::Font,
    database: &fontdb::Database,
    family: &fontdb::Family,
) -> Option<ID> {
    database.query(&fontdb::Query {
        families: std::slice::from_ref(family),
        weight: fontdb::Weight(font.weight()),
        stretch: fontdb::Stretch::from(font.stretch()),
        style: fontdb::Style::from(font.style()),
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
    let (selected, cacheable) = closest_fallback(database, base_face, character, excluded_ids);
    if !cacheable {
        return Err(());
    }
    if let Some(face_id) = selected
        && let Some(face) = database.face(face_id)
    {
        log_fallback(base_face, face);
    }
    Ok(selected)
}

fn closest_fallback(
    database: &Arc<fontdb::Database>,
    base_face: &fontdb::FaceInfo,
    character: char,
    excluded_ids: &[ID],
) -> (Option<ID>, bool) {
    super::super::font::matching_fallback_with_probe(
        database,
        base_face.id,
        character,
        base_face.weight.0,
        base_face.style != fontdb::Style::Normal,
        |face_id, glyph| fallback_probe(database, base_face, excluded_ids, face_id, glyph),
    )
}

fn fallback_probe(
    database: &Arc<fontdb::Database>,
    base_face: &fontdb::FaceInfo,
    excluded_ids: &[ID],
    face_id: ID,
    glyph: char,
) -> Option<bool> {
    let face = database.face(face_id)?;
    if excluded_ids.contains(&face_id) || !same_fallback_attributes(base_face, face) {
        return Some(false);
    }
    super::has_char(database, face_id, glyph).ok()
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

#[cfg(test)]
#[path = "svg_rasterize_text_usvg_cmap_selector_attribute_tests.rs"]
mod attribute_tests;

#[cfg(test)]
#[path = "svg_rasterize_text_usvg_cmap_selector_family_case_tests.rs"]
mod family_case_tests;

#[cfg(test)]
#[path = "svg_rasterize_text_usvg_cmap_selector_mixed_tspan_tests.rs"]
mod mixed_tspan_tests;
