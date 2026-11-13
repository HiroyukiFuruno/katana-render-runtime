use super::super::super::{html_text_provenance::ParsedHtmlDocument, text_shaping};
use super::{HtmlFontRequest, discovery_cache, html_rasterizer_options};
use resvg::usvg;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[path = "svg_rasterize_html_font_selected_face.rs"]
mod selected_face;
#[path = "svg_rasterize_html_font_lazy_selectors.rs"]
mod selectors;
use selectors::{install, install_fallback, selector_admitted};
#[path = "svg_rasterize_html_font_lazy_attempt.rs"]
mod attempt;
use attempt::known_tree;

#[cfg(test)]
pub(in crate::markdown::svg_rasterize) fn parse(
    svg: &str,
    markup: &str,
) -> Result<usvg::Tree, usvg::Error> {
    parse_document(svg, markup).map(|(tree, _)| tree)
}

fn parse_document<'input>(
    svg: &'input str,
    markup: &str,
) -> Result<(usvg::Tree, ParsedHtmlDocument<'input>), usvg::Error> {
    let document = usvg::roxmltree::Document::parse(svg);
    let tree = parse_with_document(svg, markup, &document)?;
    Ok((tree, document))
}

fn parse_with_document(
    svg: &str,
    markup: &str,
    document: &ParsedHtmlDocument<'_>,
) -> Result<usvg::Tree, usvg::Error> {
    parse_with_options(svg, markup, prepare(document, markup))
}

fn parse_with_options(
    svg: &str,
    markup: &str,
    options: Option<KnownOptions>,
) -> Result<usvg::Tree, usvg::Error> {
    let Some((options, valid, selected, context)) = options else {
        #[cfg(test)]
        super::raster_observer::prepare_unavailable();
        return eager(svg, markup);
    };
    #[cfg(test)]
    let timing = super::raster_observer::TreeParseTiming::default();
    #[cfg(test)]
    let tree = known_tree(svg, &options, &valid, &selected, context.as_ref(), &timing);
    #[cfg(not(test))]
    let tree = known_tree(svg, &options, &valid, &selected, context.as_ref());
    #[cfg(test)]
    super::raster_observer::known_result(tree.is_ok(), &valid, &selected, &timing);
    let tree = tree?;
    if valid.load(Ordering::Relaxed) && selected.load(Ordering::Relaxed) {
        #[cfg(test)]
        super::raster_observer::known_return();
        return Ok(tree);
    }
    #[cfg(test)]
    super::raster_observer::eager_reparse(&valid, &selected);
    drop(tree);
    eager(svg, markup)
}

fn eager(svg: &str, markup: &str) -> Result<usvg::Tree, usvg::Error> {
    #[cfg(not(test))]
    let options = html_rasterizer_options(markup);
    #[cfg(test)]
    let mut options = html_rasterizer_options(markup);
    #[cfg(test)]
    super::raster_observer::install_eager(&mut options);
    let result = text_shaping::with_validated_tree_parse(|| {
        #[cfg(test)]
        super::raster_observer::eager_attempt();
        #[cfg(test)]
        let result = super::raster_observer::eager_tree(svg, &options);
        #[cfg(not(test))]
        let result = usvg::Tree::from_str(svg, &options);
        result
    });
    #[cfg(test)]
    super::raster_observer::eager_result(result.is_ok());
    result
}

fn decoded_ascii(document: &ParsedHtmlDocument<'_>) -> Option<String> {
    let document = document.as_ref().ok()?;
    let text = document
        .descendants()
        .filter(|node| {
            node.is_text()
                && node
                    .ancestors()
                    .any(|ancestor| ancestor.has_tag_name("text"))
        })
        .filter_map(|node| node.text())
        .collect::<String>();
    (!text.is_empty() && text.is_ascii()).then_some(text)
}

#[cfg(not(test))]
pub(in crate::markdown::svg_rasterize) fn rasterize(
    svg: &str,
    scale: f32,
    provenance: &super::super::super::GeneratedClipProvenance,
    html_text: &super::super::super::GeneratedHtmlTextProvenance,
) -> Result<super::super::super::RasterizedSvg, super::super::super::SvgRasterizeError> {
    let compatible = super::super::super::SvgRasterizeOps::preprocess_for_rasterizer(svg);
    let (tree, document) = parse_document(&compatible, svg)
        .map_err(super::super::super::SvgRasterizeError::parse_failed)?;
    super::super::super::SvgRasterizeOps::render_html_tree_with_text_provenance(
        &tree,
        scale,
        &compatible,
        provenance,
        html_text,
        &document,
    )
}

type KnownOptions = (
    usvg::Options<'static>,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
    Option<Arc<selected_face::Context>>,
);

fn prepare(document: &ParsedHtmlDocument<'_>, markup: &str) -> Option<KnownOptions> {
    #[cfg(test)]
    if super::lazy_diagnostic::active() {
        return diagnostic::prepare(document, markup);
    }
    let request = HtmlFontRequest::from_markup(markup);
    let text = prepared_ascii(document)?;
    let value = discovery_cache::observe_known_font(discovery_cache::known_for(&request))?;
    let proof = value.proof;
    let context = selected_face::Context::new(value.database, proof, text)?;
    let database = Arc::clone(&context.original);
    let mut options = super::super::rasterizer_options_with_font_db(database);
    text_shaping::install_html_font_resolver(&mut options);
    let valid = Arc::new(AtomicBool::new(true));
    let selected = Arc::new(AtomicBool::new(false));
    context.install(&mut options, Arc::clone(&valid), Arc::clone(&selected));
    #[cfg(test)]
    super::raster_observer::prepare_ready();
    Some((options, valid, selected, Some(context)))
}

