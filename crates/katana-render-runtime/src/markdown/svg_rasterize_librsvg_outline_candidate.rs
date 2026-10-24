use cairo::{Context, Rectangle, SvgSurface, SvgUnit};
use resvg::usvg::Size;
use rsvg::CairoRenderer;

use super::super::types::{RasterizedSvg, SvgRasterizeError};

pub(super) fn render(
    svg: &str,
    size: Size,
    scale: f32,
) -> Result<RasterizedSvg, SvgRasterizeError> {
    let handle = super::load_svg(svg)?;
    let outlined_svg = render_to_svg(&handle, size)?;
    super::super::skia_candidate::render_clipped(&outlined_svg, size, scale)
}

fn render_to_svg(handle: &rsvg::SvgHandle, size: Size) -> Result<String, SvgRasterizeError> {
    let mut surface = SvgSurface::for_stream(
        f64::from(size.width()),
        f64::from(size.height()),
        Vec::<u8>::new(),
    )
    .map_err(|error| surface_error("create Cairo SVG surface", error))?;
    surface.set_document_unit(SvgUnit::Px);
    let context =
        Context::new(&surface).map_err(|error| surface_error("create Cairo context", error))?;
    CairoRenderer::new(handle)
        .render_document(
            &context,
            &Rectangle::new(0.0, 0.0, f64::from(size.width()), f64::from(size.height())),
        )
        .map_err(|error| surface_error("render with librsvg", error))?;
    drop(context);
    surface
        .finish_output_stream()
        .map_err(|error| surface_error("finish Cairo SVG stream", error))?
        .downcast::<Vec<u8>>()
        .map(|bytes| String::from_utf8(*bytes))
        .map_err(|_| surface_error("extract Cairo SVG stream", "unexpected stream type"))?
        .map_err(|error| surface_error("decode Cairo SVG stream", error))
}

fn surface_error<T: std::fmt::Debug>(stage: &str, error: T) -> SvgRasterizeError {
    SvgRasterizeError::RasterizeFailed(format!("{stage} failed: {error:?}"))
}

#[cfg(test)]
mod tests {
    use resvg::usvg::{Options, Tree};

    const CHANNEL_COUNT: usize = 4;
    const SCALE: f32 = 1.25;
    const RECTANGLE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="3"><rect width="4" height="3" fill="red"/></svg>"#;

    #[test]
    fn preserves_dimensions_and_premultiplied_pixels() -> Result<(), String> {
        super::super::super::candidate_test_support::assert_candidate_contract(super::render)
    }

    #[test]
    fn records_intermediate_svg_and_fractional_edge_rgba() -> Result<(), String> {
        let contract_result =
            super::super::super::candidate_test_support::assert_candidate_contract(super::render);
        let tree = Tree::from_str(RECTANGLE_SVG, &Options::default())
            .map_err(|error| error.to_string())?;
        let handle = super::super::load_svg(RECTANGLE_SVG).map_err(|error| error.to_string())?;
        let intermediate =
            super::render_to_svg(&handle, tree.size()).map_err(|error| error.to_string())?;
        let image =
            super::render(RECTANGLE_SVG, tree.size(), SCALE).map_err(|error| error.to_string())?;
        let last_pixel_start = image.rgba.len() - CHANNEL_COUNT;
        println!("outline intermediate SVG: {intermediate}");
        println!(
            "outline final dimensions={}x{} right_bottom_rgba={:?}",
            image.width,
            image.height,
            &image.rgba[last_pixel_start..]
        );
        contract_result
    }
}
