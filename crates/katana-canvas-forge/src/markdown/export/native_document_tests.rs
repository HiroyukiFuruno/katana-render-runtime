use super::NativeHtmlDocument;

#[test]
fn render_image_keeps_empty_document_blank() {
    let document = NativeHtmlDocument::parse("<html><body></body></html>");
    let image = document.and_then(|it| it.render_image());

    assert!(image.as_ref().is_ok_and(|it| it.width > 0));
    assert!(image.as_ref().is_ok_and(|it| it.height > 0));
}

#[test]
fn render_image_truncates_excessive_text_blocks() {
    let body = (0..605)
        .map(|index| format!("<p>line {index}</p>"))
        .collect::<Vec<_>>()
        .join("");
    let html = format!("<html><body>{body}</body></html>");
    let document = NativeHtmlDocument::parse(&html);
    let image = document.and_then(|it| it.render_image());

    assert!(image.as_ref().is_ok_and(|it| it.height > 0));
}

#[test]
fn render_image_reports_svg_rasterize_errors() {
    let html = r#"<html><body><svg width="10" height="10"><rect></svg></body></html>"#;
    let image = NativeHtmlDocument::parse(html).and_then(|it| it.render_image());

    assert!(image.is_err());
}
