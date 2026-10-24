use resvg::usvg;
use skia_safe::{AlphaType, ColorType, FontMgr, ImageInfo, Rect, surfaces, svg::Dom};
use tiny_skia::Pixmap;

use super::{RasterizedSvg, SvgRasterizeError, render::RasterTarget};

pub(super) fn render(
    svg: &str,
    size: usvg::Size,
    scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    render_inner(svg, size, scale, false)
}

pub(super) fn render_clipped(
    svg: &str,
    size: usvg::Size,
    scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    render_inner(svg, size, scale, true)
}

fn render_inner(
    svg: &str,
    size: usvg::Size,
    scale: f32,
    clip_viewport: bool,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    render_inner_with_font_mgr(svg, size, scale, clip_viewport, FontMgr::default())
}

pub(super) fn render_inner_with_font_mgr(
    svg: &str,
    size: usvg::Size,
    scale: f32,
    clip_viewport: bool,
    font_mgr: FontMgr,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    let target = RasterTarget::new(size, scale);
    let mut pixmap = allocate_pixmap(&target)?;
    let mut dom = load_dom_with_font_mgr(svg, font_mgr)?;
    render_dom(&mut dom, &target, &mut pixmap, clip_viewport)?;
    Ok(RasterizedSvg {
        width: target.width,
        height: target.height,
        display_width: target.display_width,
        display_height: target.display_height,
        rgba: pixmap.take(),
    })
}

fn allocate_pixmap(target: &RasterTarget) -> Result<Pixmap, SvgRasterizeError> {
    Pixmap::new(target.width, target.height).ok_or_else(|| {
        SvgRasterizeError::RasterizeFailed("failed to allocate Skia SVG pixmap".to_string())
    })
}

fn load_dom_with_font_mgr(svg: &str, font_mgr: FontMgr) -> Result<Dom, SvgRasterizeError> {
    Dom::from_str(svg, font_mgr)
        .map_err(|error| SvgRasterizeError::ParseFailed(format!("Skia SVG load failed: {error}")))
}

fn render_dom(
    dom: &mut Dom,
    target: &RasterTarget,
    pixmap: &mut Pixmap,
    clip_viewport: bool,
) -> Result<(), SvgRasterizeError> {
    let info = ImageInfo::new(
        (target.width as i32, target.height as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut surface =
        surfaces::wrap_pixels(&info, pixmap.data_mut(), Some(info.min_row_bytes()), None)
            .ok_or_else(|| {
                SvgRasterizeError::RasterizeFailed("failed to wrap Skia SVG pixmap".to_string())
            })?;
    dom.set_container_size((target.display_width, target.display_height));
    let canvas = surface.canvas();
    canvas.scale((target.effective_scale, target.effective_scale));
    if clip_viewport {
        canvas.clip_rect(
            Rect::from_xywh(0.0, 0.0, target.display_width, target.display_height),
            None,
            true,
        );
    }
    dom.render(canvas);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn renders_non_text_rectangle_with_expected_dimensions() -> Result<(), String> {
        super::super::candidate_test_support::assert_candidate_contract(super::render)
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_skia_font_provider_candidate_tests.rs"]
mod font_provider_tests;
