use super::Transform;

use super::HtmlShadowRasterizer;
use super::bounds::map_bounds;
use super::geometry::{Bounds, bounded_crop};
use tiny_skia::{Color, Paint as TinyPaint, Pixmap, PixmapMut, Rect as TinyRect};

const CANVAS_SIZE: u32 = 200;
const COLOR_CHANNELS: usize = 4;
const CROP_TARGET_SIZE: u32 = 8;
const UNSUPPORTED_CANVAS_SIZE: u32 = 20;
const CROP_LEFT_OUTSIDE: f32 = -1.0;
const CROP_EDGE: f32 = 4.0;
const CROP_FILL: [u8; 4] = [7, 11, 13, 255];
const SOURCE_X: f32 = 20.0;
const SOURCE_SIZE: f32 = 10.0;
const SOURCE_SAMPLE_X: u32 = 45;
const SHADOW_SAMPLE_X: u32 = 64;
const OUTSIDE_SAMPLE: u32 = 100;
const UNIT_SCALE: f32 = 1.0;
const SKEW_SCALE: f32 = 0.2;
const ZERO_TRANSLATION: f32 = 0.0;
const SOURCE_BLUE: [u8; 4] = [0, 0, 255, 255];
const SHADOW_RED: [u8; 4] = [255, 0, 0, 255];
const BACKGROUND_GRAY: [u8; 4] = [128, 128, 128, 255];

#[test]
fn shadow_bounds_reject_unsupported_and_overflowing_transforms() {
    let bounds = Bounds {
        left: SOURCE_X,
        top: SOURCE_X,
        right: SOURCE_X + SOURCE_SIZE,
        bottom: SOURCE_X + SOURCE_SIZE,
    };
    assert!(map_bounds(bounds, Transform::identity()).is_ok());
    let cases = [
        (
            Transform::from_rotate(30.0),
            "drop-shadow transform is not positive scale and translation",
        ),
        (
            Transform::from_scale(f32::MAX, UNIT_SCALE),
            "drop-shadow transformed bounds are non-finite",
        ),
    ];
    for (transform, expected) in cases {
        assert!(map_bounds(bounds, transform).is_err_and(|error| error == expected));
    }
}

#[test]
fn bounded_crop_rejects_outside_without_mutating_target() -> Result<(), String> {
    let mut target = Pixmap::new(CROP_TARGET_SIZE, CROP_TARGET_SIZE)
        .ok_or_else(|| "test target allocation".to_owned())?;
    target.fill(color(CROP_FILL));
    let before = target.data().to_vec();
    let crop = bounded_crop(
        Bounds {
            left: CROP_LEFT_OUTSIDE,
            top: ZERO_TRANSLATION,
            right: CROP_EDGE,
            bottom: CROP_EDGE,
        },
        &target.as_mut(),
    )
    .map_err(|_| "crop check failed".to_owned())?;
    assert!(crop.is_none());
    assert_eq!(target.data(), before.as_slice());
    Ok(())
}

#[test]
fn parsed_srgb_shadow_renders_source_and_shadow_at_scale() -> Result<(), String> {
    let (target, calls) = render_srgb_fixture()?;
    assert_eq!(calls, 1);
    assert_eq!(
        pixel(&target, SOURCE_SAMPLE_X, SOURCE_SAMPLE_X)?,
        SOURCE_BLUE
    );
    assert_eq!(
        pixel(&target, SHADOW_SAMPLE_X, SOURCE_SAMPLE_X)?,
        SHADOW_RED
    );
    assert_eq!(
        pixel(&target, OUTSIDE_SAMPLE, OUTSIDE_SAMPLE)?,
        BACKGROUND_GRAY
    );
    Ok(())
}

fn pixel(target: &Pixmap, x: u32, y: u32) -> Result<[u8; COLOR_CHANNELS], String> {
    let offset = (y as usize * target.width() as usize + x as usize) * COLOR_CHANNELS;
    target
        .data()
        .get(offset..offset + COLOR_CHANNELS)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| "pixel outside test canvas".to_owned())
}

fn color(bytes: [u8; COLOR_CHANNELS]) -> Color {
    let [red, green, blue, alpha] = bytes;
    Color::from_rgba8(red, green, blue, alpha)
}

fn render_srgb_fixture() -> Result<(Pixmap, u32), String> {
    let tree = resvg::usvg::Tree::from_str(SRGB_SVG, &resvg::usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let group = find_group(&tree)?;
    let mut target =
        Pixmap::new(CANVAS_SIZE, CANVAS_SIZE).ok_or_else(|| "target allocation".to_owned())?;
    target.fill(color(BACKGROUND_GRAY));
    let mut calls = 0_u32;
    let mut renderer = |transform: Transform, pixels: &mut PixmapMut<'_>| {
        calls += 1;
        paint_blue_source(transform, pixels)
    };
    let handled = HtmlShadowRasterizer::render(
        group,
        Transform::from_scale(2.0, 2.0),
        &mut target.as_mut(),
        &mut renderer,
    )?;
    assert!(handled);
    Ok((target, calls))
}

