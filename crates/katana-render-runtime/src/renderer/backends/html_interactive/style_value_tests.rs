use super::{CssLength, box_sides, css_font_weight, css_relative_px, css_resolved_px};

#[test]
fn box_sides_accepts_all_supported_notations() {
    assert_eq!(box_sides("3px", 16.0, 1000.0, 500.0, false), Some([3.0; 4]));
    assert_eq!(
        box_sides("1px 2px", 16.0, 1000.0, 500.0, false),
        Some([1.0, 2.0, 1.0, 2.0])
    );
    assert_eq!(
        box_sides("1px 2px 3px", 16.0, 1000.0, 500.0, false),
        Some([1.0, 2.0, 3.0, 2.0])
    );
    assert_eq!(
        box_sides("1px 2px 3px 4px", 16.0, 1000.0, 500.0, false),
        Some([1.0, 2.0, 3.0, 4.0])
    );
    assert!(box_sides("1px 2px 3px 4px 5px", 16.0, 1000.0, 500.0, false).is_none());
}

#[test]
fn font_weight_accepts_css_keywords() {
    assert_eq!(css_font_weight("normal", 600), Some(400));
    assert_eq!(css_font_weight("bold", 400), Some(700));
    assert_eq!(css_font_weight("bolder", 300), Some(400));
    assert_eq!(css_font_weight("bolder", 400), Some(700));
    assert_eq!(css_font_weight("bolder", 800), Some(900));
    assert_eq!(css_font_weight("lighter", 400), Some(100));
    assert_eq!(css_font_weight("lighter", 700), Some(400));
    assert_eq!(css_font_weight("lighter", 800), Some(700));
}

#[test]
fn font_weight_accepts_only_the_numeric_css_range() {
    assert_eq!(css_font_weight("600", 400), Some(600));
    assert_eq!(css_font_weight("0", 400), None);
    assert_eq!(css_font_weight("1001", 400), None);
}

#[test]
fn relative_px_parses_units_and_signing_rules() {
    assert_eq!(css_relative_px("10px", 16.0, false), Some(10.0));
    assert_eq!(css_relative_px("1.5rem", 16.0, false), Some(24.0));
    assert_eq!(css_relative_px("2em", 16.0, false), Some(32.0));
    assert_eq!(css_relative_px("-5px", 16.0, false), None);
    assert_eq!(css_relative_px("-5px", 16.0, true), Some(-5.0));
    assert_eq!(css_length_percent("40%"), Some(CssLength::Percent(0.4)));
}

#[test]
fn parse_percent_length_then_resolve() {
    assert_eq!(
        CssLength::parse("12.5%", 10.0, 1000.0, 500.0).map(|size| size.resolve(160.0)),
        Some(20.0)
    );
}

fn css_length_percent(value: &str) -> Option<CssLength> {
    const FONT_SIZE: f32 = 16.0;
    const VIEWPORT_WIDTH: f32 = 1000.0;
    const VIEWPORT_HEIGHT: f32 = 500.0;
    CssLength::parse(value, FONT_SIZE, VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
}

#[test]
fn viewport_units_and_nested_math_resolve_against_the_page_viewport() {
    assert_eq!(
        css_resolved_px("4.6vw", 16.0, 1440.0, 900.0, false),
        Some(66.24)
    );
    assert_eq!(
        css_resolved_px("100vh", 16.0, 1440.0, 900.0, false),
        Some(900.0)
    );
    assert_eq!(
        css_resolved_px("10vmin", 16.0, 1440.0, 900.0, false),
        Some(90.0)
    );
    assert_eq!(
        css_resolved_px("10vmax", 16.0, 1440.0, 900.0, false),
        Some(144.0)
    );
}

#[test]
fn nested_css_math_resolves_and_rejects_invalid_arity() {
    assert_eq!(
        css_resolved_px(
            "clamp(38px, min(4.6vw, 70px), 64px)",
            16.0,
            1440.0,
            900.0,
            false
        ),
        Some(64.0)
    );
    assert_eq!(
        css_resolved_px("max(12px, 2vw)", 16.0, 1000.0, 500.0, false),
        Some(20.0)
    );
    assert!(css_resolved_px("clamp(1px, 2px)", 16.0, 1000.0, 500.0, false).is_none());
}

#[test]
fn percentage_font_size_keeps_decimal_precision_until_resolution() {
    assert_eq!(super::css_font_size("84%", 14.0, 100.0, 50.0), Some(11.76));
    assert!(super::css_font_size("1e100%", 14.0, 100.0, 50.0).is_none());
}
