use super::PreparedSpan;
use resvg::usvg;
use std::ops::Range;
use unicode_script::{Script, UnicodeScript};

pub(super) struct ScriptFragment {
    pub(super) glyph_range: Range<usize>,
    pub(super) text: String,
}

struct FragmentsBuilder {
    fragments: Vec<ScriptFragment>,
    start: usize,
    text: String,
    current_script: Option<Script>,
}

impl FragmentsBuilder {
    fn new() -> Self {
        Self {
            fragments: Vec::new(),
            start: 0,
            text: String::new(),
            current_script: None,
        }
    }

    fn push(&mut self, index: usize, glyph: &usvg::layout::PositionedGlyph, script: Script) {
        if self.current_script.is_some_and(|current| current != script) {
            self.finish_fragment(index);
        }
        self.current_script = Some(script);
        self.text.push_str(&glyph.text);
    }

    fn finish_fragment(&mut self, end: usize) {
        self.fragments.push(ScriptFragment {
            glyph_range: self.start..end,
            text: std::mem::take(&mut self.text),
        });
        self.start = end;
    }

    fn finish(mut self, glyph_count: usize) -> Vec<ScriptFragment> {
        if self.current_script.is_some() {
            self.finish_fragment(glyph_count);
        }
        self.fragments
    }
}

#[cfg(test)]
pub(super) fn resolved_scripts(glyphs: &[usvg::layout::PositionedGlyph]) -> Option<Vec<Script>> {
    let raw = glyph_scripts(glyphs)?;
    Some(resolve_weak_scripts(raw))
}

pub(super) fn resolved_layout_scripts(layouts: &[usvg::layout::Span]) -> Option<Vec<Script>> {
    let raw = layouts
        .iter()
        .flat_map(|layout| &layout.positioned_glyphs)
        .map(glyph_script)
        .collect::<Option<Vec<_>>>()?;
    Some(resolve_weak_scripts(raw))
}

#[cfg(test)]
fn glyph_scripts(glyphs: &[usvg::layout::PositionedGlyph]) -> Option<Vec<Script>> {
    glyphs.iter().map(glyph_script).collect()
}

fn glyph_script(glyph: &usvg::layout::PositionedGlyph) -> Option<Script> {
    let mut characters = glyph.text.chars();
    let character = characters.next()?;
    characters.next().is_none().then(|| character.script())
}

fn resolve_weak_scripts(raw: Vec<Script>) -> Vec<Script> {
    let mut resolved = vec![None; raw.len()];
    apply_previous_strong_scripts(&raw, &mut resolved);
    apply_next_strong_scripts(&raw, &mut resolved);
    resolved
        .into_iter()
        .map(|script| script.unwrap_or(Script::Common))
        .collect()
}

fn apply_previous_strong_scripts(raw: &[Script], resolved: &mut [Option<Script>]) {
    let mut previous_strong = None;
    for (index, script) in raw.iter().copied().enumerate() {
        if !is_weak_script(script) {
            previous_strong = Some(script);
        }
        resolved[index] = previous_strong;
    }
}

fn apply_next_strong_scripts(raw: &[Script], resolved: &mut [Option<Script>]) {
    let mut next_strong = None;
    for index in (0..raw.len()).rev() {
        if is_weak_script(raw[index]) {
            if resolved[index].is_none() {
                resolved[index] = next_strong;
            }
        } else {
            next_strong = Some(raw[index]);
        }
    }
}

fn is_weak_script(script: Script) -> bool {
    matches!(script, Script::Common | Script::Inherited | Script::Unknown)
}

pub(super) fn matching_script_fragments(
    source_glyphs: &[usvg::layout::PositionedGlyph],
    span: &PreparedSpan,
    scripts: &[Script],
) -> Option<Vec<ScriptFragment>> {
    if !span_lengths_match(source_glyphs, span, scripts) {
        return None;
    }
    let mut builder = FragmentsBuilder::new();
    for (index, ((source, glyph), glyph_id)) in source_glyphs
        .iter()
        .zip(&span.glyphs)
        .zip(&span.glyph_ids)
        .enumerate()
    {
        if !glyph_matches_prepared(source, glyph, *glyph_id) {
            return None;
        }
        builder.push(index, glyph, scripts[index]);
    }
    Some(builder.finish(span.glyphs.len()))
}

fn span_lengths_match(
    source_glyphs: &[usvg::layout::PositionedGlyph],
    span: &PreparedSpan,
    scripts: &[Script],
) -> bool {
    source_glyphs.len() == span.glyphs.len()
        && scripts.len() == span.glyphs.len()
        && span.glyph_ids.len() == span.glyphs.len()
}

fn glyph_matches_prepared(
    source: &usvg::layout::PositionedGlyph,
    glyph: &usvg::layout::PositionedGlyph,
    glyph_id: u16,
) -> bool {
    source.id == glyph.id
        && u32::from(glyph_id) == glyph.id.0
        && source.text == glyph.text
        && source.font == glyph.font
        && source.font_size() == glyph.font_size()
        && source.transform() == glyph.transform()
}
