/* WHY: SVG rasterization utility.
Uses `resvg` + `usvg` to convert SVG text to an RGBA pixel buffer.
Returns the result as raw bytes compatible with egui's `ColorImage`. */

use resvg::{render, usvg};
use tiny_skia::Pixmap;

#[path = "svg_rasterize_font.rs"]
mod font;
#[path = "svg_rasterize_preprocess.rs"]
mod preprocess;
#[path = "svg_rasterize_text.rs"]
mod text;
#[path = "svg_rasterize_text_shaping.rs"]
mod text_shaping;
#[path = "svg_rasterize_types.rs"]
mod types;

use font::{html_rasterizer_options, rasterizer_options};
#[cfg(test)]
use preprocess::parse_light_dark_function;
use preprocess::{effective_scale, preprocess_for_rasterizer};
pub use types::{RasterizedSvg, SvgRasterizeError};

const MAX_RASTERIZED_SVG_EDGE: f32 = 8192.0;

pub struct SvgRasterizeOps;

impl SvgRasterizeOps {
    pub fn preprocess_for_rasterizer(svg_text: &str) -> String {
        preprocess_for_rasterizer(svg_text)
    }

    pub fn rasterize_svg(svg_text: &str, scale: f32) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::rasterize_with_options(svg_text, scale, &rasterizer_options())
    }

    pub(crate) fn rasterize_html_svg(
        svg_text: &str,
        scale: f32,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::rasterize_with_options(svg_text, scale, &html_rasterizer_options(svg_text))
    }

    pub(crate) fn measure_html_text(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
        letter_spacing: f32,
        font_feature_settings: Option<&str>,
    ) -> Result<f32, SvgRasterizeError> {
        text::measure_html_text(
            text,
            font_family,
            font_size,
            font_weight,
            italic,
            letter_spacing,
            font_feature_settings,
        )
    }

    pub(crate) fn shape_html_text_dx(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
        font_feature_settings: Option<&str>,
    ) -> Option<Vec<f32>> {
        text_shaping::shaped_html_text_dx(
            text,
            font_family,
            font_size,
            font_weight,
            italic,
            font_feature_settings,
        )
    }

    fn rasterize_with_options(
        svg_text: &str,
        scale: f32,
        options: &usvg::Options<'_>,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let compatible_svg = Self::preprocess_for_rasterizer(svg_text);
        let tree = usvg::Tree::from_str(&compatible_svg, options)
            .map_err(|e| SvgRasterizeError::ParseFailed(e.to_string()))?;
        let raster = RasterTarget::new(tree.size(), scale);
        let pixmap = raster.render(&tree)?;
        Ok(RasterizedSvg {
            width: raster.width,
            height: raster.height,
            display_width: raster.display_width,
            display_height: raster.display_height,
            rgba: pixmap.take(),
        })
    }
}

struct RasterTarget {
    display_width: f32,
    display_height: f32,
    effective_scale: f32,
    width: u32,
    height: u32,
}

impl RasterTarget {
    fn new(size: usvg::Size, scale: f32) -> Self {
        let display_width = size.width();
        let display_height = size.height();
        let effective_scale = effective_scale(display_width, display_height, scale);
        Self {
            display_width,
            display_height,
            effective_scale,
            width: ((display_width * effective_scale).ceil() as u32).max(1),
            height: ((display_height * effective_scale).ceil() as u32).max(1),
        }
    }

    fn render(&self, tree: &usvg::Tree) -> Result<Pixmap, SvgRasterizeError> {
        let Some(mut pixmap) = Pixmap::new(self.width, self.height) else {
            return Err(SvgRasterizeError::RasterizeFailed(
                "failed to allocate SVG pixmap".to_string(),
            ));
        };
        let transform =
            tiny_skia::Transform::from_scale(self.effective_scale, self.effective_scale);
        render(tree, transform, &mut pixmap.as_mut());
        Ok(pixmap)
    }
}

#[cfg(test)]
#[path = "svg_rasterize_tests.rs"]
mod tests;
