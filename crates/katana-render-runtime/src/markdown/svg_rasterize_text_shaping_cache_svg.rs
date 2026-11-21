use super::{
    Arc, CachedShapedRun, FontShape, FontSourceGeneration, ShapedRunKey, cache_shaped_run_record,
    cached_shaped_run_record_for_generation, font_source_generation, usvg,
};

pub(in crate::markdown::svg_rasterize::text_shaping) fn cached_svg_run(
    database: &Arc<usvg::fontdb::Database>,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
) -> Option<super::super::harfrust::SvgTextRunMetrics> {
    let key = ShapedRunKey::new(face_id, text, font, None);
    let before = font_source_generation(database, face_id);
    let metrics = match cached_shaped_run_record_for_generation(database, &key, &before) {
        Some(CachedShapedRun {
            advance,
            svg_endpoints: Some((ascent, descent)),
            ink_bounds,
        }) => {
            return Some(super::super::harfrust::SvgTextRunMetrics {
                advance,
                ascent,
                descent,
                layout_ascent: None,
                ink_bounds,
            });
        }
        cached => {
            metrics_for_svg_run_without_cached_endpoints(database, face_id, text, font, cached)?
        }
    };
    validate_and_cache_svg_run(database, face_id, key, before, metrics)
}

pub(super) fn validate_and_cache_svg_run(
    database: &Arc<usvg::fontdb::Database>,
    face_id: usvg::fontdb::ID,
    key: ShapedRunKey,
    before: FontSourceGeneration,
    metrics: super::super::harfrust::SvgTextRunMetrics,
) -> Option<super::super::harfrust::SvgTextRunMetrics> {
    let after = font_source_generation(database, face_id);
    if before != after {
        return None;
    }
    if after.reusable() {
        cache_shaped_run_record(
            database,
            key,
            CachedShapedRun {
                advance: metrics.advance,
                svg_endpoints: Some((metrics.ascent, metrics.descent)),
                ink_bounds: metrics.ink_bounds,
            },
            after,
        );
    }
    Some(metrics)
}

fn metrics_for_svg_run_without_cached_endpoints(
    database: &Arc<usvg::fontdb::Database>,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    cached: Option<CachedShapedRun>,
) -> Option<super::super::harfrust::SvgTextRunMetrics> {
    match cached {
        Some(run) => {
            let (ascent, descent) =
                super::super::harfrust::svg_run_endpoints_with_face(database, face_id, font)?;
            Some(super::super::harfrust::SvgTextRunMetrics {
                advance: run.advance,
                ascent,
                descent,
                layout_ascent: None,
                ink_bounds: run.ink_bounds,
            })
        }
        None => super::super::harfrust::shape_svg_run_metrics_with_face(
            database, face_id, text, font, false,
        )
        .map(|(metrics, _)| metrics),
    }
}
