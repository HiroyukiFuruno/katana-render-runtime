use resvg::usvg;

pub(super) fn printable_ascii(text: &str) -> bool {
    text.bytes()
        .all(|value| value.is_ascii() && !value.is_ascii_control())
}

pub(super) fn supported_ltr_text(text: &str) -> bool {
    text.chars().all(|character| {
        matches!(character, '\u{20}'..='\u{7e}' | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{ff}' | '\u{3041}'..='\u{3096}' | '\u{30a1}'..='\u{30fa}' | '\u{4e00}'..='\u{9fff}')
    })
}

pub(in super::super::super) fn ranges_cover_chunk(chunk: &usvg::TextChunk) -> bool {
    let mut end = 0;
    for span in chunk.spans() {
        if span.start() != end || !valid_span_range(chunk.text(), span.start(), span.end()) {
            return false;
        }
        end = span.end();
    }
    end == chunk.text().len()
}

pub(super) fn valid_span_range(text: &str, start: usize, end: usize) -> bool {
    start < end && text.get(start..end).is_some()
}
