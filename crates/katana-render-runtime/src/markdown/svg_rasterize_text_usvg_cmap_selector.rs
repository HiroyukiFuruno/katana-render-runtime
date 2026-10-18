use resvg::usvg::{
    self,
    fontdb::{self, ID},
};
use std::sync::Arc;

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
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if font.families().iter().any(|family| match family {
            usvg::FontFamily::Named(name) => matches!(
                name.to_ascii_lowercase().as_str(),
                "system-ui" | "-apple-system" | "blinkmacsystemfont"
            ),
            _ => false,
        }) {
            return system_selector::select(font, database);
        }
        select_font_case_insensitively(font, database).or_else(|| stock(font, database))
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
