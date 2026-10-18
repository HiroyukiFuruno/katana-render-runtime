use super::constants::MIN_LAYOUT_WIDTH;
#[cfg(test)]
use super::constants::TEXT_CHARACTER_WIDTH_FACTOR;
use super::style::{CssStyle, CssWhiteSpace};
use super::text_measurement_diagnostic::{record_wrap_timing, start_wrap_timing};
use super::text_metrics::text_width;
use unicode_linebreak::{BreakClass, break_property, linebreaks};
use unicode_width::UnicodeWidthStr;

const TEXT_LAYOUT_SUBPIXEL_SCALE: f32 = 64.0;
const TEXT_FIT_EPSILON: f32 = 1.0 / TEXT_LAYOUT_SUBPIXEL_SCALE;

#[path = "document_nodes.rs"]
mod nodes;

#[cfg(test)]
use nodes::is_input_tag;
pub(super) use nodes::{
    TableCell, attribute, input_initial_value, node_text, seed_input_values, table_rows,
};

#[cfg(test)]
pub(super) fn wrap_text(text: &str, width: f32, font_size: f32) -> Vec<String> {
    wrap_text_with_factor(text, width, font_size, TEXT_CHARACTER_WIDTH_FACTOR)
}

#[cfg(test)]
pub(super) fn wrap_text_with_factor(
    text: &str,
    width: f32,
    font_size: f32,
    width_factor: f32,
) -> Vec<String> {
    let capacity = text_capacity(width, font_size, width_factor);
    let mut lines = Vec::new();
    for forced_line in text.split('\n') {
        wrap_forced_line(forced_line, capacity, &mut lines);
    }
    lines
}

#[cfg(test)]
fn text_capacity(width: f32, font_size: f32, width_factor: f32) -> usize {
    let column_width = font_size * width_factor;
    let columns = width / column_width;
    let nearest = columns.round();
    let pixel_rounding_tolerance = 1.0 / column_width;
    let stable_columns = if (columns - nearest).abs() <= pixel_rounding_tolerance {
        nearest
    } else {
        columns.floor()
    };
    stable_columns.max(MIN_LAYOUT_WIDTH) as usize
}

#[cfg(test)]
fn wrap_forced_line(text: &str, capacity: usize, lines: &mut Vec<String>) {
    if text.is_empty() {
        lines.push(String::new());
        return;
    }

    let mut line = String::new();
    let mut segment_start = 0;
    for (segment_end, _) in linebreaks(text) {
        let segment = &text[segment_start..segment_end];
        append_breakable_segment(segment, capacity, &mut line, lines);
        segment_start = segment_end;
    }
    push_wrapped_line(line, lines);
}

#[cfg(test)]
fn append_breakable_segment(
    segment: &str,
    capacity: usize,
    line: &mut String,
    lines: &mut Vec<String>,
) {
    if !line.is_empty() && text_display_columns(line) + text_display_columns(segment) > capacity {
        lines.push(std::mem::take(line).trim_end().to_string());
        line.push_str(segment.trim_start());
        return;
    }
    line.push_str(segment);
}

#[cfg(test)]
fn push_wrapped_line(line: String, lines: &mut Vec<String>) {
    lines.push(line.trim_end().to_string());
}

pub(super) fn wrap_text_with_style(text: &str, width: f32, style: &CssStyle) -> Vec<String> {
    wrap_text_with_initial_width(text, width, width, style)
}

pub(super) fn wrap_text_with_initial_width(
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
            .is_some_and(super::layout_text::is_css_whitespace)
    {
        return false;
    }
    let break_ends = css_auto_linebreak_ends(forced_line);
    if last_fitting_end(forced_line, &break_ends, initial_width, style).is_some() {
        return false;
    }
    let Some(first_break_end) = break_ends.first().copied() else {
        return false;
    };
    let first_segment =
        forced_line[..first_break_end].trim_end_matches(super::layout_text::is_css_whitespace);
    !first_segment.is_empty()
        && text_width(first_segment, style) <= continuing_width + TEXT_FIT_EPSILON
}

