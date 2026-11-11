#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize) fn macos_svg_dom_layout_ascent(
    face: Option<&resvg::usvg::fontdb::FaceInfo>,
    ascent: f32,
    descent: f32,
) -> f32 {
    let rounded_ascent = ascent.round();
    let is_legacy_blink_family = face.is_some_and(|face| {
        face.families
            .iter()
            .any(|(family, _)| is_legacy_blink_family_name(family))
    });
    if is_legacy_blink_family {
        rounded_ascent + chromium_legacy_ascent_adjustment(ascent, descent)
    } else {
        rounded_ascent
    }
}

pub(super) fn is_legacy_blink_family_name(family: &str) -> bool {
    matches!(family, "Times" | "Helvetica" | "Courier")
}

pub(super) fn chromium_legacy_ascent_adjustment(ascent: f32, descent: f32) -> f32 {
    const ASCENT_ADJUSTMENT_RATIO: f32 = 0.15;
    const ASCENT_ROUNDING_BIAS: f32 = 0.5;
    let rounded_ascent = ascent.round();
    let rounded_descent = descent.round();
    /* WHY: BlinkはmacOSのlegacy familyに総高の15%を加える。https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/platform/fonts/font_metrics.cc */
    ((rounded_ascent + rounded_descent) * ASCENT_ADJUSTMENT_RATIO + ASCENT_ROUNDING_BIAS).floor()
}
