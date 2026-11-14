use super::super::document::{TableCell, node_text, wrap_text_with_style};
use super::super::layout_text::collapsed_anonymous_text;
use super::super::style::{CssStyle, CssTextAlign};
use super::COLLAPSED_BORDER_SHARE;
use crate::renderer::backends::html_document::HtmlDocumentNode;

const TABLE_HEADER_FONT_WEIGHT: u16 = 700;

pub(super) fn table_cell_height(
    cell: &TableCell,
    row_index: usize,
    column_width: f32,
    inherited: &CssStyle,
    previous_bottom_border: f32,
) -> f32 {
    let style = table_cell_style(cell, row_index, inherited);
    let horizontal_inset = table_cell_horizontal_inset(&style);
    let vertical_inset = table_cell_vertical_inset(&style, row_index, previous_bottom_border);
    wrap_text_with_style(
        &measured_cell_text(cell),
        (column_width - horizontal_inset).max(0.0),
        &style,
    )
    .len() as f32
        * style.line_height
        + vertical_inset
}

fn measured_cell_text(cell: &TableCell) -> String {
    if !cell
        .children
        .iter()
        .all(|node| matches!(node, HtmlDocumentNode::Text(_)))
    {
        return node_text(&cell.children);
    }
    let mut raw = String::new();
    for child in &cell.children {
        if let HtmlDocumentNode::Text(value) = child {
            raw.push_str(value);
        }
    }
    collapsed_anonymous_text(&raw)
}

pub(super) fn table_cell_horizontal_inset(style: &CssStyle) -> f32 {
    style.padding_left
        + style.padding_right
        + super::table_cell_border_inset(style.border_left_width(), style)
        + super::table_cell_border_inset(style.border_right_width(), style)
}

pub(super) fn table_cell_vertical_inset(
    style: &CssStyle,
    row_index: usize,
    previous_bottom_border: f32,
) -> f32 {
    let (top_border, bottom_border) = if style.border_collapse {
        let shared = previous_bottom_border.max(style.border_top_width()) * COLLAPSED_BORDER_SHARE;
        (
            if row_index == 0 {
                style.border_top_width() * COLLAPSED_BORDER_SHARE
            } else {
                shared
            },
            style.border_bottom_width() * COLLAPSED_BORDER_SHARE,
        )
    } else {
        (style.border_top_width(), style.border_bottom_width())
    };
    style.padding_top + style.padding_bottom + top_border + bottom_border
}

fn apply_header_style(style: &mut CssStyle, cell: &TableCell) {
    style.font_weight = style.font_weight.max(TABLE_HEADER_FONT_WEIGHT);
    let has_text_align = cell.attributes.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("style")
            && value
                .split(';')
                .any(|declaration| declaration.trim_start().starts_with("text-align"))
    });
    if !has_text_align {
        style.text_align = CssTextAlign::Center;
    }
}

pub(super) fn table_cell_style(
    cell: &TableCell,
    row_index: usize,
    inherited: &CssStyle,
) -> CssStyle {
    table_cell_styles(cell, row_index, inherited).2
}

pub(super) fn table_cell_styles(
    cell: &TableCell,
    row_index: usize,
    inherited: &CssStyle,
) -> (Option<CssStyle>, CssStyle, CssStyle) {
    let is_header = row_index == 0 && cell.tag == "th";
    let group_style = cell
        .row_group_tag
        .as_deref()
        .map(|tag| CssStyle::from_element(tag, &cell.row_group_attributes, inherited));
    let group_inherited = group_style.as_ref().unwrap_or(inherited);
    let mut row_style = CssStyle::from_element("tr", &cell.row_attributes, group_inherited);
    if cell.row_group_tag.is_none() && cell.row_attributes.is_empty() {
        row_style.vertical_align = super::super::style::CssVerticalAlign::Middle;
    }
    let mut style = CssStyle::from_element(&cell.tag, &cell.attributes, &row_style);
    if !style.has_border_edge_overrides() {
        style.border_width = 0.0;
    }
    style.border_collapse = inherited.border_collapse;
    apply_collapsed_row_borders(&mut style, &row_style);
    if is_header {
        apply_header_style(&mut style, cell);
    }
    style.resolve_text_line_height(&cell.children);
    apply_cell_colors(&mut style, &row_style, inherited, is_header);
    (group_style, row_style, style)
}

