use super::{
    block_tags_to_breaks, body_content, mark_headings, remove_tag_blocks, replace_image_alt,
};

#[test]
fn body_and_block_parsers_cover_missing_body() {
    let html = r#"<h2>Title</h2><pre><code>plain
text</code></pre><script>skip</script><style>skip</style><p>body</p>"#;
    let body = body_content(html);

    assert!(body.as_ref().is_ok_and(|it| it.contains("<h2>Title</h2>")));
    assert!(
        remove_tag_blocks(html, "script")
            .as_ref()
            .is_ok_and(|it| !it.contains("skip</script>"))
    );
    assert!(
        block_tags_to_breaks(html)
            .as_ref()
            .is_ok_and(|it| it.contains('\n'))
    );
}

#[test]
fn heading_and_image_alt_parsers_handle_empty_captures() {
    let heading = mark_headings("<h6></h6>");
    let image = replace_image_alt(r#"<img alt="A &amp; B">"#);
    let no_alt = replace_image_alt(r#"<img src="a.png">"#);

    assert!(heading.as_ref().is_ok_and(|it| it.contains("h6")));
    assert!(image.as_ref().is_ok_and(|it| it.contains("A & B")));
    assert!(no_alt.as_ref().is_ok_and(|it| it.contains("a.png")));
}

#[test]
fn tag_replacement_handles_unclosed_and_empty_tags() {
    let unclosed = block_tags_to_breaks("<p>open <");
    let empty = block_tags_to_breaks("<>");

    assert!(unclosed.as_ref().is_ok_and(|it| it.contains("open <")));
    assert!(empty.as_ref().is_ok_and(|it| it.contains("<>")));
}
