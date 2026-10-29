use super::PreparedSpan;
use resvg::usvg;

pub(super) use super::sources::Source;

pub(super) fn eligible_source<'a>(
    text: &'a usvg::Text,
    prepared: &[PreparedSpan],
) -> Option<Source<'a>> {
    let source = super::sources::collect(text)?;
    if !source_supported(text, &source.chunks, prepared) {
        return None;
    }
    Some(source)
}

fn source_supported(
    text: &usvg::Text,
    chunks: &[super::sources::Chunk<'_>],
    prepared: &[PreparedSpan],
) -> bool {
    let Some(first) = prepared.first() else {
        return false;
    };
    chunks.iter().all(|source| chunk_supported(text, source))
        && prepared_runs_supported(chunks, prepared, first.font_size)
}

fn chunk_supported(text: &usvg::Text, source: &super::sources::Chunk<'_>) -> bool {
    source.source_spans.len() == source.layout_spans.len()
        && anchor_supported(source.chunk.anchor())
        && super::super::alignment_positioning_supported_with_dy(text, source.chunk, source.dy)
        && super::source_spans::supported(source.chunk, source.source_spans, source.layout_spans)
}

fn prepared_runs_supported(
    chunks: &[super::sources::Chunk<'_>],
    prepared: &[PreparedSpan],
    font_size: f32,
) -> bool {
    if prepared.len() <= 1 || !font_size.is_finite() || font_size <= 0.0 {
        return false;
    }
    let mut run_index = 0;
    let mut has_fallback_split = false;
    chunks.iter().all(|chunk| {
        chunk.layout_spans.iter().all(|layout| {
            runs_cover_source_layout(
                layout,
                prepared,
                font_size,
                &mut run_index,
                &mut has_fallback_split,
            )
        })
    }) && has_fallback_split
        && run_index == prepared.len()
}

fn runs_cover_source_layout(
    layout: &usvg::layout::Span,
    prepared: &[PreparedSpan],
    font_size: f32,
    run_index: &mut usize,
    has_fallback_split: &mut bool,
) -> bool {
    let mut covered = 0;
    let mut previous_face = None;
    let mut run_count = 0;
    while covered < layout.positioned_glyphs.len() {
        let Some(span) = prepared.get(*run_index) else {
            return false;
        };
        let remaining = layout.positioned_glyphs.len() - covered;
        if !prepared_run_fits(span, remaining, font_size, previous_face) {
            return false;
        }
        covered += span.glyphs.len();
        previous_face = Some(span.font);
        run_count += 1;
        *run_index += 1;
    }
    *has_fallback_split |= run_count > 1;
    true
}

fn prepared_run_fits(
    span: &PreparedSpan,
    remaining_glyphs: usize,
    font_size: f32,
    previous_face: Option<usvg::fontdb::ID>,
) -> bool {
    span.glyphs.len() <= remaining_glyphs
        && previous_face != Some(span.font)
        && prepared_run_supported(span, font_size)
}

fn prepared_run_supported(span: &PreparedSpan, font_size: f32) -> bool {
    !span.glyphs.is_empty()
        && span.font_size == font_size
        && span.glyph_ids.len() == span.glyphs.len()
        && span.x_alignment.is_none()
        && span.chunk_alignment.is_none()
        && super::super::horizontal_glyphs(&span.glyphs)
        && span
            .glyphs
            .iter()
            .all(|glyph| glyph.font == span.font && glyph.font_size() == span.font_size)
}

fn anchor_supported(anchor: usvg::TextAnchor) -> bool {
    #[cfg(test)]
    if std::env::var("KRR_HTML_FEATURE_START_ONLY_FALLBACK_RUNS").as_deref() == Ok("1") {
        return anchor == usvg::TextAnchor::Start;
    }
    matches!(
        anchor,
        usvg::TextAnchor::Start | usvg::TextAnchor::Middle | usvg::TextAnchor::End
    )
}

pub(super) fn glyph_ids_match_cmap(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    glyphs: &[usvg::layout::PositionedGlyph],
) -> bool {
    database.with_face_data(face_id, |data, index| {
        let Ok(face) = rustybuzz::ttf_parser::Face::parse(data, index) else {
            return false;
        };
        text.chars().zip(glyphs).all(|(character, glyph)| {
            glyph.text == character.to_string()
                && face
                    .glyph_index(character)
                    .is_some_and(|glyph_id| u32::from(glyph_id.0) == glyph.id.0)
        })
    }) == Some(true)
}
