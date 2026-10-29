use super::super::super::{PreparedSpan, anchor, chunk};
use super::super::{RunLayout, layout};
use super::{prepare, text, tree};
use crate::markdown::svg_rasterize::text_shaping::{
    FontShape, shape_run_character_metrics_with_trim,
};
use resvg::usvg::{Text, Tree};

const ROW_COUNT: usize = 2;
const SPAN_COUNT: usize = 4;
const RUN_COUNT: usize = 5;
const FIRST_ROW_RUNS: usize = 3;
const RUN_TEXT: [&str; RUN_COUNT] = ["[Component:", " MVC", " REST", "コントローラー", "]"];
type CharacterMetrics = Vec<(f32, f32)>;

#[test]
fn multiple_chunks_keep_independent_origins_anchors_and_source_shaping() -> Result<(), String> {
    for row_position in ["x='173' y='42'", "x='0' dy='22'"] {
        for (first_anchor, second_anchor) in [("start", "start"), ("middle", "end")] {
            assert_multi_chunk_layout(first_anchor, second_anchor, row_position)?;
        }
    }
    Ok(())
}

fn assert_multi_chunk_layout(
    first_anchor: &str,
    second_anchor: &str,
    row_position: &str,
) -> Result<(), String> {
    let tree = fixture(
        first_anchor,
        second_anchor,
        row_position,
        "",
        "コントローラー]",
    )?;
    let text = text(&tree)?;
    let prepared = prepare(&tree, text)?;
    assert_fixture(text, &prepared);
    if row_position.contains("dy=") {
        let offset = text.chunks()[0].text().chars().count();
        assert_ne!(text.dy()[offset], 0.0);
    }
    let layouts = layout(tree.fontdb(), text, &prepared)?;
    if std::env::var("KRR_HTML_FEATURE_START_ONLY_FALLBACK_RUNS").as_deref() == Ok("1")
        && first_anchor != "start"
    {
        assert!(layouts.is_none());
        return Ok(());
    }
    let layouts = layouts.ok_or("multi-chunk layout was rejected")?;
    assert_eq!(layouts.len(), RUN_COUNT);
    assert_both_rows(&tree, text, &prepared, &layouts)
}

fn assert_both_rows(
    tree: &Tree,
    text: &Text,
    prepared: &[PreparedSpan],
    layouts: &[RunLayout],
) -> Result<(), String> {
    assert_row(
        tree,
        text,
        &prepared[..FIRST_ROW_RUNS],
        &layouts[..FIRST_ROW_RUNS],
        0,
    )?;
    assert_row(
        tree,
        text,
        &prepared[FIRST_ROW_RUNS..],
        &layouts[FIRST_ROW_RUNS..],
        1,
    )
}

#[test]
fn multiple_chunks_without_fallback_do_not_use_fallback_layout() -> Result<(), String> {
    let tree = fixture("start", "start", "x='173' y='42'", "", "controller]")?;
    let text = text(&tree)?;
    let prepared = prepare(&tree, text)?;
    assert_eq!(text.chunks().len(), ROW_COUNT);
    let first = prepared.first().ok_or("no prepared run")?.font;
    assert!(prepared.iter().all(|span| span.font == first));
    assert!(layout(tree.fontdb(), text, &prepared)?.is_none());
    Ok(())
}

#[test]
fn later_chunk_without_x_is_rejected_even_with_font_fallback() -> Result<(), String> {
    let tree = tree("<tspan>AB</tspan><tspan y='40'>A日本</tspan>", "")?;
    let text = text(&tree)?;
    let prepared = prepare(&tree, text)?;
    assert_eq!(text.chunks().len(), ROW_COUNT);
    assert!(text.chunks()[0].x().is_some());
    assert!(text.chunks()[1].x().is_none());
    assert_eq!(text.chunks()[1].text(), "A日本");
    assert_eq!(text.layouted().len(), ROW_COUNT);
    let glyphs = &text.layouted()[1].positioned_glyphs;
    let first = glyphs.first().ok_or("later chunk has no glyphs")?;
    assert!(glyphs.iter().any(|glyph| glyph.font != first.font));
    assert!(layout(tree.fontdb(), text, &prepared)?.is_none());
    Ok(())
}

#[test]
fn nonzero_dy_inside_row_is_rejected() -> Result<(), String> {
    let tree = fixture(
        "start",
        "start",
        "x='0' dy='22'",
        "dy='0 1'",
        "コントローラー]",
    )?;
    let text = text(&tree)?;
    let prepared = prepare(&tree, text)?;
    let offset = text.chunks()[0].text().chars().count();
    assert_ne!(text.dy()[offset + 1], 0.0);
    assert!(layout(tree.fontdb(), text, &prepared)?.is_none());
    Ok(())
}

