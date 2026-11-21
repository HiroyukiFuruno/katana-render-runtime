use super::html_css::HtmlAttributes;

#[path = "html_css_selector_ancestor.rs"]
mod ancestor;
#[path = "html_css_selector_attribute.rs"]
mod attribute;
#[path = "html_css_selector_compound.rs"]
mod compound;
#[path = "html_css_selector_match.rs"]
mod matching;
#[path = "html_css_selector_nth.rs"]
mod nth;
#[path = "html_css_selector_parse.rs"]
mod parse;

#[cfg(test)]
#[path = "html_css_selector_focus_tests.rs"]
mod focus_tests;

const CLASS_SPECIFICITY: u16 = 10;
const ID_SPECIFICITY: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CssPseudoElement {
    Before,
    After,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CssInteractionState {
    pub(super) hovered: bool,
    pub(super) focused: bool,
    pub(super) focus_visible: bool,
}

#[derive(Debug, Clone)]
pub(super) struct CssAncestor {
    tag: String,
    attributes: HtmlAttributes,
    sibling_index: usize,
    sibling_type_index: usize,
    sibling_type_count: usize,
    hovered: bool,
    focused: bool,
    focus_visible: bool,
}

#[derive(Debug)]
pub(super) struct CssSelector {
    compounds: Vec<CssCompoundSelector>,
    combinators: Vec<CssCombinator>,
    inherited_from_body: bool,
}

#[derive(Debug)]
struct CssCompoundSelector {
    tag: Option<String>,
    classes: Vec<String>,
    id: Option<String>,
    attributes: Vec<CssAttributeSelector>,
    root: bool,
    hovered: bool,
    focused: bool,
    focus_visible: bool,
    disabled: bool,
    not_disabled: bool,
    checked: bool,
    nth_child: Option<CssNthExpression>,
    first_of_type: bool,
    last_of_type: bool,
    only_of_type: bool,
    first_child: bool,
    last_child: bool,
    only_child: bool,
    pseudo_element: Option<CssPseudoElement>,
}

#[derive(Debug)]
struct CssAttributeSelector {
    name: String,
    matching: CssAttributeMatch,
}

#[derive(Debug)]
enum CssAttributeMatch {
    Exists,
    Equals(String),
    Prefix(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CssNthExpression {
    step: i32,
    offset: i32,
}

#[derive(Debug, Clone, Copy)]
enum CssCombinator {
    Descendant,
    Child,
}

impl CssSelector {
    pub(super) fn parse(raw: &str) -> Option<Self> {
        parse::selector(raw)
    }

    pub(super) fn matches_static_snapshot(&self, tag: &str, attributes: &HtmlAttributes) -> bool {
        self.inherited_from_body
            || (self.pseudo_element().is_none()
                && self.static_snapshot_supported()
                && self.matches(tag, attributes, &[]))
    }

    fn static_snapshot_supported(&self) -> bool {
        self.compounds.len() == 1
            && self.compounds[0].classes.len() <= 1
            && self.compounds[0].attributes.is_empty()
            && self.compounds[0].specificity() > 0
    }

    pub(super) fn specificity(&self) -> u16 {
        self.compounds
            .iter()
            .map(CssCompoundSelector::specificity)
            .sum()
    }

    pub(super) fn static_snapshot_specificity(&self) -> u16 {
        if self.inherited_from_body {
            0
        } else {
            self.specificity()
        }
    }

    pub(super) fn pseudo_element(&self) -> Option<CssPseudoElement> {
        self.compounds
            .last()
            .and_then(|compound| compound.pseudo_element)
    }
}

impl CssCompoundSelector {
    fn specificity(&self) -> u16 {
        let class_specificity = (self.classes.len()
            + self.attributes.len()
            + usize::from(self.root)
            + usize::from(self.hovered)
            + usize::from(self.focused)
            + usize::from(self.focus_visible)
            + usize::from(self.disabled)
            + usize::from(self.not_disabled)
            + usize::from(self.checked)
            + usize::from(self.nth_child.is_some())
            + usize::from(self.first_of_type)
            + usize::from(self.last_of_type)
            + usize::from(self.only_of_type)) as u16
            + (usize::from(self.first_child)
                + usize::from(self.last_child)
                + usize::from(self.only_child)) as u16;
        self.tag.iter().count() as u16
            + class_specificity * CLASS_SPECIFICITY
            + self.pseudo_element.iter().count() as u16
            + self.id.iter().count() as u16 * ID_SPECIFICITY
    }
}

#[cfg(test)]
mod tests {
    use super::{CssAncestor, CssNthExpression, CssPseudoElement, CssSelector};

    #[test]
    fn selector_parser_rejects_invalid_combinators() {
        assert!(CssSelector::parse("main + p").is_none());
        assert!(CssSelector::parse("main ~ p").is_none());
        assert!(CssSelector::parse("main > > p").is_none());
    }

    #[test]
    fn universal_selector_matches_every_element_with_zero_specificity() -> Result<(), String> {
        let selector = CssSelector::parse("*").ok_or("universal selector must parse")?;

        assert!(selector.matches("button", &Vec::new(), &[]));
        assert!(selector.matches("section", &Vec::new(), &[]));
        assert!(!selector.matches_static_snapshot("button", &Vec::new()));
        assert_eq!(selector.specificity(), 0);
        Ok(())
    }

    #[test]
    fn bare_dynamic_pseudo_selectors_parse_without_a_tag_or_class() {
        for source in [
            ":hover",
            ":focus",
            ":focus-visible",
            ":disabled",
            ":not(:disabled)",
            ":checked",
            ":nth-child(2)",
        ] {
            assert!(CssSelector::parse(source).is_some(), "{source}");
        }
    }

    #[test]
    fn pseudo_elements_are_terminal_and_add_type_specificity() -> Result<(), String> {
        let before = CssSelector::parse(".toggle:before").ok_or(":before must parse")?;
        let after = CssSelector::parse("li > a::after").ok_or("::after must parse")?;

        assert_eq!(before.pseudo_element(), Some(CssPseudoElement::Before));
        assert_eq!(before.specificity(), 11);
        assert_eq!(after.pseudo_element(), Some(CssPseudoElement::After));
        assert_eq!(after.specificity(), 3);
        assert!(CssSelector::parse("main::before > span").is_none());
        assert!(CssSelector::parse("main::before::after").is_none());
        Ok(())
    }

    #[test]
    fn selector_parser_rejects_invalid_suffixes_and_attributes() {
        for selector in [
            ".bad!",
            "bad!",
            "li:nth-child(broken)",
            "li:nth-child(2):nth-child(3)",
            "[data-state",
            "[data!=ready]",
            "[href^]",
            "[href^='value' extra]",
            "[href~='value']",
            "[href$='value']",
        ] {
            assert!(CssSelector::parse(selector).is_none(), "{selector}");
        }
    }

    #[test]
    fn nth_child_handles_zero_overflow_and_implicit_offsets() -> Result<(), String> {
        let even = CssNthExpression::parse("2n").ok_or("2n must parse")?;

        assert!(even.matches(2));
        assert!(!even.matches(0));
        assert!(!even.matches(usize::MAX));
        Ok(())
    }

    #[test]
    fn nth_child_matches_element_sibling_positions_and_ancestry() -> Result<(), String> {
        let odd = CssSelector::parse(".layer:nth-child(2n + 1)")
            .ok_or("nth-child selector must parse")?;
        let bounded =
            CssSelector::parse("li:nth-child(-n+3)").ok_or("bounded selector must parse")?;
        let nested = CssSelector::parse("main:nth-child(2) > p:nth-child(even)")
            .ok_or("nested selector must parse")?;
        let attributes = attributes(&[("class", "layer")]);

        assert!(odd.matches_at("div", &attributes, &[], 1));
        assert!(!odd.matches_at("div", &attributes, &[], 2));
        assert!(odd.matches_at("div", &attributes, &[], 5));
        assert!(bounded.matches_at("li", &Vec::new(), &[], 3));
        assert!(!bounded.matches_at("li", &Vec::new(), &[], 4));
        assert!(nested.matches_at(
            "p",
            &Vec::new(),
            &[CssAncestor::new_at("main", &Vec::new(), 2)],
            4,
        ));
        assert_eq!(nested.specificity(), 22);
        Ok(())
    }

    #[test]
    fn root_pseudo_class_matches_the_html_element_only() -> Result<(), String> {
        let selector = CssSelector::parse(":root").ok_or(":root selector must parse")?;

        assert!(selector.matches("html", &Vec::new(), &[]));
        assert!(!selector.matches("body", &Vec::new(), &[]));
        assert_eq!(selector.specificity(), 10);
        Ok(())
    }

    #[test]
    fn disabled_pseudo_class_matches_the_boolean_attribute() -> Result<(), String> {
        let selector =
            CssSelector::parse("button:disabled").ok_or(":disabled selector must parse")?;

        assert!(selector.matches("button", &attributes(&[("disabled", "")]), &[]));
        assert!(!selector.matches("button", &attributes(&[]), &[]));
        assert_eq!(selector.specificity(), 11);
        Ok(())
    }

    #[test]
    fn checked_pseudo_class_matches_the_boolean_attribute() -> Result<(), String> {
        let selector = CssSelector::parse("input:checked").ok_or(":checked selector must parse")?;

        assert!(selector.matches("input", &attributes(&[("checked", "")]), &[]));
        assert!(!selector.matches("input", &attributes(&[]), &[]));
        assert_eq!(selector.specificity(), 11);
        Ok(())
    }

    #[test]
    fn hover_and_not_disabled_match_dynamic_element_state() -> Result<(), String> {
        let selector = CssSelector::parse(".nav button:hover:not(:disabled)")
            .ok_or("dynamic selector must parse")?;
        let ancestors = [CssAncestor::new_at_state(
            "div",
            &attributes(&[("class", "nav")]),
            1,
            false,
        )];

        assert!(selector.matches_at_state("button", &Vec::new(), &ancestors, 1, true));
        assert!(!selector.matches_at_state("button", &Vec::new(), &ancestors, 1, false));
        assert!(!selector.matches_at_state(
            "button",
            &attributes(&[("disabled", "")]),
            &ancestors,
            1,
            true,
        ));
        assert_eq!(selector.specificity(), 31);
        Ok(())
    }

    #[test]
    fn type_pseudo_classes_use_same_tag_sibling_positions() -> Result<(), String> {
        let first = CssSelector::parse("span:first-of-type").ok_or("first selector")?;
        let last = CssSelector::parse("span:last-of-type").ok_or("last selector")?;
        let only = CssSelector::parse("span:only-of-type").ok_or("only selector")?;
        assert!(first.matches_at_with_type("span", &Vec::new(), &[], 2, 1, 2));
        assert!(!first.matches_at_with_type("span", &Vec::new(), &[], 1, 2, 2));
        assert!(last.matches_at_with_type("span", &Vec::new(), &[], 1, 2, 2));
        assert!(only.matches_at_with_type("span", &Vec::new(), &[], 1, 1, 1));
        assert!(!only.matches_at_with_type("span", &Vec::new(), &[], 1, 1, 2));
        assert_eq!(first.specificity(), 11);
        Ok(())
    }

    #[test]
    fn attribute_selector_with_scheme_url_matches_stylesheet_selector() -> Result<(), String> {
        let selector = CssSelector::parse("[href=\"https://example.com\"]")
            .ok_or("attribute selector must parse")?;
        let attributes = attributes(&[("href", "https://example.com")]);

        assert!(selector.matches("a", &attributes, &[]));
        Ok(())
    }

    #[test]
    fn attribute_selector_with_scheme_url_matches_query_selector() -> Result<(), String> {
        let selector = CssSelector::parse("a[href=\"https://example.com\"]")
            .ok_or("attribute selector must parse")?;
        let ancestors = vec![ancestor("body", &[])];
        let attributes = attributes(&[("href", "https://example.com")]);

        assert!(selector.matches("a", &attributes, &ancestors));
        Ok(())
    }

    #[test]
    fn attribute_prefix_selector_matches_nonempty_decoded_prefix_only() -> Result<(), String> {
        let selector =
            CssSelector::parse("a[href^='#']").ok_or("prefix attribute selector must parse")?;
        assert!(selector.matches("a", &attributes(&[("href", "#section")]), &[]));
        assert!(!selector.matches("a", &attributes(&[("href", "section#")]), &[]));
        assert!(!selector.matches("a", &attributes(&[]), &[]));

        let empty = CssSelector::parse("a[href^='']").ok_or("empty prefix selector must parse")?;
        assert!(!empty.matches("a", &attributes(&[("href", "section")]), &[]));
        assert!(!empty.matches("a", &attributes(&[("href", "")]), &[]));

        let escaped_close =
            CssSelector::parse(r"a[href^='a\]b']").ok_or("quoted closing bracket must parse")?;
        assert!(escaped_close.matches("a", &attributes(&[("href", "a]bc")]), &[]));
        let escaped_value =
            CssSelector::parse(r"a[href^='a\62 ']").ok_or("escaped prefix value must parse")?;
        assert!(escaped_value.matches("a", &attributes(&[("href", "abc")]), &[]));
        Ok(())
    }

    #[test]
    fn navigation_toc_prefix_selector_matches_descendant_link() -> Result<(), String> {
        let selector = CssSelector::parse(".sidebar .toc-item[href^='#']")
            .ok_or("TOC attribute prefix selector must parse")?;
        let sidebar = [ancestor("nav", &[("class", "sidebar")])];
        let link = attributes(&[("class", "toc-item"), ("href", "#section")]);

        assert!(selector.matches("a", &link, &sidebar));
        assert!(!selector.matches(
            "a",
            &attributes(&[("class", "toc-item"), ("href", "section")]),
            &sidebar
        ));
        Ok(())
    }

    #[test]
    fn compound_child_and_descendant_selectors_match_ancestry() -> Result<(), String> {
        let selector =
            CssSelector::parse("main > section.card[data-state=ready] p.message.emphasis")
                .ok_or("selector must parse")?;
        let ancestors = vec![
            ancestor("html", &[]),
            ancestor("body", &[]),
            ancestor("main", &[]),
            ancestor("section", &[("class", "card"), ("data-state", "ready")]),
        ];
        let attributes = attributes(&[("class", "message emphasis")]);

        assert!(selector.matches("p", &attributes, &ancestors));
        assert_eq!(selector.specificity(), 43);
        Ok(())
    }

    #[test]
    fn descendant_selector_does_not_match_a_child_below_its_terminal_compound() -> Result<(), String>
    {
        let selector = CssSelector::parse("#filters li").ok_or("selector must parse")?;
        let list_ancestors = [ancestor("ul", &[("id", "filters")])];
        let anchor_ancestors = [ancestor("ul", &[("id", "filters")]), ancestor("li", &[])];

        assert!(selector.matches("li", &Vec::new(), &list_ancestors));
        assert!(!selector.matches("a", &Vec::new(), &anchor_ancestors));
        Ok(())
    }

    #[test]
    fn body_selector_keeps_static_snapshot_inheritance_only() -> Result<(), String> {
        let selector = CssSelector::parse("body").ok_or("body selector must parse")?;
        let attributes = Vec::new();

        assert!(selector.matches("body", &attributes, &[]));
        assert!(!selector.matches("p", &attributes, &[]));
        assert!(selector.matches_static_snapshot("p", &attributes));
        assert_eq!(selector.static_snapshot_specificity(), 0);
        Ok(())
    }

    #[test]
    fn selector_matching_rejects_compound_mismatches() -> Result<(), String> {
        let selector = CssSelector::parse("button.primary#submit[data-action=save]")
            .ok_or("compound selector must parse")?;
        let attributes = attributes(&[
            ("class", "primary selected"),
            ("id", "submit"),
            ("data-action", "save"),
        ]);

        assert!(selector.matches("button", &attributes, &[]));
        assert!(!selector.matches("a", &attributes, &[]));
        Ok(())
    }

    fn ancestor(tag: &str, values: &[(&str, &str)]) -> CssAncestor {
        CssAncestor::new(tag, &attributes(values))
    }

    fn attributes(values: &[(&str, &str)]) -> Vec<(String, String)> {
        values
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }
}
