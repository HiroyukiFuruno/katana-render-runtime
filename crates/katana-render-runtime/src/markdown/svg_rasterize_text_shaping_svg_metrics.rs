use super::harfrust::SvgInkBounds;
use crate::markdown::svg_rasterize::text_shaping::{
    FontShape, SvgTextRunMetrics, cache, html_font_db_for_measured_text, matching_font_face,
    resolved_font_runs,
};

const HARFRUST_SCALE: f32 = 65_536.0;

pub(in crate::markdown::svg_rasterize) fn shaped_svg_text_metrics(
    text: &str,
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
) -> Option<SvgTextRunMetrics> {
    if text.contains('\0') {
        return None;
    }
    if size == 0.0 {
        return Some(empty_svg_text_metrics(0.0, 0.0));
    }
    let font = validated_svg_font_shape(size, weight, italic)?;
    if text.is_empty() {
        return Some(empty_svg_text_metrics(0.0, 0.0));
    }
    metrics_for_svg_font_runs(text, family, font)
}

fn empty_svg_text_metrics(ascent: f32, descent: f32) -> SvgTextRunMetrics {
    SvgTextRunMetrics {
        advance: 0.0,
        ascent,
        descent,
        layout_ascent: None,
        ink_bounds: None,
    }
}

pub(super) fn validated_svg_font_shape(size: f32, weight: u16, italic: bool) -> Option<FontShape> {
    let font = FontShape::new(size, weight, italic);
    let shaping_scale = font.size * HARFRUST_SCALE;
    if !font.size.is_finite()
        || font.size <= 0.0
        || !shaping_scale.is_finite()
        || f64::from(shaping_scale) > f64::from(i32::MAX)
        || shaping_scale as i32 == 0
    {
        return None;
    }
    Some(font)
}

