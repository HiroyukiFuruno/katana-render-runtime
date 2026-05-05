pub struct Location {
    pub line: usize,
    pub column: usize,
}

pub struct SpanOps;

impl SpanOps {
    pub fn start(span: proc_macro2::Span) -> Location {
        let start = span.start();
        Location {
            line: start.line,
            column: start.column + 1,
        }
    }

    pub fn end_line(span: proc_macro2::Span) -> usize {
        span.end().line
    }

    pub fn block_end_line(block: &syn::Block) -> usize {
        Self::end_line(block.brace_token.span.close())
    }
}
