use super::super::{
    PreparedSpan, alignment_positioning_supported, alignment_source_span_supported,
};
use super::anchor;
use resvg::usvg;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_chunk_row_diagnostic.rs"]
mod diagnostic;
#[cfg(test)]
pub(in crate::markdown::svg_rasterize) use diagnostic::row_diagnostic;

pub(super) struct Input<'a> {
    pub(super) chunk: &'a usvg::TextChunk,
    pub(super) source_font: &'a usvg::Font,
    pub(super) face: usvg::fontdb::ID,
    pub(super) anchor_x: f32,
    pub(super) css_scale: f32,
    pub(super) paint_css_size: f32,
}

pub(super) type RowInput<'a> = (Input<'a>, std::ops::Range<usize>);

pub(super) struct SingleMiddleInput {
    pub(super) layout: anchor::LayoutAnchor,
    pub(super) anchor_x: f32,
    pub(super) paint_css_size: f32,
}

pub(super) fn single_middle_input(
    text: &usvg::Text,
    prepared: &[PreparedSpan],
) -> Option<SingleMiddleInput> {
    let [span] = prepared else { return None };
    let [chunk] = text.chunks() else { return None };
    let [source] = chunk.spans() else { return None };
    let css_scale = anchor::run_origin_css_scale(text, prepared)?;
    if !single_source_supported(chunk, source) {
        return None;
    }
    let layout = single_middle_layout(text, chunk, span, css_scale)?;
    let paint_css_size = anchor::paint_size(span.font_size, css_scale, css_scale)?;
    if paint_css_size == span.font_size * css_scale {
        return None;
    }
    Some(SingleMiddleInput {
        layout,
        anchor_x: chunk.x()?,
        paint_css_size,
    })
}

fn single_middle_layout(
    text: &usvg::Text,
    chunk: &usvg::TextChunk,
    span: &PreparedSpan,
    css_scale: f32,
) -> Option<anchor::LayoutAnchor> {
    let width = text.bounding_box().width();
    let anchor_x = chunk.x()?;
    advance_bounds_match_origin(span, width, anchor_x).then_some(anchor::LayoutAnchor {
        width,
        anchor: chunk.anchor(),
        css_scale,
    })
}

fn single_source_supported(chunk: &usvg::TextChunk, source: &usvg::TextSpan) -> bool {
    chunk.anchor() == usvg::TextAnchor::Middle
        && printable_ascii(chunk.text())
        && source.apply_kerning()
        && !source.small_caps()
        && source.baseline_shift().is_empty()
        && single_font_variations_supported(source.font())
}

fn single_font_variations_supported(font: &usvg::Font) -> bool {
    if standard_font_variation(font) {
        return true;
    }
    let [weight, italic] = font.variations() else {
        return false;
    };
    font.style() == usvg::FontStyle::Italic
        && weight.tag == *b"wght"
        && weight.value == f32::from(font.weight())
        && italic.tag == *b"ital"
        && italic.value == 1.0
}

fn advance_bounds_match_origin(span: &PreparedSpan, width: f32, anchor_x: f32) -> bool {
    let Some(first) = span.glyphs.first() else {
        return false;
    };
    /* WHY: bboxをadvance幅として使う前に、元のmiddle penとの前方向の整合を確認する。 */
    width.is_finite()
        && width > 0.0
        && anchor_x.is_finite()
        && first.transform().tx == anchor_x - width / 2.0
}

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
    let first = prepared.first()?;
    let source = chunk.spans().first()?;
    if !chunk_supported(chunk, prepared) || !spans_supported(chunk, prepared) {
        return None;
    }
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
        && chunk.anchor() == usvg::TextAnchor::Middle
        && matches!(chunk.text_flow(), usvg::TextFlow::Linear)
        && printable_ascii(chunk.text())
        && ranges_cover_chunk(chunk)
}

fn printable_ascii(text: &str) -> bool {
    text.bytes()
        .all(|value| value.is_ascii() && !value.is_ascii_control())
}

fn ranges_cover_chunk(chunk: &usvg::TextChunk) -> bool {
    let mut end = 0;
    for span in chunk.spans() {
        if span.start() != end || span.end() <= span.start() {
            return false;
        }
        end = span.end();
    }
    end == chunk.text().len()
}

fn spans_supported(chunk: &usvg::TextChunk, prepared: &[PreparedSpan]) -> bool {
    let Some(first) = prepared.first() else {
        return false;
    };
    let Some(first_source) = chunk.spans().first() else {
        return false;
    };
    chunk.spans().iter().zip(prepared).all(|(source, span)| {
        source_supported(source)
            && source.font() == first_source.font()
            && span.font == first.font
            && span.font_size == first.font_size
            && span.baseline_shift == first.baseline_shift
            && glyphs_supported(chunk, source, span)
    })
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

fn glyphs_supported(chunk: &usvg::TextChunk, source: &usvg::TextSpan, span: &PreparedSpan) -> bool {
    let Some(content) = chunk.text().get(source.start()..source.end()) else {
        return false;
    };
    super::super::horizontal_glyphs(&span.glyphs)
        && anchor::nonempty_ordered_scalar_per_glyph(content, &span.glyphs)
        && span.x_alignment.is_none()
        && span.glyphs.iter().all(|glyph| {
            let transform = glyph.transform();
            transform.tx.is_finite() && transform.ty.is_finite()
        })
}
