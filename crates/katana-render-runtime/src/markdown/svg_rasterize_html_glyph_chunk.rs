use super::{PreparedSpan, anchor};
use resvg::usvg;
use tiny_skia::{Point, Transform};

#[path = "svg_rasterize_html_glyph_chunk_eligibility.rs"]
mod eligibility;
#[path = "svg_rasterize_html_glyph_chunk_metrics.rs"]
mod metrics;
pub(super) use eligibility::ranges_cover_chunk;
#[cfg(test)]
pub(super) use eligibility::row_diagnostic;
use metrics::{character_metrics, span_widths};
#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_single_middle_tests.rs"]
mod single_middle_tests;
#[cfg(test)]
#[path = "svg_rasterize_html_glyph_chunk_tests.rs"]
mod tests;

const LAYOUT_UNIT_SCALE: f64 = 64.0;

#[derive(Clone, Copy)]
pub(super) struct SpanAlignment {
    pub(super) css_scale: f32,
    pub(super) paint_css_size: f32,
    source_origin: Point,
    paint_origin: Point,
    advance_ratio: f32,
}

pub(super) fn apply_single_middle(
    text: &usvg::Text,
    prepared: &mut [PreparedSpan],
) -> Result<bool, String> {
    let Some(input) = eligibility::single_middle_input(text, prepared) else {
        return Ok(false);
    };
    let [span] = prepared else {
        return Err("eligible single middle span count changed".to_owned());
    };
    let width =
        f64::from(input.layout.width) * f64::from(input.paint_css_size) / f64::from(span.font_size);
    let width = ceil_layout_unit(width);
    let css_x = f64::from(input.anchor_x * input.layout.css_scale) - width / 2.0;
    span.chunk_alignment = Some(alignment_for_origin(
        span,
        input.layout.css_scale,
        input.paint_css_size,
        css_x,
    )?);
    Ok(true)
}

pub(super) fn apply(
    database: &usvg::fontdb::Database,
    text: &usvg::Text,
    prepared: &mut [PreparedSpan],
) -> Result<(), String> {
    let inputs = if let Some(input) = eligibility::input(text, prepared) {
        vec![(input, 0..prepared.len())]
    } else if let Some(inputs) = eligibility::row_inputs(text, prepared) {
        inputs
    } else {
        return Ok(());
    };
    let mut alignments = Vec::with_capacity(prepared.len());
    for (input, range) in inputs {
        append_row_alignments(database, &input, &prepared[range], &mut alignments)?;
    }
    for (span, alignment) in prepared.iter_mut().zip(alignments) {
        span.chunk_alignment = Some(alignment);
    }
    Ok(())
}

fn append_row_alignments(
    database: &usvg::fontdb::Database,
    input: &eligibility::Input<'_>,
    spans: &[PreparedSpan],
    alignments: &mut Vec<SpanAlignment>,
) -> Result<(), String> {
    let metrics = character_metrics(database, input)?;
    let widths = span_widths(input.chunk, &metrics)?;
    let mut cursor = f64::from(input.anchor_x * input.css_scale);
    /* WHY: Startは指定xから始め、Middleだけ実span幅の半分を戻す。 */
    if input.chunk.anchor() == usvg::TextAnchor::Middle {
        cursor += widths.iter().sum::<f64>() * anchor::MIDDLE_ANCHOR_FACTOR;
    }
    for ((metrics, span), width) in metrics.iter().zip(spans).zip(widths) {
        let offset = metrics.first().ok_or("chunk span offset is missing")?.1;
        alignments.push(span_alignment(span, input, cursor + f64::from(offset))?);
        cursor += width;
    }
    Ok(())
}

pub(super) fn ceil_layout_unit(width: f64) -> f64 {
    (width * LAYOUT_UNIT_SCALE).ceil() / LAYOUT_UNIT_SCALE
}

fn span_alignment(
    span: &PreparedSpan,
    input: &eligibility::Input<'_>,
    css_x: f64,
) -> Result<SpanAlignment, String> {
    alignment_for_origin(span, input.css_scale, input.paint_css_size, css_x)
}

fn alignment_for_origin(
    span: &PreparedSpan,
    css_scale: f32,
    paint_css_size: f32,
    css_x: f64,
) -> Result<SpanAlignment, String> {
    let first = span
        .glyphs
        .first()
        .ok_or("eligible SVG chunk glyph is missing")?
        .transform();
    let source_origin = Point::from_xy(first.tx, first.ty + span.baseline_shift);
    let css_y = source_origin.y * css_scale;
    let paint_origin = Point::from_xy(
        anchor::trunc_layout_unit(css_x as f32).ok_or("chunk X origin exceeds LayoutUnit")?,
        anchor::trunc_layout_unit(css_y).ok_or("chunk Y origin exceeds LayoutUnit")?,
    );
    Ok(SpanAlignment {
        css_scale,
        paint_css_size,
        source_origin,
        paint_origin,
        advance_ratio: paint_css_size / (span.font_size * css_scale),
    })
}

pub(super) fn position(
    alignment: SpanAlignment,
    mut transform: Transform,
    raster_scale: f32,
) -> Transform {
    let device_scale = raster_scale / alignment.css_scale;
    let source_x = alignment.source_origin.x * raster_scale;
    let source_y = alignment.source_origin.y * raster_scale;
    transform.tx = alignment.paint_origin.x * device_scale
        + (transform.tx - source_x) * alignment.advance_ratio;
    transform.ty = alignment.paint_origin.y * device_scale + (transform.ty - source_y);
    transform
}