fn apply_cell_colors(style: &mut CssStyle, row: &CssStyle, inherited: &CssStyle, is_header: bool) {
    if !style.explicit_background && is_header {
        style.background = Some("#0b74c7".to_string());
    }
    if !style.explicit_background && row.explicit_background {
        style.background = row.background.clone();
    }
    if !style.explicit_color && is_header {
        style.color = "#ffffff".to_string();
    }
    if !style.explicit_background && !row.explicit_background && inherited.explicit_background {
        style.background = inherited.background.clone();
    }
}

fn apply_collapsed_row_borders(style: &mut CssStyle, row: &CssStyle) {
    if !style.border_collapse {
        return;
    }
    if row.border_top_width() > style.border_top_width() {
        style.border_top_width = Some(row.border_top_width());
        style.border_top_style = Some(row.border_top_style.unwrap_or(row.border_style));
        style.border_top_color = row.border_top_color().map(str::to_string);
    }
    if row.border_bottom_width() > style.border_bottom_width() {
        style.border_bottom_width = Some(row.border_bottom_width());
        style.border_bottom_style = Some(row.border_bottom_style.unwrap_or(row.border_style));
        style.border_bottom_color = row.border_bottom_color().map(str::to_string);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::backends::html_document::HtmlDocumentNode;

    fn bordered_cell(row_border: &str, cell_border: &str) -> TableCell {
        TableCell {
            tag: "td".to_string(),
            attributes: vec![(
                "style".to_string(),
                format!("line-height:18px;padding:0;{cell_border}"),
            )],
            row_attributes: vec![("style".to_string(), row_border.to_string())],
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: vec![HtmlDocumentNode::Text("row".to_string())],
        }
    }

    #[test]
    fn separated_cell_counts_both_horizontal_borders() {
        let cell = bordered_cell(
            "",
            "border-top:2px solid black;border-bottom:4px solid black",
        );
        assert_eq!(
            table_cell_height(&cell, 0, 200.0, &CssStyle::browser_default(), 0.0),
            24.0
        );
    }

    #[test]
    fn collapsed_row_border_keeps_its_style_when_the_cell_has_no_border() {
        let cell = bordered_cell(
            "border-top:2px dashed blue;border-bottom:2px dashed blue",
            "",
        );
        let mut table = CssStyle::browser_default();
        table.border_collapse = true;
        let style = table_cell_style(&cell, 0, &table);
        assert_eq!(style.border_top_width(), 2.0);
        assert_eq!(style.border_bottom_width(), 2.0);
        assert_eq!(
            style.border_bottom_style,
            Some(crate::renderer::backends::html_interactive::style::CssBorderStyle::Dashed)
        );
        assert_eq!(table_cell_height(&cell, 0, 200.0, &table, 0.0), 20.0);
    }

    #[test]
    fn collapsed_row_border_participates_in_cell_conflict_resolution() {
        let cell = bordered_cell(
            "border-top:2px solid red;border-bottom:1px solid blue",
            "border-top:3px solid black;border-bottom-style:solid",
        );
        let mut table = CssStyle::browser_default();
        table.border_collapse = true;
        let style = table_cell_style(&cell, 1, &table);
        assert_eq!(style.border_top_width(), 3.0);
        assert_eq!(style.border_top_color(), Some("black"));
        assert_eq!(style.border_bottom_width(), 1.0);
        assert_eq!(style.border_bottom_color(), Some("blue"));
        assert_eq!(table_cell_height(&cell, 1, 200.0, &table, 0.0), 20.0);
    }

    #[test]
    fn measured_text_preserves_nbsp_for_text_only_cells() {
        let cell = TableCell {
            tag: "td".to_string(),
            attributes: Vec::new(),
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: vec![HtmlDocumentNode::Text("A\u{a0}B".to_string())],
        };
        assert_eq!(measured_cell_text(&cell), "A\u{a0}B");
    }

    #[test]
    fn measured_text_falls_back_for_mixed_children_and_br() {
        let cell = TableCell {
            tag: "td".to_string(),
            attributes: Vec::new(),
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: vec![
                HtmlDocumentNode::Text("A".to_string()),
                HtmlDocumentNode::Element {
                    node_id: 1,
                    tag: "span".to_string(),
                    attributes: Vec::new(),
                    children: vec![HtmlDocumentNode::Text("B".to_string())],
                },
                HtmlDocumentNode::Element {
                    node_id: 2,
                    tag: "br".to_string(),
                    attributes: Vec::new(),
                    children: Vec::new(),
                },
                HtmlDocumentNode::Text("C".to_string()),
            ],
        };
        assert_eq!(measured_cell_text(&cell), "AB\nC");
    }
}
