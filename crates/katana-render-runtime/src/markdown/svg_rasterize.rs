/* WHY: SVG rasterization utility.
Uses `resvg` + `usvg` to convert SVG text to an RGBA pixel buffer.
Returns the result as raw bytes compatible with egui's `ColorImage`. */

use resvg::usvg;
use tiny_skia::Pixmap;

#[path = "svg_rasterize_composite.rs"]
mod composite;
#[path = "svg_rasterize_font.rs"]
mod font;
#[cfg(test)]
#[path = "svg_rasterize_font_file_fixture.rs"]
pub(super) mod font_file_fixture;
#[path = "svg_rasterize_text_measurement_api.rs"]
mod measurement_api;
#[path = "svg_rasterize_measurement_diagnostic.rs"]
pub(crate) mod measurement_diagnostic;
#[path = "svg_rasterize_preprocess.rs"]
mod preprocess;
#[path = "svg_rasterize_composite_provenance.rs"]
mod provenance;
#[path = "svg_rasterize_text.rs"]
mod text;
#[path = "svg_rasterize_text_shaping.rs"]
mod text_shaping;
#[path = "svg_rasterize_types.rs"]
mod types;
#[cfg(not(target_os = "macos"))]
use font::html_rasterizer_options;
use font::rasterizer_options;
#[cfg(test)]
use preprocess::parse_light_dark_function;
use preprocess::{effective_scale, preprocess_for_rasterizer};
pub(crate) use text_shaping::{
    HtmlTextMeasurementToken, HtmlTextMeasurementTokenKey, SvgTextRunMetrics,
};
pub use types::{RasterizedSvg, SvgRasterizeError};

const MAX_RASTERIZED_SVG_EDGE: f32 = 8192.0;

#[cfg(test)]
mod font_source_fixture {
    #[cfg(target_os = "macos")]
    pub(super) fn mapped_source(
        path: &std::path::Path,
    ) -> Result<resvg::usvg::fontdb::Source, String> {
        let info = fontique::SourceInfo::new(
            fontique::SourceId::new(),
            fontique::SourceKind::Path(std::sync::Arc::from(path)),
        );
        let data = fontique::SourceCache::default()
            .get(&info)
            .ok_or("library map failed")?
            .into_raw_parts()
            .0;
        Ok(resvg::usvg::fontdb::Source::SharedFile(
            path.to_path_buf(),
            data,
        ))
    }
}

pub struct SvgRasterizeOps;

impl SvgRasterizeOps {
    pub(crate) fn html_text_baseline_for_text(
        text: &str,
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
        line_height: f32,
        normal_line_height: bool,
    ) -> Option<f32> {
        text_shaping::html_text_baseline_for_text(
            text,
            family,
            size,
            weight,
            italic,
            line_height,
            normal_line_height,
        )
    }
    pub(crate) fn html_line_metrics(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
    ) -> Option<(f32, f32)> {
        text_shaping::html_line_metrics(text, font_family, font_size, font_weight, italic)
    }
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
        #[cfg(target_os = "macos")]
        return font::rasterize_html_known_or_eager(svg_text, scale);
        #[cfg(not(target_os = "macos"))]
        Self::rasterize_with_options(svg_text, scale, &html_rasterizer_options(svg_text))
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

    pub(crate) fn effective_html_font_size(size: f32) -> f32 {
        text_shaping::effective_html_font_size(size)
    }

    pub(crate) fn html_input_character_metrics(
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
    ) -> Option<text_shaping::InputCharacterMetrics> {
        text_shaping::html_input_character_metrics(family, size, weight, italic)
    }

    pub(crate) fn html_text_font_runs(
        text: &str,
        font_family: &str,
        font_weight: u16,
        italic: bool,
    ) -> Option<Vec<(String, String)>> {
        text_shaping::html_text_font_runs(text, font_family, font_weight, italic)
    }

    fn rasterize_with_options(
        svg_text: &str,
        scale: f32,
        options: &usvg::Options<'_>,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let compatible_svg = Self::preprocess_for_rasterizer(svg_text);
        let tree = text_shaping::with_validated_tree_parse(|| {
            #[cfg(all(test, target_os = "macos"))]
            let result = font::observation::eager_tree(&compatible_svg, options);
            #[cfg(not(all(test, target_os = "macos")))]
            let result = usvg::Tree::from_str(&compatible_svg, options);
            result
        })
        .map_err(|e| SvgRasterizeError::ParseFailed(e.to_string()))?;
        Self::render_tree(&tree, scale, &compatible_svg)
    }

    fn render_tree(
        tree: &usvg::Tree,
        scale: f32,
        compatible_svg: &str,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let raster = RasterTarget::new(tree.size(), scale);
        #[cfg(all(test, target_os = "macos"))]
        font::observation::count(font::observation::Metric::RasterAttempt);
        let pixmap = raster.render(tree, compatible_svg)?;
        #[cfg(all(test, target_os = "macos"))]
        font::observation::count(font::observation::Metric::RasterSuccess);
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

    fn render(&self, tree: &usvg::Tree, compatible_svg: &str) -> Result<Pixmap, SvgRasterizeError> {
        let Some(mut pixmap) = Pixmap::new(self.width, self.height) else {
            return Err(SvgRasterizeError::RasterizeFailed(
                "failed to allocate SVG pixmap".to_string(),
            ));
        };
        let transform =
            tiny_skia::Transform::from_scale(self.effective_scale, self.effective_scale);
        let authored_clip_info = if composite::SvgCompositeRenderer::needs_authored_clip_ids(tree) {
            provenance::authored_clip_ids(compatible_svg)?
        } else {
            provenance::AuthoredClipInfo::default()
        };
        composite::SvgCompositeRenderer::render(
            tree,
            transform,
            &mut pixmap.as_mut(),
            &authored_clip_info,
        )
        .map_err(SvgRasterizeError::RasterizeFailed)?;
        Ok(pixmap)
    }
}

#[cfg(test)]
#[path = "svg_rasterize_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_font_observation.rs"]
mod observation;
