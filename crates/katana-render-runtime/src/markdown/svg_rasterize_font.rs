use resvg::usvg;
#[path = "svg_rasterize_html_font.rs"]
mod html;
#[cfg(test)]
pub(super) use html::{
    HtmlFontMemorySnapshot, current_process_rss_kib, html_font_db_for_markup,
    html_font_memory_snapshot,
};
pub(super) use html::{html_font_db_for_text, html_rasterizer_options};

const BUNDLED_SANS_SERIF_FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const SANS_SERIF_FAMILIES: &[&str] = &[
    "Noto Sans",
    "Noto Sans JP",
    "Noto Sans CJK JP",
    "Hiragino Sans",
    "Yu Gothic",
    "Meiryo",
    "Arial",
    "DejaVu Sans",
    "Liberation Sans",
];
const SERIF_FAMILIES: &[&str] = &[
    "Noto Serif JP",
    "Noto Serif CJK JP",
    "Hiragino Mincho ProN",
    "Yu Mincho",
    "Noto Serif",
    "Times New Roman",
    "DejaVu Serif",
    "Liberation Serif",
    "Noto Sans",
];
const MONOSPACE_FAMILIES: &[&str] = &[
    "Noto Sans Mono CJK JP",
    "Noto Sans Mono",
    "SFMono-Regular",
    "Menlo",
    "Consolas",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans",
];
const CURSIVE_FAMILIES: &[&str] = &[
    "Comic Sans MS",
    "Apple Chancery",
    "URW Chancery L",
    "Noto Sans",
];
const FANTASY_FAMILIES: &[&str] = &["Impact", "Papyrus", "Noto Sans"];

pub(super) fn rasterizer_options() -> usvg::Options<'static> {
    rasterizer_options_with_font_db(bundled_font_db())
}

pub(super) fn rasterizer_options_with_font_db(
    fontdb: std::sync::Arc<usvg::fontdb::Database>,
) -> usvg::Options<'static> {
    usvg::Options {
        fontdb,
        ..usvg::Options::default()
    }
}

pub(super) fn bundled_font_db() -> std::sync::Arc<usvg::fontdb::Database> {
    static FONT_DB: std::sync::OnceLock<std::sync::Arc<usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    std::sync::Arc::clone(FONT_DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        /* WHY: 公開 SVG rasterizer の pixel 出力を host font から独立させる。 */
        db.load_font_data(BUNDLED_SANS_SERIF_FONT.to_vec());
        configure_generic_families(&mut db);
        std::sync::Arc::new(db)
    }))
}

pub(super) fn configure_generic_families(database: &mut usvg::fontdb::Database) {
    database.set_sans_serif_family(first_available_family(database, SANS_SERIF_FAMILIES));
    database.set_serif_family(first_available_family(database, SERIF_FAMILIES));
    database.set_monospace_family(first_available_family(database, MONOSPACE_FAMILIES));
    database.set_cursive_family(first_available_family(database, CURSIVE_FAMILIES));
    database.set_fantasy_family(first_available_family(database, FANTASY_FAMILIES));
}

fn first_available_family(
    database: &usvg::fontdb::Database,
    candidates: &'static [&'static str],
) -> &'static str {
    candidates
        .iter()
        .copied()
        .find(|family| font_family_is_available(database, family))
        .unwrap_or("Noto Sans")
}

fn font_family_is_available(database: &usvg::fontdb::Database, family: &str) -> bool {
    database
        .faces()
        .any(|face| face.families.iter().any(|name| name.0 == family))
}

#[cfg(test)]
mod tests {
    use super::{BUNDLED_SANS_SERIF_FONT, configure_generic_families, first_available_family};
    use resvg::usvg;

    #[test]
    fn bundled_font_resolves_every_generic_family_without_host_fonts() {
        let mut database = usvg::fontdb::Database::new();
        database.load_font_data(BUNDLED_SANS_SERIF_FONT.to_vec());
        configure_generic_families(&mut database);

        for family in [
            usvg::fontdb::Family::SansSerif,
            usvg::fontdb::Family::Serif,
            usvg::fontdb::Family::Monospace,
            usvg::fontdb::Family::Cursive,
            usvg::fontdb::Family::Fantasy,
        ] {
            assert!(
                database
                    .query(&usvg::fontdb::Query {
                        families: &[family],
                        weight: usvg::fontdb::Weight::BOLD,
                        stretch: usvg::fontdb::Stretch::Normal,
                        style: usvg::fontdb::Style::Italic,
                    })
                    .is_some()
            );
        }
    }

    #[test]
    fn generic_sans_serif_prefers_the_bundled_font() {
        let mut database = usvg::fontdb::Database::new();
        database.load_font_data(BUNDLED_SANS_SERIF_FONT.to_vec());
        configure_generic_families(&mut database);
        let face = database
            .query(&usvg::fontdb::Query {
                families: &[usvg::fontdb::Family::SansSerif],
                weight: usvg::fontdb::Weight::NORMAL,
                stretch: usvg::fontdb::Stretch::Normal,
                style: usvg::fontdb::Style::Normal,
            })
            .and_then(|id| database.face(id));

        assert!(face.is_some_and(|face| {
            face.families.iter().any(|family| family.0 == "Noto Sans")
                && matches!(face.source, usvg::fontdb::Source::Binary(_))
        }));
    }

    #[test]
    fn unavailable_family_list_uses_the_bundled_fallback_name() {
        let database = usvg::fontdb::Database::new();

        assert_eq!(first_available_family(&database, &["Missing"]), "Noto Sans");
    }
}
