use cssparser::{ParseError, Parser, Token};

pub(crate) struct CssParserShared;

impl CssParserShared {
    pub(crate) fn consume_nested(input: &mut Parser<'_>) -> Result<(), ParseError<()>> {
        while !input.is_exhausted() {
            let token = input.next_including_whitespace_and_comments()?.clone();
            if matches!(
                token,
                Token::Function(_)
                    | Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock
            ) {
                input.parse_nested_block(Self::consume_nested)?;
            }
        }
        Ok(())
    }
}
