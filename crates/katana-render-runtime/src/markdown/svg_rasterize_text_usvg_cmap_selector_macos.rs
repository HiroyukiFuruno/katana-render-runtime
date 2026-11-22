use crate::markdown::svg_rasterize::{font, native_html_faces};
use resvg::usvg::{self, fontdb, fontdb::ID};

const INTERFACE_BOLD_FONT_WEIGHT: u16 = 700;

pub(super) fn select(font: &usvg::Font, database: &fontdb::Database) -> Option<Option<ID>> {
    if let Some(selection) = native_html_faces::select_alias(font, database) {
        return Some(selection);
    }
    if font.families().iter().any(|family| {
        matches!(
            family,
            usvg::FontFamily::Named(name)
                if font::is_internal_family(name)
        )
    }) {
        return Some(None);
    }
    is_interface_bold_request(font)
        .then(|| font::bold_alias_id(database))
        .flatten()
        .map(Some)
}

fn is_interface_bold_request(font: &usvg::Font) -> bool {
    font.weight() == INTERFACE_BOLD_FONT_WEIGHT
        && matches!(font.style(), usvg::FontStyle::Normal)
        && font.families().first().is_some_and(|family| {
            matches!(family, usvg::FontFamily::Named(name)
                if matches!(name.trim().to_ascii_lowercase().as_str(),
                    "hiragino kaku gothic interface" | ".hiragino kaku gothic interface"))
        })
}
