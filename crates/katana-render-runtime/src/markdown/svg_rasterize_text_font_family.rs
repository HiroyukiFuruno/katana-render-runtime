#[cfg(target_os = "macos")]
use super::INTERFACE_BOLD_WEIGHT;
use super::usvg;

pub(in crate::markdown::svg_rasterize::text_shaping) fn matching_font_face(
    database: &usvg::fontdb::Database,
    font_family: &str,
    font_weight: u16,
    italic: bool,
) -> Option<usvg::fontdb::ID> {
    #[cfg(target_os = "macos")]
    if contains_internal_family_name(font_family) {
        return None;
    }
    #[cfg(target_os = "macos")]
    if let Some(selected) = interface_bold_override(database, font_family, font_weight, italic) {
        return Some(selected);
    }
    let names = css_font_family_names(font_family);
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

#[cfg(target_os = "macos")]
fn contains_internal_family_name(name: &str) -> bool {
    super::super::super::font::is_internal_family(name)
        || css_font_family_names(name)
            .iter()
            .any(|family| super::super::super::font::is_internal_family(family))
}

#[cfg(target_os = "macos")]
fn interface_bold_override(
    database: &usvg::fontdb::Database,
    name: &str,
    weight: u16,
    italic: bool,
) -> Option<usvg::fontdb::ID> {
    let name = name.trim().to_ascii_lowercase();
    if weight != INTERFACE_BOLD_WEIGHT
        || italic
        || !matches!(
            name.as_str(),
            ".hiragino kaku gothic interface" | "hiragino kaku gothic interface"
        )
    {
        return None;
    }
    super::super::super::font::bold_alias_id(database)
}

fn fontdb_family_for_database<'a>(
    database: &'a usvg::fontdb::Database,
    name: &'a str,
) -> usvg::fontdb::Family<'a> {
    let family = fontdb_family(name);
    let usvg::fontdb::Family::Name(requested) = family else {
        return family;
    };
    database
        .faces()
        .flat_map(|face| face.families.iter())
        .find(|(actual, _)| actual.eq_ignore_ascii_case(requested))
        .map_or(family, |(actual, _)| usvg::fontdb::Family::Name(actual))
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
        "system-ui" | "-apple-system" | "blinkmacsystemfont" => system_font_family(),
        "cursive" => usvg::fontdb::Family::Cursive,
        "fantasy" => usvg::fontdb::Family::Fantasy,
        "monospace" => usvg::fontdb::Family::Monospace,
        _ => usvg::fontdb::Family::Name(name),
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn system_font_family() -> usvg::fontdb::Family<'static> {
    usvg::fontdb::Family::Name("System Font")
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn system_font_family() -> usvg::fontdb::Family<'static> {
    usvg::fontdb::Family::SansSerif
}
