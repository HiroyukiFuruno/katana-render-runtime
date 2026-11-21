use super::super::svg::{EMBEDDED_SVG_MARKUP_ATTRIBUTE, serialize_embedded_svg};
use super::{HtmlAttributes, HtmlDocument, HtmlDocumentNode, ProjectionState};
use crate::renderer::backends::html_css::StaticCss;
use crate::renderer::backends::html_css_selector::CssAncestor;
use markup5ever_rcdom::Handle;

pub(super) struct SvgProjectionContext<'a> {
    pub(super) document: &'a HtmlDocument,
    pub(super) css: &'a StaticCss,
    pub(super) ancestors: &'a [CssAncestor],
    pub(super) state: ProjectionState<'a>,
}

#[path = "html_document_projection_svg_variables.rs"]
mod variables;
use variables::svg_variable_overrides;

pub(super) fn embedded_svg_node(
    node_id: u64,
    tag: String,
    mut attributes: HtmlAttributes,
    node: &Handle,
    context: SvgProjectionContext<'_>,
) -> HtmlDocumentNode {
    let root_style = attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("style"))
        .map(|(_, value)| value.clone());
    attributes.push((
        EMBEDDED_SVG_MARKUP_ATTRIBUTE.to_string(),
        serialized_markup(node, root_style.as_deref(), context),
    ));
    HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children: Vec::new(),
    }
}

fn serialized_markup(
    node: &Handle,
    root_style: Option<&str>,
    context: SvgProjectionContext<'_>,
) -> String {
    let overrides = svg_variable_overrides(
        context.document,
        node,
        context.css,
        context.ancestors,
        context.state,
    );
    serialize_embedded_svg(node, root_style, &overrides)
}
