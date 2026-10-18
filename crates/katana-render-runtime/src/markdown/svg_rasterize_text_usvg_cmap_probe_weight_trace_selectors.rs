use super::{Records, face};

pub(super) fn install(
    options: &mut resvg::usvg::Options<'static>,
    selected_records: Records,
    fallback_records: Records,
) {
    install_font_selector(options, selected_records);
    install_fallback_selector(options, fallback_records);
}

fn install_font_selector(options: &mut resvg::usvg::Options<'static>, records: Records) {
    let selector =
        crate::markdown::svg_rasterize::text_shaping::usvg_cmap::selector::html_font_selector();
    options.font_resolver.select_font = Box::new(move |font, database| {
        let selected = selector(font, database);
        let detail = selected.map(|id| face::record(database, id)).transpose();
        let Ok(mut captured) = records.lock() else {
            return selected;
        };
        captured.push(serde_json::json!({
            "requestedFamilies": font.families().iter().map(|family| format!("{family:?}")).collect::<Vec<_>>(),
            "requestedWeight": font.weight(), "selected": detail,
        }));
        selected
    });
}

fn install_fallback_selector(options: &mut resvg::usvg::Options<'static>, records: Records) {
    let selector =
        crate::markdown::svg_rasterize::text_shaping::usvg_cmap::selector::html_selector();
    options.font_resolver.select_fallback = Box::new(move |character, excluded, database| {
        let selected = selector(character, excluded, database);
        let Ok(mut captured) = records.lock() else {
            return selected;
        };
        let bases = excluded.iter().map(|id| face::record(database, *id));
        let detail = selected.map(|id| face::record(database, id)).transpose();
        captured.push(serde_json::json!({"character": character.to_string(),
            "excludedBaseFaces": bases.collect::<Result<Vec<_>, _>>(),
            "selectedFallbackFace": detail}));
        selected
    });
}
