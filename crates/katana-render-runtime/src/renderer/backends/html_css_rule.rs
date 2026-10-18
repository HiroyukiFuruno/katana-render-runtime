use super::html_css::HtmlAttributes;
use super::html_css_selector::{CssAncestor, CssInteractionState, CssPseudoElement, CssSelector};
#[path = "html_css_shorthand.rs"]
mod html_css_shorthand;
#[path = "html_css_parser.rs"]
mod parser;
use html_css_shorthand::expand_box_shorthand;

#[derive(Debug)]
pub(super) struct CssRule {
    pub(super) user_agent: bool,
    selectors: Vec<CssSelector>,
    pub(super) declarations: Vec<CssDeclaration>,
    media: Vec<String>,
}

#[derive(Clone, Copy)]
pub(super) struct CssRuleMatchState<'a> {
    pub(super) tag: &'a str,
    pub(super) attributes: &'a HtmlAttributes,
    pub(super) ancestors: &'a [CssAncestor],
    pub(super) sibling_index: usize,
    pub(super) sibling_type_index: usize,
    pub(super) sibling_type_count: usize,
    pub(super) hovered: bool,
    pub(super) focused: bool,
    pub(super) focus_visible: bool,
    pub(super) pseudo_element: Option<CssPseudoElement>,
}

impl<'a> CssRuleMatchState<'a> {
    pub(super) fn element_with_type(
        tag: &'a str,
        attributes: &'a HtmlAttributes,
        ancestors: &'a [CssAncestor],
        sibling_index: usize,
        sibling_type_index: usize,
        sibling_type_count: usize,
        hovered: bool,
    ) -> Self {
        Self {
            tag,
            attributes,
            ancestors,
            sibling_index,
            sibling_type_index,
            sibling_type_count,
            hovered,
            focused: false,
            focus_visible: false,
            pseudo_element: None,
        }
    }

    pub(super) fn element_with_interaction_state(
        tag: &'a str,
        attributes: &'a HtmlAttributes,
        ancestors: &'a [CssAncestor],
        sibling_index: usize,
        sibling_type_index: usize,
        sibling_type_count: usize,
        interaction: CssInteractionState,
    ) -> Self {
        Self {
            focused: interaction.focused,
            focus_visible: interaction.focus_visible,
            ..Self::element_with_type(
                tag,
                attributes,
                ancestors,
                sibling_index,
                sibling_type_index,
                sibling_type_count,
                interaction.hovered,
            )
        }
    }

    pub(super) fn for_pseudo(mut self, pseudo_element: CssPseudoElement) -> Self {
        self.pseudo_element = Some(pseudo_element);
        self
    }
}

impl CssRule {
    #[cfg(test)]
    pub(super) fn matches(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        viewport_width: f32,
    ) -> Option<u16> {
        self.matches_at(tag, attributes, ancestors, 1, viewport_width)
    }

    #[cfg(test)]
    pub(super) fn matches_at(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        viewport_width: f32,
    ) -> Option<u16> {
        self.matches_state(
            CssRuleMatchState::element_with_type(
                tag,
                attributes,
                ancestors,
                sibling_index,
                sibling_index,
                sibling_index,
                false,
            ),
            viewport_width,
        )
    }

    pub(super) fn matches_state(
        &self,
        state: CssRuleMatchState<'_>,
        viewport_width: f32,
    ) -> Option<u16> {
        if !self
            .media
            .iter()
            .all(|query| media_query_matches(query, viewport_width))
        {
            return None;
        }
        self.selectors
            .iter()
            .filter(|selector| selector.matches_at_pseudo_state(state))
            .map(CssSelector::specificity)
            .max()
    }

    pub(super) fn matches_static_snapshot(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
    ) -> Option<u16> {
        self.selectors
            .iter()
            .filter(|selector| selector.matches_static_snapshot(tag, attributes))
            .map(CssSelector::static_snapshot_specificity)
            .max()
    }
}

#[derive(Debug)]
pub(super) struct CssDeclaration {
    pub(super) name: String,
    pub(super) value: String,
    pub(super) important: bool,
}

pub(super) fn parse_rules(source: &str) -> Vec<CssRule> {
    parser::rules(source)
}

