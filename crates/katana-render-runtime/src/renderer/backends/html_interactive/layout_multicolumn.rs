use super::super::super::html_document::HtmlDocumentNode;
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::CssStyle;
use super::super::types::{DetailsContext, LayoutContext};

const MIN_COLUMN_WIDTH: f32 = 1.0;

struct MulticolumnPaintContext<'a> {
    origin: (f32, f32),
    geometry: (usize, f32, f32, f32),
    style: &'a CssStyle,
    details: DetailsContext,
}

struct ColumnBreakContext<'a> {
    column: usize,
    count: usize,
    current_y: f32,
    origin_y: f32,
    item_height: f32,
    column_height: f32,
    child: &'a HtmlDocumentNode,
    style: &'a CssStyle,
}

impl HtmlLayoutRenderer {
    pub(super) fn render_multicolumn_children(
        &mut self,
        children: &[HtmlDocumentNode],
        geometry: &super::helpers::ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let (count, gap, width) = column_geometry(style, geometry.inner_width);
        let items = flow_items(children);
        let heights = items
            .iter()
            .map(|child| self.measure_flow_node_height(child, width, style, details))
            .collect::<Result<Vec<_>, _>>()?;
        let inner_start = geometry.start + style.border_top_width() + style.padding_top;
        let column_height = balanced_column_height(heights.iter().sum(), count);
        let context = MulticolumnPaintContext {
            origin: (geometry.inner_x, inner_start),
            geometry: (count, gap, width, column_height),
            style,
            details,
        };
        self.paint_multicolumn_items(&items, &heights, &context);
        Ok(inner_start + style.min_height.max(column_height))
    }

    fn paint_multicolumn_items(
        &mut self,
        items: &[&HtmlDocumentNode],
        heights: &[f32],
        context: &MulticolumnPaintContext<'_>,
    ) {
        let (_, origin_y) = context.origin;
        let mut column = 0usize;
        let mut current_y = origin_y;
        for (child, measured_height) in items.iter().zip(heights) {
            (column, current_y) =
                self.paint_multicolumn_item(child, *measured_height, (column, current_y), context);
        }
    }

    fn paint_multicolumn_item(
        &mut self,
        child: &HtmlDocumentNode,
        measured_height: f32,
        position: (usize, f32),
        context: &MulticolumnPaintContext<'_>,
    ) -> (usize, f32) {
        let (column, mut current_y, x) =
            positioned_multicolumn_item(child, measured_height, position, context);
        let (_, _, width, _) = context.geometry;
        current_y = self.render_flow_node(
            child,
            LayoutContext::new(x, current_y, width, context.style, context.details),
            None,
        );
        (column, current_y)
    }
}

fn positioned_multicolumn_item(
    child: &HtmlDocumentNode,
    measured_height: f32,
    position: (usize, f32),
    context: &MulticolumnPaintContext<'_>,
) -> (usize, f32, f32) {
    let (mut column, mut current_y) = position;
    let (origin_x, origin_y) = context.origin;
    let (count, gap, width, column_height) = context.geometry;
    let break_context = ColumnBreakContext {
        column,
        count,
        current_y,
        origin_y,
        item_height: measured_height,
        column_height,
        child,
        style: context.style,
    };
    if should_break_column(&break_context) {
        column += 1;
        current_y = origin_y;
    }
    let x = origin_x + column as f32 * (width + gap);
    (column, current_y, x)
}

fn column_geometry(style: &CssStyle, available_width: f32) -> (usize, f32, f32) {
    let width_count = style.column_width.map(|width| {
        ((available_width + style.column_gap) / (width + style.column_gap)).floor() as usize
    });
    let count = style
        .column_count
        .unwrap_or_else(|| width_count.unwrap_or(1))
        .min(width_count.unwrap_or(usize::MAX))
        .max(1);
    let gap = style.column_gap.max(0.0);
    let width = ((available_width - gap * count.saturating_sub(1) as f32) / count as f32)
        .max(MIN_COLUMN_WIDTH);
    (count, gap, width)
}

fn flow_items(children: &[HtmlDocumentNode]) -> Vec<&HtmlDocumentNode> {
    children
        .iter()
        .filter(|child| !matches!(child, HtmlDocumentNode::Text(text) if text.trim().is_empty()))
        .collect()
}

fn balanced_column_height(total: f32, count: usize) -> f32 {
    (total / count as f32).ceil().max(MIN_COLUMN_WIDTH)
}

fn should_break_column(context: &ColumnBreakContext<'_>) -> bool {
    context.column + 1 < context.count
        && context.current_y > context.origin_y
        && context.current_y - context.origin_y + context.item_height > context.column_height
        && child_break_inside_avoid(context.child, context.style)
}

fn child_break_inside_avoid(child: &HtmlDocumentNode, inherited: &CssStyle) -> bool {
    match child {
        HtmlDocumentNode::Element {
            tag, attributes, ..
        } => {
            CssStyle::from_element(tag, attributes, inherited).break_inside_avoid
                || tag.eq_ignore_ascii_case("p")
        }
        HtmlDocumentNode::Text(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn break_inside_avoid_handles_text_paragraph_and_style() {
        let style = CssStyle::default();
        assert!(!child_break_inside_avoid(
            &HtmlDocumentNode::Text("x".into()),
            &style
        ));
        let paragraph = HtmlDocumentNode::Element {
            node_id: 1,
            tag: "p".into(),
            attributes: Vec::new(),
            children: Vec::new(),
        };
        assert!(child_break_inside_avoid(&paragraph, &style));
        let avoid = HtmlDocumentNode::Element {
            node_id: 2,
            tag: "div".into(),
            attributes: vec![("style".into(), "break-inside: avoid".into())],
            children: Vec::new(),
        };
        assert!(child_break_inside_avoid(&avoid, &style));
    }
}
