use super::{CssAttributeMatch, CssAttributeSelector};
use cssparser::{Parser, Token};

pub(super) fn parse_attribute(source: &str) -> Option<(CssAttributeSelector, &str)> {
    let mut parser = Parser::new(source);
    parser.expect_square_bracket_block().ok()?;
    let selector = parser.parse_nested_block(parse_attribute_contents).ok()?;
    let end = parser.position().byte_index();
    Some((selector, &source[end..]))
}

fn parse_attribute_contents(
    parser: &mut Parser<'_>,
) -> Result<CssAttributeSelector, cssparser::ParseError<()>> {
    let name = parser.expect_ident_cloned()?;
    let matching = if parser.is_exhausted() {
        CssAttributeMatch::Exists
    } else {
        let is_prefix = match parser.next()? {
            Token::PrefixMatch => true,
            Token::Delim('=') => false,
            _ => return Err(cssparser::ParseError::unexpected_token()),
        };
        let value = parser.expect_ident_or_string()?.to_string();
        parser.expect_exhausted()?;
        if is_prefix {
            CssAttributeMatch::Prefix(value)
        } else {
            CssAttributeMatch::Equals(value)
        }
    };
    Ok(CssAttributeSelector {
        name: name.to_string().to_ascii_lowercase(),
        matching,
    })
}