const SRGB_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <defs><filter id="s" x="0" y="0" width="100" height="100" color-interpolation-filters="sRGB">
    <feDropShadow dx="4" dy="0" stdDeviation="0" flood-color="red" flood-opacity="1" x="0" y="0" width="100" height="100"/>
  </filter></defs><g filter="url(#s)"><rect x="20" y="20" width="10" height="10" fill="blue"/></g>
</svg>"#;

fn find_group(tree: &resvg::usvg::Tree) -> Result<&resvg::usvg::Group, String> {
    tree.root()
        .children()
        .iter()
        .find_map(|node| match node {
            resvg::usvg::Node::Group(group) => Some(group.as_ref()),
            _ => None,
        })
        .ok_or_else(|| "shadow group missing".to_owned())
}

fn paint_blue_source(transform: Transform, pixels: &mut PixmapMut<'_>) -> Result<(), String> {
    let mut paint = TinyPaint::default();
    paint.set_color(color(SOURCE_BLUE));
    let rect = TinyRect::from_xywh(SOURCE_X, SOURCE_X, SOURCE_SIZE, SOURCE_SIZE)
        .ok_or_else(|| "source rect".to_owned())?;
    pixels.fill_rect(rect, &paint, transform, None);
    Ok(())
}

#[test]
fn unsupported_shadow_forms_are_noops() -> Result<(), String> {
    let linear = SRGB_SVG.replace(
        "color-interpolation-filters=\"sRGB\"",
        "color-interpolation-filters=\"linearRGB\"",
    );
    let multi = SRGB_SVG.replace(
        "</filter>",
        "<feDropShadow dx=\"0\" dy=\"1\" stdDeviation=\"0\"/></filter>",
    );
    let clipped = SRGB_SVG
        .replace(
            "<defs>",
            "<defs><clipPath id=\"c\"><rect width=\"20\" height=\"20\"/></clipPath>",
        )
        .replace("<g filter=", "<g clip-path=\"url(#c)\" filter=");
    assert_unsupported(&linear, Transform::identity())?;
    assert_unsupported(&multi, Transform::identity())?;
    assert_unsupported(&clipped, Transform::identity())?;
    assert_unsupported(
        SRGB_SVG,
        Transform::from_row(
            UNIT_SCALE,
            SKEW_SCALE,
            ZERO_TRANSLATION,
            UNIT_SCALE,
            ZERO_TRANSLATION,
            ZERO_TRANSLATION,
        ),
    )
}

#[test]
fn empty_and_unsupported_shadow_regions_are_noops() -> Result<(), String> {
    let no_filter = r#"<svg xmlns="http://www.w3.org/2000/svg"><g id="without-filter"><rect width="10" height="10"/></g></svg>"#;
    let source_alpha = SRGB_SVG.replace(
        r#"<feDropShadow dx="#,
        r#"<feDropShadow in="SourceAlpha" dx="#,
    );
    let empty_source = SRGB_SVG
        .replace(
            r#"<filter id="s" x="#,
            r#"<filter id="s" filterUnits="userSpaceOnUse" x="#,
        )
        .replace(
            r#"<rect x="20" y="20" width="10" height="10" fill="blue"/>"#,
            r#"<rect x="20" y="20" width="0" height="0" fill="blue"/>"#,
        );
    let tight_region = SRGB_SVG.replace(r#"width="100" height="100""#, r#"width="25" height="25""#);
    for svg in [
        no_filter,
        source_alpha.as_str(),
        empty_source.as_str(),
        tight_region.as_str(),
    ] {
        assert_unsupported(svg, Transform::identity())?;
    }
    Ok(())
}

#[test]
fn supported_shadow_outside_the_canvas_skips_source_paint() -> Result<(), String> {
    assert_unsupported(SRGB_SVG, Transform::from_scale(2.0, 2.0))
}

fn assert_unsupported(svg: &str, transform: Transform) -> Result<(), String> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let group = find_group(&tree)?;
    let mut target = Pixmap::new(UNSUPPORTED_CANVAS_SIZE, UNSUPPORTED_CANVAS_SIZE)
        .ok_or_else(|| "target allocation".to_owned())?;
    target.fill(color(BACKGROUND_GRAY));
    let before = target.data().to_vec();
    let mut calls = 0_u32;
    let mut renderer = |_transform: Transform, _pixels: &mut PixmapMut<'_>| {
        calls += 1;
        Ok(())
    };
    let handled =
        HtmlShadowRasterizer::render(group, transform, &mut target.as_mut(), &mut renderer)?;
    assert!(!handled);
    assert_eq!(calls, 0);
    assert_eq!(target.data(), before.as_slice());
    Ok(())
}
