use super::super::constants::{FONT_WEIGHT_BOLD, FONT_WEIGHT_NORMAL, LINE_HEIGHT_FACTOR};
use super::CssLength;

#[path = "style_grid_value.rs"]
mod grid;
#[path = "style_math_value.rs"]
mod math;

pub(super) use grid::grid_tracks;
pub(super) use math::{css_relative_px, css_resolved_px, split_top_level_whitespace};

const CSS_BOX_SIDE_COUNT: usize = 4;
const FONT_WEIGHT_THIN: u16 = 100;
const FONT_WEIGHT_BLACK: u16 = 900;
const FONT_WEIGHT_MAX: u16 = 1_000;
const BOLDER_NORMAL_CEILING: u16 = 349;
const BOLDER_BOLD_CEILING: u16 = 549;
const LIGHTER_THIN_CEILING: u16 = 549;
const LIGHTER_NORMAL_CEILING: u16 = 749;

impl CssLength {
    pub(super) fn parse(
        value: &str,
        em_base: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<Self> {
        let value = value.trim();
        if let Some(percent) = value.strip_suffix('%') {
            return css_number(percent).map(|value| Self::Percent(value / 100.0));
        }
        css_resolved_px(value, em_base, viewport_width, viewport_height, false).map(Self::Px)
    }

    pub(in super::super) fn resolve(self, available: f32) -> f32 {
        match self {
            Self::Px(value) => value,
            Self::Percent(value) => available * value,
        }
    }
}

pub(super) fn box_sides(
    value: &str,
    em_base: f32,
    viewport_width: f32,
    viewport_height: f32,
    signed: bool,
) -> Option<[f32; CSS_BOX_SIDE_COUNT]> {
    let values = split_top_level_whitespace(value)
        .into_iter()
        .map(|value| css_resolved_px(value, em_base, viewport_width, viewport_height, signed))
        .collect::<Option<Vec<_>>>()?;
    expand_box_values(&values)
}

pub(super) fn expand_box_values<T: Copy>(values: &[T]) -> Option<[T; CSS_BOX_SIDE_COUNT]> {
    match values {
        [all] => Some([*all; CSS_BOX_SIDE_COUNT]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left] => Some([*top, *right, *bottom, *left]),
        _ => None,
    }
}

pub(super) fn css_font_size(
    value: &str,
    inherited: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<f32> {
    if let Some(value) = value.trim().strip_suffix('%') {
        return value
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| (f64::from(inherited) * value / 100.0) as f32)
            .filter(|value| value.is_finite());
    }
    css_resolved_px(value, inherited, viewport_width, viewport_height, false)
}

pub(super) fn css_line_height(
    value: &str,
    font_size: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<(f32, Option<f32>)> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("normal") {
        return Some((font_size * LINE_HEIGHT_FACTOR, Some(LINE_HEIGHT_FACTOR)));
    }
    if let Some(factor) = css_number(value) {
        return Some((factor * font_size, Some(factor)));
    }
    if let Some(value) = value.strip_suffix('%') {
        return css_number(value).map(|value| (font_size * value / 100.0, None));
    }
    css_resolved_px(value, font_size, viewport_width, viewport_height, false)
        .map(|value| (value, None))
}

pub(super) fn css_number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

pub(super) fn css_font_weight(value: &str, inherited: u16) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(FONT_WEIGHT_NORMAL),
        "bold" => Some(FONT_WEIGHT_BOLD),
        "bolder" => Some(if inherited <= BOLDER_NORMAL_CEILING {
            FONT_WEIGHT_NORMAL
        } else if inherited <= BOLDER_BOLD_CEILING {
            FONT_WEIGHT_BOLD
        } else {
            FONT_WEIGHT_BLACK
        }),
        "lighter" => Some(if inherited <= LIGHTER_THIN_CEILING {
            FONT_WEIGHT_THIN
        } else if inherited <= LIGHTER_NORMAL_CEILING {
            FONT_WEIGHT_NORMAL
        } else {
            FONT_WEIGHT_BOLD
        }),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=FONT_WEIGHT_MAX).contains(weight)),
    }
}

#[cfg(test)]
#[path = "style_value_tests.rs"]
mod tests;