fn metrics_for_svg_font_runs(
    text: &str,
    family: &str,
    font: FontShape,
) -> Option<SvgTextRunMetrics> {
    let database = html_font_db_for_measured_text(family, text, font.weight, font.italic);
    let base = matching_font_face(&database, family, font.weight, font.italic)?;
    let mut result = empty_svg_text_metrics(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let includes_primary =
        accumulate_svg_font_run_metrics(&database, base, family, text, font, &mut result)?;
    include_svg_primary_strut(&database, base, font, includes_primary, &mut result)?;
    validated_svg_run_metrics(result)
}

fn accumulate_svg_font_run_metrics(
    database: &std::sync::Arc<resvg::usvg::fontdb::Database>,
    base: resvg::usvg::fontdb::ID,
    family: &str,
    text: &str,
    font: FontShape,
    result: &mut SvgTextRunMetrics,
) -> Option<bool> {
    let mut advance_offset = 0.0_f64;
    let mut includes_primary = false;
    for (id, run) in resolved_font_runs(database, base, family, text, font.weight, font.italic) {
        #[cfg(all(test, target_os = "macos"))]
        crate::markdown::svg_rasterize::font::record_actual_selected_face(database, Some(id));
        let metrics = cache::cached_svg_run(database, id, &run, font)?;
        includes_primary |= id == base;
        result.ink_bounds = union_ink_bounds(
            result.ink_bounds,
            translate_ink_bounds(metrics.ink_bounds, advance_offset),
        );
        advance_offset += f64::from(metrics.advance);
        result.advance += metrics.advance;
        result.ascent = result.ascent.max(metrics.ascent);
        result.descent = result.descent.max(metrics.descent);
        include_layout_ascent(
            &mut result.layout_ascent,
            svg_dom_layout_ascent(database.face(id), metrics.ascent, metrics.descent),
        );
    }
    Some(includes_primary)
}

fn include_svg_primary_strut(
    database: &resvg::usvg::fontdb::Database,
    base: resvg::usvg::fontdb::ID,
    font: FontShape,
    includes_primary: bool,
    result: &mut SvgTextRunMetrics,
) -> Option<()> {
    if !includes_primary && result.ascent.is_finite() {
        /* WHY: 代替書体だけで描く文字列にも、CSS第一書体の行の高さを残す。 */
        let (ascent, descent) = super::harfrust::svg_run_endpoints_with_face(database, base, font)?;
        result.ascent = result.ascent.max(ascent);
        result.descent = result.descent.max(descent);
        include_layout_ascent(
            &mut result.layout_ascent,
            svg_dom_layout_ascent(database.face(base), ascent, descent),
        );
    }
    Some(())
}

fn include_layout_ascent(current: &mut Option<f32>, candidate: Option<f32>) {
    if let Some(candidate) = candidate {
        *current = Some(current.map_or(candidate, |current| current.max(candidate)));
    }
}

pub(in crate::markdown::svg_rasterize) fn svg_dom_layout_ascent(
    face: Option<&resvg::usvg::fontdb::FaceInfo>,
    ascent: f32,
    descent: f32,
) -> Option<f32> {
    #[cfg(target_os = "macos")]
    {
        let rounded_ascent = ascent.round();
        let is_legacy_blink_family = face.is_some_and(|face| {
            face.families
                .iter()
                .any(|(family, _)| is_legacy_blink_family_name(family))
        });
        Some(if is_legacy_blink_family {
            rounded_ascent + chromium_legacy_ascent_adjustment(ascent, descent)
        } else {
            rounded_ascent
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (face, ascent, descent);
        None
    }
}

#[cfg(any(target_os = "macos", test))]
fn is_legacy_blink_family_name(family: &str) -> bool {
    matches!(family, "Times" | "Helvetica" | "Courier")
}

#[cfg(any(target_os = "macos", test))]
fn chromium_legacy_ascent_adjustment(ascent: f32, descent: f32) -> f32 {
    const ASCENT_ADJUSTMENT_RATIO: f32 = 0.15;
    const ASCENT_ROUNDING_BIAS: f32 = 0.5;
    let rounded_ascent = ascent.round();
    let rounded_descent = descent.round();
    /* WHY: BlinkはmacOSのlegacy familyに総高の15%を加える。https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/platform/fonts/font_metrics.cc */
    ((rounded_ascent + rounded_descent) * ASCENT_ADJUSTMENT_RATIO + ASCENT_ROUNDING_BIAS).floor()
}

fn translate_ink_bounds(bounds: Option<SvgInkBounds>, advance: f64) -> Option<SvgInkBounds> {
    bounds.map(|[min_x, min_y, max_x, max_y]| [min_x + advance, min_y, max_x + advance, max_y])
}

fn union_ink_bounds(
    current: Option<SvgInkBounds>,
    next: Option<SvgInkBounds>,
) -> Option<SvgInkBounds> {
    match (current, next) {
        (
            Some([left, top, right, bottom]),
            Some([next_left, next_top, next_right, next_bottom]),
        ) => Some([
            left.min(next_left),
            top.min(next_top),
            right.max(next_right),
            bottom.max(next_bottom),
        ]),
        (Some(current), None) => Some(current),
        (None, Some(next)) => Some(next),
        (None, None) => None,
    }
}

fn validated_svg_run_metrics(result: SvgTextRunMetrics) -> Option<SvgTextRunMetrics> {
    ([result.advance, result.ascent, result.descent]
        .into_iter()
        .all(f32::is_finite)
        && result.advance >= 0.0
        && result.ascent + result.descent > 0.0
        && result
            .layout_ascent
            .is_none_or(|ascent| ascent.is_finite() && ascent >= 0.0))
    .then_some(result)
}

#[cfg(test)]
mod tests {
    use super::{
        chromium_legacy_ascent_adjustment, include_layout_ascent, is_legacy_blink_family_name,
        translate_ink_bounds, union_ink_bounds,
    };

    #[cfg(not(target_os = "macos"))]
    use super::svg_dom_layout_ascent;

    #[test]
    fn fallback_ink_union_applies_each_run_advance_in_svg_coordinates() -> Result<(), String> {
        let first = Some([0.25, -8.0, 3.5, 2.0]);
        let second = translate_ink_bounds(Some([0.5, -7.0, 2.5, 3.0]), 3.5);
        let combined = union_ink_bounds(first, second).ok_or("two runs must have ink")?;
        assert_eq!(second, Some([4.0, -7.0, 6.0, 3.0]));
        assert_eq!(combined, [0.25, -8.0, 6.0, 3.0]);
        assert!(combined.iter().all(|value| value.is_finite()));
        assert!(combined[0] <= combined[2] && combined[1] <= combined[3]);
        Ok(())
    }

    #[test]
    fn empty_run_ink_does_not_discard_the_advance_offset() {
        let offset = 4.0 + 2.0;
        let second = translate_ink_bounds(Some([0.0, -1.0, 1.0, 1.0]), offset);
        assert_eq!(second, Some([6.0, -1.0, 7.0, 1.0]));
    }

    #[test]
    fn chromium_legacy_ascent_is_derived_from_rounded_font_metrics() {
        assert_eq!(chromium_legacy_ascent_adjustment(10.780273, 3.219727), 2.0);
        assert_eq!(chromium_legacy_ascent_adjustment(8.085205, 2.414795), 2.0);
        assert_eq!(chromium_legacy_ascent_adjustment(9.240234, 2.759766), 2.0);
        assert!(is_legacy_blink_family_name("Helvetica"));
        assert!(is_legacy_blink_family_name("Times"));
        assert!(is_legacy_blink_family_name("Courier"));
        assert!(!is_legacy_blink_family_name("Helvetica Neue"));
        assert!(!is_legacy_blink_family_name("Open Sans"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn c4_legacy_platform_face_adds_layout_ascent_without_changing_natural_metrics()
    -> Result<(), String> {
        for (text, size, weight) in [
            ("Single Page Application", 14.0, 700),
            ("Container: javascript and react", 10.5, 400),
        ] {
            let metrics = super::shaped_svg_text_metrics(text, "Helvetica", size, weight, false)
                .ok_or("installed Helvetica must shape C4 labels")?;
            let expected_layout_ascent = metrics.ascent.round()
                + chromium_legacy_ascent_adjustment(metrics.ascent, metrics.descent);
            assert_eq!(metrics.layout_ascent, Some(expected_layout_ascent));
            assert_eq!(metrics.ascent + metrics.descent, size);
            assert!(metrics.ink_bounds.is_some());
        }
        Ok(())
    }

    #[test]
    fn mixed_svg_faces_keep_the_largest_layout_ascent() {
        let mut ascent = None;
        include_layout_ascent(&mut ascent, Some(13.0));
        include_layout_ascent(&mut ascent, Some(30.0));
        include_layout_ascent(&mut ascent, Some(10.0));
        assert_eq!(ascent, Some(30.0));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_svg_layout_ascent_keeps_existing_path() {
        assert_eq!(svg_dom_layout_ascent(None, 10.780273, 3.219727), None);
    }
}
