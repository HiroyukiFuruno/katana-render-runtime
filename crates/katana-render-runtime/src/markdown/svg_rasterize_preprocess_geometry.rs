use std::collections::HashMap;

use resvg::usvg::roxmltree::{Document, Node};
use simplecss::{Declaration, StyleSheet};

#[path = "svg_rasterize_preprocess_geometry_edits.rs"]
mod edits;
#[path = "svg_rasterize_preprocess_geometry_values.rs"]
mod geometry_values;
#[path = "svg_rasterize_preprocess_geometry_support.rs"]
mod support;

use support::{
    CascadeRank, CascadedGeometryValue, GeometryEdit, GeometryPlan, GeometryValue, SvgElement,
    Winner, is_geometry_element,
};

pub(super) fn materialize_geometry_properties(svg: String) -> String {
    if let Some(rewritten) = rewrite_geometry_properties(&svg) {
        rewritten
    } else {
        svg
    }
}

fn rewrite_geometry_properties(svg: &str) -> Option<String> {
    if !svg.contains("style") || (!svg.contains("rx") && !svg.contains("ry")) {
        return None;
    }
    let Ok(document) = Document::parse(svg) else {
        return None;
    };
    let mut style_sheet = StyleSheet::new();
    parse_style_sheets(&document, &mut style_sheet);
    let plans = collect_geometry_plans(&document, &style_sheet);
    if plans.is_empty() || support::selectors_change_matches(&document, &style_sheet, &plans) {
        return None;
    }
    edits::apply_geometry_plans(&document, plans)
}

fn parse_style_sheets<'doc, 'input: 'doc>(
    document: &'doc Document<'input>,
    style_sheet: &mut StyleSheet<'doc>,
) {
    for node in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "style")
    {
        if node
            .attribute("type")
            .is_some_and(|kind| kind != "text/css")
        {
            continue;
        }
        if let Some(text) = node.text() {
            style_sheet.parse_more(text);
        }
    }
}

fn collect_geometry_plans(
    document: &Document<'_>,
    style_sheet: &StyleSheet<'_>,
) -> HashMap<usize, GeometryPlan> {
    let mut plans = HashMap::new();
    for node in document
        .descendants()
        .filter(|node| is_geometry_element(*node))
    {
        let plan = GeometryPlan {
            rx: resolve_changed_property(node, "rx", style_sheet),
            ry: resolve_changed_property(node, "ry", style_sheet),
        };
        if !plan.is_empty() {
            plans.insert(node.range().start, plan);
        }
    }
    plans
}

fn resolve_changed_property(
    node: Node<'_, '_>,
    property: &str,
    style_sheet: &StyleSheet<'_>,
) -> Option<GeometryEdit> {
    let attribute = node.attribute_node(property);
    let original = attribute.map(|attribute| attribute.value());
    let mut winner = original.map(|value| Winner {
        rank: CascadeRank::presentation_attribute(),
        value: CascadedGeometryValue::Materialized(GeometryValue::Length(value.to_owned())),
    });
    consider_stylesheet_declarations(&mut winner, node, property, style_sheet);
    consider_inline_declarations(&mut winner, style_attribute(node), property);
    match winner?.value {
        CascadedGeometryValue::Unsupported => None,
        CascadedGeometryValue::Materialized(GeometryValue::Length(value)) => {
            (original != Some(value.as_str())).then_some(GeometryEdit::Set(value))
        }
        CascadedGeometryValue::Materialized(GeometryValue::Auto) => {
            attribute.map(|attribute| GeometryEdit::Remove(attribute.range()))
        }
    }
}

pub(super) fn selector_matches_source_node(
    node: Node<'_, '_>,
    selector: &simplecss::Selector<'_>,
) -> bool {
    selector.matches(&support::SvgElement::new(node, None))
}

fn consider_stylesheet_declarations(
    winner: &mut Option<Winner>,
    node: Node<'_, '_>,
    property: &str,
    style_sheet: &StyleSheet<'_>,
) {
    for (rule_order, rule) in style_sheet.rules.iter().enumerate() {
        if !rule.selector.matches(&SvgElement::new(node, None)) {
            continue;
        }
        for (declaration_order, declaration) in rule.declarations.iter().enumerate() {
            consider_declaration(
                winner,
                declaration,
                property,
                CascadeRank::stylesheet(rule, rule_order, declaration_order),
            );
        }
    }
}

fn consider_inline_declarations(winner: &mut Option<Winner>, style: Option<&str>, property: &str) {
    let Some(style) = style else {
        return;
    };
    for (declaration_order, declaration) in simplecss::DeclarationTokenizer::from(style).enumerate()
    {
        consider_declaration(
            winner,
            &declaration,
            property,
            CascadeRank::inline(declaration_order),
        );
    }
}

fn consider_declaration(
    winner: &mut Option<Winner>,
    declaration: &Declaration<'_>,
    property: &str,
    rank: CascadeRank,
) {
    if !declaration.name.eq_ignore_ascii_case(property) {
        return;
    }
    let Some(value) = geometry_values::parse_geometry_value(declaration.value) else {
        return;
    };
    let rank = rank.with_important(declaration.important);
    if winner.as_ref().is_none_or(|current| rank >= current.rank) {
        *winner = Some(Winner { rank, value });
    }
}

fn style_attribute<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.name() == "style" && attribute.namespace().is_none())
        .map(|attribute| attribute.value())
}

#[cfg(test)]
#[path = "svg_rasterize_preprocess_geometry_tests.rs"]
mod tests;
