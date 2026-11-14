use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::layout_text;
use super::super::style::{CssStyle, CssWhiteSpace};
use super::super::text_measurement_diagnostic::{record_wrap_timing, start_wrap_timing};
use super::super::text_metrics::text_width;
use super::css_auto_linebreak_ends;

const TEXT_LAYOUT_SUBPIXEL_SCALE: f32 = 64.0;
pub(super) const TEXT_FIT_EPSILON: f32 = 1.0 / TEXT_LAYOUT_SUBPIXEL_SCALE;

pub(in super::super) fn wrap_text_with_style(
    text: &str,
    width: f32,
    style: &CssStyle,
) -> Vec<String> {
    wrap_text_with_initial_width(text, width, width, style)
}

pub(in super::super) fn wrap_text_with_initial_width(
    text: &str,
    initial_width: f32,
    continuing_width: f32,
    style: &CssStyle,
) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    if style.white_space == CssWhiteSpace::NoWrap {
        return explicit_text_lines(text);
    }
    wrap_normal_text(text, initial_width, continuing_width, style)
}

fn wrap_normal_text(
    text: &str,
    initial_width: f32,
    continuing_width: f32,
    style: &CssStyle,
) -> Vec<String> {
    let started = start_wrap_timing();
    let lines = wrap_normal_text_body(text, initial_width, continuing_width, style);
    record_wrap_timing(started);
    lines
}

fn wrap_normal_text_body(
    text: &str,
    initial_width: f32,
    continuing_width: f32,
    style: &CssStyle,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut remaining = text;
    let mut width = initial_width.max(MIN_LAYOUT_WIDTH);
    while !remaining.is_empty() {
        if width + TEXT_FIT_EPSILON < continuing_width
            && should_defer_unbreakable_line(remaining, width, continuing_width, style)
        {
            lines.push(String::new());
            width = continuing_width.max(MIN_LAYOUT_WIDTH);
            continue;
        }
        let (line, consumed) = take_styled_line(remaining, width, style);
        lines.push(line);
        remaining = &remaining[consumed..];
        width = continuing_width.max(MIN_LAYOUT_WIDTH);
    }
    lines
}

fn should_defer_unbreakable_line(
    text: &str,
    initial_width: f32,
    continuing_width: f32,
    style: &CssStyle,
) -> bool {
    let forced_end = text.find('\n').unwrap_or(text.len());
    let forced_line = &text[..forced_end];
    if forced_line.is_empty()
        || forced_line
            .chars()
            .next()
            .is_some_and(layout_text::is_css_whitespace)
    {
        return false;
    }
    let break_ends = css_auto_linebreak_ends(forced_line);
    if last_fitting_end(forced_line, &break_ends, initial_width, style).is_some() {
        return false;
    }
    break_ends.first().is_some_and(|&first_break_end| {
        let first_segment =
            forced_line[..first_break_end].trim_end_matches(layout_text::is_css_whitespace);
        !first_segment.is_empty()
            && text_width(first_segment, style) <= continuing_width + TEXT_FIT_EPSILON
    })
}

fn explicit_text_lines(text: &str) -> Vec<String> {
    text.split_terminator('\n')
        .map(|line| {
            line.trim_end_matches(layout_text::is_css_whitespace)
                .to_string()
        })
        .collect()
}

pub(super) fn take_styled_line(text: &str, width: f32, style: &CssStyle) -> (String, usize) {
    if text.starts_with('\n') {
        return (String::new(), 1);
    }
    let forced_end = text.find('\n').unwrap_or(text.len());
    let forced_line = &text[..forced_end];
    let trimmed_line = forced_line.trim_end_matches(layout_text::is_css_whitespace);
    if text_width(trimmed_line, style) <= width + TEXT_FIT_EPSILON {
        let consumed = forced_end + usize::from(forced_end < text.len());
        return (trimmed_line.to_string(), consumed);
    }
    let break_ends = css_auto_linebreak_ends(forced_line);
    let mut fitted_end = last_fitting_end(forced_line, &break_ends, width, style).unwrap_or(0);
    if fitted_end == 0 {
        fitted_end = fitted_character_end(forced_line, width, style);
    }
    (
        forced_line[..fitted_end]
            .trim_end_matches(layout_text::is_css_whitespace)
            .to_string(),
        fitted_end,
    )
}

fn fitted_character_end(text: &str, width: f32, style: &CssStyle) -> usize {
    let character_ends = text
        .char_indices()
        .map(|(index, character)| index + character.len_utf8())
        .collect::<Vec<_>>();
    last_fitting_end(text, &character_ends, width, style)
        .unwrap_or_else(|| character_ends.first().copied().unwrap_or(0))
}

fn last_fitting_end(
    text: &str,
    candidate_ends: &[usize],
    width: f32,
    style: &CssStyle,
) -> Option<usize> {
    let mut low = 0;
    let mut high = candidate_ends.len();
    while low < high {
        let middle = low + (high - low) / 2;
        let end = candidate_ends[middle];
        let fits = text_width(
            text[..end].trim_end_matches(layout_text::is_css_whitespace),
            style,
        ) <= width + TEXT_FIT_EPSILON;
        if fits {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low.checked_sub(1).map(|index| candidate_ends[index])
}
