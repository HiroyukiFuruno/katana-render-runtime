use std::collections::HashMap;

use resvg::usvg::roxmltree::{Document, Node};
use simplecss::{AttributeOperator, Element, PseudoClass, Rule, StyleSheet};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
pub(super) const GEOMETRY_PROPERTIES: [&str; 2] = ["rx", "ry"];
const SELECTOR_SPECIFICITY_COMPONENTS: usize = 3;

pub(super) fn selectors_change_matches(
    document: &Document<'_>,
    style_sheet: &StyleSheet<'_>,
    plans: &HashMap<usize, GeometryPlan>,
) -> bool {
    document
        .descendants()
        .filter(|node| node.is_element())
        .any(|node| {
            style_sheet.rules.iter().any(|rule| {
                rule.selector.matches(&SvgElement::new(node, None))
                    != rule.selector.matches(&SvgElement::new(node, Some(plans)))
            })
        })
}

pub(super) fn is_geometry_element(node: Node<'_, '_>) -> bool {
    is_svg_namespace(node)
        && matches!(node.tag_name().name(), "rect" | "ellipse")
        && node.is_element()
}

fn is_svg_namespace(node: Node<'_, '_>) -> bool {
    node.tag_name()
        .namespace()
        .is_none_or(|namespace| namespace == SVG_NAMESPACE)
}

#[derive(Clone, Copy)]
pub(super) struct SvgElement<'node, 'plans, 'input> {
    node: Node<'node, 'input>,
    overrides: Option<&'plans HashMap<usize, GeometryPlan>>,
}

impl<'node, 'plans, 'input> SvgElement<'node, 'plans, 'input> {
    pub(super) fn new(
        node: Node<'node, 'input>,
        overrides: Option<&'plans HashMap<usize, GeometryPlan>>,
    ) -> Self {
        Self { node, overrides }
    }

    fn attribute_value(&self, name: &str) -> Option<&str> {
        if let Some(plan) = self
            .overrides
            .and_then(|overrides| overrides.get(&self.node.range().start))
            && let Some(value) = plan.value(name)
        {
            return value.as_attribute();
        }
        self.node.attribute(name)
    }
}

impl Element for SvgElement<'_, '_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.node
            .parent_element()
            .map(|node| Self::new(node, self.overrides))
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.node
            .prev_sibling_element()
            .map(|node| Self::new(node, self.overrides))
    }

    fn has_local_name(&self, name: &str) -> bool {
        self.node.tag_name().name() == name
    }

    fn attribute_matches(&self, local_name: &str, operator: AttributeOperator<'_>) -> bool {
        self.attribute_value(local_name)
            .is_some_and(|value| operator.matches(value))
    }

    fn pseudo_class_matches(&self, class: PseudoClass<'_>) -> bool {
        match class {
            PseudoClass::FirstChild => self.prev_sibling_element().is_none(),
            _ => false,
        }
    }
}

#[derive(Default)]
pub(super) struct GeometryPlan {
    pub(super) rx: Option<GeometryValue>,
    pub(super) ry: Option<GeometryValue>,
}

impl GeometryPlan {
    pub(super) fn value(&self, property: &str) -> Option<&GeometryValue> {
        match property {
            "rx" => self.rx.as_ref(),
            "ry" => self.ry.as_ref(),
            _ => None,
        }
    }

    pub(super) fn set_value(&mut self, property: &str, value: GeometryValue) {
        match property {
            "rx" => self.rx = Some(value),
            "ry" => self.ry = Some(value),
            _ => {}
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.rx.is_none() && self.ry.is_none()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum GeometryValue {
    Length(String),
    Auto,
}

pub(super) enum CascadedGeometryValue {
    Materialized(GeometryValue),
    Unsupported,
}

impl GeometryValue {
    pub(super) fn as_attribute(&self) -> Option<&str> {
        match self {
            Self::Length(value) => Some(value),
            Self::Auto => None,
        }
    }
}

pub(super) struct Winner {
    pub(super) rank: CascadeRank,
    pub(super) value: CascadedGeometryValue,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CascadeRank {
    important: bool,
    inline: bool,
    stylesheet: bool,
    specificity: [u8; SELECTOR_SPECIFICITY_COMPONENTS],
    order: (usize, usize),
}

impl CascadeRank {
    pub(super) fn presentation_attribute() -> Self {
        Self {
            important: false,
            inline: false,
            stylesheet: false,
            specificity: [0; SELECTOR_SPECIFICITY_COMPONENTS],
            order: (0, 0),
        }
    }

    pub(super) fn stylesheet(rule: &Rule<'_>, rule_order: usize, declaration_order: usize) -> Self {
        Self {
            important: false,
            inline: false,
            stylesheet: true,
            specificity: rule.selector.specificity(),
            order: (rule_order.saturating_add(1), declaration_order),
        }
    }

    pub(super) fn inline(order: usize) -> Self {
        Self {
            important: false,
            inline: true,
            stylesheet: false,
            specificity: [0; SELECTOR_SPECIFICITY_COMPONENTS],
            order: (0, order),
        }
    }

    pub(super) fn with_important(mut self, important: bool) -> Self {
        self.important = important;
        self
    }
}
