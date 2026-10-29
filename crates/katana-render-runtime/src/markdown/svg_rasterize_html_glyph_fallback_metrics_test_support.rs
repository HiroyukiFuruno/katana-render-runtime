use super::super::super::PreparedSpan;
use crate::markdown::svg_rasterize::text_shaping::{
    FontShape, shape_run_character_metrics_with_trim,
};

#[cfg(target_os = "macos")]
pub(super) fn assert_run_layout_metrics(
    database: &resvg::usvg::fontdb::Database,
    text: &resvg::usvg::Text,
    prepared: &[PreparedSpan],
    layouts: &[super::super::RunLayout],
) -> Result<f64, String> {
    let source = text
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .ok_or("source span is missing")?;
    let mut context = metrics_context(database, text)?;
    for (span, layout) in prepared.iter().zip(layouts) {
        assert_span_layout_metrics(span, layout, source, &mut context)?;
    }
    Ok(context.cursor)
}

#[cfg(target_os = "macos")]
struct MetricsContext<'a> {
    database: &'a resvg::usvg::fontdb::Database,
    source_glyphs: &'a [resvg::usvg::layout::PositionedGlyph],
    scripts: Vec<unicode_script::Script>,
    css_scale: f32,
    origin_x: f32,
    cursor: f64,
    glyph_offset: usize,
}

#[cfg(target_os = "macos")]
fn metrics_context<'a>(
    database: &'a resvg::usvg::fontdb::Database,
    text: &'a resvg::usvg::Text,
) -> Result<MetricsContext<'a>, String> {
    let chunk = text.chunks().first().ok_or("text chunk is missing")?;
    let css_scale = super::super::super::anchor::uniform_css_scale(text.abs_transform())
        .ok_or("uniform CSS scale is missing")?;
    let source_glyphs = &text
        .layouted()
        .first()
        .ok_or("source layout is missing")?
        .positioned_glyphs;
    let scripts =
        super::super::resolved_scripts(source_glyphs).ok_or("source scripts are missing")?;
    Ok(MetricsContext {
        database,
        source_glyphs,
        scripts,
        css_scale,
        origin_x: chunk.x().unwrap_or(0.0) * css_scale,
        cursor: 0.0,
        glyph_offset: 0,
    })
}

#[cfg(target_os = "macos")]
fn assert_span_layout_metrics(
    span: &PreparedSpan,
    layout: &super::super::RunLayout,
    source: &resvg::usvg::TextSpan,
    context: &mut MetricsContext<'_>,
) -> Result<(), String> {
    assert_eq!(layout.css_scale, context.css_scale);
    assert_eq!(layout.css_positions.len(), span.glyphs.len());
    let font = expected_span_font(span, layout, source, context.css_scale)?;
    let end = context.glyph_offset + span.glyphs.len();
    let source_run = context
        .source_glyphs
        .get(context.glyph_offset..end)
        .ok_or("prepared glyphs exceed source glyphs")?;
    let scripts = &context.scripts[context.glyph_offset..end];
    let fragments = super::super::matching_script_fragments(source_run, span, scripts)
        .ok_or("source glyphs did not match prepared span")?;
    assert_span_fragments(span, layout, &fragments, font, context)?;
    context.glyph_offset = end;
    Ok(())
}

#[cfg(target_os = "macos")]
fn expected_span_font(
    span: &PreparedSpan,
    layout: &super::super::RunLayout,
    source: &resvg::usvg::TextSpan,
    css_scale: f32,
) -> Result<FontShape, String> {
    let expected_size =
        super::super::super::anchor::paint_size(span.font_size, css_scale, css_scale)
            .ok_or("effective font size is missing")?;
    assert_eq!(layout.paint_css_size, expected_size);
    Ok(FontShape::new(
        layout.paint_css_size,
        source.font().weight(),
        source.font().style() != resvg::usvg::FontStyle::Normal,
    ))
}

#[cfg(target_os = "macos")]
fn assert_span_fragments(
    span: &PreparedSpan,
    layout: &super::super::RunLayout,
    fragments: &[super::super::ScriptFragment],
    font: FontShape,
    context: &mut MetricsContext<'_>,
) -> Result<(), String> {
    let mut painted_index = 0;
    for fragment in fragments {
        painted_index +=
            assert_fragment_metrics(span, layout, fragment, font, context, painted_index)?;
    }
    assert_eq!(painted_index, span.glyphs.len());
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_fragment_metrics(
    span: &PreparedSpan,
    layout: &super::super::RunLayout,
    fragment: &super::super::ScriptFragment,
    font: FontShape,
    context: &mut MetricsContext<'_>,
    painted_start: usize,
) -> Result<usize, String> {
    let glyphs = &span.glyphs[fragment.glyph_range.clone()];
    let metrics = shape_run_character_metrics_with_trim(
        context.database,
        span.font,
        &fragment.text,
        font,
        None,
    )
    .ok_or("fallback face fragment metrics are missing")?;
    assert_eq!(metrics.len(), glyphs.len());
    for (index, (glyph, (advance, offset))) in glyphs.iter().zip(metrics).enumerate() {
        let position = layout
            .css_positions
            .get(painted_start + index)
            .ok_or("painted glyph position is missing")?;
        assert_glyph_position(span, glyph, position, advance, offset, context)?;
    }
    Ok(glyphs.len())
}

#[cfg(target_os = "macos")]
fn assert_glyph_position(
    span: &PreparedSpan,
    glyph: &resvg::usvg::layout::PositionedGlyph,
    position: &tiny_skia::Point,
    advance: f32,
    offset: f32,
    context: &mut MetricsContext<'_>,
) -> Result<(), String> {
    let expected_x = context.origin_x
        + super::super::super::anchor::trunc_layout_unit(
            (context.cursor + f64::from(offset)) as f32,
        )
        .ok_or("expected glyph x is outside LayoutUnit range")?;
    let expected_y = (glyph.transform().ty + span.baseline_shift) * context.css_scale;
    assert_eq!(position.x, expected_x);
    assert_eq!(position.y, expected_y);
    let painted = super::super::super::paint::normalized_transform(span, glyph, context.css_scale);
    assert_eq!(painted.tx, expected_x);
    assert_eq!(painted.ty, expected_y);
    context.cursor += f64::from(advance);
    Ok(())
}