fn explicit_text_lines(text: &str) -> Vec<String> {
    text.split_terminator('\n')
        .map(|line| {
            line.trim_end_matches(super::layout_text::is_css_whitespace)
                .to_string()
        })
        .collect()
}

fn take_styled_line(text: &str, width: f32, style: &CssStyle) -> (String, usize) {
    if text.starts_with('\n') {
        return (String::new(), 1);
    }
    let forced_end = text.find('\n').unwrap_or(text.len());
    let forced_line = &text[..forced_end];
    let trimmed_line = forced_line.trim_end_matches(super::layout_text::is_css_whitespace);
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
            .trim_end_matches(super::layout_text::is_css_whitespace)
            .to_string(),
        fitted_end,
    )
}

pub(super) fn css_auto_linebreak_ends(text: &str) -> Vec<usize> {
    if !text.chars().any(is_conditional_japanese_starter) {
        return suppress_slash_letter_breaks(text, linebreaks(text).map(|(end, _)| end).collect());
    }
    /* WHY: Chromeのauto改行ではCJをIDとして扱い、他の禁則・結合制御は維持する。 */
    let mut normalized = String::with_capacity(text.len());
    let mut offsets = Vec::new();
    for (start, character) in text.char_indices() {
        normalized.push(if is_conditional_japanese_starter(character) {
            '一'
        } else {
            character
        });
        offsets.push((normalized.len(), start + character.len_utf8()));
    }
    let ends = linebreaks(&normalized)
        .filter_map(|(end, _)| {
            offsets
                .binary_search_by_key(&end, |(offset, _)| *offset)
                .ok()
                .map(|index| offsets[index].1)
        })
        .collect();
    suppress_slash_letter_breaks(text, ends)
}

fn suppress_slash_letter_breaks(text: &str, ends: Vec<usize>) -> Vec<usize> {
    ends.into_iter()
        .filter(|&end| {
            let Some('/') = text[..end].chars().next_back() else {
                return true;
            };
            !text[end..].chars().next().is_some_and(is_css_letter)
        })
        .collect()
}

/* CSS Textのslash/letter規則はGeneral Category Lを対象にする。直接の
 * category table依存がないため、現状はASCII letterを保守的に扱う。 */
fn is_css_letter(character: char) -> bool {
    character.is_ascii_alphabetic()
}

