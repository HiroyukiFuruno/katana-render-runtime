use super::{AlphaType, ColorType, ImageInfo, wrap_filter_surface};

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
