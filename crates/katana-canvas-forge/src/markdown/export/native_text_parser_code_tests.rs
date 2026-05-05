use super::{extract_code_blocks, highlight_result};

#[test]
fn extracts_code_blocks_with_dark_theme() {
    let html = r#"<p>before</p><pre><code class="language-rust">fn main() {
}
</code></pre><p>after</p>"#;
    let result = extract_code_blocks(html, true);

    assert!(result.as_ref().is_ok_and(|(_, blocks)| blocks.len() == 1));
    assert!(
        result
            .as_ref()
            .is_ok_and(|(html, _)| html.contains("\u{0004}0\u{0004}"))
    );
}

#[test]
fn leaves_html_without_code_blocks_unchanged() {
    let html = "<p>body</p>";
    let result = extract_code_blocks(html, false);

    assert!(
        result
            .as_ref()
            .is_ok_and(|(rendered, blocks)| { rendered == html && blocks.is_empty() })
    );
}

#[test]
fn extracts_plain_code_block_without_language() {
    let result = extract_code_blocks("<pre><code>plain\n</code></pre>", false);

    assert!(result.as_ref().is_ok_and(|(_, blocks)| blocks.len() == 1));
    assert!(
        result
            .as_ref()
            .is_ok_and(|(_, blocks)| blocks[0].iter().any(|line| line.text == "plain"))
    );
}

#[test]
fn highlight_result_preserves_syntect_errors() {
    let error = syntect::Error::Io(std::io::Error::other("highlight failed"));

    assert!(highlight_result::<()>(Err(error)).is_err());
}
