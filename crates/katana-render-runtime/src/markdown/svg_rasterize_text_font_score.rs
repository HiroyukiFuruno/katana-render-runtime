use super::super::weight::css_weight_match_distance;
use resvg::usvg;

pub(super) fn fallback_face_attribute_score(
    face: &usvg::fontdb::FaceInfo,
    requested_style: usvg::fontdb::Style,
    requested_weight: usvg::fontdb::Weight,
) -> (u8, u8, u8, u16) {
    fallback_attribute_score(
        face.style,
        face.weight,
        face.stretch,
        requested_style,
        requested_weight,
        usvg::fontdb::Stretch::Normal,
    )
}
pub(super) fn fallback_attribute_score(
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

pub(super) fn css_stretch_match_rank(
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

pub(super) fn css_style_match_rank(
    candidate: usvg::fontdb::Style,
    requested: usvg::fontdb::Style,
) -> u8 {
    use usvg::fontdb::Style;
    match (requested, candidate) {
        (requested, candidate) if requested == candidate => 0,
        (Style::Italic, Style::Oblique) | (Style::Oblique, Style::Italic) => 1,
        (_, Style::Normal) => 2,
        (_, _) => CSS_STYLE_FALLBACK_RANK,
    }
}
