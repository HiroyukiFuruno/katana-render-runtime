use super::{CachedFace, FaceMetadata};
use resvg::usvg;

#[path = "svg_rasterize_html_glyph_baseline_primary.rs"]
mod primary;
pub(super) use primary::resolved_policy;
#[path = "svg_rasterize_html_glyph_baseline_kind.rs"]
mod kind;
#[path = "svg_rasterize_html_glyph_baseline_middle.rs"]
mod middle;
#[path = "svg_rasterize_html_glyph_baseline_policy.rs"]
mod policy;
#[cfg(test)]
use policy::{uniform_policy, uniform_policy_with_nested_context};
#[path = "svg_rasterize_html_glyph_baseline_source_context.rs"]
mod source_context;
pub(super) use source_context::has_nested_middle_candidate;

const HALF_METRIC: f32 = 0.5;
const HANGING_BASELINE_SHIFT_FACTOR: f32 = 0.8;

fn is_middle(kind: PolicyKind) -> bool {
    matches!(kind, PolicyKind::Middle | PolicyKind::AlignmentMiddle)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PolicyKind {
    Mathematical,
    Central,
    Hanging,
    Middle,
    AlignmentMiddle,
}

#[derive(Clone, Copy)]
pub(super) struct Policy {
    kind: PolicyKind,
    font_size: f32,
    css_scale: Option<f32>,
    /* WHY: Middleは主fontの解決済み補正がある場合だけ描画へ渡す。 */
    primary_shift: Option<f32>,
}

pub(super) fn glyph_shift(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    face: &CachedFace,
    policy: Policy,
) -> f32 {
    if let Some(shift) = policy.primary_shift {
        return shift;
    }
    if is_middle(policy.kind) {
        /* WHY: Middleは解決済み主font補正を持つpolicyだけが適用される。 */
        return 0.0;
    }
    metrics_shift(
        database,
        face_id,
        &FaceMetadata {
            upem: face.upem,
            unsupported: false,
            ascent: face.ascent,
            descent: face.descent,
            x_height: face.x_height,
        },
        policy,
    )
}

fn metrics_shift(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    face: &FaceMetadata,
    policy: Policy,
) -> f32 {
    if is_middle(policy.kind) {
        return 0.0;
    }
    let scale = policy.font_size / face.upem;
    let ascent = face.ascent * scale;
    let descent = face.descent * scale;
    let raw_shift = match policy.kind {
        PolicyKind::Mathematical => ascent * HALF_METRIC,
        PolicyKind::Central => ascent - (ascent - descent) * HALF_METRIC,
        PolicyKind::Hanging => ascent * HANGING_BASELINE_SHIFT_FACTOR,
        PolicyKind::Middle | PolicyKind::AlignmentMiddle => return 0.0,
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
    face: &FaceMetadata,
    policy: Policy,
    css_scale: f32,
) -> Option<f32> {
    if is_middle(policy.kind) {
        return None;
    }
    let css_font_size = super::anchor::paint_size(policy.font_size, css_scale, css_scale)?;
    let target_css = css_baseline_target(database, face_id, face, policy, css_font_size)?;
    Some(target_css / css_scale)
}

fn css_baseline_target(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    face: &FaceMetadata,
    policy: Policy,
    css_font_size: f32,
) -> Option<f32> {
    let metric_scale = css_font_size / face.upem;
    let ascent = face.ascent * metric_scale;
    let descent = face.descent * metric_scale;
    let layout_ascent = crate::markdown::svg_rasterize::text_shaping::svg_dom_layout_ascent(
        database.face(face_id),
        ascent,
        -descent,
    )?;
    if policy.kind == PolicyKind::Hanging {
        /* WHY: Hanging fallbackはBlinkのpostprocessed ascentから補正し、origin snapはpaintで行う。 */
        return Some(layout_ascent * HANGING_BASELINE_SHIFT_FACTOR);
    }
    let layout_descent = -descent.round();
    let baseline_from_top = match policy.kind {
        PolicyKind::Mathematical => ascent_from_top(layout_ascent),
        PolicyKind::Central => central_from_top(layout_ascent, layout_descent),
        PolicyKind::Hanging => return None,
        PolicyKind::Middle | PolicyKind::AlignmentMiddle => return None,
    };
    /* WHY: line-topの丸めはCSS座標で行い、拡大前の半pixel丸めによるbaselineのずれを避ける。 */
    Some(layout_ascent + (HALF_METRIC - baseline_from_top).floor())
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
    if is_middle(policy.kind) {
        return 0.0;
    }
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
        PolicyKind::Hanging => return 0.0,
        PolicyKind::Middle | PolicyKind::AlignmentMiddle => return 0.0,
    };
    blink_shift - raw_shift
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_baseline_unit_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_baseline_origin_tests.rs"]
mod origin_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_baseline_alignment_tests.rs"]
mod alignment_tests;
#[cfg(test)]
#[path = "svg_rasterize_html_glyph_baseline_nested_context_tests.rs"]
mod nested_context_tests;
