use std::fmt::Debug;
use std::str::FromStr;

use super::{ImageRendering, NonEmptyString, NonZeroF32, ShapeRendering, TextRendering, Units};

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
