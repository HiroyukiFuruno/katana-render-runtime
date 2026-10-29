use super::{HtmlGlyphRasterizer, PaintCaptureScope, Prepared, PreparedSpan, anchor, paint};
use crate::markdown::svg_rasterize::font_file_fixture;
use resvg::usvg::{Node, Options, Tree};
use serde_json::Value;
use std::sync::Arc;

#[path = "svg_rasterize_html_glyph_multi_chunk_nested_rows_tests.rs"]
mod nested_rows;
#[path = "svg_rasterize_html_glyph_single_row_spans_tests.rs"]
mod single_row_spans;
#[path = "svg_rasterize_html_glyph_multi_chunk_single_scalar_tests.rs"]
mod single_scalar;
#[path = "svg_rasterize_html_glyph_unicode_spans_tests.rs"]
mod unicode_spans;

const CHUNK_COUNT: usize = 2;
const COORDINATE_COUNT: usize = 2;
const FIXTURE_WIDTH: i32 = 160;
const FIXTURE_HEIGHT: i32 = 80;
const LAYOUT_UNITS: f64 = 64.0;
const POSITION_TOLERANCE: f64 = 0.000_01;
const CONTENT: &str =
    "<tspan x='75.02' y='40.503'>Waves</tspan><tspan x='95.02' y='60.503'>North</tspan>";

#[test]
fn multiple_horizontal_chunks_quantize_each_run_without_changing_glyph_offsets()
-> Result<(), String> {
    let fixture = font_fixture("origins")?;
    let tree = fixture_tree(&fixture, "")?;
    let text = fixture_text(&tree)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let Prepared::Ready(spans) = rasterizer.prepare(text)? else {
        return Err("multiple-chunk fixture was not prepared".into());
    };
    assert_source_runs(text, &spans);
    for span in &spans {
        let alignment = span.x_alignment.ok_or("run XAlignment is missing")?;
        assert!(alignment.layout.is_none());
        assert_eq!(alignment.origin_snap_css_scale, Some(1.0));
    }
    assert_painted_runs(&rasterizer, &spans)
}

#[test]
fn multiple_chunk_origin_snap_rejects_non_simple_positioning_and_decoration() -> Result<(), String>
{
    let fixture = font_fixture("exclusions")?;
    for attributes in [
        "rotate='12'",
        "dx='1'",
        "text-decoration='underline'",
        "letter-spacing='1'",
        "writing-mode='tb'",
    ] {
        let tree = fixture_tree(&fixture, attributes)?;
        let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
        if let Prepared::Ready(spans) = rasterizer.prepare(fixture_text(&tree)?)? {
            assert!(
                spans.iter().all(|span| span
                    .x_alignment
                    .is_none_or(|alignment| alignment.origin_snap_css_scale.is_none())),
                "unsupported origin snap was admitted: {attributes}"
            );
        }
    }
    Ok(())
}

fn font_fixture(name: &str) -> Result<font_file_fixture::FontFileFixture, String> {
    let path =
        std::env::temp_dir().join(format!("krr-multi-chunk-{name}-{}.ttf", std::process::id()));
    font_file_fixture::create(path, font_file_fixture::FONT_BYTES)
        .map_err(|error| error.to_string())
}

fn fixture_tree(
    fixture: &font_file_fixture::FontFileFixture,
    attributes: &str,
) -> Result<Tree, String> {
    fixture_tree_with_content(fixture, attributes, CONTENT)
}

fn fixture_tree_with_content(
    fixture: &font_file_fixture::FontFileFixture,
    attributes: &str,
    content: &str,
) -> Result<Tree, String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80"><text id="run" font-family="Noto Sans" font-size="16" text-anchor="end" {attributes}>{content}</text></svg>"#
    );
    Tree::from_str(
        &svg,
        &Options {
            fontdb: Arc::new(fixture.database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

fn fixture_text(tree: &Tree) -> Result<&resvg::usvg::Text, String> {
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("multiple-chunk text node is missing".into());
    };
    Ok(text)
}

fn assert_source_runs(text: &resvg::usvg::Text, spans: &[PreparedSpan]) {
    assert_eq!(text.chunks().len(), CHUNK_COUNT);
    assert_eq!(spans.len(), CHUNK_COUNT);
    for (chunk, span) in text.chunks().iter().zip(spans) {
        assert_eq!(chunk.spans().len(), 1);
        assert_eq!(chunk.text().chars().count(), span.glyphs.len());
        assert!(anchor::one_ordered_scalar_per_glyph(
            chunk.text(),
            &span.glyphs
        ));
        assert!(span.glyph_ids.iter().all(|id| *id != 0));
        assert!(span.chunk_alignment.is_none());
    }
}

fn assert_painted_runs(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    spans: &[PreparedSpan],
) -> Result<(), String> {
    let mut surface = skia_safe::surfaces::raster_n32_premul((FIXTURE_WIDTH, FIXTURE_HEIGHT))
        .ok_or("multiple-chunk surface allocation failed")?;
    let capture = PaintCaptureScope::enter();
    for span in spans {
        let face = rasterizer
            .ensure_face(span.font)?
            .ok_or("fixture face missing")?;
        paint::draw_span(surface.canvas(), span, face.typeface.as_skia()?, 1.0)?;
    }
    let snapshot = capture.snapshot();
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(snapshot.events.len(), spans.len());
    for (span, event) in spans.iter().zip(&snapshot.events) {
        assert_painted_span(span, event)?;
    }
    Ok(())
}

fn assert_painted_span(span: &PreparedSpan, event: &Value) -> Result<(), String> {
    let positions = event["positions"]
        .as_array()
        .ok_or("draw positions missing")?;
    assert_eq!(event["glyphIds"], serde_json::json!(span.glyph_ids));
    assert_eq!(positions.len(), span.glyphs.len());
    let first = span.glyphs.first().ok_or("first fixture glyph missing")?;
    let source_origin = paint::normalized_transform(span, first, 1.0);
    let painted_origin = positions.first().ok_or("painted origin missing")?;
    let origin = recorded_point(painted_origin)?;
    assert_quantized_origin(origin, [source_origin.tx, source_origin.ty]);
    for (glyph, position) in span.glyphs.iter().zip(positions) {
        let source = paint::normalized_transform(span, glyph, 1.0);
        let actual = recorded_point(position)?;
        assert_close(
            actual[0] - origin[0],
            f64::from(source.tx - source_origin.tx),
        );
        assert_close(
            actual[1] - origin[1],
            f64::from(source.ty - source_origin.ty),
        );
    }
    Ok(())
}

fn recorded_point(value: &Value) -> Result<[f64; COORDINATE_COUNT], String> {
    Ok([
        value[0].as_f64().ok_or("draw X coordinate missing")?,
        value[1].as_f64().ok_or("draw Y coordinate missing")?,
    ])
}

fn assert_quantized_origin(actual: [f64; COORDINATE_COUNT], source: [f32; COORDINATE_COUNT]) {
    for (coordinate, original) in actual.into_iter().zip(source) {
        let expected = (f64::from(original) * LAYOUT_UNITS).trunc() / LAYOUT_UNITS;
        assert_close(coordinate, expected);
        assert_eq!((coordinate * LAYOUT_UNITS).fract(), 0.0);
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < POSITION_TOLERANCE,
        "coordinate {actual}, expected {expected}"
    );
}
