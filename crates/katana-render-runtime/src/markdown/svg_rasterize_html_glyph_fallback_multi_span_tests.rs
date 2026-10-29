use super::super::super::{HtmlGlyphRasterizer, Prepared, PreparedSpan, anchor};
use super::super::{RunLayout, layout};
use super::{text, tree};
use crate::markdown::svg_rasterize::text_shaping::{
    FontShape, shape_run_character_metrics_with_trim,
};
use resvg::usvg::{Text, Tree};

const SOURCE_SPAN_COUNT: usize = 4;
const PREPARED_RUN_COUNT: usize = 5;
const SOURCE_TEXT: [&str; SOURCE_SPAN_COUNT] = ["A", "V日本", "API", "日本"];
const RUN_TEXT: [&str; PREPARED_RUN_COUNT] = ["A", "V", "日本", "API", "日本"];

#[test]
fn multiple_source_spans_preserve_utf8_face_boundaries_and_shaping() -> Result<(), String> {
    let tree = fixture("")?;
    let text = text(&tree)?;
    let prepared = prepare(&tree, text)?;
    assert_source_boundaries(text, &prepared)?;
    let layouts = layout(tree.fontdb(), text, &prepared)?.ok_or("multi-span layout rejected")?;
    assert_eq!(layouts.len(), RUN_TEXT.len());
    assert_source_fragment_positions(&tree, text, &prepared, &layouts)?;
    Ok(())
}

#[test]
fn unsupported_noninitial_source_span_is_rejected() -> Result<(), String> {
    for attribute in ["letter-spacing='1'", "word-spacing='1'", "textLength='50'"] {
        let tree = fixture(attribute)?;
        let text = text(&tree)?;
        let prepared = prepare(&tree, text)?;
        assert_eq!(text.chunks().len(), 1);
        assert_eq!(text.chunks()[0].spans().len(), SOURCE_TEXT.len());
        assert!(!super::super::super::alignment_source_span_supported(
            &text.chunks()[0].spans()[1]
        ));
        assert!(
            layout(tree.fontdb(), text, &prepared)?.is_none(),
            "{attribute}"
        );
    }
    Ok(())
}

fn fixture(second_attributes: &str) -> Result<Tree, String> {
    let content = format!(
        "<tspan>A</tspan><tspan {second_attributes}>V日本</tspan><tspan>API</tspan><tspan>日本</tspan>"
    );
    tree(&content, "")
}

fn prepare(tree: &Tree, text: &Text) -> Result<Vec<PreparedSpan>, String> {
    let Prepared::Ready(prepared) = HtmlGlyphRasterizer::new(tree.fontdb()).prepare(text)? else {
        return Err("multi-span fixture was not prepared".to_owned());
    };
    Ok(prepared)
}

fn assert_source_boundaries(text: &Text, prepared: &[PreparedSpan]) -> Result<(), String> {
    assert_eq!(text.chunks().len(), 1);
    let chunk = &text.chunks()[0];
    assert_eq!(chunk.spans().len(), SOURCE_TEXT.len());
    assert_eq!(text.layouted().len(), SOURCE_TEXT.len());
    assert_eq!(prepared.len(), RUN_TEXT.len());
    let mut offset = 0;
    for (span, expected) in chunk.spans().iter().zip(SOURCE_TEXT) {
        assert_eq!(span.start(), offset);
        offset += expected.len();
        assert_eq!(span.end(), offset);
        assert_eq!(chunk.text().get(span.start()..span.end()), Some(expected));
    }
    assert_eq!(offset, chunk.text().len());
    assert_run_faces(prepared)?;
    assert_glyph_identity(text, prepared);
    assert_run_text(prepared);
    Ok(())
}

fn assert_run_faces(prepared: &[PreparedSpan]) -> Result<(), String> {
    let [first, second, japanese, api, last] = prepared else {
        return Err("expected five prepared font runs".to_owned());
    };
    assert_eq!(first.font, second.font);
    assert_eq!(first.font, api.font);
    assert_eq!(japanese.font, last.font);
    assert_ne!(first.font, japanese.font);
    Ok(())
}

fn assert_glyph_identity(text: &Text, prepared: &[PreparedSpan]) {
    assert_eq!(
        text.layouted()
            .iter()
            .map(|span| span.positioned_glyphs.len())
            .sum::<usize>(),
        prepared.iter().map(|span| span.glyphs.len()).sum::<usize>()
    );
    let source = text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs);
    let glyphs = prepared.iter().flat_map(|span| &span.glyphs);
    for (source, glyph) in source.zip(glyphs) {
        assert_eq!(source.id, glyph.id);
        assert_eq!(source.font, glyph.font);
        assert_eq!(source.text, glyph.text);
    }
}

fn assert_run_text(prepared: &[PreparedSpan]) {
    for (span, expected) in prepared.iter().zip(RUN_TEXT) {
        assert_eq!(
            span.glyphs
                .iter()
                .map(|glyph| glyph.text.as_str())
                .collect::<String>(),
            expected
        );
        assert_eq!(span.glyph_ids.len(), expected.chars().count());
    }
}

fn assert_source_fragment_positions(
    tree: &Tree,
    text: &Text,
    prepared: &[PreparedSpan],
    layouts: &[RunLayout],
) -> Result<(), String> {
    let source = &text.chunks()[0].spans()[0];
    let css_scale = anchor::uniform_css_scale(text.abs_transform()).ok_or("scale missing")?;
    let mut cursor = 0.0_f64;
    for (span, layout) in prepared.iter().zip(layouts) {
        let shape = FontShape::new(layout.paint_css_size, source.font().weight(), false);
        let content = span
            .glyphs
            .iter()
            .map(|glyph| glyph.text.as_str())
            .collect::<String>();
        let metrics =
            shape_run_character_metrics_with_trim(tree.fontdb(), span.font, &content, shape, None)
                .ok_or("fragment metrics missing")?;
        assert_eq!(metrics.len(), span.glyphs.len());
        let origin = text.chunks()[0].x().unwrap_or(0.0) * css_scale;
        assert_positions(span, layout, metrics, origin, &mut cursor)?;
    }
    Ok(())
}

fn assert_positions(
    span: &PreparedSpan,
    layout: &RunLayout,
    metrics: Vec<(f32, f32)>,
    origin: f32,
    cursor: &mut f64,
) -> Result<(), String> {
    assert_eq!(layout.css_positions.len(), span.glyphs.len());
    for ((position, glyph), (advance, offset)) in
        layout.css_positions.iter().zip(&span.glyphs).zip(metrics)
    {
        let value = (*cursor + f64::from(offset)) as f32;
        let snapped = anchor::trunc_layout_unit(value).ok_or("cursor out of range")?;
        let float = std::env::var("KRR_HTML_FEATURE_FLOAT_FALLBACK_ADVANCES").as_deref() == Ok("1");
        assert_eq!(position.x, origin + if float { value } else { snapped });
        assert_eq!(
            position.y,
            (glyph.transform().ty + span.baseline_shift) * layout.css_scale
        );
        *cursor += f64::from(advance);
    }
    Ok(())
}
