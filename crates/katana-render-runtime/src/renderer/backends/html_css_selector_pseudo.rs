use super::{CssCompoundSelector, CssPseudoElement};

pub(super) fn parse_pseudo_class<'a>(
    compound: &mut CssCompoundSelector,
    source: &'a str,
) -> Option<&'a str> {
    if let Some(rest) = parse_pseudo_element_or_root(compound, source) {
        return Some(rest);
    }
    if source.starts_with(":nth-child(") {
        return super::super::nth::parse_nth_child(compound, source);
    }
    if let Some(rest) = parse_structural_pseudo(compound, source) {
        return Some(rest);
    }
    parse_state_pseudo(compound, source)
}

fn parse_pseudo_element_or_root<'a>(
    compound: &mut CssCompoundSelector,
    source: &'a str,
) -> Option<&'a str> {
    for (prefix, pseudo_element) in [
        ("::before", CssPseudoElement::Before),
        ("::after", CssPseudoElement::After),
        (":before", CssPseudoElement::Before),
        (":after", CssPseudoElement::After),
    ] {
        if source.starts_with(prefix) {
            return parse_pseudo_element(compound, source, prefix, pseudo_element);
        }
    }
    source
        .starts_with(":root")
        .then(|| parse_flag(&mut compound.root, source, ":root"))
        .flatten()
}

fn parse_structural_pseudo<'a>(
    compound: &mut CssCompoundSelector,
    source: &'a str,
) -> Option<&'a str> {
    for (prefix, flag) in [
        (":first-of-type", &mut compound.first_of_type),
        (":first-child", &mut compound.first_child),
        (":last-child", &mut compound.last_child),
        (":only-child", &mut compound.only_child),
        (":last-of-type", &mut compound.last_of_type),
        (":only-of-type", &mut compound.only_of_type),
    ] {
        if source.starts_with(prefix) {
            return parse_flag(flag, source, prefix);
        }
    }
    None
}

fn parse_state_pseudo<'a>(compound: &mut CssCompoundSelector, source: &'a str) -> Option<&'a str> {
    if source.starts_with(":focus-visible") {
        return parse_flag(&mut compound.focus_visible, source, ":focus-visible");
    }
    if source.starts_with(":focus") {
        return parse_flag(&mut compound.focused, source, ":focus");
    }
    if source.starts_with(":hover") {
        return parse_flag(&mut compound.hovered, source, ":hover");
    }
    if source.starts_with(":not(:disabled)") {
        return parse_flag(&mut compound.not_disabled, source, ":not(:disabled)");
    }
    if source.starts_with(":checked") {
        return parse_flag(&mut compound.checked, source, ":checked");
    }
    parse_flag(&mut compound.disabled, source, ":disabled")
}

fn parse_pseudo_element<'a>(
    compound: &mut CssCompoundSelector,
    source: &'a str,
    prefix: &str,
    pseudo_element: CssPseudoElement,
) -> Option<&'a str> {
    let rest = source.strip_prefix(prefix)?;
    compound.pseudo_element.is_none().then(|| {
        compound.pseudo_element = Some(pseudo_element);
        rest
    })
}

fn parse_flag<'a>(flag: &mut bool, source: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = source.strip_prefix(prefix)?;
    (!*flag).then(|| {
        *flag = true;
        rest
    })
}
