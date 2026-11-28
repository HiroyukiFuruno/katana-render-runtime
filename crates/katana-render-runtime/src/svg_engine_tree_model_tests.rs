use std::fmt::{Debug, Display};
use std::str::FromStr;

use super::super::{Options, Tree};
use super::{
    BlendMode, FillRule, Group, ImageRendering, LineCap, LineJoin, MaskType, Node, NonEmptyString,
    NonZeroF32, Paint, PaintOrder, Rect, ShapeRendering, SpreadMethod, StrokeMiterlimit, Text,
    TextRendering, Transform, Units, Visibility, WritingMode,
};
use std::sync::Arc;

const NODE_TREE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g id="container" transform="translate(5 7)"><rect id="shape" x="0" y="0" width="2" height="3" transform="translate(2 3)"/><image id="raster" x="0" y="0" width="1" height="1" transform="translate(2 3)" href="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAf8AOw=="/></g></svg>"#;
const PAINT_TREE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><defs><linearGradient id="gradient"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient><pattern id="pattern" width="2" height="2"><rect width="2" height="2"/></pattern></defs><rect id="gradient-shape" width="5" height="5" fill="url(#gradient)"/><rect id="pattern-shape" x="6" width="5" height="5" fill="url(#pattern)"/></svg>"##;
const EXPECTED_MITER_LIMIT: f32 = 6.5;
const GROUP_TRANSLATION: (f32, f32) = (5.0, 7.0);
const NODE_TRANSLATION: (f32, f32) = (7.0, 10.0);
const TEXT_TRANSLATION: (f32, f32) = (11.0, 13.0);
const MODEL_ORIGIN: (f32, f32) = (0.0, 0.0);
const EMPTY_TEXT_BOUNDS: (f32, f32, f32, f32) = (0.0, 0.0, 0.0, 0.0);

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

#[test]
fn stroke_miterlimit_from_value_preserves_a_valid_limit() {
    assert_eq!(
        StrokeMiterlimit::from(EXPECTED_MITER_LIMIT).get(),
        EXPECTED_MITER_LIMIT
    );
}

#[test]
fn tree_node_lookup_and_absolute_transforms_follow_svg_ancestors() -> Result<(), String> {
    let tree = parse_model_svg(NODE_TREE_SVG)?;
    assert!(tree.node_by_id("").is_none());

    let group = tree.node_by_id("container").ok_or("group node missing")?;
    assert_node_identity_and_transform(group, "container", GROUP_TRANSLATION);
    let Node::Group(group) = group else {
        return Err("group ID resolved to another node".into());
    };
    assert_transform_origin(group.abs_transform(), GROUP_TRANSLATION);

    let path = tree.node_by_id("shape").ok_or("path node missing")?;
    assert_node_identity_and_transform(path, "shape", NODE_TRANSLATION);

    assert_svg_image_group(&tree)?;
    let node = text_node(
        "label",
        Transform::from_translate(TEXT_TRANSLATION.0, TEXT_TRANSLATION.1),
    )?;
    assert_node_identity_and_transform(&node, "label", TEXT_TRANSLATION);
    Ok(())
}

#[test]
fn paint_equality_uses_shared_gradient_and_pattern_identity() -> Result<(), String> {
    let tree = parse_model_svg(PAINT_TREE_SVG)?;
    let gradient_paint = first_gradient_paint(&tree, "linear gradient missing")?;
    let pattern_paint = first_pattern_paint(&tree, "pattern missing")?;
    let cloned_tree = tree.clone();
    let cloned_gradient = first_gradient_paint(&cloned_tree, "cloned gradient missing")?;
    let cloned_pattern = first_pattern_paint(&cloned_tree, "cloned pattern missing")?;
    let reparsed = parse_model_svg(PAINT_TREE_SVG)?;
    let distinct_gradient = first_gradient_paint(&reparsed, "reparsed gradient missing")?;

    assert_eq!(gradient_paint, cloned_gradient);
    assert_eq!(pattern_paint, cloned_pattern);
    assert_ne!(gradient_paint, distinct_gradient);
    assert_ne!(gradient_paint, pattern_paint);
    Ok(())
}

