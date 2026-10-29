use super::{
    PreparedSpan, RunLayout, anchors, eligibility, layout_context::LayoutContext, metrics,
    scripts::ScriptFragment,
};
use crate::markdown::svg_rasterize::text_shaping::FontShape;
use resvg::usvg;
use tiny_skia::Point;
use unicode_script::Script;

struct SourceRunContext<'a> {
    database: &'a usvg::fontdb::Database,
    source_span: &'a usvg::TextSpan,
    source_glyphs: &'a [usvg::layout::PositionedGlyph],
    prepared: &'a [PreparedSpan],
    prepared_offset: &'a mut usize,
    scripts: &'a [Script],
    layout_context: &'a mut LayoutContext,
    layouts: &'a mut Vec<RunLayout>,
}
pub(super) fn layout(
    database: &usvg::fontdb::Database,
    text: &usvg::Text,
    prepared: &[PreparedSpan],
    source: eligibility::Source<'_>,
) -> Option<Vec<RunLayout>> {
    let scripts = super::scripts::resolved_layout_scripts(source.layout_spans)?;
    let mut context = LayoutContext::new(text, source.chunk, prepared[0].font_size)?;
    let mut layouts = layout_prepared_spans(database, prepared, &source, &scripts, &mut context)?;
    anchors::apply(source.chunk.anchor(), context.cursor, &mut layouts)?;
    Some(layouts)
}

fn layout_prepared_spans(
    database: &usvg::fontdb::Database,
    prepared: &[PreparedSpan],
    source: &eligibility::Source<'_>,
    scripts: &[Script],
    context: &mut LayoutContext,
) -> Option<Vec<RunLayout>> {
    let mut layouts = Vec::with_capacity(prepared.len());
    let mut prepared_offset = 0_usize;
    let mut glyph_offset = 0_usize;
    for (source_span, layout_span) in source.source_spans.iter().zip(source.layout_spans) {
        let glyph_end = glyph_offset.checked_add(layout_span.positioned_glyphs.len())?;
        let source_scripts = scripts.get(glyph_offset..glyph_end)?;
        let mut source_run = SourceRunContext {
            database,
            source_span,
            source_glyphs: &layout_span.positioned_glyphs,
            prepared,
            prepared_offset: &mut prepared_offset,
            scripts: source_scripts,
            layout_context: &mut *context,
            layouts: &mut layouts,
        };
        layout_source_runs(&mut source_run)?;
        glyph_offset = glyph_end;
    }
    (prepared_offset == prepared.len() && glyph_offset == scripts.len()).then_some(layouts)
}

fn layout_source_runs(context: &mut SourceRunContext<'_>) -> Option<()> {
    let source_glyphs = context.source_glyphs;
    let prepared = context.prepared;
    let scripts = context.scripts;
    let mut covered = 0_usize;
    while covered < source_glyphs.len() {
        let span = prepared.get(*context.prepared_offset)?;
        let run_end = covered.checked_add(span.glyphs.len())?;
        let source_run = source_glyphs.get(covered..run_end)?;
        let run_scripts = scripts.get(covered..run_end)?;
        let layout = layout_prepared_span(context, span, source_run, run_scripts)?;
        context.layouts.push(layout);
        covered = run_end;
        *context.prepared_offset += 1;
    }
    (covered == source_glyphs.len()).then_some(())
}

fn layout_prepared_span(
    context: &mut SourceRunContext<'_>,
    span: &PreparedSpan,
    source_glyphs: &[usvg::layout::PositionedGlyph],
    scripts: &[Script],
) -> Option<RunLayout> {
    let database = context.database;
    let source_span = context.source_span;
    let layout_context = &mut *context.layout_context;
    let fragments = super::matching_script_fragments(source_glyphs, span, scripts)?;
    let font_shape = FontShape::new(
        layout_context.paint_css_size,
        source_span.font().weight(),
        source_span.font().style() != usvg::FontStyle::Normal,
    );
    layout_run(database, span, &fragments, font_shape, layout_context)
}

fn layout_run(
    database: &usvg::fontdb::Database,
    span: &PreparedSpan,
    fragments: &[ScriptFragment],
    font_shape: FontShape,
    context: &mut LayoutContext,
) -> Option<RunLayout> {
    let mut positions = Vec::with_capacity(span.glyphs.len());
    for fragment in fragments {
        let glyphs = span.glyphs.get(fragment.glyph_range.clone())?;
        let metrics = metrics::measure_fragment(database, span, fragment, glyphs, font_shape)?;
        append_fragment_positions(glyphs, metrics, span, context, &mut positions)?;
    }
    finish_run_layout(span, context, positions)
}

fn append_fragment_positions(
    glyphs: &[usvg::layout::PositionedGlyph],
    metrics: Vec<(f32, f32)>,
    span: &PreparedSpan,
    context: &mut LayoutContext,
    positions: &mut Vec<Point>,
) -> Option<()> {
    for (glyph, (advance, offset)) in glyphs.iter().zip(metrics) {
        positions.push(positioned_glyph_point(
            glyph, advance, offset, span, context,
        )?);
    }
    Some(())
}

fn positioned_glyph_point(
    glyph: &usvg::layout::PositionedGlyph,
    advance: f32,
    offset: f32,
    span: &PreparedSpan,
    context: &mut LayoutContext,
) -> Option<Point> {
    if !advance.is_finite() || !offset.is_finite() || advance <= 0.0 {
        return None;
    }
    /* WHY: source spanでshapingを切り、chunk全体のpen位置を累積して量子化する。 */
    let x = context.origin_x + advance_position((context.cursor + f64::from(offset)) as f32)?;
    let y = (glyph.transform().ty + span.baseline_shift) * context.css_scale;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    context.cursor += f64::from(advance);
    Some(Point::from_xy(x, y))
}

pub(super) fn advance_position(value: f32) -> Option<f32> {
    let snapped = super::super::anchor::trunc_layout_unit(value)?;
    #[cfg(test)]
    if std::env::var("KRR_HTML_FEATURE_FLOAT_FALLBACK_ADVANCES").as_deref() == Ok("1") {
        return Some(value);
    }
    Some(snapped)
}

fn finish_run_layout(
    span: &PreparedSpan,
    context: &LayoutContext,
    positions: Vec<Point>,
) -> Option<RunLayout> {
    if positions.len() != span.glyphs.len()
        || !context.cursor.is_finite()
        || super::super::anchor::trunc_layout_unit(context.cursor as f32).is_none()
    {
        return None;
    }
    Some(RunLayout {
        css_scale: context.css_scale,
        paint_css_size: context.paint_css_size,
        css_positions: positions,
    })
}
