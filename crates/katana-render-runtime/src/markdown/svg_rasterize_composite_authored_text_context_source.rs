use std::collections::HashMap;

use resvg::usvg::roxmltree::{Document, Node};

use super::css;
#[path = "svg_rasterize_composite_authored_text_context_additional.rs"]
mod additional;
#[path = "svg_rasterize_composite_authored_text_context_inventory.rs"]
mod inventory;
pub(super) use additional::{AdditionalTextProof, additional_text};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const SENSITIVE_ATTRIBUTES: [&str; 30] = [
    "alignment-baseline",
    "baseline-shift",
    "dominant-baseline",
    "font",
    "font-family",
    "font-size",
    "font-style",
    "font-variant",
    "font-weight",
    "letter-spacing",
    "line-height",
    "text-anchor",
    "text-length",
    "textLength",
    "text-orientation",
    "text-rendering",
    "text-transform",
    "white-space",
    "word-spacing",
    "writing-mode",
    "font-kerning",
    "font-feature-settings",
    "font-variation-settings",
    "font-optical-sizing",
    "dx",
    "rotate",
    "transform",
    "y",
    "direction",
    "unicode-bidi",
];

pub(super) fn stylesheet<'document, 'input>(
    document: &'document Document<'input>,
) -> Option<Vec<simplecss::Selector<'document>>> {
    let mut sensitive_selectors = Vec::new();
    for node in document
        .descendants()
        .filter(|node| node.is_element() && node.has_tag_name("style"))
    {
        if node
            .attribute("type")
            .is_some_and(|kind| kind != "text/css")
        {
            continue;
        }
        let Some(source) = node.text() else { continue };
        sensitive_selectors.extend(css::sensitive_selectors(source)?);
    }
    Some(sensitive_selectors)
}

pub(super) fn nested_single_line_text(
    root: Node<'_, '_>,
    sensitive_selectors: &[simplecss::Selector<'_>],
) -> Option<String> {
    let tspan = single_tspan(root)?;
    if tspan.attribute("dy")?.trim().parse::<f32>().ok()? != 0.0
        || !descendant_style_is_inherited(tspan, sensitive_selectors)
    {
        return None;
    }
    meaningful_text(root, tspan)
}

fn single_tspan<'document, 'input>(
    root: Node<'document, 'input>,
) -> Option<Node<'document, 'input>> {
    if !svg_namespace(root) || in_ambiguous_context(root) {
        return None;
    }
    let elements = root
        .descendants()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    if elements.len() != 2 {
        return None;
    }
    elements.into_iter().find(|node| {
        node.has_tag_name("tspan") && node.parent() == Some(root) && svg_namespace(*node)
    })
}

fn meaningful_text(root: Node<'_, '_>, tspan: Node<'_, '_>) -> Option<String> {
    let mut text = String::new();
    let mut meaningful = false;
    for node in root.descendants().filter(|node| node.is_text()) {
        let value = node.text()?;
        if value.trim().is_empty() {
            continue;
        }
        if node.parent() != Some(tspan) {
            return None;
        }
        meaningful = true;
        text.push_str(value);
    }
    meaningful.then_some(text)
}

fn descendant_style_is_inherited(
    tspan: Node<'_, '_>,
    sensitive_selectors: &[simplecss::Selector<'_>],
) -> bool {
    if SENSITIVE_ATTRIBUTES
        .iter()
        .any(|name| tspan.attribute(*name).is_some())
    {
        return false;
    }
    if let Some(style) = tspan.attribute("style")
        && !css::inline_style_is_safe(style)
    {
        return false;
    }
    sensitive_selectors
        .iter()
        .all(|selector| !super::super::super::preprocess::source_selector_matches(tspan, selector))
}

pub(super) fn has_use_element(document: &Document<'_>) -> bool {
    document
        .descendants()
        .any(|node| node.is_element() && node.tag_name().name().eq_ignore_ascii_case("use"))
}

pub(super) fn text_occurrences(nodes: &[Node<'_, '_>]) -> HashMap<String, usize> {
    inventory::text_occurrences(nodes)
}

fn svg_namespace(node: Node<'_, '_>) -> bool {
    node.tag_name()
        .namespace()
        .is_none_or(|namespace| namespace == SVG_NAMESPACE)
}

fn in_ambiguous_context(root: Node<'_, '_>) -> bool {
    root.ancestors().any(|node| {
        node.is_element()
            && matches!(
                node.tag_name().name(),
                "defs" | "marker" | "mask" | "pattern" | "symbol" | "textPath" | "use"
            )
    }) || root
        .descendants()
        .any(|node| node.is_element() && !matches!(node.tag_name().name(), "text" | "tspan"))
}
