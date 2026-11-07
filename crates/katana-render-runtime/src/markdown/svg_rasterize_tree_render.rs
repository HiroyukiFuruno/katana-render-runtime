#[cfg(all(test, target_os = "macos"))]
use super::font;
use super::{
    GeneratedClipProvenance, RasterTarget, RasterizedSvg, SvgRasterizeError, SvgRasterizeOps,
    composite, provenance, usvg,
};
#[cfg(target_os = "macos")]
use super::{GeneratedHtmlTextProvenance, html_glyph, html_text_provenance};

impl SvgRasterizeOps {
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
