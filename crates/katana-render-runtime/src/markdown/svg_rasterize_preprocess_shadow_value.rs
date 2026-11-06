use std::ops::Range;

use super::{consume_nested, is_nested_token};
use cssparser::{ParseError, Parser, Token};
use svgtypes::Color;

const LEGACY_RGBA_COMMAS: usize = 3;

struct ColorReplacement {
    range: Range<usize>,
    value: String,
}

pub(super) fn scan_filter_value(
    input: &mut Parser<'_>,
    inside_drop_shadow: bool,
    edits: &mut Vec<Range<usize>>,
) -> Result<usize, ParseError<()>> {
    let mut commas = 0;
    while !input.is_exhausted() {
        let start = input.position().byte_index();
        let token = input.next_including_whitespace_and_comments()?.clone();
        match token {
            Token::Function(name) => scan_function(input, start, &name, inside_drop_shadow, edits)?,
            Token::Comma => commas += 1,
            token if is_nested_token(&token) => input.parse_nested_block(consume_nested)?,
            _ => {}
        }
    }
    Ok(commas)
}

fn scan_function(
    input: &mut Parser<'_>,
    start: usize,
    name: &str,
    inside_drop_shadow: bool,
    edits: &mut Vec<Range<usize>>,
) -> Result<(), ParseError<()>> {
    let is_drop_shadow = name.eq_ignore_ascii_case("drop-shadow");
    let is_rgba = name.eq_ignore_ascii_case("rgba");
    let commas =
        input.parse_nested_block(|nested| scan_filter_value(nested, is_drop_shadow, edits))?;
    if inside_drop_shadow && is_rgba && commas != LEGACY_RGBA_COMMAS {
        return Err(ParseError::unexpected_token());
    }
    if inside_drop_shadow && is_rgba {
        edits.push(start..input.position().byte_index());
    }
    Ok(())
}

pub(super) fn apply_rgba_edits(source: &str, mut edits: Vec<Range<usize>>) -> Option<String> {
    edits.sort_by_key(|range| range.start);
    if edits.iter().any(|range| !valid_range(source, range))
        || edits.windows(2).any(|pair| pair[0].end > pair[1].start)
    {
        return None;
    }
    let mut replacements = parse_color_replacements(source, edits)?;
    let mut output = source.to_owned();
    for replacement in replacements.drain(..).rev() {
        output.replace_range(replacement.range, &replacement.value);
    }
    Some(output)
}

fn valid_range(source: &str, range: &Range<usize>) -> bool {
    range.start < range.end && source.get(range.clone()).is_some()
}

fn parse_color_replacements(
    source: &str,
    edits: Vec<Range<usize>>,
) -> Option<Vec<ColorReplacement>> {
    edits
        .into_iter()
        .map(|range| {
            let color = source.get(range.clone())?.parse::<Color>().ok()?;
            let replacement = format!(
                "#{:02X}{:02X}{:02X}{:02X}",
                color.red, color.green, color.blue, color.alpha
            );
            Some(ColorReplacement {
                range,
                value: replacement,
            })
        })
        .collect()
}

#[cfg(test)]
mod boundary_tests {
    use super::apply_rgba_edits;

    #[test]
    fn rgba_edit_ranges_reject_invalid_utf8_and_overlap() {
        assert!(apply_rgba_edits("é", std::iter::once(0..1).collect()).is_none());
        let source = "rgba(0,0,0,.5)";
        assert!(apply_rgba_edits(source, vec![0..source.len(), 0..source.len()]).is_none());
        assert_eq!(
            apply_rgba_edits("none", Vec::new()),
            Some("none".to_owned())
        );
    }
}
