use std::collections::BTreeSet;

use super::entities::decode_xml_entities;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct HtmlFontRequest {
    pub(super) families: Vec<String>,
    pub(super) needs_cjk: bool,
    pub(super) needs_emoji: bool,
    pub(super) needs_unicode_fallback: bool,
}

impl HtmlFontRequest {
    pub(super) fn from_markup(markup: &str) -> Self {
        let mut families = BTreeSet::new();
        for value in font_family_values(markup) {
            for family in value.split(',') {
                let decoded_family = decode_xml_entities(family);
                let family = decoded_family.trim().trim_matches(['\'', '"']);
                if !family.is_empty() {
                    families.insert(family.to_lowercase());
                }
            }
        }
        Self {
            families: families.into_iter().collect(),
            needs_cjk: markup.chars().any(is_cjk_character),
            needs_emoji: markup.chars().any(is_emoji_character),
            needs_unicode_fallback: markup.chars().any(needs_unicode_fallback),
        }
    }

    pub(super) fn from_text(font_family: &str, text: &str) -> Self {
        let mut request = Self::from_markup(font_family);
        if request.families.is_empty() {
            request.families = normalized_font_families(font_family);
        }
        request.needs_cjk = text.chars().any(is_cjk_character);
        request.needs_emoji = text.chars().any(is_emoji_character);
        request.needs_unicode_fallback = text.chars().any(needs_unicode_fallback);
        request
    }
}

fn normalized_font_families(font_family: &str) -> Vec<String> {
    font_family
        .split(',')
        .map(str::trim)
        .map(decode_xml_entities)
        .map(|family| family.trim().trim_matches(['\'', '"']).to_string())
        .filter(|family| !family.is_empty())
        .map(|family| family.to_lowercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn font_family_values(markup: &str) -> Vec<&str> {
    let lowercase = markup.to_ascii_lowercase();
    let mut values = Vec::new();
    collect_font_family_values(markup, &lowercase, "font-family=", true, &mut values);
    collect_font_family_values(markup, &lowercase, "font-family:", false, &mut values);
    values
}

fn collect_font_family_values<'a>(
    markup: &'a str,
    lowercase: &str,
    needle: &str,
    attribute_value: bool,
    values: &mut Vec<&'a str>,
) {
    let mut search_start = 0;
    while let Some(relative_index) = lowercase[search_start..].find(needle) {
        let value_start = search_start + relative_index + needle.len();
        let value = markup[value_start..].trim_start();
        let value = font_family_value(value, attribute_value);
        values.push(value);
        search_start = value_start;
    }
}

fn font_family_value(value: &str, attribute_value: bool) -> &str {
    if !attribute_value {
        return before_declaration_boundary(value).trim_matches(['\'', '"']);
    }
    match value.as_bytes().first().copied() {
        Some(quote) if matches!(quote, b'\'' | b'"') => quoted_attribute_value(&value[1..], quote),
        _ => bare_attribute_value(value),
    }
}

fn quoted_attribute_value(value: &str, quote: u8) -> &str {
    value
        .split_once(char::from(quote))
        .map_or(value, |(family, _)| family)
}

fn bare_attribute_value(value: &str) -> &str {
    value
        .split(|character: char| {
            character.is_ascii_whitespace() || matches!(character, ';' | '>' | '/')
        })
        .next()
        .unwrap_or("")
}

fn before_declaration_boundary(value: &str) -> &str {
    let value = value.split([';', '>']).next().unwrap_or("").trim_end();
    let unmatched_quote = (*b"'\"")
        .into_iter()
        .filter(|&quote| {
            value
                .bytes()
                .filter(|&character| character == quote)
                .count()
                % 2
                == 1
        })
        .filter_map(|quote| value.bytes().position(|character| character == quote))
        .min();
    unmatched_quote.map_or(value, |index| &value[..index])
}

fn needs_unicode_fallback(character: char) -> bool {
    !character.is_ascii() && !is_cjk_character(character) && !is_emoji_character(character)
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character as u32,
        0x2E80..=0x2EFF
            | 0x3000..=0x303F
            | 0x3040..=0x30FF
            | 0x3100..=0x312F
            | 0x3130..=0x318F
            | 0x31A0..=0x31BF
            | 0x31C0..=0x31EF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFF00..=0xFFEF
            | 0x20000..=0x2FA1F
    )
}

fn is_emoji_character(character: char) -> bool {
    matches!(
        character as u32,
        0x2300..=0x23FF | 0x2600..=0x27BF | 0x1F000..=0x1FAFF
    )
}

#[cfg(test)]
mod tests {
    use super::{HtmlFontRequest, font_family_value, is_cjk_character};

    #[test]
    fn parses_css_and_bare_attribute_font_family_values() {
        assert_eq!(
            font_family_value("'Noto Sans'; color: red", false),
            "Noto Sans"
        );
        assert_eq!(font_family_value("Arial class=label", true), "Arial");
    }

    #[test]
    fn stops_css_font_family_at_the_end_of_an_inline_style_attribute() {
        assert_eq!(font_family_value("Arial\" class=label", false), "Arial");
        assert_eq!(font_family_value("\"Noto Sans\"'", false), "Noto Sans");
        assert_eq!(
            font_family_value("Arial, \"Noto Sans\">", false),
            "Arial, \"Noto Sans"
        );
        assert_eq!(
            HtmlFontRequest::from_markup(r#"<span style="font-family: Arial">text</span>"#)
                .families,
            vec!["arial"]
        );
    }

    #[test]
    fn recognizes_the_upper_cjk_extension_boundary() {
        assert!(is_cjk_character('\u{20000}'));
    }
}
