use cssparser::{Parser, Token};

pub(super) fn consume_nested(input: &mut Parser<'_>) -> Result<(), cssparser::ParseError<()>> {
    crate::css_parser_shared::CssParserShared::consume_nested(input)
}

#[path = "svg_rasterize_preprocess_marker_token.rs"]
mod token;

#[path = "svg_rasterize_preprocess_marker_exact.rs"]
mod exact;
#[path = "svg_rasterize_preprocess_marker_selector_member.rs"]
mod member;

const MAX_SELECTOR_MEMBERS: usize = 512;

struct MemberEdit {
    range: std::ops::Range<usize>,
    replacement: String,
}

pub(super) fn rewrite_selector_list(
    source: &str,
    ids: &[String],
    max_output_bytes: usize,
) -> Option<String> {
    let replacements = collect_members(source, ids, max_output_bytes)?;
    let mut output = source.to_owned();
    for edit in replacements.into_iter().rev() {
        output.replace_range(edit.range, &edit.replacement);
    }
    (output != source).then_some(output)
}

fn collect_members(
    source: &str,
    ids: &[String],
    max_output_bytes: usize,
) -> Option<Vec<MemberEdit>> {
    let mut parser = Parser::new(source);
    let mut replacements = Vec::new();
    let (mut start, mut output_size) = (0, source.len());
    while !parser.is_exhausted() {
        collect_next_member(
            source,
            ids,
            max_output_bytes,
            &mut parser,
            &mut start,
            &mut output_size,
            &mut replacements,
        )?;
    }
    push_member(
        source,
        start..source.len(),
        ids,
        max_output_bytes,
        &mut output_size,
        &mut replacements,
    )?;
    Some(replacements)
}

fn collect_next_member(
    source: &str,
    ids: &[String],
    max_output_bytes: usize,
    parser: &mut Parser<'_>,
    start: &mut usize,
    output_size: &mut usize,
    replacements: &mut Vec<MemberEdit>,
) -> Option<()> {
    let token_start = parser.position().byte_index();
    let token = parser
        .next_including_whitespace_and_comments()
        .ok()?
        .clone();
    if is_block_token(&token) {
        parser.parse_nested_block(consume_nested).ok()?;
    } else if matches!(token, Token::Comma) {
        push_member(
            source,
            *start..token_start,
            ids,
            max_output_bytes,
            output_size,
            replacements,
        )?;
        *start = parser.position().byte_index();
    }
    Some(())
}

fn is_block_token(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
    )
}

fn push_member(
    source: &str,
    range: std::ops::Range<usize>,
    ids: &[String],
    max_output_bytes: usize,
    output_size: &mut usize,
    replacements: &mut Vec<MemberEdit>,
) -> Option<()> {
    if replacements.len() >= MAX_SELECTOR_MEMBERS {
        return None;
    }
    let raw = source.get(range.clone())?;
    let remaining_source_size = output_size.checked_sub(range.len())?;
    let member_budget = max_output_bytes.checked_sub(remaining_source_size)?;
    let replacement = member::rewrite_member(raw, ids, member_budget)?;
    /* WHY: 置換結果はメンバーに割り当てた残容量以内に制限済み。 */
    *output_size = remaining_source_size + replacement.len();
    replacements.push(MemberEdit { range, replacement });
    Some(())
}

pub(super) fn parse_suffix_value(
    input: &mut Parser<'_>,
) -> Result<Option<String>, cssparser::ParseError<()>> {
    input.skip_whitespace();
    let value = match input.next()?.clone() {
        Token::Ident(value) | Token::QuotedString(value) => value.to_string(),
        _ => return Ok(None),
    };
    input.skip_whitespace();
    if !input.is_exhausted() {
        return Err(cssparser::ParseError::unexpected_token());
    }
    Ok(Some(value))
}

pub(super) fn parse_escaped_exact_attribute(
    input: &mut Parser<'_>,
    escaped_exact_seen: &mut bool,
) -> Result<Option<String>, cssparser::ParseError<()>> {
    recognize_escaped_exact_id(input, escaped_exact_seen)?;
    Ok(None)
}

pub(super) fn recognize_escaped_exact_id(
    input: &mut Parser<'_>,
    escaped_exact_seen: &mut bool,
) -> Result<(), cssparser::ParseError<()>> {
    input.skip_whitespace();
    let start = input.position();
    let value = match input.next()?.clone() {
        Token::QuotedString(value) => value.to_string(),
        _ => return Err(cssparser::ParseError::unexpected_token()),
    };
    let end = input.position();
    let has_escape = input.slice(start..end).as_bytes().contains(&b'\\');
    input.skip_whitespace();
    if !has_escape
        || *escaped_exact_seen
        || !input.is_exhausted()
        || !exact::is_simplecss_literal(&value)
    {
        return Err(cssparser::ParseError::unexpected_token());
    }
    *escaped_exact_seen = true;
    Ok(())
}

#[cfg(test)]
#[path = "svg_rasterize_preprocess_marker_exact_unit_tests.rs"]
mod exact_unit_tests;

#[cfg(test)]
mod boundary_tests {
    use super::parse_suffix_value;
    use cssparser::Parser;

    #[test]
    fn non_string_suffix_value_is_unsupported() {
        let mut parser = Parser::new("7");
        assert!(parse_suffix_value(&mut parser).is_ok_and(|value| value.is_none()));
    }
}
