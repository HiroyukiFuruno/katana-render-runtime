use super::super::diagnostic_binary_fixture::real_database as shared_real_database;
use super::{
    Capture, MAX_CAPTURED_RECORDS, Mode, SourceRoute, active, measure, record_selected_face,
    with_route,
};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::{Arc, Weak};

const RECORDS_PER_BINARY_FACE: usize = 2;
const RETAINED_STAGES: usize = 2;
const DROPPED_RECORD_ATTEMPTS: usize = 2;
const REGULAR_FONT_WEIGHT: u16 = 400;
const REGULAR_FONT_WEIGHT_STYLE: &str = "font-weight:400";
const INITIAL_MARKUP_RECORDS: usize = 3;
type FontBytesWeak = Weak<dyn AsRef<[u8]> + Send + Sync>;
type RealDatabase = (Arc<Database>, ID, FontBytesWeak);
type MissingCaptureRouteCase = (fn() -> Capture, &'static str);

#[test]
fn lazy_capture_real_face_enforces_weighted_limit_and_restores_nested_route() -> Result<(), String>
{
    let (database, id, bytes) = real_database()?;
    let owner = Arc::downgrade(&database);
    assert!(!active());
    let (inner, capture) = with_route(SourceRoute::File, Mode::Measure, || {
        fill_real_faces(&database, id);
        let inner = audit_binary_nested(&database, id);
        finish_at_capacity(&database, id);
        inner
    });
    assert!(!active());
    assert_weighted_capture(&capture, &database)?;
    assert_eq!(inner.errors, ["selected font source could not be audited"]);
    assert!(inner.faces.is_empty());
    assert!(inner.stages.is_empty());
    assert_eq!(inner.dropped, 0);
    assert_disabled_record_not_retained(&database, id);
    assert_eq!(Arc::strong_count(&database), 1);
    drop(database);
    assert_released_backing(&capture, &owner, &bytes);
    Ok(())
}

fn real_database() -> Result<RealDatabase, String> {
    shared_real_database("real bundled source is not Binary")
}

fn fill_real_faces(database: &Arc<Database>, id: ID) {
    let retained = (MAX_CAPTURED_RECORDS - RETAINED_STAGES) / RECORDS_PER_BINARY_FACE;
    for _ in 0..retained {
        record_selected_face(database, Some(id));
    }
}

fn audit_binary_nested(database: &Arc<Database>, id: ID) -> Capture {
    let (returned, capture) = with_route(SourceRoute::SharedFile, Mode::Audit, || {
        assert!(active());
        assert_eq!(measure("audit-no-clock", || id), id);
        record_selected_face(database, Some(id));
        id
    });
    assert_eq!(returned, id);
    assert!(active());
    assert!(matches!(
        super::ROUTE.with(std::cell::Cell::get),
        Some(SourceRoute::File)
    ));
    capture
}

fn finish_at_capacity(database: &Arc<Database>, id: ID) {
    assert_eq!(measure("accepted-511", || id), id);
    assert_eq!(measure("accepted-512", || id), id);
    /* WHY: 実 face の二記録と stage の一記録を混在させ、合計予算を検査する。 */
    assert_eq!(measure("rejected-513", || id), id);
    record_selected_face(database, Some(id));
}

fn assert_weighted_capture(capture: &Capture, database: &Arc<Database>) -> Result<(), String> {
    assert_record_budget(capture);
    assert_real_sources(capture, database)
}

fn assert_record_budget(capture: &Capture) {
    let retained = (MAX_CAPTURED_RECORDS - RETAINED_STAGES) / RECORDS_PER_BINARY_FACE;
    assert_eq!(capture.faces.len(), retained);
    assert_eq!(capture.selected_database_owners.len(), retained);
    assert_eq!(
        capture
            .stages
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        ["accepted-511", "accepted-512"]
    );
    assert_eq!(
        capture.faces.len() + capture.selected_database_owners.len() + capture.stages.len(),
        MAX_CAPTURED_RECORDS
    );
    assert_eq!(capture.dropped, DROPPED_RECORD_ATTEMPTS);
    assert!(capture.errors.is_empty());
    assert!(capture.database_owners.is_empty());
    assert!(capture.shared_byte_owners.is_empty());
}

fn assert_real_sources(capture: &Capture, database: &Arc<Database>) -> Result<(), String> {
    let face = database.faces().next().ok_or("real Noto face missing")?;
    for record in &capture.faces {
        assert_eq!(record.kind, "Binary");
        assert_eq!(record.index, face.index);
        assert_eq!(record.post_script_name, face.post_script_name);
        assert_eq!(
            record.families,
            face.families
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>()
        );
        assert!(record.path.is_none() && record.stamp.is_none() && record.bytes_sha256.is_none());
    }
    assert!(
        capture
            .selected_database_owners
            .iter()
            .all(|weak| Weak::ptr_eq(weak, &Arc::downgrade(database)))
    );
    Ok(())
}

fn assert_disabled_record_not_retained(database: &Arc<Database>, id: ID) {
    record_selected_face(database, Some(id));
    let (_, fresh) = with_route(SourceRoute::File, Mode::Measure, || {});
    assert!(fresh.faces.is_empty());
    assert!(fresh.selected_database_owners.is_empty());
    assert!(fresh.stages.is_empty());
    assert_eq!(fresh.dropped, 0);
    assert!(!active());
}

fn assert_released_backing(capture: &Capture, owner: &Weak<Database>, bytes: &FontBytesWeak) {
    assert!(owner.upgrade().is_none());
    assert!(bytes.upgrade().is_none());
    assert!(
        capture
            .selected_database_owners
            .iter()
            .all(|owner| owner.upgrade().is_none())
    );
}

#[test]
fn actual_markup_audit_records_resolution_and_capacity() {
    let family = String::from("KRR audit fixture");
    let capture = record_markup_samples(&family);
    assert_eq!(capture.html_font_markups.len(), MAX_CAPTURED_RECORDS);
    assert_eq!(capture.html_font_markups_dropped, 1);
    assert_eq!(
        capture.html_font_markups[..INITIAL_MARKUP_RECORDS]
            .iter()
            .map(|record| record["resolvedRunStatus"].as_str())
            .collect::<Vec<_>>(),
        [Some("unresolved"), Some("empty"), Some("resolved")]
    );
}

fn record_markup_samples(family: &str) -> Capture {
    let family = family.to_owned();
    let ((), capture) = super::with_actual_route(|| {
        let record = |run_families| {
            crate::markdown::svg_rasterize::SvgRasterizeOps::record_actual_html_font_markup(
                "KRR audit fixture",
                REGULAR_FONT_WEIGHT,
                run_families,
                REGULAR_FONT_WEIGHT_STYLE,
            );
        };
        record(None);
        record(Some(&[]));
        record(Some(std::slice::from_ref(&family)));
        for _ in INITIAL_MARKUP_RECORDS..MAX_CAPTURED_RECORDS {
            record(None);
        }
        record(None);
    });
    capture
}

#[test]
fn actual_binary_face_audit_keeps_fallback_evidence_and_ignores_missing_ids() -> Result<(), String>
{
    let (database, id, _) = real_database()?;
    let (_, capture) = super::with_actual_route(|| {
        super::record_actual_selected_face(&database, None);
        super::record_actual_selected_face(&database, Some(ID::dummy()));
        super::record_actual_selected_face(&database, Some(id));
    });
    assert_eq!(capture.faces.len(), 1);
    assert_eq!(capture.faces[0].kind, "Binary");
    assert_eq!(
        capture.errors,
        ["selected font source bytes or stable physical stamp could not be audited"]
    );
    assert_eq!(capture.dropped, 0);
    Ok(())
}

#[test]
fn inner_actual_route_panic_restores_outer_diagnostic_scope() {
    let ((), capture) = with_route(SourceRoute::File, Mode::Audit, || {
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::with_actual_route(|| std::panic::resume_unwind(Box::new("inner audit panic")))
        }));
        assert!(panic.is_err());
        assert!(active());
        assert!(matches!(
            super::ROUTE.with(std::cell::Cell::get),
            Some(SourceRoute::File)
        ));
        assert!(matches!(
            super::MODE.with(std::cell::Cell::get),
            Mode::Audit
        ));
        assert!(!super::PASSIVE_AUDIT.with(std::cell::Cell::get));
        crate::markdown::svg_rasterize::SvgRasterizeOps::record_actual_html_font_markup(
            "scope restored",
            REGULAR_FONT_WEIGHT,
            None,
            REGULAR_FONT_WEIGHT_STYLE,
        );
    });
    assert!(!active());
    assert_eq!(capture.html_font_markups.len(), 1);
    assert_eq!(
        capture.html_font_markups[0]["computedFamily"],
        "scope restored"
    );
}

