/* WHY: SVG rasterization utility.
Uses `resvg` + `usvg` to convert SVG text to an RGBA pixel buffer.
Returns the result as raw bytes compatible with egui's `ColorImage`. */

use resvg::usvg;

pub(super) fn escape_xml_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[path = "svg_rasterize_composite.rs"]
mod composite;
#[path = "svg_rasterize_font.rs"]
mod font;
#[cfg(test)]
#[path = "svg_rasterize_font_file_fixture.rs"]
pub(super) mod font_file_fixture;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph.rs"]
mod html_glyph;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_variations.rs"]
mod html_glyph_variations;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_shadow.rs"]
mod html_shadow;
#[path = "svg_rasterize_text_measurement_api.rs"]
mod measurement_api;
#[path = "svg_rasterize_measurement_diagnostic.rs"]
pub(crate) mod measurement_diagnostic;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_glyph_ttc.rs"]
mod physical_font_face;
#[path = "svg_rasterize_preprocess.rs"]
mod preprocess;
#[path = "svg_rasterize_composite_provenance.rs"]
mod provenance;
#[path = "svg_rasterize_render.rs"]
mod render;
#[path = "svg_rasterize_text.rs"]
mod text;
#[cfg(test)]
pub(in crate::markdown::svg_rasterize) use text::probe_test_support;
#[path = "svg_rasterize_html_measurement.rs"]
mod html_measurement;
#[path = "svg_rasterize_text_shaping.rs"]
mod text_shaping;
#[path = "svg_rasterize_tree_render.rs"]
mod tree_render;
#[path = "svg_rasterize_types.rs"]
mod types;
#[cfg(not(target_os = "macos"))]
use font::html_rasterizer_options;
use font::rasterizer_options;
#[cfg(test)]
use preprocess::parse_light_dark_function;
use preprocess::{effective_scale, preprocess_for_rasterizer};
pub(crate) use provenance::{GeneratedClip, GeneratedClipProvenance};
#[path = "svg_rasterize_html_text_provenance.rs"]
mod html_text_provenance;
pub(crate) use html_text_provenance::GeneratedHtmlTextProvenance;
use render::RasterTarget;
pub(crate) use text_shaping::{
    HtmlTextMeasurementToken, HtmlTextMeasurementTokenKey, SvgTextRunMetrics, TextSpacingTrim,
};
pub use types::{RasterizedSvg, SvgRasterizeError, SvgRasterizeOps};

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

impl SvgRasterizeOps {
    pub fn preprocess_for_rasterizer(svg_text: &str) -> String {
        preprocess_for_rasterizer(svg_text)
    }

    pub fn rasterize_svg(svg_text: &str, scale: f32) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::rasterize_with_options(svg_text, scale, &rasterizer_options())
    }

    #[cfg(test)]
    pub(crate) fn rasterize_html_svg(
        svg_text: &str,
        scale: f32,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::rasterize_html_svg_with_provenance(
            svg_text,
            scale,
            &GeneratedClipProvenance::default(),
        )
    }

    #[cfg(test)]
    pub(crate) fn rasterize_html_svg_with_provenance(
        svg_text: &str,
        scale: f32,
        provenance: &GeneratedClipProvenance,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::rasterize_html_svg_with_text_provenance(
            svg_text,
            scale,
            provenance,
            &GeneratedHtmlTextProvenance::default(),
        )
    }

    pub(crate) fn rasterize_html_svg_with_text_provenance(
        svg_text: &str,
        scale: f32,
        provenance: &GeneratedClipProvenance,
        html_text: &GeneratedHtmlTextProvenance,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        #[cfg(target_os = "macos")]
        return font::rasterize_html_known_or_eager(svg_text, scale, provenance, html_text);
        #[cfg(not(target_os = "macos"))]
        {
            let _ = html_text;
            Self::rasterize_with_options_and_provenance(
                svg_text,
                scale,
                &html_rasterizer_options(svg_text),
                provenance,
            )
        }
    }

    fn rasterize_with_options(
        svg_text: &str,
        scale: f32,
        options: &usvg::Options<'_>,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::rasterize_with_options_and_provenance(
            svg_text,
            scale,
            options,
            &GeneratedClipProvenance::default(),
        )
    }

    fn rasterize_with_options_and_provenance(
        svg_text: &str,
        scale: f32,
        options: &usvg::Options<'_>,
        provenance: &GeneratedClipProvenance,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let compatible_svg = Self::preprocess_for_rasterizer(svg_text);
        let tree = text_shaping::with_validated_tree_parse(|| {
            #[cfg(all(test, target_os = "macos"))]
            let result = font::observation::eager_tree(&compatible_svg, options);
            #[cfg(not(all(test, target_os = "macos")))]
            let result = usvg::Tree::from_str(&compatible_svg, options);
            result
        })
        .map_err(SvgRasterizeError::parse_failed)?;
        Self::render_tree_with_provenance(&tree, scale, &compatible_svg, provenance)
    }

    #[cfg(test)]
    fn render_tree(
        tree: &usvg::Tree,
        scale: f32,
        compatible_svg: &str,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::render_tree_with_provenance(
            tree,
            scale,
            compatible_svg,
            &GeneratedClipProvenance::default(),
        )
    }
}

#[cfg(test)]
#[path = "svg_rasterize_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_css_ex_tests.rs"]
mod css_ex_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_font_observation.rs"]
mod observation;
