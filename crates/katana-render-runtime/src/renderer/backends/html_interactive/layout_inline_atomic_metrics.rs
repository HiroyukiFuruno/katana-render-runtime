use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::document::{node_text, wrap_text_with_style};
use super::super::style::CssStyle;
use super::InlineMeasurement;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use taffy::style::{AlignItems, AlignItemsKeyword, Display};

pub(super) struct AtomicLineMetrics {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) baseline: f32,
}

pub(super) fn inline_flex_metrics(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    available: f32,
) -> Option<AtomicLineMetrics> {
    let HtmlDocumentNode::Element {
        tag,
        attributes,
        children,
        ..
    } = node
    else {
        return None;
    };
    if children.is_empty()
        || !children
            .iter()
            .all(|node| matches!(node, HtmlDocumentNode::Text(_)))
    {
        return None;
    }
    let mut style = CssStyle::from_element(tag, attributes, inherited);
    if style.display != Display::Flex || !style.inline_block {
        return None;
    }
    style.resolve_text_line_height(children);
    text_flex_metrics(node, children, &style, inherited, available)
}

fn text_flex_metrics(
    node: &HtmlDocumentNode,
    children: &[HtmlDocumentNode],
    style: &CssStyle,
    inherited: &CssStyle,
    available: f32,
) -> Option<AtomicLineMetrics> {
    let width = InlineMeasurement::node_width(node, inherited, available)?;
    let text = node_text(children);
    let lines = wrap_text_with_style(
        &text,
        style.content_width(width).max(MIN_LAYOUT_WIDTH),
        style,
    );
    let content_height = lines.len() as f32 * style.line_height;
    let height = flex_outer_height(style, content_height);
    let baseline =
        inline_flex_baseline(&text, style)? + flex_cross_alignment(style, height, content_height);
    Some(AtomicLineMetrics {
        width,
        height,
        baseline,
    })
}

fn flex_outer_height(style: &CssStyle, content_height: f32) -> f32 {
    style.height.map_or(
        content_height
            + style.padding_top
            + style.padding_bottom
            + style.border_top_width()
            + style.border_bottom_width(),
        |height| style.outer_height(height),
    )
}

fn flex_cross_alignment(style: &CssStyle, height: f32, content_height: f32) -> f32 {
    let extra = (style.content_height(height) - content_height).max(0.0);
    /* WHY: 安全指定で従来の位置を変えないため、safe 指定は独自のベースライン補正に含めない。 */
    match style
        .align_items
        .filter(|alignment| !alignment.is_safe())
        .map(AlignItems::keyword)
    {
        Some(AlignItemsKeyword::Center) => extra / 2.0,
        Some(AlignItemsKeyword::End | AlignItemsKeyword::FlexEnd) => extra,
        _ => 0.0,
    }
}

fn inline_flex_baseline(text: &str, style: &CssStyle) -> Option<f32> {
    SvgRasterizeOps::html_text_baseline_for_text(
        text,
        &style.font_family,
        style.font_size,
        style.font_weight,
        style.italic,
        style.line_height,
        style.normal_line_height,
    )
    .map(|baseline| style.border_top_width() + style.padding_top + baseline)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::{CssStyle, HtmlDocumentNode, InlineMeasurement, inline_flex_metrics};

    const AVAILABLE_WIDTH: f32 = 200.0;

    #[test]
    fn fixed_height_flex_baselines_follow_chrome_cross_alignment() -> Result<(), String> {
        for (alignment, expected_baseline) in [("start", 13.0), ("center", 26.0), ("end", 39.0)] {
            let node = fixed_height_text_flex(alignment, "Arial");
            let metrics = inline_flex_metrics(&node, &CssStyle::browser_default(), AVAILABLE_WIDTH)
                .ok_or_else(|| "text inline flex must have baseline metrics".to_string())?;
            assert_eq!(metrics.height, 46.0);
            assert_eq!(metrics.baseline, expected_baseline, "{alignment}");
        }
        Ok(())
    }

    #[test]
    fn reserved_internal_font_family_falls_back_from_atomic_baseline() {
        let node = fixed_height_text_flex("center", "__krr_internal_system_ui_interface_bold_700");
        let inherited = CssStyle::browser_default();
        assert!(InlineMeasurement::node_width(&node, &inherited, AVAILABLE_WIDTH).is_some());
        assert!(inline_flex_metrics(&node, &inherited, AVAILABLE_WIDTH).is_none());
    }

    fn fixed_height_text_flex(alignment: &str, family: &str) -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id: 1,
            tag: "span".to_string(),
            attributes: vec![(
                "style".to_string(),
                format!(
                    "display:inline-flex;height:40px;border:1px solid;padding:2px 8px;font-family:{family};font-size:10px;line-height:14px;align-items:{alignment}"
                ),
            )],
            children: vec![HtmlDocumentNode::Text("Alpha".to_string())],
        }
    }
}

#[cfg(test)]
mod alignment_tests {
    use super::{CssStyle, flex_cross_alignment};

    #[test]
    fn parsed_cross_alignment_preserves_unsafe_and_safe_offsets() {
        let alignments = [
            (None, 0.0),
            (Some("center"), 10.0),
            (Some("unsafe center"), 10.0),
            (Some("safe center"), 0.0),
            (Some("end"), 20.0),
            (Some("unsafe end"), 20.0),
            (Some("safe end"), 0.0),
            (Some("flex-end"), 20.0),
            (Some("unsafe flex-end"), 20.0),
            (Some("safe flex-end"), 0.0),
        ];
        for direction in ["row", "row-reverse", "column", "column-reverse"] {
            for (alignment, expected_offset) in alignments {
                assert_cross_alignment_case(direction, alignment, expected_offset);
            }
        }
    }

    fn assert_cross_alignment_case(direction: &str, alignment: Option<&str>, expected_offset: f32) {
        let mut style = CssStyle::browser_default();
        style.apply("flex-direction", direction);
        assert_eq!(style.align_items, None);
        if let Some(alignment) = alignment {
            style.apply("align-items", alignment);
            assert!(style.align_items.is_some(), "{alignment}");
        }
        assert_eq!(
            flex_cross_alignment(&style, 40.0, 20.0),
            expected_offset,
            "{direction} {alignment:?}"
        );
        assert_eq!(flex_cross_alignment(&style, 10.0, 20.0), 0.0);
    }
}
