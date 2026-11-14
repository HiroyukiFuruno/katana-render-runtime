use super::request::HtmlFontRequest;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_public_route_tests.rs"]
mod public_routes;

#[test]
fn decodes_xml_entities_before_removing_font_family_quotes() {
    let markup = r#"<text font-family="&apos;DejaVu Sans&apos;, sans-serif">text</text>"#;

    assert_eq!(
        HtmlFontRequest::from_markup(markup).families,
        vec!["dejavu sans", "sans-serif"]
    );
}

#[test]
fn markup_and_text_font_requests_share_the_decoded_family() {
    let markup = r#"<text font-family="&apos;DejaVu Sans&apos;">text</text>"#;

    assert_eq!(
        HtmlFontRequest::from_markup(markup).families,
        HtmlFontRequest::from_text("&apos;DejaVu Sans&apos;", "text").families
    );
}
