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

    pub(crate) fn shape_html_text_dx_with_trim(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
        font_feature_settings: Option<&str>,
        trim: TextSpacingTrim,
    ) -> Option<Vec<f32>> {
        text_shaping::shaped_html_text_dx_with_trim(
            text,
            font_family,
            font_size,
            font_weight,
            italic,
            font_feature_settings,
            trim,
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
        .map_err(|e| SvgRasterizeError::ParseFailed(e.to_string()))?;
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

    pub(crate) fn render_tree_with_provenance(
        tree: &usvg::Tree,
        scale: f32,
        compatible_svg: &str,
        provenance: &GeneratedClipProvenance,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::render_tree_with_text(tree, scale, compatible_svg, provenance, None, None)
    }

    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn render_html_tree_with_provenance(
        tree: &usvg::Tree,
        scale: f32,
        compatible_svg: &str,
        provenance: &GeneratedClipProvenance,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let document = usvg::roxmltree::Document::parse(compatible_svg);
        Self::render_html_tree_with_text_provenance(
            tree,
            scale,
            compatible_svg,
            provenance,
            &GeneratedHtmlTextProvenance::default(),
            &document,
        )
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn render_html_tree_with_text_provenance(
        tree: &usvg::Tree,
        scale: f32,
        compatible_svg: &str,
        provenance: &GeneratedClipProvenance,
        html_text: &GeneratedHtmlTextProvenance,
        document: &html_text_provenance::ParsedHtmlDocument<'_>,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        #[cfg(test)]
        if std::env::var("KRR_HTML_FEATURE_RESVG_TEXT_RASTER").is_ok_and(|value| value == "1") {
            return Self::render_tree_with_provenance(tree, scale, compatible_svg, provenance);
        }
        let routing = html_text.validated_routing_with_document(document, tree)?;
        if routing.all_text_verified {
            return Self::render_tree_with_provenance(tree, scale, compatible_svg, provenance);
        }
        Self::render_unverified_html_text(tree, scale, compatible_svg, provenance, routing)
    }

    #[cfg(target_os = "macos")]
    fn render_unverified_html_text(
        tree: &usvg::Tree,
        scale: f32,
        compatible_svg: &str,
        provenance: &GeneratedClipProvenance,
        routing: html_text_provenance::HtmlTextRouting,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let source_context = Self::authored_context_if_needed(tree, compatible_svg, provenance)?;
        Self::render_html_with_routing(
            tree,
            scale,
            compatible_svg,
            provenance,
            routing,
            source_context.as_ref(),
        )
    }

    #[cfg(target_os = "macos")]
    fn authored_context_if_needed(
        tree: &usvg::Tree,
        compatible_svg: &str,
        provenance: &GeneratedClipProvenance,
    ) -> Result<Option<provenance::AuthoredSvgContext>, SvgRasterizeError> {
        if !html_glyph::has_nested_middle_candidate(tree) {
            return Ok(None);
        }
        let needs_authored_clip_ids =
            composite::SvgCompositeRenderer::needs_authored_clip_ids(tree);
        provenance::authored_svg_context(compatible_svg, provenance, needs_authored_clip_ids)
            .map(Some)
    }

    #[cfg(target_os = "macos")]
    fn render_html_with_routing(
        tree: &usvg::Tree,
        scale: f32,
        compatible_svg: &str,
        provenance: &GeneratedClipProvenance,
        routing: html_text_provenance::HtmlTextRouting,
        source_context: Option<&provenance::AuthoredSvgContext>,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let glyphs = match source_context {
            Some(context) => html_glyph::HtmlGlyphRasterizer::with_resvg_text_and_source_context(
                tree.fontdb(),
                routing.ids,
                &context.text,
            ),
            None => html_glyph::HtmlGlyphRasterizer::with_resvg_text(tree.fontdb(), routing.ids),
        };
        let authored_clip_info = source_context.map(|context| &context.clips);
        Self::render_tree_with_text(
            tree,
            scale,
            compatible_svg,
            provenance,
            Some(&glyphs),
            authored_clip_info,
        )
    }

    fn render_tree_with_text(
        tree: &usvg::Tree,
        scale: f32,
        compatible_svg: &str,
        provenance: &GeneratedClipProvenance,
        text: Option<&dyn composite::TextRasterizer>,
        authored_clip_info: Option<&provenance::AuthoredClipInfo>,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let raster = RasterTarget::new(tree.size(), scale);
        #[cfg(all(test, target_os = "macos"))]
        font::observation::count(font::observation::Metric::RasterAttempt);
        let pixmap =
            raster.render_with_text(tree, compatible_svg, provenance, text, authored_clip_info)?;
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

#[cfg(test)]
#[path = "svg_rasterize_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_css_ex_tests.rs"]
mod css_ex_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_font_observation.rs"]
mod observation;
