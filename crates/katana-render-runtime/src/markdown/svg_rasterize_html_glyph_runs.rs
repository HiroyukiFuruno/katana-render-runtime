use resvg::usvg;

pub(super) struct GlyphRun {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) font: usvg::fontdb::ID,
    pub(super) font_size: f32,
}

pub(super) fn consecutive_runs(glyphs: &[usvg::layout::PositionedGlyph]) -> ConsecutiveRuns<'_> {
    ConsecutiveRuns { glyphs, start: 0 }
}

pub(super) struct ConsecutiveRuns<'a> {
    glyphs: &'a [usvg::layout::PositionedGlyph],
    start: usize,
}

impl Iterator for ConsecutiveRuns<'_> {
    type Item = GlyphRun;

    fn next(&mut self) -> Option<Self::Item> {
        let first = self.glyphs.get(self.start)?;
        let font = first.font;
        let font_size = first.font_size();
        let mut end = self.start + 1;
        while let Some(glyph) = self.glyphs.get(end) {
            if glyph.font != font || glyph.font_size() != font_size {
                break;
            }
            end += 1;
        }
        let run = GlyphRun {
            start: self.start,
            end,
            font,
            font_size,
        };
        self.start = end;
        Some(run)
    }
}
