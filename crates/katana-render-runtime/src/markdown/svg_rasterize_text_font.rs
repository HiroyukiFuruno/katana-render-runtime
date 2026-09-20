use super::weight::css_weight_match_distance;
use resvg::usvg;
const EXACT_FACE_MATCH: u8 = 0;
const STYLE_AND_WEIGHT_MATCH: u8 = 1;
const STYLE_AND_STRETCH_MATCH: u8 = 2;
const WEIGHT_AND_STRETCH_MATCH: u8 = 3;
const STYLE_MATCH: u8 = 4;
const WEIGHT_MATCH: u8 = 5;
const STRETCH_MATCH: u8 = 6;
const NO_FACE_ATTRIBUTE_MATCH: u8 = 7;
pub(super) fn matching_font_face(
    database: &usvg::fontdb::Database,
    font_family: &str,
    font_weight: u16,
    italic: bool,
) -> Option<usvg::fontdb::ID> {
    let names = css_font_family_names(font_family);
    let families = names
        .iter()
        .map(|name| fontdb_family(name))
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
#[cfg(test)]
pub(super) fn font_runs(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    text: &str,
) -> Vec<(usvg::fontdb::ID, String)> {
    let mut runs: Vec<(usvg::fontdb::ID, String)> = Vec::new();
    for character in text.chars() {
        let face_id = resolved_base_face(database, base_face_id, character);
        if let Some((run_face_id, run)) = runs.last_mut()
            && *run_face_id == face_id
        {
            run.push(character);
        } else {
            runs.push((face_id, character.to_string()));
        }
    }
    runs
}
#[cfg(test)]
fn resolved_base_face(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
) -> usvg::fontdb::ID {
    if font_has_char(database, base_face_id, character) {
        return base_face_id;
    }
    let (weight, italic) = database
        .face(base_face_id)
        .map_or((usvg::fontdb::Weight::NORMAL.0, false), |face| {
            (face.weight.0, face.style == usvg::fontdb::Style::Italic)
        });
    matching_fallback_face(database, base_face_id, character, weight, italic)
        .unwrap_or(base_face_id)
}
pub(super) fn matching_fallback_face(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
) -> Option<usvg::fontdb::ID> {
    let base_face = database.face(base_face_id)?;
    let requested_style = if requested_italic {
        usvg::fontdb::Style::Italic
    } else {
        usvg::fontdb::Style::Normal
    };
    let requested_weight = usvg::fontdb::Weight(requested_weight);
    database
        .faces()
        .filter(|face| face.id != base_face_id)
        .filter(|face| font_has_char(database, face.id, character))
        .min_by_key(|face| {
            fallback_attribute_score(
                face.style,
                face.weight,
                face.stretch,
                requested_style,
                requested_weight,
                base_face.stretch,
            )
        })
        .map(|face| face.id)
}
fn fallback_attribute_score(
    candidate_style: usvg::fontdb::Style,
    candidate_weight: usvg::fontdb::Weight,
    candidate_stretch: usvg::fontdb::Stretch,
    requested_style: usvg::fontdb::Style,
    requested_weight: usvg::fontdb::Weight,
    requested_stretch: usvg::fontdb::Stretch,
) -> (u8, u16) {
    let same_style = candidate_style == requested_style;
    let same_weight = candidate_weight == requested_weight;
    let same_stretch = candidate_stretch == requested_stretch;
    let attribute_match = match (same_style, same_weight, same_stretch) {
        (true, true, true) => EXACT_FACE_MATCH,
        (true, true, false) => STYLE_AND_WEIGHT_MATCH,
        (true, false, true) => STYLE_AND_STRETCH_MATCH,
        (false, true, true) => WEIGHT_AND_STRETCH_MATCH,
        (true, false, false) => STYLE_MATCH,
        (false, true, false) => WEIGHT_MATCH,
        (false, false, true) => STRETCH_MATCH,
        (false, false, false) => NO_FACE_ATTRIBUTE_MATCH,
    };
    (
        attribute_match,
        css_weight_match_distance(candidate_weight, requested_weight),
    )
}
pub(super) fn font_has_char(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    character: char,
) -> bool {
    database
        .with_face_data(face_id, |data, face_index| {
            rustybuzz::Face::from_slice(data, face_index)
                .is_some_and(|face| face.glyph_index(character).is_some())
        })
        .unwrap_or(false)
}
fn css_font_family_names(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .map(|name| name.trim_matches(['\'', '"']).to_string())
        .filter(|name| !name.is_empty())
        .collect()
}
pub(super) fn fontdb_family(name: &str) -> usvg::fontdb::Family<'_> {
    match name.to_ascii_lowercase().as_str() {
        "serif" => usvg::fontdb::Family::Serif,
        "sans-serif" | "system-ui" => usvg::fontdb::Family::SansSerif,
        "cursive" => usvg::fontdb::Family::Cursive,
        "fantasy" => usvg::fontdb::Family::Fantasy,
        "monospace" => usvg::fontdb::Family::Monospace,
        _ => usvg::fontdb::Family::Name(name),
    }
}

#[cfg(test)]
mod tests {
    use super::fallback_attribute_score;
    use resvg::usvg::fontdb::{Stretch, Style, Weight};

    #[test]
    fn styled_cjk_fallback_prefers_the_exact_style_and_weight_face() {
        let styled = fallback_attribute_score(
            Style::Italic,
            Weight::BOLD,
            Stretch::Normal,
            Style::Italic,
            Weight::BOLD,
            Stretch::Normal,
        );
        let regular = fallback_attribute_score(
            Style::Normal,
            Weight::NORMAL,
            Stretch::Normal,
            Style::Italic,
            Weight::BOLD,
            Stretch::Normal,
        );

        assert!(styled < regular);
    }

    #[test]
    fn non_exact_weight_uses_css_directional_matching() {
        let bold = fallback_attribute_score(
            Style::Normal,
            Weight::BOLD,
            Stretch::Normal,
            Style::Normal,
            Weight(600),
            Stretch::Normal,
        );
        let regular = fallback_attribute_score(
            Style::Normal,
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
            Weight(600),
            Stretch::Normal,
        );

        assert!(bold < regular);
    }

    #[test]
    fn css_weight_matching_prefers_four_hundred_for_a_five_hundred_request() {
        let bold = fallback_attribute_score(
            Style::Normal,
            Weight::BOLD,
            Stretch::Normal,
            Style::Normal,
            Weight(500),
            Stretch::Normal,
        );
        let regular = fallback_attribute_score(
            Style::Normal,
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
            Weight(500),
            Stretch::Normal,
        );

        assert!(regular < bold);
    }
}
