use super::{CssStyle, DetailsContext, HtmlBrowserViewport, HtmlDocumentNode, HtmlLayoutRenderer};
use std::collections::HashMap;

#[cfg(target_os = "macos")]
pub(super) const CHROME_INLINE_VIEWPORT: HtmlBrowserViewport = HtmlBrowserViewport {
    width: 400,
    height: 200,
    device_scale_factor: 1.0,
};

#[cfg(target_os = "macos")]
pub(super) const CHROME_INLINE_FONT_SIZE: f32 = 11.62;

#[cfg(target_os = "macos")]
pub(super) const CHROME_INLINE_LINE_HEIGHT: f32 = 19.75;

#[cfg(target_os = "macos")]
pub(super) const CHROME_INLINE_LINE_HEIGHT_FACTOR: f32 = 1.7;

#[cfg(target_os = "macos")]
pub(super) const STATUS_FONT_SIZE: f32 = 11.76;

#[cfg(target_os = "macos")]
pub(super) const STATUS_LINE_HEIGHT: f32 = 20.0;

#[cfg(target_os = "macos")]
pub(super) const STATUS_BADGE_ID: u64 = 41;

#[cfg(target_os = "macos")]
pub(super) const STATUS_SMALL_ID: u64 = 42;

#[cfg(target_os = "macos")]
pub(super) const STATUS_BREAK_ID: u64 = 43;

#[cfg(target_os = "macos")]
pub(super) fn inline_status_fixture() -> (CssStyle, Vec<HtmlDocumentNode>) {
    let mut style = CssStyle::browser_default();
    style.font_family = "Helvetica Neue, Arial, Hiragino Kaku Gothic ProN, sans-serif".to_string();
    style.font_size = STATUS_FONT_SIZE;
    style.line_height = STATUS_LINE_HEIGHT;
    style.line_height_factor = Some(CHROME_INLINE_LINE_HEIGHT_FACTOR);
    style.normal_line_height = false;
    let badge = styled_inline_node(
        STATUS_BADGE_ID,
        "display:inline-flex;align-items:center;font-size:.71em;font-weight:700;padding:2px 8px;border:1px solid green;white-space:nowrap",
        "確定",
    );
    let small = styled_inline_node(STATUS_SMALL_ID, "font-size:.85em", "一覧表示を基準とする");
    let line_break = HtmlDocumentNode::Element {
        node_id: STATUS_BREAK_ID,
        tag: "br".to_string(),
        attributes: vec![],
        children: vec![],
    };
    (style, vec![badge, line_break, small])
}

#[cfg(target_os = "macos")]
pub(super) fn styled_inline_node(node_id: u64, style: &str, text: &str) -> HtmlDocumentNode {
    HtmlDocumentNode::Element {
        node_id,
        tag: "span".to_string(),
        attributes: vec![("style".to_string(), style.to_string())],
        children: vec![HtmlDocumentNode::Text(text.to_string())],
    }
}

#[cfg(target_os = "macos")]
pub(super) fn strong_text_node(text: &str) -> HtmlDocumentNode {
    HtmlDocumentNode::Element {
        node_id: 1,
        tag: "strong".to_string(),
        attributes: vec![],
        children: vec![HtmlDocumentNode::Text(text.to_string())],
    }
}

#[cfg(target_os = "macos")]
pub(super) fn inline_text_height(nodes: &[HtmlDocumentNode]) -> f32 {
    inline_text_height_at_width(nodes, CHROME_INLINE_VIEWPORT.width as f32)
}

#[cfg(target_os = "macos")]
pub(super) fn inline_text_height_at_width(nodes: &[HtmlDocumentNode], width: f32) -> f32 {
    let mut style = CssStyle::browser_default();
    style.font_family = "Helvetica Neue, Arial, Hiragino Kaku Gothic ProN, sans-serif".to_string();
    style.font_size = CHROME_INLINE_FONT_SIZE;
    style.line_height = CHROME_INLINE_LINE_HEIGHT;
    style.line_height_factor = Some(CHROME_INLINE_LINE_HEIGHT_FACTOR);
    style.normal_line_height = false;
    let mut renderer = HtmlLayoutRenderer::new(CHROME_INLINE_VIEWPORT, 0.0, &HashMap::new(), None);
    renderer.render_nodes(nodes, 0.0, 0.0, width, &style, DetailsContext::NONE)
}
