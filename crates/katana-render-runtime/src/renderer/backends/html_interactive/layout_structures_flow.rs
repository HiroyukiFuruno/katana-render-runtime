use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::LIST_MARKER_WIDTH;
use super::super::layout::HtmlLayoutRenderer;
use super::super::layout_flow::ListItemMarker;
use super::super::style::{CssStyle, CssTextAlign};
use super::super::text_metrics::text_width;
use super::super::types::{DetailsContext, LayoutContext};

impl HtmlLayoutRenderer {
    pub(super) fn paint_list_marker(
        &mut self,
        ordered: bool,
        index: usize,
        x: f32,
        y: f32,
        style: &CssStyle,
    ) {
        let marker = list_marker(ordered, index);
        let marker_advance = text_width(&marker, style);
        let marker_gap = text_width(" ", style);
        let available_width = LIST_MARKER_WIDTH - marker_gap;
        let alignment_width = available_width.max(marker_advance);
        let mut marker_style = style.clone();
        marker_style.text_align = CssTextAlign::End;
        self.paint_text_lines(
            &[marker],
            x + available_width - alignment_width,
            alignment_width,
            y + style.font_size,
            &marker_style,
        );
    }

    pub(in crate::renderer::backends::html_interactive) fn render_list_item_with_marker(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        marker: ListItemMarker,
    ) -> f32 {
        let marker_x = layout.x - LIST_MARKER_WIDTH;
        let content_y = layout.y
            + layout.style.margin_top
            + layout.style.border_top_width()
            + layout.style.padding_top;
        self.list_marker_width(
            Some((marker.ordered, marker.index)),
            marker_x,
            content_y,
            layout.style,
        );
        self.render_container(
            children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
            DetailsContext::NONE,
        )
    }
}

fn list_marker(ordered: bool, index: usize) -> String {
    if ordered {
        format!("{index}.")
    } else {
        "•".to_string()
    }
}
