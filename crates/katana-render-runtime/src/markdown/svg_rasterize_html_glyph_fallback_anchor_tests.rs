use super::super::super::{HtmlGlyphRasterizer, Prepared, PreparedSpan};
use super::super::layout;
use super::{TITLE, assert_run_layout_metrics, text, tree};
use std::collections::HashSet;

#[cfg(target_os = "macos")]
#[test]
fn single_source_span_fallback_runs_use_each_face_and_quantized_metrics() -> Result<(), String> {
    let tree = tree(TITLE, "")?;
    let text = text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(prepared) = rasterizer.prepare(text)? else {
        return Err("mixed fallback title was not prepared".to_owned());
    };
    assert_fallback_title_preconditions(text, &prepared);
    let layouts =
        layout(tree.fontdb(), text, &prepared)?.ok_or("fallback run layout was rejected")?;
    assert_eq!(layouts.len(), prepared.len());
    assert_run_layout_metrics(tree.fontdb(), text, &prepared, &layouts)?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn mixed_fallback_runs_preserve_start_middle_and_end_anchor_positions() -> Result<(), String> {
    let start_only =
        std::env::var("KRR_HTML_FEATURE_START_ONLY_FALLBACK_RUNS").as_deref() == Ok("1");
    let mut start_case = None;
    let mut middle_case = None;
    for anchor in ["start", "middle", "end"] {
        let (faces, positions, advance) = anchor_layout_snapshot(anchor)?;
        if anchor == "start" {
            start_case = Some((faces, positions, advance.ok_or("start metrics missing")?));
            continue;
        }
        let (start_faces, start_positions, advance) =
            start_case.as_ref().ok_or("start anchor case is missing")?;
        assert_eq!(&faces, start_faces);
        if start_only {
            assert!(positions.is_empty(), "{anchor} anchor should stay disabled");
            continue;
        }
        assert_eq!(start_positions.len(), faces.len());
        if anchor == "middle" {
            assert_middle_absolute(&positions, *advance)?;
            middle_case = Some(positions);
        } else {
            assert_end_positions(&positions, &middle_case, *advance)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_end_positions(
    positions: &[Vec<tiny_skia::Point>],
    middle: &Option<Vec<Vec<tiny_skia::Point>>>,
    advance: f64,
) -> Result<(), String> {
    let middle = middle.as_deref().ok_or("middle anchor case is missing")?;
    let shift = (super::super::super::chunk::ceil_layout_unit(advance)
        * super::super::super::anchor::MIDDLE_ANCHOR_FACTOR) as f32;
    assert_anchored_positions("end", positions, middle, shift);
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_middle_absolute(positions: &[Vec<tiny_skia::Point>], advance: f64) -> Result<(), String> {
    let tree = tree(TITLE, "text-anchor='middle'")?;
    let text = text(&tree)?;
    let spans = prepare_anchor_title(tree.fontdb(), text)?;
    let span = spans.first().ok_or("middle span missing")?;
    let glyph = span.glyphs.first().ok_or("middle glyph missing")?;
    let scale = super::super::super::anchor::uniform_css_scale(text.abs_transform())
        .ok_or("middle scale missing")?;
    let chunk = text.chunks().first().ok_or("middle chunk missing")?;
    assert_eq!(chunk.x(), Some(216.0));
    let shift = (super::super::super::chunk::ceil_layout_unit(advance)
        * super::super::super::anchor::MIDDLE_ANCHOR_FACTOR) as f32;
    let first = positions
        .first()
        .and_then(|run| run.first())
        .ok_or("middle position missing")?;
    assert_eq!(first.x, 216.0 * scale + shift);
    assert_eq!(
        first.y,
        (glyph.transform().ty + span.baseline_shift) * scale
    );
    Ok(())
}

#[cfg(target_os = "macos")]
type AnchorLayoutSnapshot = (
    Vec<resvg::usvg::fontdb::ID>,
    Vec<Vec<tiny_skia::Point>>,
    Option<f64>,
);

#[cfg(target_os = "macos")]
fn anchor_layout_snapshot(anchor: &str) -> Result<AnchorLayoutSnapshot, String> {
    let tree = tree(TITLE, &format!("text-anchor='{anchor}'"))?;
    let text = text(&tree)?;
    let prepared = prepare_anchor_title(tree.fontdb(), text)?;
    let faces = prepared.iter().map(|span| span.font).collect();
    if std::env::var("KRR_HTML_FEATURE_START_ONLY_FALLBACK_RUNS").as_deref() == Ok("1")
        && anchor != "start"
    {
        assert!(layout(tree.fontdb(), text, &prepared)?.is_none());
        return Ok((faces, Vec::new(), None));
    }
    assert_fallback_title_preconditions(text, &prepared);
    let Some(layouts) = layout(tree.fontdb(), text, &prepared)? else {
        return Ok((faces, Vec::new(), None));
    };
    assert_actual_paint_positions(&prepared, &layouts);
    let advance = start_anchor_advance(anchor, &tree, text, &prepared, &layouts)?;
    let positions = layouts
        .iter()
        .map(|layout| layout.css_positions.clone())
        .collect();
    Ok((faces, positions, advance))
}

#[cfg(target_os = "macos")]
fn start_anchor_advance(
    anchor: &str,
    tree: &resvg::usvg::Tree,
    text: &resvg::usvg::Text,
    prepared: &[PreparedSpan],
    layouts: &[super::super::RunLayout],
) -> Result<Option<f64>, String> {
    if anchor == "start" {
        Ok(Some(assert_run_layout_metrics(
            tree.fontdb(),
            text,
            prepared,
            layouts,
        )?))
    } else {
        Ok(None)
    }
}

#[cfg(target_os = "macos")]
fn prepare_anchor_title(
    database: &resvg::usvg::fontdb::Database,
    text: &resvg::usvg::Text,
) -> Result<Vec<PreparedSpan>, String> {
    let rasterizer = HtmlGlyphRasterizer::new(database);
    let Prepared::Ready(prepared) = rasterizer.prepare(text)? else {
        return Err("mixed fallback title was not prepared".to_owned());
    };
    Ok(prepared)
}

#[cfg(target_os = "macos")]
fn assert_anchored_positions(
    anchor: &str,
    actual: &[Vec<tiny_skia::Point>],
    start: &[Vec<tiny_skia::Point>],
    shift: f32,
) {
    assert_eq!(actual.len(), start.len());
    for (actual_run, start_run) in actual.iter().zip(start) {
        assert_eq!(actual_run.len(), start_run.len());
        for (actual, start) in actual_run.iter().zip(start_run) {
            assert_eq!(actual.x, start.x + shift, "{anchor} anchor x");
            assert_eq!(actual.y, start.y, "{anchor} anchor y");
        }
    }
}

#[cfg(target_os = "macos")]
fn assert_actual_paint_positions(prepared: &[PreparedSpan], layouts: &[super::super::RunLayout]) {
    for (span, layout) in prepared.iter().zip(layouts) {
        assert!(span.fallback_alignment.is_some());
        for (glyph, position) in span.glyphs.iter().zip(&layout.css_positions) {
            let painted =
                super::super::super::paint::normalized_transform(span, glyph, layout.css_scale);
            assert_eq!(painted.tx, position.x);
            assert_eq!(painted.ty, position.y);
        }
    }
}

#[cfg(target_os = "macos")]
fn assert_fallback_title_preconditions(text: &resvg::usvg::Text, prepared: &[PreparedSpan]) {
    assert_eq!(text.chunks().len(), 1);
    assert_eq!(text.chunks()[0].spans().len(), 1);
    assert_eq!(text.layouted().len(), 1);
    let fonts = prepared
        .iter()
        .map(|span| span.font)
        .collect::<HashSet<_>>();
    assert!(
        fonts.len() > 1,
        "title did not resolve to multiple font faces"
    );
    assert!(
        prepared
            .iter()
            .all(|span| span.fallback_alignment.is_some())
    );
}
