use super::super::html_document::HtmlDocumentNode;
use super::document::node_text;
use super::layout::HtmlLayoutRenderer;
use super::svg::escape_xml;
use super::types::ControlLayout;

const SUMMARY_MARKER_SIZE: f32 = 8.0;
const SUMMARY_MARKER_TEXT_GAP: f32 = 6.0;

impl HtmlLayoutRenderer {
    pub(super) fn paint_summary(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: ControlLayout<'_>,
        open: bool,
    ) {
        self.paint_box(
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            layout.style,
        );
        let marker_x = self.paint_summary_marker(layout, open);
        self.paint_control_text(
            &node_text(children),
            marker_x + SUMMARY_MARKER_SIZE + SUMMARY_MARKER_TEXT_GAP,
            summary_text_width(layout),
            layout.y + layout.style.padding_top,
            layout.height,
            layout.style,
        );
    }

    fn paint_summary_marker(&mut self, layout: ControlLayout<'_>, open: bool) -> f32 {
        let marker_x = layout.x + layout.style.padding_left;
        let marker_view_x = marker_x - self.scroll_x;
        let marker_y = layout.y + (layout.height - SUMMARY_MARKER_SIZE) / 2.0 - self.scroll_y;
        self.svg.push_str(&format!(
            r#"<path d="{}" fill="{}"/>"#,
            disclosure_marker_path(marker_view_x, marker_y, open),
            escape_xml(&layout.style.color),
        ));
        marker_x
    }
}

fn summary_text_width(layout: ControlLayout<'_>) -> f32 {
    (layout.width
        - layout.style.padding_left
        - layout.style.padding_right
        - SUMMARY_MARKER_SIZE
        - SUMMARY_MARKER_TEXT_GAP)
        .max(0.0)
}

fn disclosure_marker_path(marker_x: f32, marker_y: f32, open: bool) -> String {
    let marker_end_x = marker_x + SUMMARY_MARKER_SIZE;
    let marker_end_y = marker_y + SUMMARY_MARKER_SIZE;
    let marker_center_x = marker_x + SUMMARY_MARKER_SIZE / 2.0;
    let marker_center_y = marker_y + SUMMARY_MARKER_SIZE / 2.0;
    if open {
        format!(
            "M {marker_x} {marker_y} L {marker_end_x} {marker_y} L {marker_center_x} {marker_end_y} Z"
        )
    } else {
        format!(
            "M {marker_x} {marker_y} L {marker_x} {marker_end_y} L {marker_end_x} {marker_center_y} Z"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    #[test]
    fn summary_layout_uses_a_vector_disclosure_marker() -> Result<(), String> {
        let svg = render_summary_svg(false)?;

        assert!(svg.contains("<path d=\"M "));
        assert!(!svg.contains("▸"));
        Ok(())
    }

    #[test]
    fn disclosure_marker_changes_direction_when_details_is_open() {
        let closed = disclosure_marker_path(10.0, 20.0, false);
        let open = disclosure_marker_path(10.0, 20.0, true);

        assert_eq!(closed, "M 10 20 L 10 28 L 18 24 Z");
        assert_eq!(open, "M 10 20 L 18 20 L 14 28 Z");
    }

    #[test]
    fn details_open_attribute_changes_the_rendered_marker_direction() -> Result<(), String> {
        let closed_svg = render_summary_svg(false)?;
        let open_svg = render_summary_svg(true)?;
        let closed_marker = summary_marker_path(&closed_svg);
        let open_marker = summary_marker_path(&open_svg);

        assert!(closed_marker.is_some());
        assert!(open_marker.is_some());
        assert_ne!(closed_marker, open_marker);
        Ok(())
    }

    #[test]
    fn disclosure_marker_tracks_the_summary_through_viewport_scroll() -> Result<(), String> {
        let unscrolled = render_summary_svg_at_scroll(true, 0.0)?;
        let scrolled = render_summary_svg_at_scroll(true, 12.0)?;
        let unscrolled_origin = marker_origin(&unscrolled).ok_or("unscrolled marker missing")?;
        let scrolled_origin = marker_origin(&scrolled).ok_or("scrolled marker missing")?;

        assert_eq!(scrolled_origin.0, unscrolled_origin.0);
        assert_eq!(scrolled_origin.1, unscrolled_origin.1 - 12.0);
        Ok(())
    }

    fn render_summary_svg(open: bool) -> Result<String, String> {
        render_summary_svg_at_scroll(open, 0.0)
    }

    fn render_summary_svg_at_scroll(open: bool, scroll_y: f32) -> Result<String, String> {
        HtmlLayoutRenderer::render(
            &details_nodes(open),
            HtmlBrowserViewport {
                width: 320,
                height: 240,
                device_scale_factor: 1.0,
            },
            scroll_y,
            &HashMap::new(),
            None,
        )
        .map(|layout| layout.svg)
    }

    fn details_nodes(open: bool) -> Vec<HtmlDocumentNode> {
        let attributes = if open {
            vec![("open".to_string(), String::new())]
        } else {
            Vec::new()
        };
        vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "details".to_string(),
            attributes,
            children: vec![HtmlDocumentNode::Element {
                node_id: 2,
                tag: "summary".to_string(),
                attributes: Vec::new(),
                children: vec![HtmlDocumentNode::Text("More".to_string())],
            }],
        }]
    }

    fn summary_marker_path(svg: &str) -> Option<&str> {
        svg.split_once("<path d=\"")
            .and_then(|(_, path)| path.split_once('"').map(|(path, _)| path))
    }

    fn marker_origin(svg: &str) -> Option<(f32, f32)> {
        let path = summary_marker_path(svg)?;
        let mut parts = path.split_whitespace();
        (parts.next()? == "M").then_some(())?;
        Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
    }
}
