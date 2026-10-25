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
    css_scale: Option<f32>,
}

pub(super) fn uniform_policy(text: &usvg::Text) -> Option<Policy> {
    if text.writing_mode() != usvg::WritingMode::LeftToRight {
        return None;
    }
    let css_scale = super::anchor::uniform_css_scale(text.abs_transform());
    let mut policy = None;
    for chunk in text.chunks() {
        if !matches!(chunk.text_flow(), TextFlow::Linear) {
            return None;
        }
        for span in chunk.spans() {
            let current = Policy {
                kind: span_kind(span)?,
                font_size: span.font_size().get(),
                css_scale,
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
    let raw_shift = match policy.kind {
        PolicyKind::Mathematical => ascent * HALF_METRIC,
        PolicyKind::Central => ascent - (ascent - descent) * HALF_METRIC,
    };
    if let Some(target_local) = policy
        .css_scale
        .and_then(|css_scale| css_baseline_shift(database, face_id, face, policy, css_scale))
    {
        return target_local - raw_shift;
    }
    legacy_glyph_shift(database, face_id, policy, ascent, descent, raw_shift)
}

fn css_baseline_shift(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    face: &CachedFace,
    policy: Policy,
    css_scale: f32,
) -> Option<f32> {
    let css_font_size = super::anchor::paint_size(policy.font_size, css_scale, css_scale)?;
    let metric_scale = css_font_size / face.upem;
    let ascent = face.ascent * metric_scale;
    let descent = face.descent * metric_scale;
    let layout_ascent = crate::markdown::svg_rasterize::text_shaping::svg_dom_layout_ascent(
        database.face(face_id),
        ascent,
        -descent,
    )?;
    let layout_descent = -descent.round();
    let baseline_from_top = match policy.kind {
        PolicyKind::Mathematical => ascent_from_top(layout_ascent),
        PolicyKind::Central => central_from_top(layout_ascent, layout_descent),
    };
    /* WHY: line-topの丸めはCSS座標で行い、拡大前の半pixel丸めによるbaselineのずれを避ける。 */
    let target_css = layout_ascent + (HALF_METRIC - baseline_from_top).floor();
    Some(target_css / css_scale)
}

fn ascent_from_top(layout_ascent: f32) -> f32 {
    layout_ascent * HALF_METRIC
}

fn central_from_top(layout_ascent: f32, layout_descent: f32) -> f32 {
    (layout_ascent + layout_descent) * HALF_METRIC
}

fn legacy_glyph_shift(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    policy: Policy,
    ascent: f32,
    descent: f32,
    raw_shift: f32,
) -> f32 {
    let Some(layout_ascent) = crate::markdown::svg_rasterize::text_shaping::svg_dom_layout_ascent(
        database.face(face_id),
        ascent,
        -descent,
    ) else {
        return 0.0;
    };
    let layout_descent = -descent.round();
    let blink_shift = match policy.kind {
        PolicyKind::Mathematical => layout_ascent * HALF_METRIC,
        PolicyKind::Central => (layout_ascent - layout_descent) * HALF_METRIC,
    };
    blink_shift - raw_shift
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_baseline_unit_tests.rs"]
mod tests;
