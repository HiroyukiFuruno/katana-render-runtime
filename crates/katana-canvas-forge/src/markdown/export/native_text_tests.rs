use super::{NativeTextLine, extract_lines, is_dark_background, parse_hex_rgb};

#[test]
fn constructors_and_color_parsing_cover_heading_code_and_short_hex() {
    let h2 = NativeTextLine::heading("heading".to_string(), 2);
    let h4 = NativeTextLine::heading("small".to_string(), 4);
    let code = NativeTextLine::code_highlighted("let x = 1;".to_string(), vec![]);

    assert!(h2.is_heading());
    assert!(!h4.is_heading());
    assert!(code.is_code);
    assert_eq!(parse_hex_rgb("#abc"), Some([0xaa, 0xbb, 0xcc]));
    assert_eq!(parse_hex_rgb("#abcd"), None);
    assert!(is_dark_background("#000"));
    assert!(!is_dark_background("not-a-color"));
}

#[test]
fn extract_lines_handles_headings_code_and_image_alt() {
    let html = r#"
        <html><body>
        <h1><em>Title</em></h1>
        <p>body &amp; text <img alt="diagram &lt;one&gt;"></p>
        <pre><code class="language-rust">fn main() {}</code></pre>
        </body></html>
    "#;
    let lines = extract_lines(html, false);

    assert!(
        lines
            .as_ref()
            .is_ok_and(|it| it.iter().any(|line| line.bold))
    );
    assert!(
        lines
            .as_ref()
            .is_ok_and(|it| it.iter().any(|line| line.is_code))
    );
    assert!(
        lines
            .as_ref()
            .is_ok_and(|it| it.iter().any(|line| line.text.contains("[image:")))
    );
}
