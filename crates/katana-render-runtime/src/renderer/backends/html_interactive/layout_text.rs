use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::document::wrap_text_with_style;
use super::layout::HtmlLayoutRenderer;
use super::style::{CssStyle, CssWhiteSpace};
use super::text_metrics::text_width;
use super::types::{HitTarget, HitTargetKind};

impl HtmlLayoutRenderer {
    pub(super) fn render_link(
        &mut self,
        node_id: u64,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let style = link_style(style);
        let text = match style.white_space {
            CssWhiteSpace::Normal | CssWhiteSpace::NoWrap => normalized_link_text(children),
        };
        let start = y + style.margin_top;
        let box_x = x + style.margin_left;
        let available = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let box_width = text_box_width(&text, available, &style);
        let lines = wrap_text_with_style(&text, style.content_width(box_width), &style);
        let height = self.paint_wrapped_box(&lines, box_x, start, box_width, &style);
        self.hit_targets.push(HitTarget {
            node_id,
            x: box_x,
            y: start,
            width: box_width,
            height,
            kind: HitTargetKind::Click,
        });
        start + height + style.margin_bottom
    }

    pub(super) fn render_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let text = collapsed_anonymous_text(text);
        if text.is_empty() {
            return y;
        }
        let lines = wrap_text_with_style(&text, width, style);
        self.paint_text_lines(&lines, x, width, y + style.font_size, style);
        y + lines.len() as f32 * style.line_height
    }

    fn paint_wrapped_box(
        &mut self,
        lines: &[String],
        x: f32,
        start: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let height = text_box_height(lines, style);
        self.paint_box(x, start, width, height, style);
        let content_x = x + style.border_left_width() + style.padding_left;
        let content_width = style.content_width(width).max(MIN_LAYOUT_WIDTH);
        self.paint_text_lines(
            lines,
            content_x,
            content_width,
            start + style.border_top_width() + style.padding_top + style.font_size,
            style,
        );
        height
    }
}

