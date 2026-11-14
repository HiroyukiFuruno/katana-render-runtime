#[cfg(test)]
use super::constants::MIN_LAYOUT_WIDTH;
#[cfg(test)]
use super::constants::TEXT_CHARACTER_WIDTH_FACTOR;
#[cfg(test)]
use super::style::{CssStyle, CssWhiteSpace};
#[cfg(test)]
use super::text_metrics::text_width;
#[cfg(test)]
use unicode_linebreak::linebreaks;
use unicode_width::UnicodeWidthStr;

#[path = "document_line_break.rs"]
mod line_break;
#[path = "document_text_wrap.rs"]
mod text_wrap;

pub(super) use line_break::css_auto_linebreak_ends;
#[cfg(test)]
use line_break::{is_css_letter, suppress_slash_letter_breaks};
#[cfg(test)]
use text_wrap::{TEXT_FIT_EPSILON, take_styled_line};
pub(super) use text_wrap::{wrap_text_with_initial_width, wrap_text_with_style};

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
            let forbidden_ends = text
                .match_indices('/')
                .map(|(offset, slash)| offset + slash.len())
                .filter(|&end| text[end..].chars().next().is_some_and(is_css_letter));
            for end in forbidden_ends {
                assert!(!ends.contains(&end), "{text:?}: {ends:?}");
            }
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
        assert!(seed_input_values(&inputs, &mut values, &|_| Ok(String::new())).is_ok());
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
