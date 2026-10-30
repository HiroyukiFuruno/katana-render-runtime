#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_final_glyphs.rs"]
mod final_glyphs;
#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_font_path_paint_diagnostic.rs"]
mod path_paint;
#[path = "svg_rasterize_html_font_lazy_diagnostic_routes.rs"]
mod routes;
#[path = "svg_rasterize_html_font_lazy_diagnostic_sources.rs"]
mod sources;
use resvg::usvg::fontdb::{Database, ID};
pub(super) use routes::{file_fixture_database, record_selected_face, select_database};
pub(super) use sources::FaceSource;
use std::{
    cell::{Cell, RefCell},
    sync::Weak,
    time::Instant,
};

#[cfg(test)]
type TestRasterResult = Result<crate::markdown::svg_rasterize::RasterizedSvg, String>;
#[cfg(test)]
type TestAuditedRasterResult = Result<(TestRasterResult, serde_json::Value), String>;

const MAX_CAPTURED_RECORDS: usize = 512;
const CROP_RECT_COMPONENTS: usize = 4;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceRoute {
    File,
    SharedFile,
}
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Measure,
    Audit,
}
#[derive(Clone, Default)]
pub(super) struct Capture {
    pub(super) stages: Vec<(&'static str, u128)>,
    pub(super) faces: Vec<FaceSource>,
    pub(super) database_owners: Vec<Weak<Database>>,
    pub(super) selected_database_owners: Vec<Weak<Database>>,
    pub(super) shared_byte_owners: Vec<Weak<dyn AsRef<[u8]> + Send + Sync>>,
    pub(super) errors: Vec<String>,
    pub(super) dropped: usize,
    #[cfg(test)]
    final_glyph_trees: Vec<final_glyphs::FinalGlyphTree>,
    #[cfg(test)]
    final_glyph_errors: Vec<final_glyphs::FinalGlyphError>,
    #[cfg(test)]
    final_glyph_dropped: usize,
    #[cfg(test)]
    final_glyph_paint_calls: u64,
    #[cfg(test)]
    final_glyph_record_count: usize,
    #[cfg(test)]
    final_glyph_shared_byte_owners: Vec<sources::ByteWeak>,
    #[cfg(test)]
    html_font_markups: Vec<serde_json::Value>,
    #[cfg(test)]
    html_font_markups_dropped: usize,
    #[cfg(test)]
    render_dispatches: Vec<RenderDispatch>,
    #[cfg(test)]
    path_custom_hits: u64,
    #[cfg(test)]
    path_fallback_hits: u64,
    #[cfg(all(test, target_os = "macos"))]
    path_paints: Vec<serde_json::Value>,
    #[cfg(all(test, target_os = "macos"))]
    path_paints_dropped: usize,
    #[cfg(all(test, target_os = "macos"))]
    crop_stack: Vec<[i64; CROP_RECT_COMPONENTS]>,
    #[cfg(all(test, target_os = "macos"))]
    crop_overflow: usize,
}
#[cfg(test)]
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderDispatch {
    text_rasterizer_present: bool,
    object_bounding_box_clip: bool,
}
thread_local! {
    static ROUTE: Cell<Option<SourceRoute>> = const { Cell::new(None) };
    static MODE: Cell<Mode> = const { Cell::new(Mode::Measure) };
    static PASSIVE_AUDIT: Cell<bool> = const { Cell::new(false) };
    static ACTIVE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}
struct Restore {
    route: Option<SourceRoute>,
    mode: Mode,
    passive_audit: bool,
    capture: Option<Capture>,
}
impl Drop for Restore {
    fn drop(&mut self) {
        ROUTE.with(|value| value.set(self.route));
        MODE.with(|value| value.set(self.mode));
        PASSIVE_AUDIT.with(|value| value.set(self.passive_audit));
        ACTIVE.with(|value| {
            value.replace(self.capture.take());
        });
    }
}
pub(super) fn with_route<T>(
    route: SourceRoute,
    mode: Mode,
    operation: impl FnOnce() -> T,
) -> (T, Capture) {
    let old_route = ROUTE.with(|value| value.replace(Some(route)));
    let old_mode = MODE.with(|value| value.replace(mode));
    let old_passive_audit = PASSIVE_AUDIT.with(Cell::get);
    let old_capture = ACTIVE.with(|value| value.replace(Some(Capture::default())));
    let restore = Restore {
        route: old_route,
        mode: old_mode,
        passive_audit: old_passive_audit,
        capture: old_capture,
    };
    let result = operation();
    let capture = ACTIVE.with(|value| match value.borrow().as_ref() {
        Some(capture) => capture.clone(),
        None => Capture {
            errors: vec!["diagnostic scope missing after operation".into()],
            ..Capture::default()
        },
    });
    drop(restore);
    (result, capture)
}
pub(super) fn active() -> bool {
    ROUTE.with(|value| value.get().is_some())
}

pub(super) fn with_actual_route<T>(operation: impl FnOnce() -> T) -> (T, Capture) {
    let old_route = ROUTE.with(|value| value.replace(None));
    let old_mode = MODE.with(|value| value.replace(Mode::Audit));
    let old_passive_audit = PASSIVE_AUDIT.with(|value| value.replace(true));
    let old_capture = ACTIVE.with(|value| value.replace(Some(Capture::default())));
    let restore = Restore {
        route: old_route,
        mode: old_mode,
        passive_audit: old_passive_audit,
        capture: old_capture,
    };
    let result = operation();
    let capture = ACTIVE.with(|value| {
        value.borrow().as_ref().cloned().unwrap_or_else(|| Capture {
            errors: vec!["actual font audit scope missing after operation".into()],
            ..Capture::default()
        })
    });
    drop(restore);
    (result, capture)
}

fn actual_audit_json(capture: &Capture) -> Result<serde_json::Value, serde_json::Error> {
    let faces = serde_json::to_value(&capture.faces)?;
    let errors = serde_json::to_value(&capture.errors)?;
    let final_glyph_trees = serde_json::to_value(&capture.final_glyph_trees)?;
    let final_glyph_errors = serde_json::to_value(&capture.final_glyph_errors)?;
    Ok(serde_json::json!({
        "faces": faces,
        "errors": errors,
        "dropped": capture.dropped,
        "route": "actual-current-route",
        "finalGlyphTrees": final_glyph_trees,
        "finalGlyphErrors": final_glyph_errors,
        "finalGlyphDropped": capture.final_glyph_dropped,
        "finalGlyphPaintCalls": capture.final_glyph_paint_calls,
        "htmlFontMarkups": capture.html_font_markups,
        "htmlFontMarkupsDropped": capture.html_font_markups_dropped,
        "renderDispatches": capture.render_dispatches,
        "pathCustomHits": capture.path_custom_hits,
        "pathFallbackHits": capture.path_fallback_hits,
        "pathPaints": capture.path_paints,
        "pathPaintsDropped": capture.path_paints_dropped,
    }))
}

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

#[cfg(test)]
pub(super) fn begin_final_glyph_paint_call() -> Option<u64> {
    if !PASSIVE_AUDIT.with(Cell::get) {
        return None;
    }
    let mut paint_call = None;
    update(0, |capture| {
        capture.final_glyph_paint_calls = capture.final_glyph_paint_calls.saturating_add(1);
        paint_call = Some(capture.final_glyph_paint_calls);
    });
    paint_call
}

#[cfg(test)]
pub(super) fn record_final_glyph_tree(tree: &resvg::usvg::Tree, source_svg: &str, paint_call: u64) {
    if !PASSIVE_AUDIT.with(Cell::get) {
        return;
    }
    final_glyphs::record(tree, source_svg, paint_call);
}

pub(in crate::markdown::svg_rasterize) fn record_actual_selected_face(
    db: &Database,
    id: Option<ID>,
) {
    if !PASSIVE_AUDIT.with(Cell::get) {
        return;
    }
    let Some(face) = id.and_then(|id| db.face(id)) else {
        return;
    };
    let (record, weak) = match sources::source_record(face, Mode::Audit) {
        Some((record, weak)) => (record, weak),
        None => {
            error("selected font source bytes or stable physical stamp could not be audited");
            sources::measured_source_record(face)
        }
    };
    let cost = if weak.is_some() { 2 } else { 1 };
    update(cost, |capture| {
        capture.faces.push(record);
        if let Some(weak) = weak {
            capture.shared_byte_owners.push(weak);
        }
    });
}
pub(super) fn measure<T>(name: &'static str, operation: impl FnOnce() -> T) -> T {
    if !active() || matches!(MODE.with(Cell::get), Mode::Audit) {
        return operation();
    }
    let started = Instant::now();
    let result = operation();
    stage(name, started.elapsed().as_nanos());
    result
}
fn stage(name: &'static str, elapsed: u128) {
    update(1, |capture| capture.stages.push((name, elapsed)));
}
fn update(cost: usize, operation: impl FnOnce(&mut Capture)) {
    ACTIVE.with(|active| {
        if let Some(capture) = active.borrow_mut().as_mut() {
            let records = capture.stages.len()
                + capture.faces.len()
                + capture.database_owners.len()
                + capture.selected_database_owners.len()
                + capture.shared_byte_owners.len()
                + capture.errors.len();
            if records + cost > MAX_CAPTURED_RECORDS {
                capture.dropped += 1;
            } else {
                operation(capture);
            }
        }
    });
}
fn error(message: &str) {
    update(1, |capture| capture.errors.push(message.into()));
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_diagnostic_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
fn rasterize_normally(svg: &str) -> Result<crate::markdown::svg_rasterize::RasterizedSvg, String> {
    crate::markdown::svg_rasterize::SvgRasterizeOps::rasterize_html_svg(svg, 1.0)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
fn rasterize_with_audit(svg: &str, observe: impl FnOnce()) -> TestAuditedRasterResult {
    let (audited, audit) =
        crate::markdown::svg_rasterize::SvgRasterizeOps::with_actual_font_source_audit(|| {
            observe();
            crate::markdown::svg_rasterize::SvgRasterizeOps::rasterize_html_svg(svg, 1.0)
                .map_err(|error| error.to_string())
        })
        .map_err(|error| error.to_string())?;
    Ok((audited, audit))
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_diagnostic_passive_actual_route_tests.rs"]
mod passive_actual_route_tests;
