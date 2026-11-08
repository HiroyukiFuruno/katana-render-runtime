use super::{
    Error, PixmapExt, apply, apply_morphology, apply_offset, apply_primitives, apply_turbulence,
    get_input,
};
use crate::svg_engine::usvg::{self, filter};

const CANVAS_SIZE: u32 = 4;
const RGBA_CHANNELS: usize = 4;
const SOURCE_RGBA: [u8; RGBA_CHANNELS] = [11, 29, 47, 255];
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

fn source_image() -> Result<super::Image, String> {
    get_input(&filter::Input::SourceGraphic, region()?, &source()?, &[])
        .map_err(|error| format!("{error:?}"))
}

fn assert_offset_pixels(image: &tiny_skia::Pixmap, shift_x: i32, shift_y: i32) {
    for y in 0..CANVAS_SIZE as i32 {
        for x in 0..CANVAS_SIZE as i32 {
            let source_x = x - shift_x;
            let source_y = y - shift_y;
            let inside = (0..CANVAS_SIZE as i32).contains(&source_x)
                && (0..CANVAS_SIZE as i32).contains(&source_y);
            let expected = if inside {
                SOURCE_RGBA
            } else {
                [0; RGBA_CHANNELS]
            };
            let index = (y * CANVAS_SIZE as i32 + x) as usize * RGBA_CHANNELS;
            assert_eq!(
                &image.data()[index..index + RGBA_CHANNELS],
                &expected,
                "pixel ({x}, {y})"
            );
        }
    }
}

#[test]
fn offset_moves_and_clips_pixels_at_finite_scaled_coordinates() -> Result<(), String> {
    for (dx, dy, scale_x, shift_x, shift_y) in [
        (0.0, 0.0, 1.0, 0, 0),
        (1.0, 0.0, 1.0, 1, 0),
        (-1.0, 0.0, 1.0, -1, 0),
        (0.0, 1.0, 1.0, 0, 1),
        (0.0, -1.0, 1.0, 0, -1),
        (1.0, 0.0, 2.0, 2, 0),
        (-1.0, 0.0, 2.0, -2, 0),
    ] {
        let offset = filter::Offset {
            input: filter::Input::SourceGraphic,
            dx,
            dy,
        };
        let image = apply_offset(
            &offset,
            usvg::Transform::from_scale(scale_x, 1.0),
            source_image()?,
        )
        .map_err(|error| format!("{error:?}"))?;
        assert_offset_pixels(&image.image, shift_x, shift_y);
    }
    Ok(())
}

#[test]
fn morphology_rejects_radius_overflow_after_scaling() -> Result<(), String> {
    let max = strict_num::PositiveF32::new(f32::MAX).ok_or("invalid maximum radius".to_owned())?;
    let one = strict_num::PositiveF32::new(1.0).ok_or("invalid radius".to_owned())?;
    for (radius_x, radius_y, scale_x, scale_y) in [(max, one, 2.0, 1.0), (one, max, 1.0, 2.0)] {
        let morphology = filter::Morphology {
            input: filter::Input::SourceGraphic,
            operator: filter::MorphologyOperator::Dilate,
            radius_x,
            radius_y,
        };
        assert!(matches!(
            apply_morphology(
                &morphology,
                filter::ColorInterpolation::SRGB,
                usvg::Transform::from_scale(scale_x, scale_y),
                source_image()?,
            ),
            Err(Error::InvalidRegion)
        ));
    }
    Ok(())
}