fn first_gradient_paint(tree: &Tree, missing: &'static str) -> Result<Paint, String> {
    tree.linear_gradients()
        .first()
        .map(|gradient| Paint::LinearGradient(Arc::clone(gradient)))
        .ok_or_else(|| missing.into())
}

fn first_pattern_paint(tree: &Tree, missing: &'static str) -> Result<Paint, String> {
    tree.patterns()
        .first()
        .map(|pattern| Paint::Pattern(Arc::clone(pattern)))
        .ok_or_else(|| missing.into())
}

fn parse_model_svg(source: &str) -> Result<Tree, String> {
    Tree::from_str(source, &Options::default()).map_err(|error| error.to_string())
}

fn assert_transform_origin(transform: Transform, expected: (f32, f32)) {
    let mut origin = super::super::tiny_skia_path::Point::from_xy(MODEL_ORIGIN.0, MODEL_ORIGIN.1);
    transform.map_point(&mut origin);
    assert_eq!(origin.x, expected.0);
    assert_eq!(origin.y, expected.1);
}

fn assert_node_identity_and_transform(
    node: &Node,
    expected_id: &str,
    expected_transform: (f32, f32),
) {
    assert_eq!(node.id(), expected_id);
    assert_transform_origin(node.abs_transform(), expected_transform);
}

fn assert_svg_image_group(tree: &Tree) -> Result<(), String> {
    let image_node = tree.node_by_id("raster").ok_or("image node missing")?;
    assert_node_identity_and_transform(image_node, "raster", NODE_TRANSLATION);
    let Node::Group(image_group) = image_node else {
        return Err("image ID resolved to another node".into());
    };
    assert_transform_origin(image_group.abs_transform(), NODE_TRANSLATION);
    let image_node = image_group.children.first().ok_or("image child missing")?;
    let Node::Image(image) = image_node else {
        return Err("image group child is not an image".into());
    };
    assert_eq!(image.id(), "");
    assert_transform_origin(image.abs_transform(), NODE_TRANSLATION);
    Ok(())
}

fn text_node(id: &str, abs_transform: Transform) -> Result<Node, String> {
    let empty_rect = Rect::from_xywh(
        EMPTY_TEXT_BOUNDS.0,
        EMPTY_TEXT_BOUNDS.1,
        EMPTY_TEXT_BOUNDS.2,
        EMPTY_TEXT_BOUNDS.3,
    )
    .ok_or("empty text bounds invalid")?;
    let text = Text {
        id: id.to_owned(),
        rendering_mode: TextRendering::OptimizeLegibility,
        dx: Vec::new(),
        dy: Vec::new(),
        rotate: Vec::new(),
        writing_mode: WritingMode::LeftToRight,
        chunks: Vec::new(),
        abs_transform,
        bounding_box: empty_rect,
        abs_bounding_box: empty_rect,
        stroke_bounding_box: empty_rect,
        abs_stroke_bounding_box: empty_rect,
        flattened: Box::new(Group::empty()),
        layouted: Vec::new(),
    };
    Ok(Node::Text(Box::new(text)))
}

#[path = "svg_engine_tree_filter_model_tests.rs"]
mod filter_model_tests;
#[path = "svg_engine_tree_image_model_tests.rs"]
mod image_model_tests;

#[test]
fn f64_valid_length_requires_positive_finite_values() {
    use super::IsValidLength;

    for (value, expected) in [
        (f64::NEG_INFINITY, false),
        (-1.0, false),
        (0.0, false),
        (f64::NAN, false),
        (f64::INFINITY, false),
        (f64::MIN_POSITIVE, true),
        (1.0, true),
    ] {
        assert_eq!(value.is_valid_length(), expected, "value={value}");
    }
}
