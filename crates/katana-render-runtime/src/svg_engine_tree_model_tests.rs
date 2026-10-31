use std::fmt::{Debug, Display};
use std::str::FromStr;

use super::{
    BlendMode, FillRule, ImageRendering, LineCap, LineJoin, MaskType, NonEmptyString, NonZeroF32,
    PaintOrder, ShapeRendering, SpreadMethod, TextRendering, Units, Visibility,
};

const BLEND_MODE_DISPLAY_CASES: &[(BlendMode, &str)] = &[
    (BlendMode::Normal, "normal"),
    (BlendMode::Multiply, "multiply"),
    (BlendMode::Screen, "screen"),
    (BlendMode::Overlay, "overlay"),
    (BlendMode::Darken, "darken"),
    (BlendMode::Lighten, "lighten"),
    (BlendMode::ColorDodge, "color-dodge"),
    (BlendMode::ColorBurn, "color-burn"),
    (BlendMode::HardLight, "hard-light"),
    (BlendMode::SoftLight, "soft-light"),
    (BlendMode::Difference, "difference"),
    (BlendMode::Exclusion, "exclusion"),
    (BlendMode::Hue, "hue"),
    (BlendMode::Saturation, "saturation"),
    (BlendMode::Color, "color"),
    (BlendMode::Luminosity, "luminosity"),
    (BlendMode::PlusLighter, "plus-lighter"),
];

fn assert_enum_tokens<T>(cases: &[(&str, T)]) -> Result<(), String>
where
    T: FromStr<Err = &'static str> + Copy + Debug + PartialEq,
{
    for (token, expected) in cases {
        let actual = T::from_str(token).map_err(str::to_owned)?;
        assert_eq!(actual, *expected);
    }
    assert_eq!(T::from_str("unknown"), Err("invalid"));
    Ok(())
}

fn assert_display_tokens<T>(cases: &[(T, &str)])
where
    T: Copy + Display,
{
    for (value, expected) in cases {
        assert_eq!(value.to_string(), *expected);
    }
}

fn assert_default<T>(expected: T)
where
    T: Copy + Debug + Default + PartialEq,
{
    assert_eq!(T::default(), expected);
}

#[test]
fn non_empty_string_rejects_empty_and_whitespace_but_preserves_valid_input() -> Result<(), String> {
    for value in [String::new(), " \t\n".to_owned()] {
        assert!(NonEmptyString::new(value).is_none());
    }

    let value = NonEmptyString::new(" label ".to_owned())
        .ok_or_else(|| "non-whitespace content was rejected".to_owned())?;
    assert_eq!(value.get(), " label ");
    assert_eq!(value.take(), " label ");
    Ok(())
}

#[test]
fn units_display_matches_svg_attribute_tokens() -> Result<(), String> {
    for (units, expected) in [
        (Units::UserSpaceOnUse, "userSpaceOnUse"),
        (Units::ObjectBoundingBox, "objectBoundingBox"),
    ] {
        assert_eq!(units.to_string(), expected);
    }
    Ok(())
}

#[test]
fn non_zero_float_rejects_both_zero_signs_and_returns_nonzero_values() -> Result<(), String> {
    assert!(NonZeroF32::new(0.0).is_none());
    assert!(NonZeroF32::new(-0.0).is_none());

    for expected in [1.0, -1.0] {
        let value = NonZeroF32::new(expected).ok_or_else(|| "nonzero value rejected".to_owned())?;
        assert_eq!(value.get(), expected);
    }
    Ok(())
}

#[test]
fn rendering_enums_parse_supported_tokens_and_reject_unknown_values() -> Result<(), String> {
    assert_enum_tokens(&[
        ("optimizeSpeed", ShapeRendering::OptimizeSpeed),
        ("crispEdges", ShapeRendering::CrispEdges),
        ("geometricPrecision", ShapeRendering::GeometricPrecision),
    ])?;
    assert_enum_tokens(&[
        ("optimizeSpeed", TextRendering::OptimizeSpeed),
        ("optimizeLegibility", TextRendering::OptimizeLegibility),
        ("geometricPrecision", TextRendering::GeometricPrecision),
    ])?;
    assert_enum_tokens(&[
        ("optimizeQuality", ImageRendering::OptimizeQuality),
        ("optimizeSpeed", ImageRendering::OptimizeSpeed),
        ("smooth", ImageRendering::Smooth),
        ("high-quality", ImageRendering::HighQuality),
        ("crisp-edges", ImageRendering::CrispEdges),
        ("pixelated", ImageRendering::Pixelated),
    ])?;
    Ok(())
}

#[test]
fn blend_mode_display_matches_each_standard_token() {
    assert_display_tokens(BLEND_MODE_DISPLAY_CASES);
    assert_default(BlendMode::Normal);
}

#[test]
fn tree_rendering_defaults_match_model_contract() {
    assert_default(Visibility::Visible);
    assert_default(ShapeRendering::GeometricPrecision);
    assert_default(TextRendering::OptimizeLegibility);
    assert_default(ImageRendering::OptimizeQuality);
    assert_default(SpreadMethod::Pad);
    assert_default(MaskType::Luminance);
    assert_default(LineCap::Butt);
    assert_default(LineJoin::Miter);
    assert_default(FillRule::NonZero);
    assert_default(PaintOrder::FillAndStroke);
}
