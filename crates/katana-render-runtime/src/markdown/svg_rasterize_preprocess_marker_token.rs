use cssparser::{Parser, Token};

use super::consume_nested;

pub(super) enum MemberToken {
    Attribute,
    Other,
}

pub(super) fn next_member_token(parser: &mut Parser<'_>) -> Option<(usize, MemberToken)> {
    let start = parser.position().byte_index();
    let token = parser
        .next_including_whitespace_and_comments()
        .ok()?
        .clone();
    match token {
        Token::Function(_)
        | Token::ParenthesisBlock
        | Token::CurlyBracketBlock
        | Token::Colon
        | Token::BadString(_)
        | Token::BadUrl(_) => None,
        Token::SquareBracketBlock => {
            parser.parse_nested_block(consume_nested).ok()?;
            Some((start, MemberToken::Attribute))
        }
        _ => Some((start, MemberToken::Other)),
    }
}
