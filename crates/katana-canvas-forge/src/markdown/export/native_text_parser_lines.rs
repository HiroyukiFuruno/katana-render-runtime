use super::native_text::{
    CODE_FENCE_MARKER, HEADING_COLUMN_SIZE_H1, HEADING_COLUMN_SIZE_H2, HEADING_COLUMN_SIZE_H3,
    HEADING_END_MARKER, HEADING_LEVEL_1, HEADING_LEVEL_2, HEADING_LEVEL_3, HEADING_LEVEL_4,
    HEADING_LEVEL_5, HEADING_LEVEL_6, HEADING_SEP_MARKER, HEADING_START_MARKER, NativeTextLine,
    TEXT_COLUMNS, WORD_SPACING,
};
use super::native_text_parser_html::{decode_entities, strip_tags};

pub(super) fn parse_typed_lines(
    text: &str,
    code_blocks: &[Vec<NativeTextLine>],
) -> Vec<NativeTextLine> {
    let mut result = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(code_lines) = parse_code_fence(line, code_blocks) {
            result.extend(code_lines.iter().cloned());
            continue;
        }
        if let Some(wrapped) = parse_heading_line(line) {
            result.extend(wrapped);
            continue;
        }

        append_wrapped_body_lines(line, &mut result);
    }
    result
}

fn parse_code_fence<'a>(
    line: &'a str,
    code_blocks: &'a [Vec<NativeTextLine>],
) -> Option<&'a Vec<NativeTextLine>> {
    if !line.starts_with(CODE_FENCE_MARKER)
        || !line.ends_with(CODE_FENCE_MARKER)
        || line.len() <= CODE_FENCE_MARKER.len()
    {
        return None;
    }
    let inner = &line[CODE_FENCE_MARKER.len()..line.len() - CODE_FENCE_MARKER.len()];
    let Ok(idx) = inner.parse::<usize>() else {
        return None;
    };
    code_blocks.get(idx)
}

fn parse_heading_line(line: &str) -> Option<Vec<NativeTextLine>> {
    if !line.starts_with(HEADING_START_MARKER) {
        return None;
    }
    let rest = &line[HEADING_START_MARKER.len()..];
    let sep_pos = rest.find(HEADING_SEP_MARKER)?;
    let level_str = &rest[..sep_pos];
    let after_sep = &rest[sep_pos + HEADING_SEP_MARKER.len()..];
    let content_raw = after_sep.trim_end_matches(HEADING_END_MARKER).trim();
    let level = heading_level(level_str)?;
    let clean = strip_tags(content_raw);
    let text = decode_entities(&clean);
    let mut rows = Vec::new();
    for wrapped in wrap_line_n(&text, heading_columns(level)) {
        rows.push(NativeTextLine::heading(wrapped, level));
    }
    Some(rows)
}

fn heading_level(level_str: &str) -> Option<u8> {
    match level_str {
        "h1" => Some(HEADING_LEVEL_1),
        "h2" => Some(HEADING_LEVEL_2),
        "h3" => Some(HEADING_LEVEL_3),
        "h4" => Some(HEADING_LEVEL_4),
        "h5" => Some(HEADING_LEVEL_5),
        "h6" => Some(HEADING_LEVEL_6),
        _ => None,
    }
}

fn heading_columns(level: u8) -> usize {
    match level {
        HEADING_LEVEL_1 => HEADING_COLUMN_SIZE_H1,
        HEADING_LEVEL_2 => HEADING_COLUMN_SIZE_H2,
        HEADING_LEVEL_3 => HEADING_COLUMN_SIZE_H3,
        _ => TEXT_COLUMNS,
    }
}

fn append_wrapped_body_lines(line: &str, result: &mut Vec<NativeTextLine>) {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    for wrapped in wrap_line_n(&normalized, TEXT_COLUMNS) {
        result.push(NativeTextLine::body(wrapped));
    }
}

fn wrap_line_n(line: &str, columns: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let current_width = current.chars().count();
        let word_width = word.chars().count();
        if should_wrap_line(current_width, word_width, columns) {
            rows.push(std::mem::take(&mut current));
        }
        append_word(&mut current, word);
    }
    if current.is_empty() {
        return rows;
    }
    rows.push(current);
    rows
}

fn should_wrap_line(current_width: usize, word_width: usize, columns: usize) -> bool {
    if current_width == 0 {
        return false;
    }
    current_width + word_width + WORD_SPACING > columns
}

fn append_word(current: &mut String, word: &str) {
    if current.is_empty() {
        current.push_str(word);
        return;
    }
    current.push(' ');
    current.push_str(word);
}

#[cfg(test)]
#[path = "native_text_parser_lines_tests.rs"]
mod tests;
