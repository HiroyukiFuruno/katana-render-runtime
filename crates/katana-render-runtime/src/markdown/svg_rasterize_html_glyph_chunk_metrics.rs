use super::{ceil_layout_unit, eligibility};
use resvg::usvg;

type CharacterMetric = (f32, f32);
type SpanMetrics = Vec<CharacterMetric>;

pub(super) fn character_metrics(
    database: &usvg::fontdb::Database,
    input: &eligibility::Input<'_>,
) -> Result<Vec<SpanMetrics>, String> {
    let font = super::super::super::text_shaping::FontShape::new(
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
        let shaped = super::super::super::text_shaping::shape_run_character_metrics_with_trim(
            database, input.face, text, font, None,
        )
        .ok_or_else(|| "eligible SVG chunk character metrics could not be shaped".to_owned())?;
        metrics.push(shaped);
    }
    Ok(metrics)
}

pub(super) fn span_widths(
    chunk: &usvg::TextChunk,
    metrics: &[SpanMetrics],
) -> Result<Vec<f64>, String> {
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
