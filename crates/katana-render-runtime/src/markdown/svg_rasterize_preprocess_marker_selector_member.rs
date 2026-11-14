use cssparser::{Parser, Token};

const MAX_EXPANSION: usize = 128;
pub(super) const MAX_REWRITTEN_SELECTOR_BYTES: usize = 64 * 1024;

pub(super) fn rewrite_member(
    source: &str,
    ids: &[String],
    max_output_bytes: usize,
) -> Option<String> {
    let mut escaped_exact_seen = false;
    let Some(suffix) = scan_member(source, &mut escaped_exact_seen)? else {
        let normalized = super::exact::rewrite_member(source, max_output_bytes);
        return if escaped_exact_seen {
            normalized
        } else {
            normalized.or_else(|| (source.len() <= max_output_bytes).then(|| source.to_owned()))
        };
    };
    expand_suffix(suffix, ids, max_output_bytes)
}

fn expand_suffix(suffix: Suffix, ids: &[String], max_output_bytes: usize) -> Option<String> {
    let mut matches: Vec<String> = if suffix.value.is_empty() {
        Vec::new()
    } else {
        ids.iter()
            .filter(|id| id.ends_with(&suffix.value))
            .cloned()
            .collect()
    };
    if matches.len() > MAX_EXPANSION {
        return None;
    }
    if matches.is_empty() {
        matches.push(missing_sentinel(ids)?);
    }
    let mut replacement = String::new();
    for (index, id) in matches.iter().enumerate() {
        append_alternative(&mut replacement, index, id, &suffix, max_output_bytes)?;
    }
    Some(replacement)
}

fn append_alternative(
    output: &mut String,
    index: usize,
    id: &str,
    suffix: &Suffix,
    max_output_bytes: usize,
) -> Option<()> {
    let alternative = selector_alternative(id, suffix)?;
    let separator = usize::from(index > 0) * 2;
    let limit = max_output_bytes.min(MAX_REWRITTEN_SELECTOR_BYTES);
    if output.len() + separator + alternative.len() > limit {
        return None;
    }
    if index > 0 {
        output.push_str(", ");
    }
    output.push_str(&alternative);
    Some(())
}

fn selector_alternative(id: &str, suffix: &Suffix) -> Option<String> {
    let mut serialized = String::new();
    cssparser::serialize_string(id, &mut serialized).ok()?;
    let mut alternative = String::new();
    alternative.push_str(&suffix.prefix);
    alternative.push_str("[id=");
    alternative.push_str(&super::exact::xml_escape_selector(&serialized));
    alternative.push(']');
    alternative.push_str(&suffix.tail);
    Some(alternative)
}

struct Suffix {
    prefix: String,
    tail: String,
    value: String,
}

fn scan_member(source: &str, escaped_exact_seen: &mut bool) -> Option<Option<Suffix>> {
    let mut parser = Parser::new(source);
    let mut found = None;
    while !parser.is_exhausted() {
        inspect_next_token(source, &mut parser, &mut found, escaped_exact_seen)?;
    }
    Some(found)
}

fn inspect_next_token(
    source: &str,
    parser: &mut Parser<'_>,
    found: &mut Option<Suffix>,
    escaped_exact_seen: &mut bool,
) -> Option<()> {
    let (start, token_kind) = super::token::next_member_token(parser)?;
    match token_kind {
        super::token::MemberToken::Attribute => {
            inspect_attribute(source, parser, start, found, escaped_exact_seen)
        }
        super::token::MemberToken::Other => Some(()),
    }
}

fn inspect_attribute(
    source: &str,
    parser: &Parser<'_>,
    start: usize,
    found: &mut Option<Suffix>,
    escaped_exact_seen: &mut bool,
) -> Option<()> {
    let end = parser.position().byte_index();
    let raw = source.get(start..end)?;
    if raw.as_bytes().last() != Some(&b']') {
        return None;
    }
    let parsed = parse_suffix_attribute(raw, escaped_exact_seen)?;
    let Some(value) = parsed else {
        if *escaped_exact_seen {
            return found.is_none().then_some(());
        }
        return (!raw.as_bytes().contains(&b'$')).then_some(());
    };
    if *escaped_exact_seen || found.is_some() {
        return None;
    }
    *found = Some(Suffix {
        prefix: source.get(..start)?.to_owned(),
        tail: source.get(end..)?.to_owned(),
        value,
    });
    Some(())
}

