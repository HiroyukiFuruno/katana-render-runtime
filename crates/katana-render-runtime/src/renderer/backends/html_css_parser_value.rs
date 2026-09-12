use super::consume_nested;
use cssparser::{CowRcStr, ParseError, Parser, Token};

pub(super) fn parse_declaration_value(
    input: &mut Parser<'_>,
) -> Result<(String, bool), ParseError<()>> {
    let start = input.position();
    let mut value_end = start;
    let mut important = false;
    while !input.is_exhausted() {
        let token_start = input.state();
        let token = input.next_including_whitespace_and_comments()?.clone();
        value_end = input.position();
        if is_important_delimiter(&token, input) {
            value_end = token_start.position();
            important = true;
            break;
        }
        if is_nested_block(&token) {
            input.parse_nested_block(consume_nested)?;
            value_end = input.position();
        }
    }
    let value = input.slice(start..value_end).trim().to_string();
    if value.is_empty() {
        return Err(ParseError::custom(()));
    }
    Ok((value, important))
}

pub(super) fn normalized_property_name(name: CowRcStr<'_>) -> String {
    if name.starts_with("--") {
        name.to_string()
    } else {
        name.to_ascii_lowercase()
    }
}

fn is_important_delimiter(token: &Token<'_>, input: &mut Parser<'_>) -> bool {
    *token == Token::Delim('!')
        && input
            .try_parse(|tail| {
                tail.expect_ident_matching("important")?;
                tail.expect_exhausted()
            })
            .is_ok()
}

fn is_nested_block(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
    )
}
