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
type FontBytesWeak = Weak<dyn AsRef<[u8]> + Send + Sync>;
type RealDatabase = (Arc<Database>, ID, FontBytesWeak);

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

#[path = "svg_rasterize_html_font_lazy_diagnostic_file_audit_tests.rs"]
mod file_audit;
