use super::{PreparedSpan, anchor};
use resvg::usvg;
use tiny_skia::{Point, Transform};

#[path = "svg_rasterize_html_glyph_chunk_eligibility.rs"]
mod eligibility;
#[cfg(test)]
pub(super) use eligibility::row_diagnostic;
#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_single_middle_tests.rs"]
mod single_middle_tests;
#[cfg(test)]
#[path = "svg_rasterize_html_glyph_chunk_tests.rs"]
mod tests;

const LAYOUT_UNIT_SCALE: f64 = 64.0;
type CharacterMetric = (f32, f32);
type SpanMetrics = Vec<CharacterMetric>;

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
    let mut cursor = f64::from(input.anchor_x * input.css_scale) - widths.iter().sum::<f64>() / 2.0;
    for ((metrics, span), width) in metrics.iter().zip(spans).zip(widths) {
        let offset = metrics.first().ok_or("chunk span offset is missing")?.1;
        alignments.push(span_alignment(span, input, cursor + f64::from(offset))?);
        cursor += width;
    }
    Ok(())
}

fn character_metrics(
    database: &usvg::fontdb::Database,
    input: &eligibility::Input<'_>,
) -> Result<Vec<SpanMetrics>, String> {
    let font = super::super::text_shaping::FontShape::new(
        input.paint_css_size,
        input.source_font.weight(),
        input.source_font.style() != usvg::FontStyle::Normal,
    );
    let mut metrics = Vec::with_capacity(input.chunk.spans().len());
    for span in input.chunk.spans() {
        let text = input
            .chunk
            .text()
            .get(span.start()..span.end())
            .ok_or("eligible SVG chunk span text is missing")?;
        /* WHY: SVGのspan境界でkerningが分断されるため、span内だけをshapeする。 */
        let shaped = super::super::text_shaping::shape_run_character_metrics_with_trim(
            database, input.face, text, font, None,
        )
        .ok_or_else(|| "eligible SVG chunk character metrics could not be shaped".to_owned())?;
        metrics.push(shaped);
    }
    Ok(metrics)
}

fn span_widths(chunk: &usvg::TextChunk, metrics: &[SpanMetrics]) -> Result<Vec<f64>, String> {
    if metrics.len() != chunk.spans().len() {
        return Err("eligible SVG chunk character metrics count mismatch".to_owned());
    }
    chunk
        .spans()
        .iter()
        .zip(metrics)
        .map(|(span, values)| {
            let text = chunk
                .text()
                .get(span.start()..span.end())
                .ok_or("chunk span text is missing")?;
            if values.len() != text.chars().count() {
                return Err("eligible SVG chunk character metrics count mismatch".to_owned());
            }
            span_width(values)
        })
        .collect()
}

fn span_width(values: &[CharacterMetric]) -> Result<f64, String> {
    if values
        .iter()
        .any(|(advance, offset)| !advance.is_finite() || !offset.is_finite())
    {
        return Err("eligible SVG chunk metrics are not finite".to_owned());
    }
    let width = values
        .iter()
        .map(|(advance, _)| f64::from(*advance))
        .sum::<f64>();
    if width <= 0.0 {
        return Err("eligible SVG chunk span advance is not positive".to_owned());
    }
    Ok(ceil_layout_unit(width))
}

fn ceil_layout_unit(width: f64) -> f64 {
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
