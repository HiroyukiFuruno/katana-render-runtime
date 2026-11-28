use super::{Policy, PolicyKind, middle};
use resvg::usvg::{self, TextFlow};

#[cfg(test)]
pub(super) fn uniform_policy(text: &usvg::Text) -> Option<Policy> {
    uniform_policy_with_nested_context(text, false)
}

pub(super) fn uniform_policy_with_nested_context(
    text: &usvg::Text,
    nested_text_context: bool,
) -> Option<Policy> {
    uniform_policy_with_context(text, nested_text_context, false)
}

pub(super) fn uniform_policy_with_context(
    text: &usvg::Text,
    nested_text_context: bool,
    additional_text_context: bool,
) -> Option<Policy> {
    uniform_policy_and_primary_span(text, nested_text_context, additional_text_context)
        .map(|(policy, _)| policy)
}

pub(super) fn uniform_policy_and_primary_span(
    text: &usvg::Text,
    nested_text_context: bool,
    additional_text_context: bool,
) -> Option<(Policy, &usvg::TextSpan)> {
    if text.writing_mode() != usvg::WritingMode::LeftToRight {
        return None;
    }
    let css_scale = super::super::anchor::uniform_css_scale(text.abs_transform());
    let middle_supported = css_scale.is_some()
        && (middle::layout_supported(text)
            || (additional_text_context && middle::additional_middle_layout_supported(text)));
    let nested_middle_supported =
        css_scale.is_some() && nested_text_context && middle::nested_layout_supported(text);
    let mut candidate = None;
    for chunk in text.chunks() {
        if !matches!(chunk.text_flow(), TextFlow::Linear) {
            return None;
        }
        for span in chunk.spans() {
            let current =
                policy_for_span(span, css_scale, middle_supported, nested_middle_supported)?;
            if candidate.is_some_and(|(policy, _)| !same_policy(policy, current)) {
                return None;
            }
            candidate.get_or_insert((current, span));
        }
    }
    candidate
}

fn policy_for_span(
    span: &usvg::TextSpan,
    css_scale: Option<f32>,
    middle_supported: bool,
    nested_middle_supported: bool,
) -> Option<Policy> {
    let kind = super::kind::span_kind_with_nested_context(
        span,
        middle_supported,
        nested_middle_supported,
    )?;
    if kind == PolicyKind::Hanging && css_scale.is_none() {
        return None;
    }
    Some(Policy {
        kind,
        font_size: span.font_size().get(),
        css_scale,
        primary_shift: None,
    })
}

fn same_policy(left: Policy, right: Policy) -> bool {
    left.font_size == right.font_size && left.kind == right.kind
}
