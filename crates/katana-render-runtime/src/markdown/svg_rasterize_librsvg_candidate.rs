use cairo::{Context, Format, ImageSurface, Rectangle};
use gio::MemoryInputStream;
use glib::Bytes;
use resvg::usvg;
use rsvg::{CairoRenderer, Loader};

use super::{
    MAX_RASTERIZED_SVG_EDGE, effective_scale,
    types::{RasterizedSvg, SvgRasterizeError},
};

#[cfg(all(target_os = "macos", feature = "svg-skia-evaluation"))]
#[path = "svg_rasterize_librsvg_outline_candidate.rs"]
mod outline_candidate;

#[cfg(all(target_os = "macos", feature = "svg-skia-evaluation"))]
#[path = "svg_rasterize_librsvg_registered_candidate.rs"]
mod registered_candidate;

#[cfg(all(target_os = "macos", feature = "svg-skia-evaluation"))]
pub(super) fn render_registered(
    svg: &str,
    tree: &usvg::Tree,
    scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    registered_candidate::render(svg, tree, scale)
}

#[cfg(all(target_os = "macos", feature = "svg-skia-evaluation"))]
pub(super) fn render_outlined_with_skia(
    svg: &str,
    size: usvg::Size,
    scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    outline_candidate::render(svg, size, scale)
}

const CAIRO_CHANNEL_COUNT: usize = 4;
const CAIRO_BLUE_OFFSET: usize = 0;
const CAIRO_GREEN_OFFSET: usize = 1;
const CAIRO_RED_OFFSET: usize = 2;
const CAIRO_ALPHA_OFFSET: usize = 3;

pub(super) fn render(
    svg: &str,
    size: usvg::Size,
    requested_scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    let scale = effective_scale(size.width(), size.height(), requested_scale);
    let width = scaled_edge(size.width(), scale)?;
    let height = scaled_edge(size.height(), scale)?;
    let handle = load_svg(svg)?;
    let mut surface = ImageSurface::create(Format::ARgb32, width, height).map_err(|error| {
        SvgRasterizeError::RasterizeFailed(format!("Cairo surface failed: {error:?}"))
    })?;
    render_surface(&handle, &mut surface, size, scale)?;
    let rgba = copy_premultiplied_rgba(&mut surface, width, height)?;
    Ok(RasterizedSvg {
        width: u32::try_from(width).map_err(|_| dimension_error())?,
        height: u32::try_from(height).map_err(|_| dimension_error())?,
        display_width: size.width(),
        display_height: size.height(),
        rgba,
    })
}

fn load_svg(svg: &str) -> Result<rsvg::SvgHandle, SvgRasterizeError> {
    let bytes = Bytes::from_owned(svg.as_bytes().to_vec());
    let stream = MemoryInputStream::from_bytes(&bytes);
    Loader::new()
        .read_stream(&stream, None::<&gio::File>, None::<&gio::Cancellable>)
        .map_err(|error| {
            SvgRasterizeError::RasterizeFailed(format!("librsvg load failed: {error:?}"))
        })
}

fn render_surface(
    handle: &rsvg::SvgHandle,
    surface: &mut ImageSurface,
    size: usvg::Size,
    scale: f32,
) -> Result<(), SvgRasterizeError> {
    let context = Context::new(&*surface).map_err(|error| {
        SvgRasterizeError::RasterizeFailed(format!("Cairo context failed: {error:?}"))
    })?;
    context.scale(f64::from(scale), f64::from(scale));
    CairoRenderer::new(handle)
        .render_document(
            &context,
            &Rectangle::new(0.0, 0.0, f64::from(size.width()), f64::from(size.height())),
        )
        .map_err(|error| {
            SvgRasterizeError::RasterizeFailed(format!("librsvg render failed: {error:?}"))
        })?;
    drop(context);
    surface.flush();
    Ok(())
}

fn scaled_edge(edge: f32, scale: f32) -> Result<i32, SvgRasterizeError> {
    let pixels = (edge * scale).ceil();
    if !pixels.is_finite() || !(1.0..=MAX_RASTERIZED_SVG_EDGE).contains(&pixels) {
        return Err(dimension_error());
    }
    i32::try_from(pixels as u32).map_err(|_| dimension_error())
}

fn copy_premultiplied_rgba(
    surface: &mut ImageSurface,
    width: i32,
    height: i32,
) -> Result<Vec<u8>, SvgRasterizeError> {
    let stride = usize::try_from(surface.stride()).map_err(|_| dimension_error())?;
    let data = surface.data().map_err(|error| {
        SvgRasterizeError::RasterizeFailed(format!("Cairo data failed: {error:?}"))
    })?;
    let width = usize::try_from(width).map_err(|_| dimension_error())?;
    let height = usize::try_from(height).map_err(|_| dimension_error())?;
    let mut rgba = Vec::with_capacity(
        width
            .saturating_mul(height)
            .saturating_mul(CAIRO_CHANNEL_COUNT),
    );
    for row in data.as_ref().chunks(stride).take(height) {
        for pixel in row.chunks_exact(CAIRO_CHANNEL_COUNT).take(width) {
            let alpha = pixel[CAIRO_ALPHA_OFFSET];
            rgba.extend_from_slice(&[
                pixel[CAIRO_RED_OFFSET],
                pixel[CAIRO_GREEN_OFFSET],
                pixel[CAIRO_BLUE_OFFSET],
                alpha,
            ]);
        }
    }
    Ok(rgba)
}

fn dimension_error() -> SvgRasterizeError {
    SvgRasterizeError::RasterizeFailed("invalid librsvg candidate dimensions".to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn renders_non_text_rectangle_with_expected_dimensions() -> Result<(), String> {
        super::super::candidate_test_support::assert_candidate_contract(super::render)
    }
}
