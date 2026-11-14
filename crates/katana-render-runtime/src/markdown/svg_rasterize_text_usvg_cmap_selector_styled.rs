use resvg::usvg::{
    self,
    fontdb::{self, ID},
};
use std::sync::Arc;

pub(in super::super::super) fn html_styled_selector() -> usvg::StyledFallbackSelectionFn<'static> {
    Box::new(move |font, character, excluded, database| {
        match select_styled(font, character, excluded, database) {
            Ok(selected) => selected,
            Err(()) => {
                log::warn!("SVG fallback cache unavailable; selecting with uncached font data");
                select_styled_uncached(font, character, excluded, database)
            }
        }
    })
}

fn select_styled_uncached(
    font: &usvg::Font,
    character: char,
    excluded: &[ID],
    database: &Arc<fontdb::Database>,
) -> Option<ID> {
    let base = *excluded.first()?;
    super::super::super::font::matching_fallback_with_request(
        database,
        base,
        character,
        font,
        |face, glyph| {
            if excluded.contains(&face) {
                return Some(false);
            }
            super::super::probe::stock_probe(database, face, glyph).ok()
        },
    )
    .0
}

fn select_styled(
    font: &usvg::Font,
    character: char,
    excluded: &[ID],
    database: &Arc<fontdb::Database>,
) -> Result<Option<ID>, ()> {
    let Some(base) = excluded.first().and_then(|id| database.face(*id)) else {
        return Ok(None);
    };
    let (selected, cacheable) = super::super::super::font::matching_fallback_with_request(
        database,
        base.id,
        character,
        font,
        |face_id, glyph| {
            if excluded.contains(&face_id) {
                return Some(false);
            }
            super::super::has_char(database, face_id, glyph).ok()
        },
    );
    if !cacheable {
        return Err(());
    }
    if let Some(face) = selected.and_then(|id| database.face(id)) {
        super::log_fallback(base, face);
    }
    Ok(selected)
}
