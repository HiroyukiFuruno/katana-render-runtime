use resvg::usvg::{self, fontdb};

pub(super) fn select(font: &usvg::Font, database: &fontdb::Database) -> Option<fontdb::ID> {
    let mut families = font.families().iter().map(query_family).collect::<Vec<_>>();
    families.push(fontdb::Family::Serif);
    database.query(&fontdb::Query {
        families: &families,
        weight: fontdb::Weight(font.weight()),
        stretch: query_stretch(font.stretch()),
        style: match font.style() {
            usvg::FontStyle::Normal => fontdb::Style::Normal,
            usvg::FontStyle::Italic => fontdb::Style::Italic,
            usvg::FontStyle::Oblique => fontdb::Style::Oblique,
        },
    })
}

fn query_family(family: &usvg::FontFamily) -> fontdb::Family<'_> {
    match family {
        usvg::FontFamily::Named(name)
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "system-ui" | "-apple-system" | "blinkmacsystemfont"
            ) =>
        {
            fontdb::Family::Name("System Font")
        }
        usvg::FontFamily::Named(name) => fontdb::Family::Name(name),
        usvg::FontFamily::Serif => fontdb::Family::Serif,
        usvg::FontFamily::SansSerif => fontdb::Family::SansSerif,
        usvg::FontFamily::Cursive => fontdb::Family::Cursive,
        usvg::FontFamily::Fantasy => fontdb::Family::Fantasy,
        usvg::FontFamily::Monospace => fontdb::Family::Monospace,
    }
}

fn query_stretch(stretch: usvg::FontStretch) -> fontdb::Stretch {
    match stretch {
        usvg::FontStretch::UltraCondensed => fontdb::Stretch::UltraCondensed,
        usvg::FontStretch::ExtraCondensed => fontdb::Stretch::ExtraCondensed,
        usvg::FontStretch::Condensed => fontdb::Stretch::Condensed,
        usvg::FontStretch::SemiCondensed => fontdb::Stretch::SemiCondensed,
        usvg::FontStretch::Normal => fontdb::Stretch::Normal,
        usvg::FontStretch::SemiExpanded => fontdb::Stretch::SemiExpanded,
        usvg::FontStretch::Expanded => fontdb::Stretch::Expanded,
        usvg::FontStretch::ExtraExpanded => fontdb::Stretch::ExtraExpanded,
        usvg::FontStretch::UltraExpanded => fontdb::Stretch::UltraExpanded,
    }
}
