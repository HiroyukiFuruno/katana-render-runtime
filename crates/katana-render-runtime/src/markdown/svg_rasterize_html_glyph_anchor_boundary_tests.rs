use super::{
    HtmlGlyphRasterizer, Prepared, anchor, fixture_text, fixture_tree_with_content, font_fixture,
};

#[test]
fn layout_unit_and_paint_size_reject_unrepresentable_values() {
    for value in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        33_554_432.0,
        -33_554_432.0,
    ] {
        assert_eq!(anchor::trunc_layout_unit(value), None);
    }
    for (value, expected) in [(1.02, 1.015625), (-1.02, -1.015625)] {
        assert_eq!(anchor::trunc_layout_unit(value), Some(expected));
    }
    assert_eq!(anchor::paint_size(f32::MAX, 2.0, 2.0), None);
}

#[test]
fn decorated_single_and_multiple_glyph_runs_do_not_use_anchor_shortcuts() -> Result<(), String> {
    let fixture = font_fixture("decorated-anchor-boundary")?;
    for decoration in ["underline", "overline", "line-through"] {
        for content in ["A", "AB"] {
            assert_decorated_case(&fixture, decoration, content)?;
        }
    }
    Ok(())
}

fn assert_decorated_case(
    fixture: &crate::markdown::svg_rasterize::font_file_fixture::FontFileFixture,
    decoration: &str,
    content: &str,
) -> Result<(), String> {
    let attributes = format!("text-decoration='{decoration}'");
    let tree = fixture_tree_with_content(fixture, &attributes, content)?;
    let text = fixture_text(&tree)?;
    let (chunk, source) = single_source(text)?;
    assert_eq!(chunk.text(), content);
    assert_decoration(source, decoration);
    let glyph_count: usize = text
        .layouted()
        .iter()
        .map(|span| span.positioned_glyphs.len())
        .sum();
    assert_eq!(glyph_count, content.chars().count());
    if content == "A" {
        assert!(anchor::layout_anchor(text).is_none());
    } else {
        assert_decorated_run(&tree, text, content)?;
    }
    Ok(())
}

fn assert_decoration(source: &resvg::usvg::TextSpan, decoration: &str) {
    let style = source.decoration();
    assert!(match decoration {
        "underline" => style.underline().is_some(),
        "overline" => style.overline().is_some(),
        _ => style.line_through().is_some(),
    });
}

fn single_source(
    text: &resvg::usvg::Text,
) -> Result<(&resvg::usvg::TextChunk, &resvg::usvg::TextSpan), String> {
    let [chunk] = text.chunks() else {
        return Err("expected one chunk".into());
    };
    let [source] = chunk.spans() else {
        return Err("expected one source span".into());
    };
    Ok((chunk, source))
}

fn assert_decorated_run(
    tree: &resvg::usvg::Tree,
    text: &resvg::usvg::Text,
    content: &str,
) -> Result<(), String> {
    let (chunk, source) = single_source(text)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("decorated run was not prepared".into());
    };
    let [span] = spans.as_slice() else {
        return Err("expected one prepared span".into());
    };
    assert!(super::super::alignment_text_supported(
        text, chunk, source, span
    ));
    assert!(super::super::horizontal_glyphs(&span.glyphs));
    assert!(anchor::one_ordered_scalar_per_glyph(content, &span.glyphs));
    assert!(anchor::run_origin_css_scale(text, &spans).is_none());
    Ok(())
}

#[test]
fn differently_painted_source_spans_reject_single_span_anchor_shortcuts() -> Result<(), String> {
    let fixture = font_fixture("multi-source-anchor-boundary")?;
    let tree = fixture_tree_with_content(
        &fixture,
        "",
        "<tspan fill='red'>A</tspan><tspan fill='blue'>B</tspan>",
    )?;
    let text = fixture_text(&tree)?;
    let [chunk] = text.chunks() else {
        return Err("expected one text chunk".into());
    };
    assert_eq!(chunk.spans().len(), 2);
    assert!(anchor::layout_anchor(text).is_none());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("painted spans were not prepared".into());
    };
    assert_eq!(spans.len(), 2);
    assert!(anchor::run_origin_css_scale(text, &spans[..1]).is_none());
    Ok(())
}