fn parse_suffix_attribute(source: &str, escaped_exact_seen: &mut bool) -> Option<Option<String>> {
    let mut parser = Parser::new(source);
    if !matches!(parser.next().ok()?, Token::SquareBracketBlock) {
        return None;
    }
    let parsed = parser
        .parse_nested_block(|input| parse_suffix_contents(input, escaped_exact_seen))
        .ok()?;
    Some(parsed)
}

fn parse_suffix_contents(
    input: &mut Parser<'_>,
    escaped_exact_seen: &mut bool,
) -> Result<Option<String>, cssparser::ParseError<()>> {
    input.skip_whitespace();
    let name = match input.next()?.clone() {
        Token::Ident(name) => name,
        _ => return Ok(None),
    };
    input.skip_whitespace();
    let operator = input.next()?.clone();
    if matches!(operator, Token::Delim('=')) && name.as_ref() == "id" {
        return super::parse_escaped_exact_attribute(input, escaped_exact_seen);
    }
    if !matches!(operator, Token::SuffixMatch) {
        return Ok(None);
    }
    if name.as_ref() != "id" {
        return Err(cssparser::ParseError::unexpected_token());
    }
    super::parse_suffix_value(input)
}

fn missing_sentinel(ids: &[String]) -> Option<String> {
    (0..=ids.len())
        .map(|index| format!("__krr_missing_marker_id_{index}__"))
        .find(|candidate| !ids.iter().any(|id| id == candidate))
}

#[cfg(test)]
mod boundary_tests {
    use super::{MAX_EXPANSION, parse_suffix_attribute, parse_suffix_contents, rewrite_member};
    use cssparser::Parser;

    #[test]
    fn suffix_expansion_over_limit_rejects_the_complete_member() {
        let ids = (0..=MAX_EXPANSION)
            .map(|index| format!("{index}-end"))
            .collect::<Vec<_>>();
        assert!(rewrite_member("[id$='end']", &ids, usize::MAX).is_none());
    }

    #[test]
    fn escaped_class_member_is_preserved_and_escaped_id_is_normalized() {
        let class = r".plain\26 class";
        assert!(super::super::exact::rewrite_member(class, usize::MAX).is_none());
        assert_eq!(
            rewrite_member(class, &[], usize::MAX),
            Some(class.to_owned())
        );
        assert!(rewrite_member(class, &[], class.len() - 1).is_none());
        assert_eq!(
            rewrite_member(r#"[id="a\26-end"].other"#, &[], usize::MAX),
            Some(r#"[id="a&amp;-end"].other"#.to_owned()),
        );
    }

    #[test]
    fn unsupported_plain_id_attribute_is_not_partially_rewritten() {
        let plain = "[id='plain'][class='other']";
        assert!(rewrite_member(plain, &[], usize::MAX).is_none());
        assert!(super::super::rewrite_selector_list(plain, &[], usize::MAX).is_none());
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><style>{plain} {{ fill: red }}</style></svg>"#,
        );
        assert_eq!(
            super::super::super::normalize_marker_selectors(svg.clone()),
            svg
        );
    }

    #[test]
    fn suffix_parser_rejects_non_attribute_and_ignores_numeric_name() {
        assert!(parse_suffix_attribute("rect", &mut false).is_none());
        let mut parser = Parser::new("7");
        assert!(parse_suffix_contents(&mut parser, &mut false).is_ok_and(|value| value.is_none()));
    }

    #[test]
    fn numeric_attribute_tokens_preserve_original_invalid_css() {
        let raw = "[7]";
        assert!(parse_suffix_attribute(raw, &mut false).is_some_and(|value| value.is_none()));
        assert_eq!(rewrite_member(raw, &[], usize::MAX), Some(raw.to_owned()));
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><style>{raw} {{ fill: red }}</style></svg>"#,
        );
        assert_eq!(
            super::super::super::normalize_marker_selectors(svg.clone()),
            svg
        );
    }

    #[test]
    fn unsupported_attribute_tokens_reject_without_rewriting_svg() {
        for raw in [r#"["$"]"#, r#"[class="other"]"#] {
            assert!(rewrite_member(raw, &[], usize::MAX).is_none());
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg"><style>{raw} {{ fill: red }}</style></svg>"#,
            );
            assert_eq!(
                super::super::super::normalize_marker_selectors(svg.clone()),
                svg
            );
        }
    }
}