#[test]
fn missing_active_capture_reports_route_error_and_restores_scope() {
    let cases: [MissingCaptureRouteCase; 2] = [
        (
            missing_regular_route_capture,
            "diagnostic scope missing after operation",
        ),
        (
            missing_actual_route_capture,
            "actual font audit scope missing after operation",
        ),
    ];
    for (run, expected_error) in cases {
        let capture = run();
        assert_eq!(capture.errors, [expected_error]);
        assert_scope_is_restored();
    }
}

fn missing_regular_route_capture() -> Capture {
    let (_, capture) = with_route(SourceRoute::File, Mode::Audit, remove_active_capture);
    capture
}

fn missing_actual_route_capture() -> Capture {
    let (_, capture) = super::with_actual_route(remove_active_capture);
    capture
}

fn remove_active_capture() {
    let _discarded = super::ACTIVE.with(|active| active.take());
}

fn assert_scope_is_restored() {
    assert!(!active());
    assert!(super::ROUTE.with(std::cell::Cell::get).is_none());
    assert!(matches!(
        super::MODE.with(std::cell::Cell::get),
        Mode::Measure
    ));
    assert!(!super::PASSIVE_AUDIT.with(std::cell::Cell::get));
    assert!(super::ACTIVE.with(|active| active.borrow().is_none()));
}

#[path = "svg_rasterize_html_font_lazy_diagnostic_file_audit_tests.rs"]
mod file_audit;
