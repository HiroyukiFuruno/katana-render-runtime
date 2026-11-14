use super::super::super::super::html_document::HtmlDocumentNode;
use crate::renderer::backends::html_interactive::style::CssStyle;

#[derive(Debug, Clone)]
pub(in crate::renderer::backends::html_interactive) struct TableCell {
    pub(in crate::renderer::backends::html_interactive) tag: String,
    pub(in crate::renderer::backends::html_interactive) attributes: Vec<(String, String)>,
    pub(in crate::renderer::backends::html_interactive) row_attributes: Vec<(String, String)>,
    pub(in crate::renderer::backends::html_interactive) row_group_tag: Option<String>,
    pub(in crate::renderer::backends::html_interactive) row_group_attributes: Vec<(String, String)>,
    pub(in crate::renderer::backends::html_interactive) element_ids: Option<TableElementIds>,
    pub(in crate::renderer::backends::html_interactive) children: Vec<HtmlDocumentNode>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::renderer::backends::html_interactive) struct TableElementIds {
    pub(in crate::renderer::backends::html_interactive) cell: u64,
    pub(in crate::renderer::backends::html_interactive) row: u64,
    pub(in crate::renderer::backends::html_interactive) row_group: Option<u64>,
}

#[derive(Clone, Copy)]
struct TableRowGroup<'a> {
    tag: Option<&'a str>,
    attributes: &'a [(String, String)],
    node_id: Option<u64>,
}

pub(in crate::renderer::backends::html_interactive) fn table_rows(
    nodes: &[HtmlDocumentNode],
) -> Vec<Vec<TableCell>> {
    let mut rows = Vec::new();
    collect_table_rows(
        nodes,
        TableRowGroup {
            tag: None,
            attributes: &[],
            node_id: None,
        },
        &mut rows,
    );
    rows
}

fn collect_table_rows(
    nodes: &[HtmlDocumentNode],
    row_group: TableRowGroup<'_>,
    rows: &mut Vec<Vec<TableCell>>,
) {
    for node in nodes {
        visit_table_node(node, row_group, rows);
    }
}

fn visit_table_node(
    node: &HtmlDocumentNode,
    row_group: TableRowGroup<'_>,
    rows: &mut Vec<Vec<TableCell>>,
) {
    let HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children,
        ..
    } = node
    else {
        return;
    };
    if CssStyle::element_has_display_none(attributes) {
        return;
    }
    if append_table_row(tag, row_group, *node_id, attributes, children, rows) {
        return;
    }
    collect_table_rows(children, row_group.nested(tag, *node_id, attributes), rows);
}

impl<'a> TableRowGroup<'a> {
    fn nested(self, tag: &'a str, node_id: u64, attributes: &'a [(String, String)]) -> Self {
        if matches!(tag, "tbody" | "thead" | "tfoot") {
            Self {
                tag: Some(tag),
                attributes,
                node_id: Some(node_id),
            }
        } else {
            self
        }
    }
}

fn append_table_row(
    tag: &str,
    row_group: TableRowGroup<'_>,
    row_node_id: u64,
    attributes: &[(String, String)],
    children: &[HtmlDocumentNode],
    rows: &mut Vec<Vec<TableCell>>,
) -> bool {
    if tag != "tr" {
        return false;
    }
    push_table_row(row_group, row_node_id, attributes, children, rows);
    true
}

fn push_table_row(
    row_group: TableRowGroup<'_>,
    row_node_id: u64,
    row_attributes: &[(String, String)],
    children: &[HtmlDocumentNode],
    rows: &mut Vec<Vec<TableCell>>,
) {
    let cells = children
        .iter()
        .filter_map(|node| table_cell(node, row_group, row_node_id, row_attributes))
        .collect::<Vec<_>>();
    if !cells.is_empty() {
        rows.push(cells);
    }
}

fn table_cell(
    node: &HtmlDocumentNode,
    row_group: TableRowGroup<'_>,
    row_node_id: u64,
    row_attributes: &[(String, String)],
) -> Option<TableCell> {
    let HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children,
        ..
    } = node
    else {
        return None;
    };
    is_visible_cell(tag, attributes).then(|| TableCell {
        tag: tag.clone(),
        attributes: attributes.clone(),
        row_attributes: row_attributes.to_vec(),
        row_group_tag: row_group.tag.map(str::to_string),
        row_group_attributes: row_group.attributes.to_vec(),
        element_ids: Some(TableElementIds {
            cell: *node_id,
            row: row_node_id,
            row_group: row_group.node_id,
        }),
        children: children.clone(),
    })
}

fn is_visible_cell(tag: &str, attributes: &[(String, String)]) -> bool {
    matches!(tag, "th" | "td") && !CssStyle::element_has_display_none(attributes)
}
