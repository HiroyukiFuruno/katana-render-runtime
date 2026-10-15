#[path = "svg_rasterize_html_font_lazy_diagnostic_routes.rs"]
mod routes;
#[path = "svg_rasterize_html_font_lazy_diagnostic_sources.rs"]
mod sources;
use resvg::usvg::fontdb::Database;
pub(super) use routes::{record_selected_face, select_database};
pub(super) use sources::FaceSource;
use std::{
    cell::{Cell, RefCell},
    sync::Weak,
    time::Instant,
};

const MAX_CAPTURED_RECORDS: usize = 512;
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
}
thread_local! {
    static ROUTE: Cell<Option<SourceRoute>> = const { Cell::new(None) };
    static MODE: Cell<Mode> = const { Cell::new(Mode::Measure) };
    static ACTIVE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}
struct Restore {
    route: Option<SourceRoute>,
    mode: Mode,
    capture: Option<Capture>,
}
impl Drop for Restore {
    fn drop(&mut self) {
        ROUTE.with(|value| value.set(self.route));
        MODE.with(|value| value.set(self.mode));
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
    let old_capture = ACTIVE.with(|value| value.replace(Some(Capture::default())));
    let restore = Restore {
        route: old_route,
        mode: old_mode,
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
