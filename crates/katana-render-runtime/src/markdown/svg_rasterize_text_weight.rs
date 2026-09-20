use resvg::usvg;

const CSS_WEIGHT_LOWER_BOUND: u16 = 400;
const CSS_WEIGHT_UPPER_BOUND: u16 = 500;
const CSS_WEIGHT_PREFERRED_PHASE: u8 = 0;
const CSS_WEIGHT_FALLBACK_PHASE: u8 = 1;
const CSS_WEIGHT_BELOW_RANGE_PHASE: u8 = 2;
const CSS_WEIGHT_ABOVE_RANGE_PHASE: u8 = 3;

pub(super) fn css_weight_match_distance(
    candidate_weight: usvg::fontdb::Weight,
    requested_weight: usvg::fontdb::Weight,
) -> (u8, u16) {
    let candidate_weight = candidate_weight.0;
    let requested_weight = requested_weight.0;
    if candidate_weight == requested_weight {
        return (CSS_WEIGHT_PREFERRED_PHASE, 0);
    }
    /* WHY: 単純な絶対距離ではなくCSS仕様の候補順を保つため、探索方向を順位に変換する。 */
    if requested_weight > CSS_WEIGHT_UPPER_BOUND {
        return css_weight_match_directional(candidate_weight, requested_weight, true);
    }
    if requested_weight < CSS_WEIGHT_LOWER_BOUND {
        return css_weight_match_directional(candidate_weight, requested_weight, false);
    }
    css_weight_match_middle_range(candidate_weight, requested_weight)
}

fn css_weight_match_directional(
    candidate_weight: u16,
    requested_weight: u16,
    higher: bool,
) -> (u8, u16) {
    let preferred = if higher {
        candidate_weight > requested_weight
    } else {
        candidate_weight < requested_weight
    };
    if preferred {
        (
            CSS_WEIGHT_PREFERRED_PHASE,
            candidate_weight.abs_diff(requested_weight),
        )
    } else {
        (
            CSS_WEIGHT_FALLBACK_PHASE,
            candidate_weight.abs_diff(requested_weight),
        )
    }
}

fn css_weight_match_middle_range(candidate_weight: u16, requested_weight: u16) -> (u8, u16) {
    if candidate_weight >= requested_weight && candidate_weight <= CSS_WEIGHT_UPPER_BOUND {
        return (
            CSS_WEIGHT_PREFERRED_PHASE,
            candidate_weight - requested_weight,
        );
    }
    if candidate_weight >= CSS_WEIGHT_LOWER_BOUND && candidate_weight < requested_weight {
        return (
            CSS_WEIGHT_FALLBACK_PHASE,
            requested_weight - candidate_weight,
        );
    }
    if candidate_weight < CSS_WEIGHT_LOWER_BOUND {
        return (
            CSS_WEIGHT_BELOW_RANGE_PHASE,
            CSS_WEIGHT_LOWER_BOUND - candidate_weight,
        );
    }
    (
        CSS_WEIGHT_ABOVE_RANGE_PHASE,
        candidate_weight - CSS_WEIGHT_UPPER_BOUND,
    )
}

#[cfg(test)]
mod tests {
    use super::css_weight_match_distance;
    use resvg::usvg::fontdb::Weight;

    #[test]
    fn middle_range_request_prefers_the_upper_face_before_the_lower_face() {
        assert!(
            css_weight_match_distance(Weight(500), Weight(425))
                < css_weight_match_distance(Weight(400), Weight(425))
        );
    }

    #[test]
    fn middle_range_request_still_searches_the_lower_face_before_faces_above_500() {
        assert!(
            css_weight_match_distance(Weight(400), Weight(500))
                < css_weight_match_distance(Weight(700), Weight(500))
        );
    }
}
