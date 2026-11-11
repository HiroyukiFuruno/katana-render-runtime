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
    let spans = chunk.spans().iter().zip(std::iter::repeat(()));
    ranges_match(chunk.text(), spans, |_, _, ()| true)
}

pub(super) fn ranges_match<'a, T>(
    text: &str,
    spans: impl Iterator<Item = (&'a usvg::TextSpan, T)>,
    mut matches: impl FnMut(&usvg::TextSpan, &str, T) -> bool,
) -> bool {
    let mut end = 0;
    for (span, value) in spans {
        if span.start() != end {
            return false;
        }
        let Some(content) = span_content(text, span.start(), span.end()) else {
            return false;
        };
        if !matches(span, content, value) {
            return false;
        }
        end = span.end();
    }
    end == text.len()
}

pub(super) fn span_content(text: &str, start: usize, end: usize) -> Option<&str> {
    if start >= end {
        return None;
    }
    text.get(start..end)
}