fn is_conditional_japanese_starter(character: char) -> bool {
    break_property(character as u32) == BreakClass::ConditionalJapaneseStarter
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
            text[..end].trim_end_matches(super::layout_text::is_css_whitespace),
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

pub(super) fn text_display_columns(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub(super) fn css_px(value: &str) -> Option<f32> {
    css_number(value).filter(|value| *value >= 0.0)
}

fn css_number(value: &str) -> Option<f32> {
    value
        .trim()
        .strip_suffix("px")
        .unwrap_or(value.trim())
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

pub(super) fn border_color(value: &str) -> Option<String> {
    let parts = css_value_components(value);
    parts
        .iter()
        .find(|part| part.starts_with('#') || part.starts_with("rgb"))
        .or_else(|| parts.iter().find(|part| is_named_border_color(part)))
        .map(|part| (*part).to_string())
}

fn css_value_components(value: &str) -> Vec<&str> {
    let mut components = Vec::new();
    let mut start = None;
    let mut depth = 0_usize;
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if character.is_ascii_whitespace() && depth == 0 {
            if let Some(component_start) = start.take() {
                components.push(&value[component_start..index]);
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(component_start) = start {
        components.push(&value[component_start..]);
    }
    components
}

fn is_named_border_color(value: &&str) -> bool {
    value.chars().all(char::is_alphabetic)
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "solid" | "dashed" | "dotted" | "double" | "none"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::backends::html_document::HtmlDocumentNode;
    use std::collections::HashMap;

    #[test]
    fn text_overflow_smaller_than_one_pixel_still_wraps() {
        let style = CssStyle::browser_default();
        let width = text_width("資料", &style) - 0.5;
        assert_eq!(wrap_text_with_style("資料", width, &style), ["資", "料"]);
    }

    #[test]
    fn auto_kana_breaks_match_chrome_without_breaking_joiner_controls() {
        let style = CssStyle::browser_default();
        let width = text_width("レビュ", &style) + TEXT_FIT_EPSILON;
        assert_eq!(
            wrap_text_with_style("レビュー中", width, &style),
            ["レビュ", "ー中"]
        );
        assert_eq!(
            super::css_auto_linebreak_ends("レ\u{2060}ュ"),
            ["レ\u{2060}ュ".len()]
        );
        assert!(!super::css_auto_linebreak_ends("日本、").contains(&"日本".len()));
        assert_eq!(
            super::css_auto_linebreak_ends("日本"),
            ["日".len(), "日本".len()]
        );
    }

    #[test]
    fn auto_line_break_does_not_split_slash_before_letters() {
        for text in ["https://alpha.example/path", "23/Jan/2024", "/src/main.rs"] {
            let ends = super::css_auto_linebreak_ends(text);
            assert!(
                !ends.iter().any(
                    |&end| text.as_bytes().get(end.wrapping_sub(1)) == Some(&b'/')
                        && text[end..].chars().next().is_some_and(is_css_letter)
                ),
                "{text:?}: {ends:?}"
            );
        }
        assert!(!super::css_auto_linebreak_ends("https://documents").contains(&"https:/".len()));
    }

    #[test]
    fn slash_breaks_before_non_letters_and_forced_lines_remain_available() {
        assert_eq!(super::suppress_slash_letter_breaks("a/1", vec![2]), [2]);
        let style = CssStyle::browser_default();
        let width = text_width("https://alpha.example/path", &style) + TEXT_FIT_EPSILON;
        assert_eq!(
            wrap_text_with_initial_width(
                "https://alpha.example/path",
                text_width("https://", &style) + TEXT_FIT_EPSILON,
                width,
                &style
            ),
            ["", "https://alpha.example/path"]
        );
        assert_eq!(
            wrap_text_with_initial_width("https://alpha\nbeta", width, width, &style),
            ["https://alpha", "beta"]
        );
    }

    #[test]
    fn structural_helpers_preserve_rows_text_and_empty_input_defaults() {
        assert_table_rows_preserve_cells();
        assert_input_defaults_are_seeded();
    }

    #[test]
    fn explicit_breaks_survive_html_whitespace_collapse_and_text_wrapping() {
        let nodes = [element(
            "h1",
            vec![
                HtmlDocumentNode::Text(" First   line ".to_string()),
                element("br", Vec::new()),
                HtmlDocumentNode::Text(" Second   line ".to_string()),
            ],
        )];

        let text = node_text(&nodes);
        assert_eq!(text, "First line\nSecond line");
        assert_eq!(wrap_text(&text, 400.0, 16.0), ["First line", "Second line"]);
        assert_eq!(
            wrap_text("first\n\nthird", 400.0, 16.0),
            ["first", "", "third"]
        );
        assert_eq!(
            wrap_text("日本語の長い文章です", 44.0, 10.0),
            ["日本語の", "長い文章", "です"]
        );
        assert_eq!(
            wrap_text("日本語、句読点。", 44.0, 10.0),
            ["日本語、", "句読点。"]
        );
        assert_eq!(wrap_text("Summary text", 105.0, 16.0), ["Summary text"]);
        assert_eq!(text_capacity(105.0, 16.0, TEXT_CHARACTER_WIDTH_FACTOR), 12);
        assert_eq!(text_capacity(104.0, 16.0, TEXT_CHARACTER_WIDTH_FACTOR), 11);
    }

    #[test]
    fn css_value_components_respects_nested_function_depth() {
        let components = css_value_components("  linear-gradient(red, blue)   1px solid  ");
        assert_eq!(
            components,
            vec!["linear-gradient(red, blue)", "1px", "solid"]
        );
    }

    #[test]
    fn styled_wrapping_uses_shaped_font_width_and_splits_long_segments() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Noto Sans".to_string();
        style.font_size = 42.842;
        style.letter_spacing = 0.42842;
        style.font_weight = 700;
        style.font_feature_settings = Some(r#""palt" 1"#.to_string());
        let title =
            "Fictional archive service records documents and classifications for public browsing";

        assert_eq!(
            wrap_text_with_style(
                title,
                text_width(
                    "Fictional archive service records documents and classifications",
                    &style
                ) + 0.5,
                &style
            ),
            [
                "Fictional archive service records documents and classifications",
                "for public browsing"
            ]
        );
        assert_eq!(wrap_text_with_style("abcdef", 1.0, &style).len(), 6);
        assert_eq!(
            wrap_text_with_style("first\n\nthird", 1230.0, &style),
            ["first", "", "third"]
        );
    }

    #[test]
    fn fitting_styled_line_keeps_its_complete_text() {
        let style = CssStyle::browser_default();
        let text = "性能回帰フィクスチャ 1744";
        let measured = text_width(text, &style);

        assert_eq!(wrap_text_with_style(text, measured + 1.0, &style), [text]);
    }

    #[test]
    fn fitting_line_preserves_consumed_newline_and_geometry() {
        let style = CssStyle::browser_default();
        let first_line = "性能回帰フィクスチャ 1744";
        let text = format!("{first_line}\n次の行");
        let width = text_width(first_line, &style) + 1.0;

        let (line, consumed) = take_styled_line(&text, width, &style);

        assert_eq!(line, first_line);
        assert_eq!(consumed, first_line.len() + "\n".len());
        assert!(text_width(&line, &style) <= width);
        assert_eq!(
            wrap_text_with_style(&text, width, &style),
            [first_line, "次の行"]
        );
    }

    #[test]
    fn first_unbreakable_token_moves_to_continuing_line_without_character_split() {
        let style = CssStyle::browser_default();
        let initial_width = text_width("v", &style) + TEXT_FIT_EPSILON;
        let continuing_width = text_width("v3.0", &style) + TEXT_FIT_EPSILON;

        assert_eq!(
            wrap_text_with_initial_width("v3.0", initial_width, continuing_width, &style),
            ["", "v3.0"]
        );
    }

    #[test]
    fn first_unbreakable_token_defers_even_when_following_words_need_another_line() {
        let style = CssStyle::browser_default();
        let initial_width = text_width("v", &style) + TEXT_FIT_EPSILON;
        let continuing_width = text_width("v3.0 remaining", &style) + TEXT_FIT_EPSILON;

        assert_eq!(
            wrap_text_with_initial_width(
                "v3.0 remaining words",
                initial_width,
                continuing_width,
                &style
            ),
            ["", "v3.0 remaining", "words"]
        );
    }

    #[test]
    fn leading_nbsp_keeps_the_nonbreaking_token_together_on_continuing_line() {
        let style = CssStyle::browser_default();
        let text = "\u{00a0}v3.0";
        let initial_width = text_width("v", &style) + TEXT_FIT_EPSILON;
        let continuing_width = text_width(text, &style) + TEXT_FIT_EPSILON;

        assert_eq!(
            wrap_text_with_initial_width(text, initial_width, continuing_width, &style),
            ["", text]
        );
    }

    #[test]
    fn mixed_japanese_latin_palt_wrap_uses_selected_face_metrics() {
        let mut style = CssStyle::browser_default();
        style.font_family =
            "\"Noto Sans JP\", \"Hiragino Kaku Gothic ProN\", \"Hiragino Sans\", \"Yu Gothic\", Meiryo, system-ui, sans-serif".to_string();
        style.font_size = 20.0;
        style.font_weight = 400;
        style.line_height = 33.0;
        style.font_feature_settings = Some(r#""palt" 1"#.to_string());
        let text = "架空の資料整理サービスでは、Local Index v2 による分類候補を確認し、担当者が登録内容と公開範囲を決定する";
        let first_line = "架空の資料整理サービスでは、";
        let measured = text_width(first_line, &style);
        let lines = wrap_text_with_style(text, measured + 0.5, &style);

        assert!(measured > 0.0, "{measured}");
        assert_eq!(lines.first().map(String::as_str), Some(first_line));
        assert!(lines.len() > 1, "{lines:?}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mixed_japanese_latin_palt_wrap_matches_mac_browser_line_rects() {
        let mut style = CssStyle::browser_default();
        style.font_family =
            "\"Noto Sans JP\", \"Hiragino Kaku Gothic ProN\", \"Hiragino Sans\", \"Yu Gothic\", Meiryo, system-ui, sans-serif".to_string();
        style.font_size = 20.0;
        style.font_weight = 400;
        style.line_height = 33.0;
        style.font_feature_settings = Some(r#""palt" 1"#.to_string());
        let text = "架空の資料整理サービスでは、Local Index v2 による分類候補を確認し、担当者が登録内容と公開範囲を決定する";

        assert!((text_width("架空の資料整理サービスでは、", &style) - 261.265_63).abs() < 0.75);
        assert_eq!(
            wrap_text_with_style(text, 288.671_88, &style),
            [
                "架空の資料整理サービスでは、",
                "Local Index v2 による分類候補",
                "を確認し、担当者が登録内容と公",
                "開範囲を決定する",
            ]
        );
    }

    #[test]
    fn nowrap_suppresses_soft_wraps_but_preserves_explicit_breaks() {
        let mut style = CssStyle::browser_default();
        style.white_space = CssWhiteSpace::NoWrap;

        assert_eq!(
            wrap_text_with_style("first second", 1.0, &style),
            ["first second"]
        );
        assert_eq!(
            wrap_text_with_style("first\nsecond", 1.0, &style),
            ["first", "second"]
        );
        assert_eq!(wrap_text_with_style("\n", 1.0, &style), [""]);
        assert_eq!(wrap_text_with_style("first\n", 1.0, &style), ["first"]);
        assert_eq!(wrap_text_with_style("\n\n", 1.0, &style), ["", ""]);
        assert_eq!(
            wrap_text_with_style("first\u{00a0} \t", 1.0, &style),
            ["first\u{00a0}"]
        );
    }

    #[test]
    fn inline_wrapping_uses_remaining_first_line_then_the_complete_width() {
        let style = CssStyle::browser_default();
        let lines = wrap_text_with_initial_width(
            " next remaining words",
            text_width(" next ", &style) + 0.5,
            text_width("remaining words", &style) + 0.5,
            &style,
        );

        assert_eq!(lines, [" next", "remaining words"]);
    }

    fn assert_table_rows_preserve_cells() {
        let rows = table_rows(&[
            HtmlDocumentNode::Text("ignored".to_string()),
            table_fixture(),
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 2);
        assert_eq!(rows[0][0].tag, "th");
        assert!(rows[0][0].attributes.is_empty());
        assert_eq!(
            node_text(&[element(
                "span",
                vec![HtmlDocumentNode::Text("Ready".to_string())],
            )]),
            "Ready"
        );
    }

    fn table_fixture() -> HtmlDocumentNode {
        element(
            "table",
            vec![
                HtmlDocumentNode::Text("ignored table text".to_string()),
                element(
                    "tbody",
                    vec![element(
                        "tr",
                        vec![
                            HtmlDocumentNode::Text("ignored row text".to_string()),
                            element("th", vec![HtmlDocumentNode::Text("Feature".to_string())]),
                            element("td", vec![HtmlDocumentNode::Text("Ready".to_string())]),
                        ],
                    )],
                ),
            ],
        )
    }

    fn assert_input_defaults_are_seeded() {
        let inputs = vec![element(
            "section",
            vec![
                element_with_id(1, "input", Vec::new()),
                element_with_id(2, "textarea", Vec::new()),
                element("p", vec![HtmlDocumentNode::Text("plain".to_string())]),
            ],
        )];
        let mut values = HashMap::new();
        seed_input_values(&inputs, &mut values);
        assert_eq!(values.len(), 2);
        assert!(values.values().all(String::is_empty));
        assert!(is_input_tag("textarea"));
        assert_eq!(input_initial_value(&[]), "");
    }

    fn element(tag: &str, children: Vec<HtmlDocumentNode>) -> HtmlDocumentNode {
        element_with_id(0, tag, children)
    }

    fn element_with_id(
        node_id: u64,
        tag: &str,
        children: Vec<HtmlDocumentNode>,
    ) -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id,
            tag: tag.to_string(),
            attributes: Vec::new(),
            children,
        }
    }
}
