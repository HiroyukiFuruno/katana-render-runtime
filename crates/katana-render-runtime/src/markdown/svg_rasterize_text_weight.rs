use resvg::usvg;

const CSS_WEIGHT_LOWER_BOUND: u16 = 400;
const CSS_WEIGHT_UPPER_BOUND: u16 = 500;
const CSS_WEIGHT_IN_RANGE_OFFSET: u16 = 1;
const CSS_WEIGHT_FALLBACK_OFFSET: u16 = 1001;
const CSS_WEIGHT_ABOVE_RANGE_OFFSET: u16 = 2001;

pub(super) fn css_weight_match_distance(
    candidate_weight: usvg::fontdb::Weight,
    requested_weight: usvg::fontdb::Weight,
) -> u16 {
    let candidate_weight = candidate_weight.0;
    let requested_weight = requested_weight.0;
    if candidate_weight == requested_weight {
        return 0;
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

fn css_weight_match_directional(candidate_weight: u16, requested_weight: u16, higher: bool) -> u16 {
    let preferred = if higher {
        candidate_weight > requested_weight
    } else {
        candidate_weight < requested_weight
    };
    if preferred {
        candidate_weight.abs_diff(requested_weight)
    } else {
        CSS_WEIGHT_FALLBACK_OFFSET + candidate_weight.abs_diff(requested_weight)
    }
}

fn css_weight_match_middle_range(candidate_weight: u16, requested_weight: u16) -> u16 {
    if candidate_weight >= requested_weight && candidate_weight <= CSS_WEIGHT_UPPER_BOUND {
        return candidate_weight - requested_weight;
    }
    if candidate_weight >= CSS_WEIGHT_LOWER_BOUND && candidate_weight < requested_weight {
        return CSS_WEIGHT_IN_RANGE_OFFSET + requested_weight - candidate_weight;
    }
    if candidate_weight < CSS_WEIGHT_LOWER_BOUND {
        return CSS_WEIGHT_FALLBACK_OFFSET + CSS_WEIGHT_LOWER_BOUND - candidate_weight;
    }
    CSS_WEIGHT_ABOVE_RANGE_OFFSET + candidate_weight - CSS_WEIGHT_UPPER_BOUND
}
