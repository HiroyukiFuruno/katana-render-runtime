use super::CachedFace;
use resvg::usvg::{self, AlignmentBaseline, DominantBaseline, TextFlow};

const HALF_METRIC: f32 = 0.5;

#[derive(Clone, Copy, Debug)]
pub(super) enum PolicyKind {
    Mathematical,
    Central,
}

#[derive(Clone, Copy)]
pub(super) struct Policy {
    kind: PolicyKind,
    font_size: f32,
}

pub(super) fn uniform_policy(text: &usvg::Text) -> Option<Policy> {
    if text.writing_mode() != usvg::WritingMode::LeftToRight {
        return None;
    }
    let mut policy = None;
    for chunk in text.chunks() {
        if !matches!(chunk.text_flow(), TextFlow::Linear) {
            return None;
        }
        for span in chunk.spans() {
            let current = Policy {
                kind: span_kind(span)?,
                font_size: span.font_size().get(),
            };
            if policy.is_some_and(|value: Policy| {
                value.font_size != current.font_size || !same_kind(value.kind, current.kind)
            }) {
                return None;
            }
            policy = Some(current);
        }
    }
    policy
}

fn same_kind(left: PolicyKind, right: PolicyKind) -> bool {
    matches!(
        (left, right),
        (PolicyKind::Mathematical, PolicyKind::Mathematical)
            | (PolicyKind::Central, PolicyKind::Central)
    )
}

fn span_kind(span: &usvg::TextSpan) -> Option<PolicyKind> {
    let baseline = span.alignment_baseline();
    if !matches!(
        baseline,
        AlignmentBaseline::Auto | AlignmentBaseline::Baseline
    ) {
        return match baseline {
            AlignmentBaseline::Mathematical => Some(PolicyKind::Mathematical),
            AlignmentBaseline::Central => Some(PolicyKind::Central),
            _ => None,
        };
    }
    match span.dominant_baseline() {
        DominantBaseline::Mathematical => Some(PolicyKind::Mathematical),
        DominantBaseline::Central => Some(PolicyKind::Central),
        _ => None,
    }
}

pub(super) fn glyph_shift(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    face: &CachedFace,
    policy: Policy,
) -> f32 {
    let scale = policy.font_size / face.upem;
    let ascent = face.ascent * scale;
    let descent = face.descent * scale;
    let Some(layout_ascent) = crate::markdown::svg_rasterize::text_shaping::svg_dom_layout_ascent(
        database.face(face_id),
        ascent,
        -descent,
    ) else {
        return 0.0;
    };
    let layout_descent = -descent.round();
    let raw_shift = match policy.kind {
        PolicyKind::Mathematical => ascent * HALF_METRIC,
        PolicyKind::Central => ascent - (ascent - descent) * HALF_METRIC,
    };
    let blink_shift = match policy.kind {
        PolicyKind::Mathematical => layout_ascent * HALF_METRIC,
        PolicyKind::Central => (layout_ascent - layout_descent) * HALF_METRIC,
    };
    blink_shift - raw_shift
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_baseline_unit_tests.rs"]
mod tests;
