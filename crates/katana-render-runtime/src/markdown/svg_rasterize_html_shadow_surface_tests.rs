use super::{AlphaType, ColorType, ImageInfo, wrap_filter_surface};
use tiny_skia::Pixmap;

#[test]
fn color_filter_reuses_the_buffer_without_compositing_over_the_original_pixels()
-> Result<(), String> {
    let mut source = Pixmap::new(2, 1).ok_or("test pixmap allocation failed")?;
    let original = [64, 32, 16, 128, 0, 0, 0, 0];
    source.data_mut().copy_from_slice(&original);
    let allocation = source.data().as_ptr();
    let matrix = [
        1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        1.0, 0.0,
    ];
    let filter = skia_safe::color_filters::matrix_row_major(&matrix, None);
    let result = super::apply_color_filter(source, filter)?;
    assert_eq!(result.data().as_ptr(), allocation);
    assert_eq!(result.data(), &original);
    Ok(())
}

#[test]
fn filter_surface_rejects_short_buffer_and_wraps_valid_pixels() -> Result<(), String> {
    let info = ImageInfo::new((2, 2), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut short = [0_u8; 15];
    assert_eq!(
        wrap_filter_surface(&info, &mut short).err().as_deref(),
        Some("drop-shadow surface creation failed")
    );

    let mut pixels = [0_u8; 16];
    let mut surface = wrap_filter_surface(&info, &mut pixels)?;
    surface.canvas().clear(skia_safe::Color::RED);
    drop(surface);
    assert_eq!(pixels, [255, 0, 0, 255].repeat(4).as_slice());
    Ok(())
}