fn fixture(
    first_anchor: &str,
    second_anchor: &str,
    row_position: &str,
    inner_attributes: &str,
    second_text: &str,
) -> Result<Tree, String> {
    let content = format!(
        "<tspan x='216' y='20' text-anchor='{first_anchor}'><tspan>[Component:</tspan><tspan> MVC</tspan><tspan> REST</tspan></tspan><tspan {row_position} text-anchor='{second_anchor}'><tspan {inner_attributes}>{second_text}</tspan></tspan>"
    );
    tree(&content, "xml:space='preserve'")
}

fn assert_fixture(text: &Text, prepared: &[PreparedSpan]) {
    assert_eq!(text.chunks().len(), ROW_COUNT);
    assert_eq!(text.chunks()[0].spans().len(), FIRST_ROW_RUNS);
    assert_eq!(text.chunks()[1].spans().len(), 1);
    assert_eq!(text.layouted().len(), SPAN_COUNT);
    assert_eq!(prepared.len(), RUN_COUNT);
    assert_ne!(prepared[0].font, prepared[FIRST_ROW_RUNS].font);
    for (span, expected) in prepared.iter().zip(RUN_TEXT) {
        let actual = span
            .glyphs
            .iter()
            .map(|glyph| glyph.text.as_str())
            .collect::<String>();
        assert_eq!(actual, expected);
        assert_eq!(span.glyph_ids.len(), expected.chars().count());
    }
}

fn assert_row(
    tree: &Tree,
    text: &Text,
    prepared: &[PreparedSpan],
    layouts: &[RunLayout],
    row: usize,
) -> Result<(), String> {
    let source = &text.chunks()[row];
    let css_scale = anchor::uniform_css_scale(text.abs_transform()).ok_or("scale missing")?;
    let metrics = row_metrics(tree, source, prepared, layouts)?;
    let width = metrics
        .iter()
        .flatten()
        .map(|(advance, _)| f64::from(*advance))
        .sum();
    let factor = match source.anchor() {
        resvg::usvg::TextAnchor::Start => 0.0,
        resvg::usvg::TextAnchor::Middle => anchor::MIDDLE_ANCHOR_FACTOR,
        resvg::usvg::TextAnchor::End => -1.0,
    };
    let origin = source.x().unwrap_or(0.0) * css_scale;
    let shift = (chunk::ceil_layout_unit(width) * factor) as f32;
    assert_positions(prepared, layouts, &metrics, origin, shift)
}

fn row_metrics(
    tree: &Tree,
    chunk: &resvg::usvg::TextChunk,
    prepared: &[PreparedSpan],
    layouts: &[RunLayout],
) -> Result<Vec<CharacterMetrics>, String> {
    let font = chunk.spans().first().ok_or("missing source span")?.font();
    prepared
        .iter()
        .zip(layouts)
        .map(|(span, layout)| {
            let content = span
                .glyphs
                .iter()
                .map(|glyph| glyph.text.as_str())
                .collect::<String>();
            let shape = FontShape::new(layout.paint_css_size, font.weight(), false);
            shape_run_character_metrics_with_trim(tree.fontdb(), span.font, &content, shape, None)
                .ok_or_else(|| format!("metrics missing for {content}"))
        })
        .collect()
}

fn assert_positions(
    prepared: &[PreparedSpan],
    layouts: &[RunLayout],
    metrics: &[CharacterMetrics],
    origin: f32,
    shift: f32,
) -> Result<(), String> {
    let mut cursor = 0.0_f64;
    for ((span, layout), metrics) in prepared.iter().zip(layouts).zip(metrics) {
        assert_eq!(layout.css_positions.len(), span.glyphs.len());
        assert_eq!(metrics.len(), span.glyphs.len());
        for ((glyph, point), (advance, offset)) in
            span.glyphs.iter().zip(&layout.css_positions).zip(metrics)
        {
            let x = expected_x(cursor, *offset, origin, shift)?;
            assert_eq!(point.x, x);
            assert_eq!(
                point.y,
                (glyph.transform().ty + span.baseline_shift) * layout.css_scale
            );
            cursor += f64::from(*advance);
        }
    }
    Ok(())
}

fn expected_x(cursor: f64, offset: f32, origin: f32, shift: f32) -> Result<f32, String> {
    let value = (cursor + f64::from(offset)) as f32;
    let snapped = anchor::trunc_layout_unit(value).ok_or("cursor out of range")?;
    let float = std::env::var("KRR_HTML_FEATURE_FLOAT_FALLBACK_ADVANCES").as_deref() == Ok("1");
    Ok(origin + if float { value } else { snapped } + shift)
}