pub(super) fn collapsed_anonymous_text(text: &str) -> String {
    /* WHY: 匿名flex itemのHTML整形用改行は、CSS normal/nowrapでは強制改行にならない。 */
    text.split([' ', '\t', '\n', '\r', '\u{000c}'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalized_link_text(nodes: &[HtmlDocumentNode]) -> String {
    let mut current_line = String::new();
    let mut completed_lines = Vec::new();
    for node in nodes {
        append_link_text(node, &mut current_line, &mut completed_lines);
    }
    completed_lines.push(current_line);
    completed_lines
        .into_iter()
        .map(|line| collapsed_anonymous_text(&line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn append_link_text(
    node: &HtmlDocumentNode,
    current_line: &mut String,
    completed_lines: &mut Vec<String>,
) {
    match node {
        HtmlDocumentNode::Text(value) => {
            current_line.push_str(value);
        }
        HtmlDocumentNode::Element { tag, .. } if tag == "br" => {
            completed_lines.push(std::mem::take(current_line));
        }
        HtmlDocumentNode::Element { children, .. } => {
            for child in children {
                append_link_text(child, current_line, completed_lines);
            }
        }
    }
}

pub(super) fn is_css_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{000c}')
}

pub(super) fn collapsed_inline_whitespace(text: &str) -> String {
    let mut normalized = collapsed_anonymous_text(text);
    if text.chars().next().is_some_and(is_css_whitespace) {
        normalized.insert(0, ' ');
    }
    if !normalized.is_empty()
        && text.chars().next_back().is_some_and(is_css_whitespace)
        && !normalized.ends_with(' ')
    {
        normalized.push(' ');
    }
    normalized
}

fn text_box_width(text: &str, available: f32, style: &CssStyle) -> f32 {
    let explicit = style.box_width(available).min(available);
    if !style.inline_block || style.width.is_some() || style.max_width.is_some() {
        return explicit.max(MIN_LAYOUT_WIDTH);
    }
    let content = text_width(text, style);
    style
        .intrinsic_outer_width(content)
        .min(available)
        .max(MIN_LAYOUT_WIDTH)
}

fn text_box_height(lines: &[String], style: &CssStyle) -> f32 {
    let content_height = lines.len() as f32 * style.line_height;
    style.height.map_or_else(
        || {
            style
                .intrinsic_outer_height(content_height)
                .max(style.minimum_outer_height())
        },
        |height| style.outer_height(height).max(style.minimum_outer_height()),
    )
}

fn link_style(style: &CssStyle) -> CssStyle {
    let mut style = style.clone();
    if !style.explicit_color {
        style.color = "#0000ee".to_string();
    }
    if !style.explicit_text_decoration {
        style.underline = true;
    }
    style
}

#[cfg(test)]
mod tests {
    use super::super::super::html_browser::HtmlBrowserSource;
    use super::super::super::html_browser::HtmlBrowserViewport;
    use super::super::session::HtmlInteractiveSession;
    use super::super::tests::test_error;
    use super::{CssStyle, HtmlLayoutRenderer, text_box_width};
    use std::collections::HashMap;

    fn start_public_session(html: &str) -> Result<HtmlInteractiveSession, String> {
        let source =
            HtmlBrowserSource::new(html, "https://example.test/index.html").map_err(test_error)?;
        let viewport = HtmlBrowserViewport::new(320, 200, 1.0).map_err(test_error)?;
        HtmlInteractiveSession::start(source, viewport).map_err(test_error)
    }

    fn anchor_height(session: &mut HtmlInteractiveSession, id: &str) -> Result<f32, String> {
        let node_id = session
            .runtime
            .node_for_element_id(id)
            .ok_or(format!("{id} anchor node missing"))?
            .0;
        session
            .hit_targets
            .iter()
            .find(|target| target.node_id == node_id)
            .map(|target| target.height)
            .ok_or(format!("{id} anchor hit target missing"))
    }

    fn anchor_width(session: &mut HtmlInteractiveSession, id: &str) -> Result<f32, String> {
        let node_id = session
            .runtime
            .node_for_element_id(id)
            .ok_or(format!("{id} anchor node missing"))?
            .0;
        session
            .hit_targets
            .iter()
            .find(|target| target.node_id == node_id)
            .map(|target| target.width)
            .ok_or(format!("{id} anchor hit target missing"))
    }

    fn assert_anchor_br_line_heights(html: &str) -> Result<(), String> {
        start_public_session(html).and_then(|mut session| {
            assert_eq!(anchor_height(&mut session, "only")?, 20.0);
            assert_eq!(anchor_height(&mut session, "one")?, 20.0);
            assert_eq!(anchor_height(&mut session, "two")?, 40.0);
            assert_eq!(anchor_height(&mut session, "one-two")?, 40.0);
            assert_eq!(anchor_height(&mut session, "after")?, 20.0);
            Ok(())
        })
    }

    #[test]
    fn border_box_auto_link_height_includes_padding() {
        let style = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "display:block;box-sizing:border-box;padding:9px 10px;line-height:24px".to_string(),
            )],
            &CssStyle::browser_default(),
        );
        assert_eq!(super::text_box_height(&["Link".to_string()], &style), 42.0);
        let explicit = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "box-sizing:border-box;padding:9px 10px;line-height:24px;height:50px".to_string(),
            )],
            &CssStyle::browser_default(),
        );
        assert_eq!(
            super::text_box_height(&["Link".to_string()], &explicit),
            50.0
        );
    }

    #[test]
    fn paragraph_keeps_nested_strong_and_span_styles() -> Result<(), String> {
        let document = crate::renderer::backends::html_document::HtmlDocument::parse(
            "<p>Normal <strong>Important</strong> <span style='color: #ff0000'>Red</span></p>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let viewport = HtmlBrowserViewport::new(320, 200, 1.0).map_err(test_error)?;
        let result = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;
        assert!(result.svg.contains("font-weight=\"700\""), "{}", result.svg);
        assert!(result.svg.contains("fill=\"#ff0000\""), "{}", result.svg);
        Ok(())
    }

    #[test]
    fn inline_block_text_shrinks_to_content_and_respects_explicit_width() {
        let mut style = CssStyle::browser_default();
        style.inline_block = true;
        style.padding_left = 6.0;
        style.padding_right = 6.0;
        assert!(text_box_width("Open link", 300.0, &style) < 100.0);

        style.width = Some(super::super::style::CssLength::Px(140.0));
        assert_eq!(text_box_width("Open link", 300.0, &style), 152.0);
    }

    #[test]
    fn empty_text_preserves_vertical_position() {
        let viewport = HtmlBrowserViewport {
            width: 100,
            height: 100,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);

        assert_eq!(
            renderer.render_text(" \n ", 0.0, 42.0, 100.0, &CssStyle::browser_default()),
            42.0
        );
    }

    #[test]
    fn public_empty_block_anchor_keeps_css_box_geometry() -> Result<(), String> {
        start_public_session(
            r#"<main><a id="empty" href="/empty" style="display:block;box-sizing:border-box;padding:9px 10px;line-height:24px;min-height:50px"></a></main>"#,
        )
        .and_then(|mut session| {
            assert_eq!(anchor_height(&mut session, "empty")?, 50.0);
            Ok(())
        })
    }

    #[test]
    fn public_empty_block_anchor_padding_and_border_form_line_box() -> Result<(), String> {
        start_public_session(
            r#"<main><a id="empty" href="/empty" style="display:block;padding:9px 10px;border:2px solid red;line-height:24px"></a></main>"#,
        )
        .and_then(|mut session| {
            assert_eq!(anchor_height(&mut session, "empty")?, 22.0);
            Ok(())
        })
    }

    #[test]
    fn public_anchor_trailing_newline_collapses_in_normal_flow() -> Result<(), String> {
        start_public_session(
            "<main><a id='one' href='/one' style='display:block;line-height:20px'>one</a><a id='two' href='/two' style='display:block;line-height:20px'>two\n</a><a id='three' href='/three' style='display:block;line-height:20px;white-space:nowrap'>three</a><a id='four' href='/four' style='display:block;line-height:20px;white-space:nowrap'>four\n</a></main>",
        )
        .and_then(|mut session| {
            assert_eq!(
                anchor_height(&mut session, "two")?,
                anchor_height(&mut session, "one")?,
                "normal white-space must collapse trailing newline"
            );
            assert_eq!(
                anchor_height(&mut session, "four")?,
                anchor_height(&mut session, "three")?
            );
            Ok(())
        })
    }

    #[test]
    fn public_anchor_br_keeps_explicit_line_break() -> Result<(), String> {
        start_public_session(
            "<main><a id='source' href='/source' style='display:block;line-height:20px'>one\ntwo</a><a id='br' href='/br' style='display:block;line-height:20px'>one<br>two</a></main>",
        )
        .and_then(|mut session| {
            assert!(anchor_height(&mut session, "br")? > anchor_height(&mut session, "source")?);
            Ok(())
        })
    }

    #[test]
    fn public_anchor_br_does_not_add_phantom_trailing_line() -> Result<(), String> {
        assert_anchor_br_line_heights(
            "<main><a id='only' href='/only' style='display:block;line-height:20px'><br></a><a id='one' href='/one' style='display:block;line-height:20px'>one<br></a><a id='two' href='/two' style='display:block;line-height:20px'><br><br></a><a id='one-two' href='/one-two' style='display:block;line-height:20px'>one<br>two</a><a id='after' href='/after' style='display:block;line-height:20px'>one<br> \n</a></main>",
        )
    }

    #[test]
    fn public_nowrap_anchor_br_does_not_add_phantom_trailing_line() -> Result<(), String> {
        assert_anchor_br_line_heights(
            "<main><a id='only' href='/only' style='display:block;line-height:20px;white-space:nowrap'><br></a><a id='one' href='/one' style='display:block;line-height:20px;white-space:nowrap'>one<br></a><a id='two' href='/two' style='display:block;line-height:20px;white-space:nowrap'><br><br></a><a id='one-two' href='/one-two' style='display:block;line-height:20px;white-space:nowrap'>one<br>two</a><a id='after' href='/after' style='display:block;line-height:20px;white-space:nowrap'>one<br> \n</a></main>",
        )
    }

    #[test]
    fn public_anchor_br_preserves_mixed_text_and_nbsp() -> Result<(), String> {
        start_public_session(
            "<main><a id='mixed' href='/mixed' style='display:block;line-height:20px'>one\n two<br>three\n four</a><a id='mixed-nowrap' href='/mixed-nowrap' style='display:block;line-height:20px;white-space:nowrap'>one\n two<br>three\n four</a><a id='nbsp' href='/nbsp' style='display:inline-block;line-height:20px'>one&nbsp;&nbsp;two</a><a id='space' href='/space' style='display:inline-block;line-height:20px'>one  two</a></main>",
        )
        .and_then(|mut session| {
            assert_eq!(anchor_height(&mut session, "mixed")?, 40.0);
            assert_eq!(anchor_height(&mut session, "mixed-nowrap")?, 40.0);
            assert!(anchor_width(&mut session, "nbsp")? > anchor_width(&mut session, "space")?);
            Ok(())
        })
    }

    #[test]
    fn public_nested_anchor_text_preserves_br_and_nbsp() -> Result<(), String> {
        start_public_session(
            "<main><a id='flat' href='/flat' style='display:inline-block;line-height:20px'>alpha beta<br>gamma&nbsp;&nbsp;delta</a><a id='nested' href='/nested' style='display:inline-block;line-height:20px'><span>alpha \n</span><span> beta</span><span><br></span><span>gamma&nbsp;&nbsp;delta</span></a><a id='nested-nbsp' href='/nested-nbsp' style='display:inline-block;line-height:20px'><span>gamma&nbsp;&nbsp;delta</span></a><a id='nested-space' href='/nested-space' style='display:inline-block;line-height:20px'><span>gamma  delta</span></a><a id='flat-nowrap' href='/flat-nowrap' style='display:inline-block;line-height:20px;white-space:nowrap'>alpha beta<br>gamma&nbsp;&nbsp;delta</a><a id='nested-nowrap' href='/nested-nowrap' style='display:inline-block;line-height:20px;white-space:nowrap'><span>alpha \n</span><span> beta</span><span><br></span><span>gamma&nbsp;&nbsp;delta</span></a></main>",
        )
        .and_then(|mut session| {
            assert_eq!(anchor_height(&mut session, "nested")?, 40.0);
            assert_eq!(
                anchor_height(&mut session, "nested")?,
                anchor_height(&mut session, "flat")?
            );
            assert_eq!(
                anchor_width(&mut session, "nested")?,
                anchor_width(&mut session, "flat")?
            );
            assert_eq!(
                anchor_height(&mut session, "nested-nowrap")?,
                anchor_height(&mut session, "flat-nowrap")?
            );
            assert_eq!(anchor_height(&mut session, "nested-nowrap")?, 40.0);
            assert_eq!(
                anchor_width(&mut session, "nested-nowrap")?,
                anchor_width(&mut session, "flat-nowrap")?
            );
            assert!(
                anchor_width(&mut session, "nested-nbsp")?
                    > anchor_width(&mut session, "nested-space")?
            );
            Ok(())
        })
    }

    #[test]
    fn anonymous_text_collapses_source_indentation_without_removing_nonbreaking_spaces() {
        let viewport = HtmlBrowserViewport {
            width: 300,
            height: 100,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let style = CssStyle::browser_default();
        let bottom = renderer.render_text("\n  Section\t title\n", 0.0, 0.0, 300.0, &style);
        assert_eq!(bottom, style.line_height);
        assert_eq!(renderer.svg.matches("<text ").count(), 1);
        assert!(renderer.svg.contains("Section title"));
        assert_eq!(super::collapsed_anonymous_text("A\u{a0}B"), "A\u{a0}B");
        assert_eq!(super::collapsed_anonymous_text("\u{a0}"), "\u{a0}");
    }

    #[test]
    fn link_default_underline_respects_explicit_text_decoration_none() {
        let style = CssStyle::browser_default();
        assert!(super::link_style(&style).underline);

        let without_underline = CssStyle::from_attributes(
            &[("style".to_string(), "text-decoration: none".to_string())],
            &style,
        );
        assert!(!super::link_style(&without_underline).underline);
    }

    #[test]
    fn test_error_preserves_real_invalid_viewport() -> Result<(), String> {
        super::super::tests::assert_invalid_viewport_error()
    }

    #[test]
    fn anchor_helpers_report_missing_nodes_and_hit_targets() -> Result<(), String> {
        let mut session = start_public_session("<a id=link href='#end'>link</a>")?;
        assert_eq!(
            anchor_height(&mut session, "absent").err(),
            Some("absent anchor node missing".to_string())
        );
        assert_eq!(
            anchor_width(&mut session, "absent").err(),
            Some("absent anchor node missing".to_string())
        );
        session.hit_targets.clear();
        assert_eq!(
            anchor_height(&mut session, "link").err(),
            Some("link anchor hit target missing".to_string())
        );
        assert_eq!(
            anchor_width(&mut session, "link").err(),
            Some("link anchor hit target missing".to_string())
        );
        Ok(())
    }
}
