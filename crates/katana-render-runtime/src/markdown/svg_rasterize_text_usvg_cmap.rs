#[path = "svg_rasterize_text_usvg_cmap_probe.rs"]
mod probe;
#[path = "svg_rasterize_text_usvg_cmap_selector.rs"]
mod selector;
#[path = "svg_rasterize_text_usvg_cmap_sfnt.rs"]
mod sfnt;
#[path = "svg_rasterize_text_usvg_cmap_storage.rs"]
mod storage;

use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

const MAX_ENTRY_BYTES: usize = 1024 * 1024;
pub(super) use selector::html_font_selector;
pub(super) use selector::html_selector;
pub(in crate::markdown::svg_rasterize) use selector::loaded_html_font;

pub(in super::super) fn install(options: &mut resvg::usvg::Options<'static>) {
    options.font_resolver.select_font = html_font_selector();
    options.font_resolver.select_fallback = html_selector();
    super::install_html_fallback_memo(options);
}

fn has_char(database: &Arc<Database>, face_id: ID, character: char) -> Result<bool, ()> {
    probe::has_char(database, face_id, character)
}

#[cfg(test)]
#[path = "svg_rasterize_text_usvg_cmap_tests.rs"]
mod tests;
