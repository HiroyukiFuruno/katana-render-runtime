use super::super::{
    PreparedSpan, alignment_positioning_supported, alignment_source_span_supported,
};
use super::anchor;
use resvg::usvg;

#[path = "svg_rasterize_html_glyph_chunk_single_middle.rs"]
mod single_middle;
#[path = "svg_rasterize_html_glyph_chunk_text.rs"]
mod text;
pub(super) use single_middle::single_middle_input;
pub(in super::super) use text::ranges_cover_chunk;
#[cfg(test)]
use text::span_content;
use text::{printable_ascii, ranges_match, supported_ltr_text};

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_chunk_row_diagnostic.rs"]
mod diagnostic;
#[cfg(test)]
pub(in crate::markdown::svg_rasterize) use diagnostic::row_diagnostic;
#[cfg(test)]
#[path = "svg_rasterize_html_glyph_chunk_unicode_boundary_tests.rs"]
mod unicode_boundary_tests;

pub(super) struct Input<'a> {
    pub(super) chunk: &'a usvg::TextChunk,
    pub(super) source_font: &'a usvg::Font,
    pub(super) face: usvg::fontdb::ID,
    pub(super) anchor_x: f32,
    pub(super) css_scale: f32,
    pub(super) paint_css_size: f32,
}

pub(super) type RowInput<'a> = (Input<'a>, std::ops::Range<usize>);

pub(super) fn input<'a>(text: &'a usvg::Text, prepared: &[PreparedSpan]) -> Option<Input<'a>> {
    let [chunk] = text.chunks() else { return None };
    if prepared.len() <= 1
        || text.layouted().len() != prepared.len()
        || !alignment_positioning_supported(text, chunk)
    {
        return None;
    }
    chunk_input(text, chunk, prepared)
}

pub(super) fn row_inputs<'a>(
    text: &'a usvg::Text,
    prepared: &[PreparedSpan],
) -> Option<Vec<RowInput<'a>>> {
    if !rows_supported(text, prepared) {
        return None;
    }
    let mut span_offset = 0_usize;
    let mut character_offset = 0_usize;
    let mut result = Vec::with_capacity(text.chunks().len());
    for chunk in text.chunks() {
        let (input, range, character_end) =
            row_input(text, chunk, prepared, span_offset, character_offset)?;
        span_offset = range.end;
        character_offset = character_end;
        result.push((input, range));
    }
    (span_offset == prepared.len() && character_offset == text.dy().len()).then_some(result)
}

fn rows_supported(text: &usvg::Text, prepared: &[PreparedSpan]) -> bool {
    text.chunks().len() > 1
        && text.layouted().len() == prepared.len()
        && text.writing_mode() == usvg::WritingMode::LeftToRight
        && super::super::all_zero(text.dx())
        && super::super::all_zero(text.rotate())
}

fn row_input<'a>(
    text: &'a usvg::Text,
    chunk: &'a usvg::TextChunk,
    prepared: &[PreparedSpan],
    span_offset: usize,
    character_offset: usize,
) -> Option<(Input<'a>, std::ops::Range<usize>, usize)> {
    let span_end = span_offset.checked_add(chunk.spans().len())?;
    let character_end = character_offset.checked_add(chunk.text().chars().count())?;
    let spans = prepared.get(span_offset..span_end)?;
    let layouts = text.layouted().get(span_offset..span_end)?;
    let dy = text.dy().get(character_offset..character_end)?;
    if !super::super::alignment_dy_supported(dy)
        || !layouts
            .iter()
            .zip(spans)
            .all(|(layout, span)| super::super::multi_chunk_alignment::layout_matches(layout, span))
    {
        return None;
    }
    Some((
        chunk_input(text, chunk, spans)?,
        span_offset..span_end,
        character_end,
    ))
}

fn chunk_input<'a>(
    text: &'a usvg::Text,
    chunk: &'a usvg::TextChunk,
    prepared: &[PreparedSpan],
) -> Option<Input<'a>> {
    if !chunk_supported(chunk, prepared) {
        return None;
    }
    let (source, first) = first_supported_spans(chunk, prepared)?;
    let css_scale = anchor::uniform_css_scale(text.abs_transform())?;
    let paint_css_size = anchor::paint_size(first.font_size, css_scale, css_scale)?;
    if paint_css_size == first.font_size * css_scale {
        return None;
    }
    let anchor_x = chunk.x()?;
    anchor_x.is_finite().then_some(Input {
        chunk,
        source_font: source.font(),
        face: first.font,
        anchor_x,
        css_scale,
        paint_css_size,
    })
}

fn chunk_supported(chunk: &usvg::TextChunk, prepared: &[PreparedSpan]) -> bool {
    !prepared.is_empty()
        && prepared.len() == chunk.spans().len()
        && matches!(
            chunk.anchor(),
            usvg::TextAnchor::Start | usvg::TextAnchor::Middle
        )
        && matches!(chunk.text_flow(), usvg::TextFlow::Linear)
        && supported_ltr_text(chunk.text())
}

fn first_supported_spans<'a, 'b>(
    chunk: &'a usvg::TextChunk,
    prepared: &'b [PreparedSpan],
) -> Option<(&'a usvg::TextSpan, &'b PreparedSpan)> {
    let first = prepared.first()?;
    let first_source = chunk.spans().first()?;
    let pairs = chunk.spans().iter().zip(prepared);
    let supported = ranges_match(chunk.text(), pairs, |source, content, span| {
        source_supported(source)
            && source.font() == first_source.font()
            && span.font == first.font
            && span.font_size == first.font_size
            && span.baseline_shift == first.baseline_shift
            && glyphs_supported(content, span)
    });
    supported.then_some((first_source, first))
}

fn source_supported(source: &usvg::TextSpan) -> bool {
    let decoration = source.decoration();
    alignment_source_span_supported(source)
        && source.apply_kerning()
        && !source.small_caps()
        && standard_font_variation(source.font())
        && decoration.underline().is_none()
        && decoration.overline().is_none()
        && decoration.line_through().is_none()
        && source.baseline_shift().is_empty()
}

fn standard_font_variation(font: &usvg::Font) -> bool {
    let [axis] = font.variations() else {
        return false;
    };
    /* WHY: usvgが通常のfont-weightにも自動追加するwght軸だけを許可する。 */
    axis.tag == *b"wght" && axis.value == f32::from(font.weight())
}

fn glyphs_supported(content: &str, span: &PreparedSpan) -> bool {
    super::super::horizontal_glyphs(&span.glyphs)
        && anchor::nonempty_ordered_scalar_per_glyph(content, &span.glyphs)
        && span.x_alignment.is_none()
        && span.glyphs.iter().all(|glyph| {
            let transform = glyph.transform();
            glyph.id.0 != 0 && transform.tx.is_finite() && transform.ty.is_finite()
        })
}
