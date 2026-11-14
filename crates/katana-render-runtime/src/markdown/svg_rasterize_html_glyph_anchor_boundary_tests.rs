use super::{
    HtmlGlyphRasterizer, Prepared, anchor, fixture_text, fixture_tree_with_content, font_fixture,
};

#[test]
fn tiny_svg_scale_preserves_anchor_when_css_paint_size_rounds_to_zero() -> Result<(), String> {
    let fixture = font_fixture("tiny-css-anchor-boundary")?;
    let tree = fixture_tree_with_content(&fixture, "x='50' y='20' transform='scale(.0005)'", "A")?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("tiny text was not prepared".into());
    };
    let [span] = spans.as_slice() else {
        return Err("expected one prepared span".into());
    };
    let [glyph] = span.glyphs.as_slice() else {
        return Err("expected one positioned glyph".into());
    };
    let alignment = span.x_alignment.ok_or("text alignment missing")?;
    let layout = alignment.layout.ok_or("layout anchor missing")?;
    let scale = text.abs_transform().sx;
    assert!(span.font_size * scale > 0.0);
    assert_eq!(
        anchor::paint_size(span.font_size, layout.css_scale, scale),
        None
    );
    let transformed = super::super::paint::normalized_transform(span, glyph, scale);
    assert_eq!(transformed.tx, alignment.anchor_x * scale);
    assert_eq!(transformed.ty, glyph.transform().ty * scale);
    Ok(())
}

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
fn explicit_backward_glyph_position_rejects_the_ordered_anchor_shortcut() -> Result<(), String> {
    let fixture = font_fixture("backward-anchor-boundary")?;
    for (attributes, ordered) in [("x='50' y='20'", true), ("x='50' y='20' dx='0 -40'", false)] {
        let tree = fixture_tree_with_content(&fixture, attributes, "AB")?;
        let text = fixture_text(&tree)?;
        let (chunk, _) = single_source(text)?;
        let [span] = text.layouted() else {
            return Err("expected one shaped glyph span".into());
        };
        let glyphs = &span.positioned_glyphs;
        let [first, second] = glyphs.as_slice() else {
            return Err("expected two positioned glyphs".into());
        };
        assert_eq!((first.text.as_str(), second.text.as_str()), ("A", "B"));
        assert_eq!(first.transform().tx < second.transform().tx, ordered);
        assert_eq!(
            anchor::first_ordered_scalar_glyph(chunk.text(), glyphs).is_some(),
            ordered
        );
    }
    Ok(())
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
    assert!(anchor::first_ordered_scalar_glyph(content, &span.glyphs).is_some());
    assert!(anchor::run_origin(text, &spans).is_none());
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
    assert!(anchor::run_origin(text, &spans[..1]).is_none());
    Ok(())
}

#[test]
fn second_chunk_per_glyph_dy_rejects_all_origin_snapping() -> Result<(), String> {
    let fixture = font_fixture("later-chunk-dy")?;
    let control = "<tspan x='20' y='30'>AB</tspan><tspan x='50' y='50'>CD</tspan>";
    let shifted = "<tspan x='20' y='30'>AB</tspan><tspan x='50' y='50' dy='0 1'>CD</tspan>";
    for (content, expected) in [(control, true), (shifted, false)] {
        let tree = fixture_tree_with_content(&fixture, "", content)?;
        assert_two_chunk_snapping(&tree, expected)?;
    }
    Ok(())
}

fn assert_two_chunk_snapping(tree: &resvg::usvg::Tree, expected: bool) -> Result<(), String> {
    let text = fixture_text(tree)?;
    assert_eq!(text.chunks().len(), 2);
    assert_eq!(text.layouted().len(), 2);
    assert_eq!(
        text.dy().get(3).copied(),
        Some(if expected { 0.0 } else { 1.0 })
    );
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("two-chunk SVG did not prepare".into());
    };
    assert_eq!(spans.len(), 2);
    for span in &spans {
        assert!(super::super::single_horizontal_alignment(text, span).is_none());
        assert_eq!(
            span.x_alignment
                .is_some_and(|alignment| alignment.origin_snap_css_scale.is_some()),
            expected
        );
    }
    Ok(())
}

#[test]
fn single_horizontal_alignment_rejects_a_rotated_shaped_glyph() -> Result<(), String> {
    let fixture = font_fixture("rotated-shaped-glyph")?;
    let plain = fixture_tree_with_content(&fixture, "x='50' y='30'", "AB")?;
    let rotated = fixture_tree_with_content(&fixture, "x='50' y='30' rotate='15'", "AB")?;
    let text = fixture_text(&plain)?;
    let rasterizer = HtmlGlyphRasterizer::new(plain.fontdb());
    let Prepared::Ready(mut spans) = rasterizer.prepare(text)? else {
        return Err("plain SVG did not prepare".into());
    };
    let [span] = spans.as_mut_slice() else {
        return Err("expected one prepared span".into());
    };
    assert_eq!(
        super::super::single_horizontal_alignment(text, span),
        Some(50.0)
    );
    span.glyphs = rotated_glyphs(&rotated)?;
    assert!(!super::super::horizontal_glyphs(&span.glyphs));
    assert!(super::super::single_horizontal_alignment(text, span).is_none());
    Ok(())
}

fn rotated_glyphs(
    tree: &resvg::usvg::Tree,
) -> Result<Vec<resvg::usvg::layout::PositionedGlyph>, String> {
    let [span] = fixture_text(tree)?.layouted() else {
        return Err("expected one rotated glyph span".into());
    };
    assert!(
        span.positioned_glyphs
            .iter()
            .any(|glyph| glyph.transform().kx != 0.0)
    );
    Ok(span.positioned_glyphs.clone())
}
