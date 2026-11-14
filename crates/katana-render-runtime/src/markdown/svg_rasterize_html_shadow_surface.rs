use super::geometry::{Bounds, Crop, ShadowSpec};
use skia_safe::{self, AlphaType, ColorType, ImageInfo, Paint, Rect as SkRect};
use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, Transform};

#[cfg(test)]
#[path = "svg_rasterize_html_shadow_surface_tests.rs"]
mod tests;

pub(super) fn apply_color_filter(
    source: Pixmap,
    color_filter: skia_safe::ColorFilter,
) -> Result<Pixmap, String> {
    apply_filter_paint(source, None, Some(color_filter), "color-filter")
}

pub(super) fn apply_skia_shadow(
    source: Pixmap,
    spec: ShadowSpec,
    transform: Transform,
    crop: Crop,
) -> Result<Pixmap, String> {
    let dimensions = (crop.width() as i32, crop.height() as i32);
    let filter = make_skia_filter(spec, transform, Some(dimensions))?;
    apply_filter_paint(source, Some(filter), None, "drop-shadow")
}

fn apply_filter_paint(
    mut source: Pixmap,
    image_filter: Option<skia_safe::ImageFilter>,
    color_filter: Option<skia_safe::ColorFilter>,
    label: &str,
) -> Result<Pixmap, String> {
    let dimensions = (source.width() as i32, source.height() as i32);
    let info = ImageInfo::new(dimensions, ColorType::RGBA8888, AlphaType::Premul, None);
    let image = filter_source_image(&info, source.data(), label)?;
    /* WHY: Skia画像は元画素のコピーを保持する。出力には元の領域を透明に戻して使う。 */
    source.data_mut().fill(0);
    let mut surface = wrap_filter_surface(&info, source.data_mut())?;
    let mut paint = Paint::default();
    if let Some(filter) = image_filter {
        paint.set_image_filter(filter);
    }
    if let Some(filter) = color_filter {
        paint.set_color_filter(filter);
    }
    surface.canvas().draw_image(image, (0, 0), Some(&paint));
    drop(surface);
    Ok(source)
}

fn filter_source_image(
    info: &ImageInfo,
    pixels: &[u8],
    label: &str,
) -> Result<skia_safe::Image, String> {
    let data = skia_safe::Data::new_copy(pixels);
    skia_safe::images::raster_from_data(info, data, info.min_row_bytes())
        .ok_or_else(|| format!("{label} source image creation failed"))
}

pub(super) fn make_skia_filter(
    spec: ShadowSpec,
    transform: Transform,
    dimensions: Option<(i32, i32)>,
) -> Result<skia_safe::ImageFilter, String> {
    let offset = (spec.dx * transform.sx, spec.dy * transform.sy);
    let sigma = (spec.sigma_x * transform.sx, spec.sigma_y * transform.sy);
    /* WHY: Skiaの合成フィルターは無効な子の生成に失敗しても、残りのグラフを返し得る。 */
    if [offset.0, offset.1, sigma.0, sigma.1]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err("drop-shadow scaled parameters are non-finite".to_owned());
    }
    let crop = dimensions
        .map(|(width, height)| SkRect::from_xywh(0.0, 0.0, width as f32, height as f32))
        .map(skia_safe::image_filters::CropRect::from);
    skia_safe::image_filters::drop_shadow(
        offset,
        sigma,
        spec.color,
        skia_safe::ColorSpace::new_srgb(),
        None,
        crop,
    )
    .ok_or_else(|| "drop-shadow filter creation failed".to_owned())
}

pub(super) fn skia_bounds(rect: SkRect) -> Result<Bounds, String> {
    let bounds = Bounds {
        left: rect.left(),
        top: rect.top(),
        right: rect.right(),
        bottom: rect.bottom(),
    };
    if bounds.left < bounds.right
        && bounds.top < bounds.bottom
        && [bounds.left, bounds.top, bounds.right, bounds.bottom]
            .iter()
            .all(|value| value.is_finite())
    {
        Ok(bounds)
    } else {
        Err("drop-shadow filter bounds are invalid".to_owned())
    }
}

fn wrap_filter_surface<'a>(
    info: &ImageInfo,
    pixels: &'a mut [u8],
) -> Result<skia_safe::Borrows<'a, skia_safe::Surface>, String> {
    super::super::render::wrap_skia_surface(info, pixels, "drop-shadow surface creation failed")
}

pub(super) fn copy_source_over(target: &mut PixmapMut<'_>, source: &Pixmap, left: i32, top: i32) {
    target.draw_pixmap(
        left,
        top,
        source.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}
