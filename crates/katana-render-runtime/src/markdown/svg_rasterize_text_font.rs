use super::weight::css_weight_match_distance;
use resvg::usvg;
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
    database.face(base_face_id)?;
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
                usvg::fontdb::Stretch::Normal,
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
) -> (u8, u8, u8, u16) {
    let stretch_rank = css_stretch_match_rank(candidate_stretch, requested_stretch);
    let style_rank = css_style_match_rank(candidate_style, requested_style);
    let (weight_phase, weight_distance) =
        css_weight_match_distance(candidate_weight, requested_weight);
    /* WHY: CSS fallback matching compares stretch, style, then weight. 重みの距離は
    属性一致の判定後に比較し、近い weight が stretch/style の一致を越えないようにする。 */
    (stretch_rank, style_rank, weight_phase, weight_distance)
}

const CSS_STRETCH_MIN: u8 = 1;
const CSS_STRETCH_EXTRA_CONDENSED: u8 = 2;
const CSS_STRETCH_CONDENSED: u8 = 3;
const CSS_STRETCH_SEMI_CONDENSED: u8 = 4;
const CSS_STRETCH_NORMAL: u8 = 5;
const CSS_STRETCH_SEMI_EXPANDED: u8 = 6;
const CSS_STRETCH_EXPANDED: u8 = 7;
const CSS_STRETCH_EXTRA_EXPANDED: u8 = 8;
const CSS_STRETCH_MAX: u8 = 9;
const CSS_STYLE_FALLBACK_RANK: u8 = 3;

fn css_stretch_match_rank(
    candidate: usvg::fontdb::Stretch,
    requested: usvg::fontdb::Stretch,
) -> u8 {
    let candidate = css_stretch_value(candidate);
    let requested = css_stretch_value(requested);
    if candidate == requested {
        return 0;
    }

    let candidate_is_preferred_direction =
        if requested <= css_stretch_value(usvg::fontdb::Stretch::Normal) {
            candidate < requested
        } else {
            candidate > requested
        };
    let distance = candidate.abs_diff(requested);
    u8::from(!candidate_is_preferred_direction) * CSS_STRETCH_MAX + distance
}

fn css_stretch_value(stretch: usvg::fontdb::Stretch) -> u8 {
    match stretch {
        usvg::fontdb::Stretch::UltraCondensed => CSS_STRETCH_MIN,
        usvg::fontdb::Stretch::ExtraCondensed => CSS_STRETCH_EXTRA_CONDENSED,
        usvg::fontdb::Stretch::Condensed => CSS_STRETCH_CONDENSED,
        usvg::fontdb::Stretch::SemiCondensed => CSS_STRETCH_SEMI_CONDENSED,
        usvg::fontdb::Stretch::Normal => CSS_STRETCH_NORMAL,
        usvg::fontdb::Stretch::SemiExpanded => CSS_STRETCH_SEMI_EXPANDED,
        usvg::fontdb::Stretch::Expanded => CSS_STRETCH_EXPANDED,
        usvg::fontdb::Stretch::ExtraExpanded => CSS_STRETCH_EXTRA_EXPANDED,
        usvg::fontdb::Stretch::UltraExpanded => CSS_STRETCH_MAX,
    }
}

fn css_style_match_rank(candidate: usvg::fontdb::Style, requested: usvg::fontdb::Style) -> u8 {
    use usvg::fontdb::Style;
    match (requested, candidate) {
        (requested, candidate) if requested == candidate => 0,
        (Style::Italic, Style::Oblique) | (Style::Oblique, Style::Italic) => 1,
        (_, Style::Normal) => 2,
        (_, _) => CSS_STYLE_FALLBACK_RANK,
    }
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
    use super::{css_stretch_match_rank, css_style_match_rank, fallback_attribute_score};
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

    #[test]
    fn non_exact_stretch_uses_css_directional_matching() {
        assert!(
            css_stretch_match_rank(Stretch::SemiCondensed, Stretch::Normal)
                < css_stretch_match_rank(Stretch::SemiExpanded, Stretch::Normal)
        );
        assert!(
            css_stretch_match_rank(Stretch::Expanded, Stretch::SemiExpanded)
                < css_stretch_match_rank(Stretch::Normal, Stretch::SemiExpanded)
        );
    }

    #[test]
    fn italic_request_prefers_oblique_before_normal() {
        assert!(
            css_style_match_rank(Style::Oblique, Style::Italic)
                < css_style_match_rank(Style::Normal, Style::Italic)
        );
    }

    #[test]
    fn fallback_matching_prefers_normal_stretch_regular_over_condensed_bold() {
        let regular = fallback_attribute_score(
            Style::Normal,
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
            Weight::BOLD,
            Stretch::Normal,
        );
        let condensed_bold = fallback_attribute_score(
            Style::Normal,
            Weight::BOLD,
            Stretch::Condensed,
            Style::Normal,
            Weight::BOLD,
            Stretch::Normal,
        );

        assert!(regular < condensed_bold);
    }
}
