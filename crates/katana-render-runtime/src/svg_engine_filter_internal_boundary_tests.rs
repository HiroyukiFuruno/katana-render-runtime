use super::{Error, PixmapExt, apply, apply_primitives, apply_turbulence, get_input};
use crate::svg_engine::usvg::{self, filter};

const CANVAS_SIZE: u32 = 4;
const SOURCE_RGBA: [u8; 4] = [11, 29, 47, 255];
const FILTER_FREQUENCY: f32 = 0.1;

fn region() -> Result<tiny_skia::IntRect, String> {
    tiny_skia::IntRect::from_xywh(0, 0, CANVAS_SIZE, CANVAS_SIZE).ok_or("invalid region".to_owned())
}

fn source() -> Result<tiny_skia::Pixmap, String> {
    let mut pixmap =
        tiny_skia::Pixmap::new(CANVAS_SIZE, CANVAS_SIZE).ok_or("invalid pixmap".to_owned())?;
    let [red, green, blue, alpha] = SOURCE_RGBA;
    pixmap.fill(tiny_skia::Color::from_rgba8(red, green, blue, alpha));
    Ok(pixmap)
}

fn empty_filter() -> Result<filter::Filter, String> {
    Ok(filter::Filter {
        id: usvg::NonEmptyString::new("empty".to_owned()).ok_or("invalid id".to_owned())?,
        rect: usvg::NonZeroRect::from_xywh(0.0, 0.0, CANVAS_SIZE as f32, CANVAS_SIZE as f32)
            .ok_or("invalid filter rectangle".to_owned())?,
        primitives: Vec::new(),
    })
}

fn turbulence() -> Result<filter::Turbulence, String> {
    let frequency =
        strict_num::PositiveF32::new(FILTER_FREQUENCY).ok_or("invalid frequency".to_owned())?;
    Ok(filter::Turbulence {
        base_frequency_x: frequency,
        base_frequency_y: frequency,
        num_octaves: 1,
        seed: 1,
        stitch_tiles: false,
        kind: filter::TurbulenceKind::FractalNoise,
    })
}

#[test]
fn empty_filter_has_no_results_and_clears_source() -> Result<(), String> {
    let filter = empty_filter()?;
    let mut image = source()?;
    let transform = usvg::Transform::identity();
    assert!(matches!(
        apply_primitives(&filter, transform, &image, region()?),
        Err(Error::NoResults)
    ));
    apply(&filter, transform, &mut image);
    assert!(image.data().iter().all(|byte| *byte == 0));
    Ok(())
}

#[test]
fn missing_reference_clones_source_graphic() -> Result<(), String> {
    let mut source = source()?;
    let original = source.data().to_vec();
    let region = region()?;
    let image = get_input(
        &filter::Input::Reference("missing".to_owned()),
        region,
        &source,
        &[],
    )
    .map_err(|error| format!("{error:?}"))?;
    source.fill(tiny_skia::Color::TRANSPARENT);
    assert_eq!(image.image.data(), original.as_slice());
    assert_eq!(image.region, region);
    assert_eq!(image.color_space, filter::ColorInterpolation::SRGB);
    Ok(())
}

#[test]
fn turbulence_with_zero_or_tiny_scale_is_transparent() -> Result<(), String> {
    let tiny = f32::from_bits(1);
    for (x, y) in [(0.0, 1.0), (1.0, 0.0), (tiny, 1.0), (1.0, tiny)] {
        let image = apply_turbulence(
            &turbulence()?,
            region()?,
            filter::ColorInterpolation::LinearRGB,
            usvg::Transform::from_scale(x, y),
        )
        .map_err(|error| format!("{error:?}"))?;
        assert_eq!(
            (image.image.width(), image.image.height()),
            (CANVAS_SIZE, CANVAS_SIZE)
        );
        assert_eq!(image.color_space, filter::ColorInterpolation::LinearRGB);
        assert!(image.image.data().iter().all(|byte| *byte == 0));
    }
    Ok(())
}

#[test]
fn clear_makes_all_pixels_transparent() -> Result<(), String> {
    let mut image = source()?;
    PixmapExt::clear(&mut image);
    assert!(image.data().iter().all(|byte| *byte == 0));
    assert_eq!((image.width(), image.height()), (CANVAS_SIZE, CANVAS_SIZE));
    Ok(())
}
