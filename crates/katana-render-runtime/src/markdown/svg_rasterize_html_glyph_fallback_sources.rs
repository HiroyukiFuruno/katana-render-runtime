use resvg::usvg;

pub(super) struct Chunk<'a> {
    pub(super) chunk: &'a usvg::TextChunk,
    pub(super) source_spans: &'a [usvg::TextSpan],
    pub(super) layout_spans: &'a [usvg::layout::Span],
    pub(super) dy: &'a [f32],
}

pub(super) struct Source<'a> {
    pub(super) chunks: Vec<Chunk<'a>>,
}

pub(super) fn collect(text: &usvg::Text) -> Option<Source<'_>> {
    let mut layout_offset = 0;
    let mut character_offset = 0;
    let chunks = text
        .chunks()
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            if index > 0 && chunk.x().is_none() {
                return None;
            }
            collect_chunk(chunk, text, &mut layout_offset, &mut character_offset)
        })
        .collect::<Option<Vec<_>>>()?;
    (layout_offset == text.layouted().len()
        && character_offset == text.dy().len()
        && !chunks.is_empty())
    .then_some(Source { chunks })
}

fn collect_chunk<'a>(
    chunk: &'a usvg::TextChunk,
    text: &'a usvg::Text,
    layout_offset: &mut usize,
    character_offset: &mut usize,
) -> Option<Chunk<'a>> {
    let source_spans = chunk.spans();
    if source_spans.is_empty() {
        return None;
    }
    let layout_end = (*layout_offset).checked_add(source_spans.len())?;
    let character_end = (*character_offset).checked_add(chunk.text().chars().count())?;
    let layout_spans = text.layouted().get(*layout_offset..layout_end)?;
    let dy = text.dy().get(*character_offset..character_end)?;
    *layout_offset = layout_end;
    *character_offset = character_end;
    Some(Chunk {
        chunk,
        source_spans,
        layout_spans,
        dy,
    })
}
