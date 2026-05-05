use super::{NativeDocumentBlock, NativeDocumentBlocks};

#[test]
fn parse_handles_svg_dimensions_from_view_box_and_attributes() {
    let html = r#"
        <p>before</p>
        <svg viewBox="0 0 10.2 20.1"><rect /></svg>
        <svg width="12px" height="8px"><rect /></svg>
        <p>after</p>
    "#;
    let blocks = NativeDocumentBlocks::parse(html, false);

    assert!(blocks.as_ref().is_ok_and(|it| it.len() >= 4));
    assert!(blocks.as_ref().is_ok_and(|it| {
        it.iter().any(|block| matches!(block, NativeDocumentBlock::Svg(svg) if svg.width == 11 && svg.height == 21))
    }));
    assert!(blocks.as_ref().is_ok_and(|it| {
        it.iter().any(|block| matches!(block, NativeDocumentBlock::Svg(svg) if svg.width == 12 && svg.height == 8))
    }));
}

#[test]
fn parse_uses_default_size_for_invalid_svg_metadata_and_nested_svg() {
    let html = r#"
        <svg viewBox="0 0 bad 0"><rect /></svg>
        <svg viewBox="0 0 0 10"><rect /></svg>
        <svg><g><svg width="2" height="2"></svg></g></svg>
        <not-svg></not-svg>
    "#;
    let blocks = NativeDocumentBlocks::parse(html, false);

    assert!(blocks.as_ref().is_ok_and(|it| {
        it.iter().any(|block| matches!(block, NativeDocumentBlock::Svg(svg) if svg.width == 320 && svg.height == 320))
    }));
}

#[test]
fn svg_range_finder_skips_unclosed_and_non_boundary_tags() {
    let html = r#"
        <svgx></svgx>
        <svg width="7" height="9"><rect /></svg>
    "#;
    let blocks = NativeDocumentBlocks::parse(html, false);
    let unclosed = NativeDocumentBlocks::parse("<svg><g>", false);

    assert!(blocks.as_ref().is_ok_and(|it| {
        it.iter().any(|block| matches!(block, NativeDocumentBlock::Svg(svg) if svg.width == 7 && svg.height == 9))
    }));
    assert!(unclosed.as_ref().is_ok_and(Vec::is_empty));
}

#[test]
fn parse_rejects_malformed_svg_dimension_attribute() {
    let parsed = NativeDocumentBlocks::parse(r#"<svg width="10><rect /></svg>"#, false);

    assert!(parsed.is_err());
}
