use std::ops::Range;

use cssparser::{Parser, Token};

use super::consume_nested;

pub(super) fn rewrite_member(source: &str, max_output_bytes: usize) -> Option<String> {
    if !source.as_bytes().contains(&b'\\') {
        return (source.len() <= max_output_bytes).then(|| source.to_owned());
    }
    let mut parser = Parser::new(source);
    let mut id_count = 0;
    let mut edit = None;
    while !parser.is_exhausted() {
        inspect_next_token(source, &mut parser, &mut id_count, &mut edit)?;
    }
    let Some(edit) = edit else {
        return (source.len() <= max_output_bytes).then(|| source.to_owned());
    };
    let output_size = source.len() - edit.range.len() + edit.replacement.len();
    if output_size > max_output_bytes.min(super::member::MAX_REWRITTEN_SELECTOR_BYTES) {
        return None;
    }
    let mut output = source.to_owned();
    output.replace_range(edit.range, &edit.replacement);
    Some(output)
}

fn inspect_next_token(
    source: &str,
    parser: &mut Parser<'_>,
    id_count: &mut usize,
    edit: &mut Option<ValueEdit>,
) -> Option<()> {
    let (start, token_kind) = super::token::next_member_token(parser)?;
    match token_kind {
        super::token::MemberToken::Attribute => {
            inspect_attribute(source, parser, start, id_count, edit)
        }
        super::token::MemberToken::Other => {
            let end = parser.position().byte_index();
            (!source.get(start..end)?.as_bytes().contains(&b'\\')).then_some(())
        }
    }
}

fn inspect_attribute(
    source: &str,
    parser: &Parser<'_>,
    start: usize,
    id_count: &mut usize,
    edit: &mut Option<ValueEdit>,
) -> Option<()> {
    let end = parser.position().byte_index();
    let raw = source.get(start..end)?;
    if raw.as_bytes().last() != Some(&b']') {
        return None;
    }
    let Some(value) = parse_id_attribute(raw)? else {
        if raw.as_bytes().contains(&b'\\') {
            return None;
        }
        return Some(());
    };
    if value.had_escape
        && (raw.get(..value.range.start) != Some("[id=") || raw.get(value.range.end..) != Some("]"))
    {
        return None;
    }
    record_exact_id(value, start, id_count, edit)
}

fn record_exact_id(
    value: ExactValue,
    attribute_start: usize,
    id_count: &mut usize,
    edit: &mut Option<ValueEdit>,
) -> Option<()> {
    *id_count += 1;
    if *id_count > 1 {
        return None;
    }
    if !value.had_escape {
        return Some(());
    }
    if !is_simplecss_literal(&value.decoded) {
        return None;
    }
    let mut serialized = String::new();
    cssparser::serialize_string(&value.decoded, &mut serialized).ok()?;
    *edit = Some(ValueEdit {
        range: attribute_start + value.range.start..attribute_start + value.range.end,
        replacement: xml_escape_selector(&serialized),
    });
    Some(())
}

struct ValueEdit {
    range: Range<usize>,
    replacement: String,
}

struct ExactValue {
    decoded: String,
    range: Range<usize>,
    had_escape: bool,
}

fn parse_id_attribute(source: &str) -> Option<Option<ExactValue>> {
    let mut parser = Parser::new(source);
    if !matches!(parser.next().ok()?, Token::SquareBracketBlock) {
        return None;
    }
    parser.parse_nested_block(parse_id_attribute_contents).ok()
}

fn parse_id_attribute_contents(
    input: &mut Parser<'_>,
) -> Result<Option<ExactValue>, cssparser::ParseError<()>> {
    input.skip_whitespace();
    let name = match input.next()?.clone() {
        Token::Ident(name) => name,
        _ => return Err(cssparser::ParseError::unexpected_token()),
    };
    if name.as_ref() != "id" {
        consume_nested(input)?;
        return Ok(None);
    }
    parse_id_equals_value(input).map(Some)
}

fn parse_id_equals_value(input: &mut Parser<'_>) -> Result<ExactValue, cssparser::ParseError<()>> {
    input.skip_whitespace();
    if !matches!(input.next()?, Token::Delim('=')) {
        return Err(cssparser::ParseError::unexpected_token());
    }
    input.skip_whitespace();
    let start_position = input.position();
    let decoded = match input.next()?.clone() {
        Token::QuotedString(value) => value.to_string(),
        _ => return Err(cssparser::ParseError::unexpected_token()),
    };
    let end_position = input.position();
    let start = start_position.byte_index();
    let end = end_position.byte_index();
    let had_escape = input
        .slice(start_position..end_position)
        .as_bytes()
        .contains(&b'\\');
    input.skip_whitespace();
    if !input.is_exhausted() {
        return Err(cssparser::ParseError::unexpected_token());
    }
    Ok(ExactValue {
        decoded,
        range: start..end,
        had_escape,
    })
}

pub(super) fn is_simplecss_literal(value: &str) -> bool {
    value.chars().all(|character| {
        !matches!(character, '\\' | '"' | '\n' | '\r' | '\u{000c}') && !character.is_control()
    })
}

pub(super) fn xml_escape_selector(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
    output
}

#[cfg(test)]
mod boundary_tests {
    use super::{inspect_next_token, parse_id_attribute, record_exact_id, rewrite_member};
    use cssparser::Parser;

    #[test]
    fn exact_selector_rejects_colon_tokens_and_unclosed_attributes() {
        let mut parser = Parser::new(":hover");
        assert!(inspect_next_token(":hover", &mut parser, &mut 0, &mut None).is_none());
        assert!(rewrite_member(r#"[id="a\26""#, usize::MAX).is_none());
    }

    #[test]
    fn exact_attribute_parser_rejects_non_attributes_and_numeric_names() {
        assert!(parse_id_attribute("rect").is_none());
        assert!(parse_id_attribute("[7]").is_none());
    }

    #[test]
    fn plain_exact_id_does_not_schedule_a_replacement() -> Result<(), String> {
        let value = parse_id_attribute("[id='plain']")
            .flatten()
            .ok_or("plain quoted ID did not parse")?;
        let mut count = 0;
        let mut edit = None;
        assert_eq!(record_exact_id(value, 0, &mut count, &mut edit), Some(()));
        assert_eq!(count, 1);
        assert!(edit.is_none());
        Ok(())
    }
    #[test]
    fn escaped_trailing_comment_preserves_bytes_within_budget() {
        let source = r"rect/* \26 */";
        assert_eq!(
            rewrite_member(source, source.len()),
            Some(source.to_owned())
        );
        assert!(rewrite_member(source, 0).is_none());
    }
}
