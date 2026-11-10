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

#[path = "svg_rasterize_text_shaping_svg_metrics_fallback_tests.rs"]
mod fallback_tests;
