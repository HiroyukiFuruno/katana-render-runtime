use super::super::super::PreparedSpan;
use super::{anchor, printable_ascii, standard_font_variation};
use resvg::usvg;

pub(in super::super) struct SingleMiddleInput {
    pub(in super::super) layout: anchor::LayoutAnchor,
    pub(in super::super) anchor_x: f32,
    pub(in super::super) paint_css_size: f32,
}

pub(in super::super) fn single_middle_input(
    text: &usvg::Text,
    span: &PreparedSpan,
) -> Option<SingleMiddleInput> {
    let [chunk] = text.chunks() else { return None };
    let [source] = chunk.spans() else { return None };
    let css_scale = anchor::run_origin_css_scale(text, std::slice::from_ref(span))?;
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
