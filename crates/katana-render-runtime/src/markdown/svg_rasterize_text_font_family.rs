use super::usvg;
use crate::markdown::svg_rasterize::font::platform::{PlatformFontStrategy, font_platform};
use std::ops::ControlFlow;

pub(in crate::markdown::svg_rasterize::text_shaping) fn matching_font_face(
    database: &usvg::fontdb::Database,
    font_family: &str,
    font_weight: u16,
    italic: bool,
) -> Option<usvg::fontdb::ID> {
    let names = css_font_family_names(font_family);
    if let ControlFlow::Break(selection) =
        font_platform().face_override(database, font_family, &names, font_weight, italic)
    {
        return selection;
    }
    let families = names
        .iter()
        .map(|name| fontdb_family_for_database(database, name))
        .collect::<Vec<_>>();
    database.query(&usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font_weight),
        stretch: usvg::fontdb::Stretch::Normal,
        style: if italic {
            usvg::fontdb::Style::Italic
        } else {
            usvg::fontdb::Style::Normal
        },
    })
}

fn fontdb_family_for_database<'a>(
    database: &'a usvg::fontdb::Database,
    name: &'a str,
) -> usvg::fontdb::Family<'a> {
    if name.eq_ignore_ascii_case("system-ui")
        && let Some(family) = font_platform().installed_system_ui_family()
        && let Some(actual) = family_name_in_database(database, family)
    {
        return usvg::fontdb::Family::Name(actual);
    }
    let family = fontdb_family(name);
    let usvg::fontdb::Family::Name(requested) = family else {
        return family;
    };
    family_name_in_database(database, requested).map_or(family, usvg::fontdb::Family::Name)
}

fn family_name_in_database<'a>(
    database: &'a usvg::fontdb::Database,
    requested: &str,
) -> Option<&'a str> {
    database
        .faces()
        .flat_map(|face| face.families.iter())
        .find(|(actual, _)| actual.eq_ignore_ascii_case(requested))
        .map(|(actual, _)| actual.as_str())
}
pub(in crate::markdown::svg_rasterize::text_shaping) fn css_font_family_names(
    value: &str,
) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .map(|name| name.trim_matches(['\'', '"']).to_string())
        .filter(|name| !name.is_empty())
        .collect()
}
pub(in crate::markdown::svg_rasterize::text_shaping) fn fontdb_family(
    name: &str,
) -> usvg::fontdb::Family<'_> {
    match name.to_ascii_lowercase().as_str() {
        "serif" => usvg::fontdb::Family::Serif,
        "sans-serif" => usvg::fontdb::Family::SansSerif,
        "system-ui" | "-apple-system" | "blinkmacsystemfont" => font_platform().system_alias(),
        "cursive" => usvg::fontdb::Family::Cursive,
        "fantasy" => usvg::fontdb::Family::Fantasy,
        "monospace" => usvg::fontdb::Family::Monospace,
        _ => usvg::fontdb::Family::Name(name),
    }
}