type LegacyOptions = (usvg::Options<'static>, Arc<AtomicBool>, Arc<AtomicBool>);

fn legacy_options(context: &selected_face::Context) -> Option<LegacyOptions> {
    let (database, proof) = context.proof.transient(&context.original)?;
    let mut options = super::super::rasterizer_options_with_font_db(database);
    text_shaping::install_html_font_resolver(&mut options);
    let valid = Arc::new(AtomicBool::new(true));
    let selected = Arc::new(AtomicBool::new(false));
    install(
        &mut options,
        proof,
        context.text.clone(),
        Arc::clone(&valid),
        Arc::clone(&selected),
    );
    Some((options, valid, selected))
}

fn prepared_ascii(document: &ParsedHtmlDocument<'_>) -> Option<String> {
    let Some(text) = decoded_ascii(document) else {
        #[cfg(test)]
        super::raster_observer::prepare_ascii_unavailable();
        return None;
    };
    Some(text)
}
#[cfg(test)]
pub(in crate::markdown::svg_rasterize) use diagnostic::rasterize;
#[cfg(test)]
mod diagnostic {
    use super::super::super::super::{
        GeneratedClipProvenance, GeneratedHtmlTextProvenance, RasterizedSvg, SvgRasterizeError,
        SvgRasterizeOps,
    };
    use super::super::discovery::KnownCacheValue;
    use super::super::{discovery_cache, lazy_diagnostic, raster_observer};
    use super::{
        HtmlFontRequest, KnownOptions, ParsedHtmlDocument, install, parse_document, prepared_ascii,
        text_shaping,
    };
    use resvg::usvg;
    use std::sync::{Arc, atomic::AtomicBool};

    pub(in crate::markdown::svg_rasterize) fn rasterize(
        svg: &str,
        scale: f32,
        provenance: &GeneratedClipProvenance,
        html_text: &GeneratedHtmlTextProvenance,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let paint_call = lazy_diagnostic::begin_final_glyph_paint_call();
        let compatible = SvgRasterizeOps::preprocess_for_rasterizer(svg);
        let (tree, document) =
            lazy_diagnostic::measure("lazy_parse_inclusive", || parse_document(&compatible, svg))
                .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
        let rendered = lazy_diagnostic::measure("render_tree", || {
            SvgRasterizeOps::render_html_tree_with_text_provenance(
                &tree,
                scale,
                &compatible,
                provenance,
                html_text,
                &document,
            )
        });
        if rendered.is_ok()
            && let Some(paint_call) = paint_call
        {
            lazy_diagnostic::record_final_glyph_tree(&tree, svg, paint_call);
        }
        rendered
    }

    pub(in crate::markdown::svg_rasterize) fn prepare(
        document: &ParsedHtmlDocument<'_>,
        markup: &str,
    ) -> Option<KnownOptions> {
        let request = HtmlFontRequest::from_markup(markup);
        let text = prepared_ascii(document)?;
        let value = lazy_diagnostic::measure("file_fixture_database_build", || {
            lazy_diagnostic::file_fixture_database(&request)
        });
        prepare_file_options(value, text)
    }

    pub(super) fn prepare_file_options(
        value: Option<KnownCacheValue>,
        text: String,
    ) -> Option<KnownOptions> {
        let value = discovery_cache::observe_known_font(value)?;
        let proof = value.proof;
        let (database, proof) = lazy_diagnostic::select_database(&value.database, proof)?;
        Some(finish_options(database, proof, text))
    }

    fn finish_options(
        database: Arc<usvg::fontdb::Database>,
        proof: Arc<super::super::proof::CreationProof>,
        text: String,
    ) -> KnownOptions {
        let mut options = super::super::super::rasterizer_options_with_font_db(database);
        text_shaping::install_html_font_resolver(&mut options);
        let valid = Arc::new(AtomicBool::new(true));
        let selected = Arc::new(AtomicBool::new(false));
        install(
            &mut options,
            proof,
            text,
            Arc::clone(&valid),
            Arc::clone(&selected),
        );
        capture_selected(&mut options);
        raster_observer::prepare_ready();
        (options, valid, selected, None)
    }

    fn capture_selected(options: &mut usvg::Options<'static>) {
        if !lazy_diagnostic::active() {
            return;
        }
        let selector = std::mem::replace(
            &mut options.font_resolver.select_font,
            usvg::FontResolver::default_font_selector(),
        );
        options.font_resolver.select_font = Box::new(move |font, database| {
            let actual = selector(font, database);
            lazy_diagnostic::record_selected_face(database, actual);
            actual
        });
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_text_coverage_gap_tests.rs"]
mod coverage_gap_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_xml_shared_tests.rs"]
mod xml_shared_tests;
