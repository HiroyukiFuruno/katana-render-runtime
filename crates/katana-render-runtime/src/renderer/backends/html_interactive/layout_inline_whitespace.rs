use super::super::super::style::CssStyle;
use super::super::InlineMeasurement;
use super::super::state::InlineFlowState;

#[cfg(test)]
#[path = "layout_inline_whitespace_tests.rs"]
mod tests;

pub(in super::super) fn advance_pending_inline_space(
    inline: &mut InlineFlowState,
    _style: &CssStyle,
) {
    if let Some(width) = inline.pending_space_width.take()
        && inline.has_items
    {
        inline.cursor_x += width;
    }
}

pub(in super::super) fn remember_pending_inline_space(
    inline: &mut InlineFlowState,
    style: &CssStyle,
) {
    if inline.has_items && inline.pending_space_width.is_none() {
        inline.pending_space_width = Some(InlineMeasurement::text_width(" ", style));
    }
}

pub(in super::super) fn normalized_inline_text(
    text: &str,
    inline: &mut InlineFlowState,
    style: &CssStyle,
) -> String {
    let leading_space = text.starts_with([' ', '\t', '\n', '\r', '\u{000c}']);
    let mut normalized = super::super::super::layout_text::collapsed_anonymous_text(text);
    let pending_space = (!normalized.is_empty())
        .then(|| inline.pending_space_width.take())
        .flatten();
    if let Some(width) = pending_space
        && inline.has_items
    {
        inline.cursor_x += width;
    }
    if !normalized.is_empty() && inline.has_items && leading_space && pending_space.is_none() {
        normalized.insert(0, ' ');
    }
    if text.ends_with([' ', '\t', '\n', '\r', '\u{000c}']) {
        if !normalized.is_empty() {
            inline.pending_space_width = Some(InlineMeasurement::text_width(" ", style));
        } else {
            remember_pending_inline_space(inline, style);
        }
    }
    normalized
}
