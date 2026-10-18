use super::{CssCompoundSelector, CssPseudoElement, CssSelector};

#[path = "html_css_selector_identifier.rs"]
mod identifier;
#[path = "html_css_selector_token.rs"]
mod token;

use identifier::{is_selector_character, parse_identifier};
use token::{SelectorToken, selector_tokens};

pub(super) fn selector(raw: &str) -> Option<CssSelector> {
    let selector = raw.trim();
    let tokens = selector_tokens(selector)?;
    let mut compounds = Vec::new();
    let mut combinators = Vec::new();
    for token in tokens {
        match token {
            SelectorToken::Compound(value) => compounds.push(compound(&value)?),
            SelectorToken::Combinator(value) => combinators.push(value),
        }
    }
    if compounds
        .iter()
        .take(compounds.len().saturating_sub(1))
        .any(|compound| compound.pseudo_element.is_some())
    {
        return None;
    }
    Some(CssSelector {
        compounds,
        combinators,
        inherited_from_body: selector.eq_ignore_ascii_case("body"),
    })
}

fn compound(source: &str) -> Option<CssCompoundSelector> {
    let source = source.trim();
    let (tag, mut remaining) = parse_tag(source)?;
    let mut compound = empty_compound(tag);
    while !remaining.is_empty() {
        remaining = parse_suffix(&mut compound, remaining)?;
    }
    (source == "*" || has_selector_part(&compound)).then_some(compound)
}

fn parse_tag(source: &str) -> Option<(Option<String>, &str)> {
    let tag_end = source
        .find(|character: char| ['.', '#', '[', ':'].contains(&character))
        .unwrap_or(source.len());
    let raw_tag = &source[..tag_end];
    if !raw_tag.is_empty() && raw_tag != "*" && !raw_tag.chars().all(is_selector_character) {
        return None;
    }
    let tag = (!raw_tag.is_empty() && raw_tag != "*").then(|| raw_tag.to_ascii_lowercase());
    Some((tag, &source[tag_end..]))
}

fn empty_compound(tag: Option<String>) -> CssCompoundSelector {
    CssCompoundSelector {
        tag,
        classes: Vec::new(),
        id: None,
        attributes: Vec::new(),
        root: false,
        hovered: false,
        focused: false,
        focus_visible: false,
        disabled: false,
        not_disabled: false,
        checked: false,
        nth_child: None,
        first_of_type: false,
        last_of_type: false,
        only_of_type: false,
        first_child: false,
        last_child: false,
        only_child: false,
        pseudo_element: None,
    }
}

fn has_selector_part(compound: &CssCompoundSelector) -> bool {
    compound.tag.is_some()
        || !compound.classes.is_empty()
        || compound.id.is_some()
        || !compound.attributes.is_empty()
        || compound.root
        || compound.hovered
        || compound.focused
        || compound.focus_visible
        || compound.disabled
        || compound.not_disabled
        || compound.checked
        || compound.nth_child.is_some()
        || compound.first_of_type
        || compound.last_of_type
        || compound.only_of_type
        || compound.first_child
        || compound.last_child
        || compound.only_child
        || compound.pseudo_element.is_some()
}

fn parse_suffix<'a>(compound: &mut CssCompoundSelector, source: &'a str) -> Option<&'a str> {
    match source.chars().next()? {
        '.' => parse_identifier(&source[1..]).map(|(value, rest)| {
            compound.classes.push(value.to_string());
            rest
        }),
        '#' => parse_identifier(&source[1..]).and_then(|(value, rest)| {
            compound
                .id
                .replace(value.to_string())
                .is_none()
                .then_some(rest)
        }),
        '[' => super::attribute::parse_attribute(source).map(|(attribute, rest)| {
            compound.attributes.push(attribute);
            rest
        }),
        ':' => pseudo::parse_pseudo_class(compound, source),
        _ => None,
    }
}

#[path = "html_css_selector_pseudo.rs"]
mod pseudo;

#[cfg(test)]
mod tests {
    use super::{CssSelector, selector};

    fn must_parse(source: &str) -> CssSelector {
        let parsed = selector(source);
        assert!(parsed.is_some(), "{source} selector should parse");
        let mut selectors = parsed.into_iter().collect::<Vec<_>>();
        selectors.remove(0)
    }

    #[test]
    fn selector_parses_wildcard_selector() {
        assert!(selector("*").is_some());
    }

    #[test]
    fn selector_rejects_no_selector_parts_without_wildcard() {
        assert!(selector("").is_none());
    }

    #[test]
    fn selector_parses_pseudo_element() {
        assert!(selector("div:before").is_some());
    }

    #[test]
    fn selector_parses_pseudo_flags_without_tag() {
        let pseudo = must_parse(":before");
        let compound = &pseudo.compounds[0];
        assert!(compound.pseudo_element.is_some());
    }

    #[test]
    fn selector_parses_state_pseudo_classes() {
        let disabled = must_parse(":disabled");
        assert!(disabled.compounds[0].disabled);

        let hover = must_parse(":hover");
        assert!(hover.compounds[0].hovered);
    }
}
