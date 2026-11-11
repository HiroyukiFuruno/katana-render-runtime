use super::HtmlAttributes;

const MAX_CSS_ESCAPE_DIGITS: usize = 6;
const HEXADECIMAL_RADIX: u32 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::renderer::backends) enum CssGeneratedContent {
    Text(String),
    Image(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::renderer::backends) struct CssPseudoStyle {
    pub(in crate::renderer::backends) attributes: HtmlAttributes,
    pub(in crate::renderer::backends) content: CssGeneratedContent,
}

pub(super) fn parse_generated_content(value: &str) -> Option<CssGeneratedContent> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("normal") {
        return None;
    }
    if let Some(inner) = value
        .strip_prefix("url(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let source = inner.trim();
        let source = parse_css_string(source).unwrap_or_else(|| source.to_string());
        return (!source.is_empty()).then_some(CssGeneratedContent::Image(source));
    }
    parse_css_string(value).map(CssGeneratedContent::Text)
}

fn parse_css_string(value: &str) -> Option<String> {
    let quote = value.chars().next()?;
    if !matches!(quote, '\'' | '"') || !value.ends_with(quote) || value.len() < 2 {
        return None;
    }
    let inner = &value[quote.len_utf8()..value.len() - quote.len_utf8()];
    let mut characters = inner.chars().peekable();
    let mut parsed = String::with_capacity(inner.len());
    while let Some(character) = characters.next() {
        if character == '\\' {
            decode_css_escape(&mut characters, &mut parsed);
        } else {
            parsed.push(character);
        }
    }
    Some(parsed)
}

fn decode_css_escape(
    characters: &mut std::iter::Peekable<std::str::Chars<'_>>,
    parsed: &mut String,
) {
    let mut hexadecimal = String::new();
    while hexadecimal.len() < MAX_CSS_ESCAPE_DIGITS {
        let Some(character) = characters.next_if(|candidate| candidate.is_ascii_hexdigit()) else {
            break;
        };
        hexadecimal.push(character);
    }
    if hexadecimal.is_empty() {
        if let Some(escaped) = characters.next() {
            parsed.push(escaped);
        }
        return;
    }
    consume_escape_whitespace(characters);
    if let Ok(codepoint) = u32::from_str_radix(&hexadecimal, HEXADECIMAL_RADIX)
        && let Some(decoded) = char::from_u32(codepoint)
    {
        parsed.push(decoded);
    }
}

fn consume_escape_whitespace(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    if characters
        .peek()
        .is_some_and(|candidate| candidate.is_ascii_whitespace())
    {
        characters.next();
    }
}

#[cfg(test)]
mod tests {
    use super::{CssGeneratedContent, parse_generated_content};

    #[test]
    fn generated_content_requires_a_nonempty_url() {
        for source in ["", "url()", "url(  )", "url('')"] {
            assert_eq!(parse_generated_content(source), None, "{source}");
        }
    }

    #[test]
    fn generated_content_parses_strings_escapes_urls_and_suppression() {
        assert_eq!(
            parse_generated_content(r#""\2713 ready""#),
            Some(CssGeneratedContent::Text("✓ready".to_string()))
        );
        assert_eq!(
            parse_generated_content("url('data:image/svg+xml,<svg/>')"),
            Some(CssGeneratedContent::Image(
                "data:image/svg+xml,<svg/>".to_string()
            ))
        );
        assert_eq!(
            parse_generated_content("url(icon.svg)"),
            Some(CssGeneratedContent::Image("icon.svg".to_string()))
        );
        assert_eq!(
            parse_generated_content("''"),
            Some(CssGeneratedContent::Text(String::new()))
        );
        assert_eq!(parse_generated_content("normal"), None);
        assert_eq!(parse_generated_content("none"), None);
        assert_eq!(parse_generated_content("counter(item)"), None);
    }

    #[test]
    fn generated_content_uses_non_hex_escape_character_as_literal() {
        assert_eq!(
            parse_generated_content(r#""\!""#),
            Some(CssGeneratedContent::Text("!".to_string()))
        );
    }

    #[test]
    fn generated_content_uses_hex_escape_character_with_trailing_whitespace() {
        assert_eq!(
            parse_generated_content(r#""\61 abc""#),
            Some(CssGeneratedContent::Text("aabc".to_string()))
        );
    }

    #[test]
    fn generated_content_drops_dangling_escape_at_end_of_string() {
        assert_eq!(
            parse_generated_content(r#""\""#),
            Some(CssGeneratedContent::Text(String::new()))
        );
    }

    #[test]
    fn generated_content_ignores_invalid_unicode_escape_codepoint() {
        assert_eq!(
            parse_generated_content(r#""\110000""#),
            Some(CssGeneratedContent::Text(String::new()))
        );
    }

    #[test]
    fn generated_content_preserves_character_after_hex_escape_without_whitespace() {
        assert_eq!(
            parse_generated_content(r#""\61z""#),
            Some(CssGeneratedContent::Text("az".to_string()))
        );
    }
}
