use super::super::super::text_shaping;
use super::{HtmlFontRequest, discovery_cache, html_rasterizer_options};
use resvg::usvg;
#[path = "svg_rasterize_html_font_selected_face.rs"]
mod selected_face;
#[path = "svg_rasterize_html_font_lazy_selectors.rs"]
mod selectors;
use selectors::{install, install_fallback, selector_admitted};
#[path = "svg_rasterize_html_font_lazy_attempt.rs"]
mod attempt;
use attempt::known_tree;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(in crate::markdown::svg_rasterize) fn parse(
    svg: &str,
    markup: &str,
) -> Result<usvg::Tree, usvg::Error> {
    let Some((options, valid, selected, context)) = prepare(svg, markup) else {
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

fn decoded_ascii(svg: &str) -> Option<String> {
    let document = usvg::roxmltree::Document::parse(svg).ok()?;
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
) -> Result<super::super::super::RasterizedSvg, super::super::super::SvgRasterizeError> {
    let compatible = super::super::super::SvgRasterizeOps::preprocess_for_rasterizer(svg);
    let tree = parse(&compatible, svg)
        .map_err(|error| super::super::super::SvgRasterizeError::ParseFailed(error.to_string()))?;
    super::super::super::SvgRasterizeOps::render_tree(&tree, scale, &compatible)
}

type KnownOptions = (
    usvg::Options<'static>,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
    Option<Arc<selected_face::Context>>,
);

fn prepare(svg: &str, markup: &str) -> Option<KnownOptions> {
    #[cfg(test)]
    if super::lazy_diagnostic::active() {
        return diagnostic::prepare(svg, markup);
    }
    let request = HtmlFontRequest::from_markup(markup);
    let text = prepared_ascii(svg)?;
    let Some(value) = discovery_cache::known_for(&request) else {
        #[cfg(test)]
        super::raster_observer::prepare_known_unavailable();
        return None;
    };
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

fn prepared_ascii(svg: &str) -> Option<String> {
    let Some(text) = decoded_ascii(svg) else {
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
    use super::super::{lazy_diagnostic, raster_observer};
    use super::{
        HtmlFontRequest, KnownOptions, discovery_cache, install, parse, prepared_ascii,
        text_shaping,
    };
    use resvg::usvg;
    use std::sync::{Arc, atomic::AtomicBool};

    pub(in crate::markdown::svg_rasterize) fn rasterize(
        svg: &str,
        scale: f32,
    ) -> Result<
        super::super::super::super::RasterizedSvg,
        super::super::super::super::SvgRasterizeError,
    > {
        let paint_call = lazy_diagnostic::begin_final_glyph_paint_call();
        let compatible =
            super::super::super::super::SvgRasterizeOps::preprocess_for_rasterizer(svg);
        let tree = lazy_diagnostic::measure("lazy_parse_inclusive", || parse(&compatible, svg))
            .map_err(|error| {
                super::super::super::super::SvgRasterizeError::ParseFailed(error.to_string())
            })?;
        let rendered = lazy_diagnostic::measure("render_tree", || {
            super::super::super::super::SvgRasterizeOps::render_tree(&tree, scale, &compatible)
        });
        if rendered.is_ok()
            && let Some(paint_call) = paint_call
        {
            lazy_diagnostic::record_final_glyph_tree(&tree, svg, paint_call);
        }
        rendered
    }

    pub(in crate::markdown::svg_rasterize) fn prepare(
        svg: &str,
        markup: &str,
    ) -> Option<KnownOptions> {
        let request = HtmlFontRequest::from_markup(markup);
        let text = prepared_ascii(svg)?;
        let Some(value) = lazy_diagnostic::measure("known_database_lookup", || {
            discovery_cache::known_for(&request)
        }) else {
            raster_observer::prepare_known_unavailable();
            return None;
        };
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
