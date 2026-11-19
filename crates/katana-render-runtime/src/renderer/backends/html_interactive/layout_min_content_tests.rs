use super::{CssStyle, HtmlDocumentNode, longest_unbreakable_width, measured_text_width};
use crate::renderer::backends::html_browser::{HtmlBrowserSource, HtmlBrowserViewport};
use crate::renderer::backends::html_interactive::HtmlInteractiveSession;
use crate::renderer::backends::html_interactive::tests::test_error;

#[test]
fn session_sizes_min_content_to_the_longest_word() -> Result<(), String> {
    let html = "<style>body{margin:24px;font-family:system-ui} \
        p{width:min-content;max-width:100%;border:1px solid;padding:16px}</style> \
        <p id='min-content'>min-content の折り返し境界を観察する日本語の架空説明文です \
        unbreakabletoken</p>";
    let source =
        HtmlBrowserSource::new(html, "https://example.test/layout.html").map_err(test_error)?;
    let viewport = HtmlBrowserViewport::new(620, 900, 1.0).map_err(test_error)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(test_error)?;
    let id = session
        .runtime
        .node_for_element_id("min-content")
        .ok_or("fixture paragraph is missing")?;
    let rect = session
        .element_boxes
        .iter()
        .find(|rect| rect.node_id == id.0)
        .ok_or("fixture paragraph box is missing")?;
    let mut style = CssStyle::browser_default();
    style.font_family = "system-ui".into();
    let expected = measured_text_width("unbreakabletoken", &style) + 34.0;
    assert!((rect.width - expected).abs() < 1.0, "{rect:?} {expected}");
    Ok(())
}

#[test]
fn min_content_uses_css_breaks_and_preserves_nonbreaking_spaces() {
    let mut style = CssStyle::browser_default();
    assert_eq!(
        longest_unbreakable_width("日 日本日 ab\u{a0}cd", &style),
        measured_text_width("ab\u{a0}cd", &style)
    );
    style.white_space = super::CssWhiteSpace::NoWrap;
    assert_eq!(
        longest_unbreakable_width("日 本 ab\u{a0}cd", &style),
        measured_text_width("日 本 ab\u{a0}cd", &style)
    );
}

#[test]
fn min_content_respects_box_constraints_and_the_last_valid_width() {
    let text = &[HtmlDocumentNode::Text("unbreakabletoken".into())];
    for (declarations, expected) in [
        ("width:min-content;max-width:25%;box-sizing:border-box", 50.0),
        ("width:min-content;max-width:25%;padding:5px", 60.0),
        ("width:min-content;min-width:150px", 150.0),
        ("width:min-content;width:60px;width:invalid", 60.0),
        ("width:min-content;width:auto", 200.0),
    ] {
        let attributes = [("style".into(), declarations.into())];
        let mut style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        style.resolve_min_content_width(text);
        assert_eq!(style.box_width(200.0), expected, "{declarations}");
    }
}
