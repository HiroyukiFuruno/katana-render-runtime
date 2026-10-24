use resvg::usvg;
use thorvg::{ColorSpace, EngineOption, MimeType, Paint, Picture, Shape, Thorvg};

use super::{
    render::RasterTarget,
    types::{RasterizedSvg, SvgRasterizeError},
};

const CHANNEL_COUNT: usize = 4;
const EDGE_MIDPOINT_FRACTION: f32 = 0.5;

pub(super) fn render(
    svg: &str,
    size: usvg::Size,
    requested_scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    let target = RasterTarget::new(size, requested_scale);
    let mut buffer = vec![0u32; target.width as usize * target.height as usize];
    let engine = Thorvg::init(0).map_err(thorvg_error("engine init"))?;
    let picture = create_picture(&engine, svg, size, target.effective_scale)?;
    render_pixels(&engine, picture, &mut buffer, target.width, target.height)?;
    let rgba = buffer
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    debug_assert_eq!(
        rgba.len(),
        target.width as usize * target.height as usize * CHANNEL_COUNT
    );
    Ok(RasterizedSvg {
        width: target.width,
        height: target.height,
        display_width: target.display_width,
        display_height: target.display_height,
        rgba,
    })
}

fn create_picture<'engine>(
    engine: &'engine Thorvg,
    svg: &str,
    size: usvg::Size,
    scale: f32,
) -> Result<Picture<'engine>, SvgRasterizeError> {
    let mut picture = engine
        .picture()
        .map_err(thorvg_error("picture allocation"))?;
    picture
        .load_data(svg.as_bytes(), MimeType::Svg, None)
        .map_err(thorvg_error("SVG load"))?;
    picture
        .set_size(size.width() * scale, size.height() * scale)
        .map_err(thorvg_error("picture sizing"))?;
    Ok(picture)
}

fn render_pixels<'engine>(
    engine: &'engine Thorvg,
    mut picture: Picture<'engine>,
    buffer: &mut [u32],
    width: u32,
    height: u32,
) -> Result<(), SvgRasterizeError> {
    let clip = full_target_clip(engine, width, height)?;
    /* WHY: 上辺の中点を含む5頂点で整数キャンバス全域を表し、ThorVGの4頂点矩形高速経路だけを避ける。 */
    picture
        .set_clip(clip)
        .map_err(thorvg_error("full-target clip"))?;
    let mut canvas = engine
        .sw_canvas(EngineOption::Default)
        .map_err(thorvg_error("software canvas"))?;
    /* WHY: bufferはcanvasより先に確保され、canvas破棄まで有効なまま保持される。 */
    unsafe {
        canvas
            .set_target(buffer, width, width, height, ColorSpace::ABGR8888)
            .map_err(thorvg_error("canvas target"))?;
    }
    canvas
        .add(picture)
        .and_then(|_| canvas.render())
        .map_err(thorvg_error("software render"))
}

fn full_target_clip<'engine>(
    engine: &'engine Thorvg,
    width: u32,
    height: u32,
) -> Result<Shape<'engine>, SvgRasterizeError> {
    let width = width as f32;
    let height = height as f32;
    let mut clip = engine.shape().map_err(thorvg_error("clip allocation"))?;
    clip.move_to(0.0, 0.0)
        .and_then(|_| clip.line_to(width * EDGE_MIDPOINT_FRACTION, 0.0))
        .and_then(|_| clip.line_to(width, 0.0))
        .and_then(|_| clip.line_to(width, height))
        .and_then(|_| clip.line_to(0.0, height))
        .and_then(|_| clip.close())
        .map_err(thorvg_error("clip path"))?;
    Ok(clip)
}

fn thorvg_error(context: &'static str) -> impl FnOnce(thorvg::Error) -> SvgRasterizeError {
    move |error| SvgRasterizeError::RasterizeFailed(format!("ThorVG {context} failed: {error:?}"))
}

#[cfg(test)]
#[path = "svg_rasterize_thorvg_font_candidate_tests.rs"]
mod font_candidate_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn renders_non_text_rectangle_with_expected_dimensions() -> Result<(), String> {
        super::super::candidate_test_support::assert_candidate_contract(super::render)
    }
}
