use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::document::node_text;
use super::super::style::{CssStyle, CssTextAlign};
use super::super::text_metrics::text_width;

pub(in crate::renderer::backends::html_interactive) struct InlineMeasurement;

impl InlineMeasurement {
    pub(in crate::renderer::backends::html_interactive) fn run_start_x(
        nodes: &[HtmlDocumentNode],
        x: f32,
        available: f32,
        style: &CssStyle,
    ) -> f32 {
        let center = match style.text_align {
            CssTextAlign::Start => return x,
            CssTextAlign::Center => true,
            CssTextAlign::End => false,
        };
        let width = nodes
            .iter()
            .filter(|node| !matches!(node, HtmlDocumentNode::Text(text) if text.trim().is_empty()))
            .map_while(|node| Self::node_width(node, style, available))
            .sum::<f32>()
            .min(available);
        let remaining = (available - width).max(0.0);
        if center {
            x + remaining / 2.0
        } else {
            x + remaining
        }
    }

    pub(in crate::renderer::backends::html_interactive) fn node_width(
        node: &HtmlDocumentNode,
        inherited: &CssStyle,
        available: f32,
    ) -> Option<f32> {
        match node {
            HtmlDocumentNode::Text(text) => Some(Self::text_width(text, inherited).min(available)),
            HtmlDocumentNode::Element {
                tag,
                attributes,
                children,
                ..
            } => Self::element_width(tag, attributes, children, inherited, available),
        }
    }

    fn element_width(
        tag: &str,
        attributes: &[(String, String)],
        children: &[HtmlDocumentNode],
        inherited: &CssStyle,
        available: f32,
    ) -> Option<f32> {
        let mut style = CssStyle::from_element(tag, attributes, inherited);
        style.resolve_text_line_height(children);
        if !style.inline_block || style.display == taffy::style::Display::None {
            return None;
        }
        Some(Self::inline_element_width(children, &style, available))
    }

    pub(super) fn inline_element_width(
        children: &[HtmlDocumentNode],
        style: &CssStyle,
        available: f32,
    ) -> f32 {
        let available_box =
            (available - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let box_width = Self::content_box_width(children, style, available_box);
        (box_width + style.margin_left + style.margin_right)
            .min(available)
            .max(MIN_LAYOUT_WIDTH)
    }

    pub(in crate::renderer::backends::html_interactive) fn content_box_width(
        children: &[HtmlDocumentNode],
        style: &CssStyle,
        available: f32,
    ) -> f32 {
        if style.width.is_some() || style.max_width.is_some() {
            style.box_width(available)
        } else {
            let content = children
                .iter()
                .map(|child| {
                    Self::node_width(child, style, available).unwrap_or_else(|| {
                        Self::text_width(&node_text(std::slice::from_ref(child)), style)
                    })
                })
                .sum::<f32>();
            style.intrinsic_outer_width(content)
        }
    }

    pub(super) fn text_width(text: &str, style: &CssStyle) -> f32 {
        text_width(
            &super::super::layout_text::collapsed_inline_whitespace(text),
            style,
        )
    }

    pub(super) fn visible_line<'a>(line: &'a str, style: &CssStyle) -> (f32, &'a str) {
        let visible = line.trim_start_matches(super::super::layout_text::is_css_whitespace);
        let leading = &line[..line.len() - visible.len()];
        (text_width(leading, style), visible)
    }
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, CssTextAlign, HtmlDocumentNode, InlineMeasurement};

    #[test]
    fn border_box_auto_inline_size_keeps_padding_but_explicit_size_is_outer() {
        let inherited = CssStyle::browser_default();
        let auto = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "display:inline-block;box-sizing:border-box;padding:4px 10px".to_string(),
            )],
            &inherited,
        );
        assert_eq!(
            InlineMeasurement::content_box_width(&[], &auto, 200.0),
            20.0
        );
        let explicit = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "display:inline-block;box-sizing:border-box;padding:4px 10px;width:80px"
                    .to_string(),
            )],
            &inherited,
        );
        assert_eq!(
            InlineMeasurement::content_box_width(&[], &explicit, 200.0),
            80.0
        );
    }

    #[test]
    fn end_aligned_inline_run_starts_after_available_space() {
        let nodes = [HtmlDocumentNode::Text("short".to_string())];
        let mut style = CssStyle::browser_default();
        style.text_align = CssTextAlign::End;

        assert!(InlineMeasurement::run_start_x(&nodes, 10.0, 200.0, &style) > 100.0);
    }

    #[test]
    fn start_aligned_inline_run_skips_width_and_style_measurements() {
        let nodes = [
            HtmlDocumentNode::Text("short".to_string()),
            HtmlDocumentNode::Element {
                node_id: 1,
                tag: "span".to_string(),
                attributes: Vec::new(),
                children: vec![HtmlDocumentNode::Text("inline".to_string())],
            },
        ];
        let style = CssStyle::browser_default();
        let _cache = super::super::super::text_metrics::TextMeasurementCacheScope::enter();
        let mut diagnostic =
            super::super::super::text_measurement_diagnostic::TextMeasurementDiagnosticScope::enter(
                true,
            );

        assert_eq!(
            InlineMeasurement::run_start_x(&nodes, 10.0, 200.0, &style),
            10.0
        );
        let stats = diagnostic.finish();
        assert_eq!((stats.style_calls, stats.lookups, stats.misses), (0, 0, 0));
    }

    #[test]
    fn center_aligned_inline_run_uses_measured_width() {
        let nodes = [HtmlDocumentNode::Text("short".to_string())];
        let mut style = CssStyle::browser_default();
        style.text_align = CssTextAlign::Center;
        let expected_width = InlineMeasurement::text_width("short", &style);

        assert_eq!(
            InlineMeasurement::run_start_x(&nodes, 10.0, 200.0, &style),
            10.0 + (200.0 - expected_width) / 2.0
        );
    }

    #[test]
    fn inline_parent_intrinsic_width_includes_descendant_padding_and_margins() -> Result<(), String>
    {
        let node = HtmlDocumentNode::Element {
            node_id: 1,
            tag: "li".to_string(),
            attributes: vec![("style".to_string(), "display:inline".to_string())],
            children: vec![HtmlDocumentNode::Element {
                node_id: 2,
                tag: "a".to_string(),
                attributes: vec![(
                    "style".to_string(),
                    "display:inline;padding:3px 7px;margin:3px".to_string(),
                )],
                children: vec![HtmlDocumentNode::Text("Active".to_string())],
            }],
        };
        let style = CssStyle::browser_default();
        let width = InlineMeasurement::node_width(&node, &style, 300.0)
            .ok_or("inline width was missing")?;

        assert!(width > InlineMeasurement::text_width("Active", &style) + 15.0);
        Ok(())
    }
}
