use super::{Policy, PolicyKind, middle};
use resvg::usvg::{self, TextFlow};

pub(super) fn uniform_policy(text: &usvg::Text) -> Option<Policy> {
    if text.writing_mode() != usvg::WritingMode::LeftToRight {
        return None;
    }
    let css_scale = super::super::anchor::uniform_css_scale(text.abs_transform());
    let middle_supported = css_scale.is_some() && middle::layout_supported(text);
    let mut policy = None;
    for chunk in text.chunks() {
        if !matches!(chunk.text_flow(), TextFlow::Linear) {
            return None;
        }
        for span in chunk.spans() {
            let current = policy_for_span(span, css_scale, middle_supported)?;
            if policy.is_some_and(|value| !same_policy(value, current)) {
                return None;
            }
            policy = Some(current);
        }
    }
    policy
}

fn policy_for_span(
    span: &usvg::TextSpan,
    css_scale: Option<f32>,
    middle_supported: bool,
) -> Option<Policy> {
    let kind = super::kind::span_kind(span, middle_supported)?;
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