pub(super) fn parse_declarations(source: &str) -> Vec<CssDeclaration> {
    parser::declarations(source)
}

pub(super) fn declarations_for(
    name: String,
    value: String,
    important: bool,
) -> Vec<CssDeclaration> {
    expand_box_shorthand(&name, &value, important).unwrap_or_else(|| {
        vec![CssDeclaration {
            name,
            value,
            important,
        }]
    })
}

fn media_query_matches(query: &str, viewport_width: f32) -> bool {
    query.split(',').any(|alternative| {
        let normalized = alternative.trim().to_ascii_lowercase();
        if normalized.contains("print") || normalized.contains("not screen") {
            return false;
        }
        media_width(&normalized, "min-width").is_none_or(|minimum| viewport_width >= minimum)
            && media_width(&normalized, "max-width").is_none_or(|maximum| viewport_width <= maximum)
    })
}

fn media_width(query: &str, feature: &str) -> Option<f32> {
    let start = query.find(feature)? + feature.len();
    let value = query[start..].trim_start().strip_prefix(':')?.trim_start();
    let value = value.split(')').next()?.trim().strip_suffix("px")?.trim();
    value.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{media_query_matches, parse_declarations, parse_rules};

    #[test]
    fn rules_keep_layout_declarations_for_the_interactive_runtime() {
        let rules = parse_rules(
            "main { margin: 24px; padding: 16px; width: 600px; min-height: 400px; font-size: 18px; }",
        );

        assert_eq!(rules.len(), 1);
        let rule = &rules[0];
        let declarations = rule
            .declarations
            .iter()
            .map(|declaration| declaration.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            declarations,
            vec![
                "margin-top",
                "margin-right",
                "margin-bottom",
                "margin-left",
                "padding-top",
                "padding-right",
                "padding-bottom",
                "padding-left",
                "width",
                "min-height",
                "font-size",
            ]
        );
    }

    #[test]
    fn valid_declarations_are_preserved_for_typed_computed_style() {
        let rules = parse_rules("p { display: none; position: fixed; }");

        assert_eq!(rules[0].declarations.len(), 2);
        assert_eq!(rules[0].declarations[0].name, "display");
        assert_eq!(rules[0].declarations[1].name, "position");
    }

    #[test]
    fn parse_box_shorthand_is_expanded_into_longhands() {
        let declarations = parse_declarations(
            "padding: 4px 5px 6px 7px; margin: 10px 20px; margin-top: 30px; padding: 1px 2px 3px 4px 5px;",
        );
        let actual = declarations
            .iter()
            .map(|declaration| (declaration.name.as_str(), declaration.value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(actual, expected_box_declarations());
    }

    fn expected_box_declarations() -> Vec<(&'static str, &'static str)> {
        vec![
            ("padding-top", "4px"),
            ("padding-right", "5px"),
            ("padding-bottom", "6px"),
            ("padding-left", "7px"),
            ("margin-top", "10px"),
            ("margin-right", "20px"),
            ("margin-bottom", "10px"),
            ("margin-left", "20px"),
            ("margin-top", "30px"),
            ("padding", "1px 2px 3px 4px 5px"),
        ]
    }

    #[test]
    fn structured_parser_keeps_delimiters_in_strings_functions_and_important() {
        let rules = parse_rules(
            r#".card { --label: "a;b:c}"; color: var(--label); background: rgb(1, 2, 3) !important; }"#,
        );

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].declarations[0].value, r#""a;b:c}""#);
        assert_eq!(rules[0].declarations[1].value, "var(--label)");
        assert!(rules[0].declarations[2].important);
    }

    #[test]
    fn media_query_matches_the_logical_viewport_width() {
        assert!(media_query_matches("screen and (min-width: 600px)", 800.0));
        assert!(!media_query_matches("screen and (max-width: 600px)", 800.0));
        assert!(!media_query_matches("print", 800.0));
    }

    #[test]
    fn media_query_rules_are_respected_in_interactive_matching() {
        let rules = parse_rules("@media (max-width: 500px) { .card { color: red; } }");
        let selectors = vec![(String::from("class"), String::from("card"))];

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].matches("div", &selectors, &[], 600.0), None);
        assert!(rules[0].matches("div", &selectors, &[], 400.0).is_some());
    }
}
