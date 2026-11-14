use super::{
    ACTIVE, MAX_CAPTURED_RECORDS, RenderDispatch, actual_audit_json, update, with_actual_route,
};

impl crate::markdown::svg_rasterize::SvgRasterizeOps {
    pub(crate) fn with_actual_font_source_audit<T>(
        operation: impl FnOnce() -> T,
    ) -> Result<(T, serde_json::Value), serde_json::Error> {
        let (result, capture) = with_actual_route(operation);
        Ok((result, actual_audit_json(&capture)?))
    }

    #[cfg(test)]
    pub(in crate::markdown::svg_rasterize) fn record_render_dispatch(
        text_rasterizer_present: bool,
        object_bounding_box_clip: bool,
    ) {
        update(1, |capture| {
            capture.render_dispatches.push(RenderDispatch {
                text_rasterizer_present,
                object_bounding_box_clip,
            });
        });
    }

    #[cfg(test)]
    pub(in crate::markdown::svg_rasterize) fn record_path_route(custom: bool) {
        update(1, |capture| {
            if custom {
                capture.path_custom_hits = capture.path_custom_hits.saturating_add(1);
            } else {
                capture.path_fallback_hits = capture.path_fallback_hits.saturating_add(1);
            }
        });
    }

    #[cfg(test)]
    pub(crate) fn record_actual_html_font_markup(
        computed_family: &str,
        computed_weight: u16,
        run_families: Option<&[String]>,
        svg_attributes: &str,
    ) {
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let Some(capture) = active.as_mut() else {
                return;
            };
            if capture.html_font_markups.len() >= MAX_CAPTURED_RECORDS {
                capture.html_font_markups_dropped += 1;
                return;
            }
            capture.html_font_markups.push(serde_json::json!({
                "computedFamily": computed_family,
                "computedWeight": computed_weight,
                "effectiveSvgWeight": computed_weight,
                "resolvedRunStatus": match run_families {
                    None => "unresolved",
                    Some([]) => "empty",
                    Some(_) => "resolved",
                },
                "runFamilies": run_families,
                "svgAttributes": svg_attributes,
            }));
        });
    }
}
