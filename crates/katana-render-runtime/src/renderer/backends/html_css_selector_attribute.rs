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

#[cfg(test)]
mod tests {
    use super::super::CssSelector;

    #[test]
    fn attribute_selector_requires_identifier_name() -> Result<(), String> {
        for source in ["[]", "[123]", "[=ready]"] {
            assert!(CssSelector::parse(source).is_none(), "{source}");
        }
        let selector = CssSelector::parse("[DATA-state]").ok_or("attribute selector")?;
        let attributes = vec![("data-state".to_string(), String::new())];
        assert!(selector.matches("div", &attributes, &[]));
        assert!(!selector.matches("div", &Vec::new(), &[]));
        Ok(())
    }

    #[test]
    fn attribute_equals_requires_a_value() -> Result<(), String> {
        for source in ["[data-state=]", "[data-state=  ]"] {
            assert!(CssSelector::parse(source).is_none(), "{source}");
        }
        let selector = CssSelector::parse("[data-state=ready]").ok_or("equals selector")?;
        let ready = vec![("data-state".to_string(), "ready".to_string())];
        let other = vec![("data-state".to_string(), "READY".to_string())];
        assert!(selector.matches("div", &ready, &[]));
        assert!(!selector.matches("div", &other, &[]));
        Ok(())
    }

    #[test]
    fn attribute_prefix_requires_a_value() -> Result<(), String> {
        assert!(CssSelector::parse("[data-code^=]").is_none());
        let selector = CssSelector::parse("[data-code^=ab]").ok_or("prefix selector")?;
        let matching = vec![("data-code".to_string(), "abc".to_string())];
        let other = vec![("data-code".to_string(), "cab".to_string())];
        assert!(selector.matches("div", &matching, &[]));
        assert!(!selector.matches("div", &other, &[]));
        Ok(())
    }
}
