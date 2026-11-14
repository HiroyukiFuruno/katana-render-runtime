use super::super::diagnostic_binary_fixture::real_database;
use super::{
    MAX_EVENTS, Scope, TreeParseTiming, admission_result, eager_attempt, eager_reparse,
    eager_result, install_eager, known_attempt, known_fallback, known_result, known_return,
    prepare_ascii_unavailable, prepare_known_unavailable, prepare_ready, prepare_unavailable,
    reject_family, reject_stretch, reject_style,
};
use crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT;
use resvg::usvg::fontdb::{Database, ID};
use std::sync::{Arc, Weak};

type FontBytesWeak = Weak<dyn AsRef<[u8]> + Send + Sync>;
type RealDatabase = (Arc<Database>, ID, FontBytesWeak);

#[path = "svg_rasterize_html_font_raster_observer_styled_tests.rs"]
mod styled_tests;

#[test]
fn raster_observer_real_binary_face_retains_256_and_drops_257() -> Result<(), String> {
    let (database, id, bytes) = real_database()?;
    let owner = Arc::downgrade(&database);
    let observer = Scope::enter();
    for _ in 0..MAX_EVENTS {
        admission_result(&database, Some(id), Some(id));
    }
    let before = observer.snapshot();
    assert_real_face_events(&before, id);
    admission_result(&database, Some(id), Some(id));
    assert_eq!(observer.snapshot().events, before.events);
    assert_eq!(observer.snapshot().dropped, 1);
    assert_nested_restoration(&observer, &database, id);
    assert_eq!(Arc::strong_count(&database), 1);
    drop(observer);
    assert_disabled_record_is_not_retained(&database, id);
    drop(database);
    assert!(owner.upgrade().is_none());
    assert!(bytes.upgrade().is_none());
    assert_eq!(before.events.len(), MAX_EVENTS);
    Ok(())
}

fn assert_real_face_events(snapshot: &super::Snapshot, id: ID) {
    assert_eq!(snapshot.events.len(), MAX_EVENTS);
    assert_eq!(snapshot.dropped, 0);
    let expected_id = format!("{id:?}");
    for event in &snapshot.events {
        assert_eq!(event["stage"], "admission_result");
        assert_eq!(event["reason"], "accepted");
        assert_eq!(event["expectedFace"]["sourceKind"], "Binary");
        assert_eq!(
            event["expectedFace"]["databaseFaceIdLocalOnly"],
            expected_id
        );
        assert!(
            event["expectedFace"]["families"]
                .as_array()
                .is_some_and(|families| !families.is_empty())
        );
    }
}

fn assert_nested_restoration(observer: &Scope, database: &Database, id: ID) {
    std::thread::scope(|threads| {
        threads.spawn(|| {
            super::record_work("isolated-font", BUNDLED_SANS_SERIF_FONT);
            let snapshot = observer.snapshot();
            assert!(snapshot.events.is_empty() && snapshot.complete());
        });
    });
    let inner = Scope::enter();
    admission_result(database, Some(id), Some(id));
    assert_eq!(inner.snapshot().events.len(), 1);
    assert_eq!(inner.snapshot().dropped, 0);
    drop(inner);
    assert_eq!(observer.snapshot().events.len(), MAX_EVENTS);
    assert_eq!(observer.snapshot().dropped, 1);
}

fn assert_disabled_record_is_not_retained(database: &Database, id: ID) {
    admission_result(database, Some(id), Some(id));
    let next = Scope::enter();
    assert!(next.snapshot().events.is_empty());
    assert_eq!(next.snapshot().dropped, 0);
    drop(next);
}

#[test]
fn raster_observer_real_event_accepts_json_limit_and_rejects_next_byte() -> Result<(), String> {
    let (database, id, bytes) = sized_real_database(super::MAX_EVENT_JSON_BYTES)?;
    let owner = Arc::downgrade(&database);
    let observer = Scope::enter();
    admission_result(&database, Some(id), Some(id));
    let accepted = observer.snapshot();
    assert_eq!(accepted.events.len(), 1);
    assert_eq!(accepted.dropped, 0);
    assert_eq!(
        accepted.events[0].to_string().len(),
        super::MAX_EVENT_JSON_BYTES
    );
    let overflow = overflow_database(&database, id)?;
    assert_overflow_event_size(&accepted.events[0], &overflow, id)?;
    admission_result(&overflow, Some(id), Some(id));
    assert_eq!(observer.snapshot().events, accepted.events);
    assert_eq!(observer.snapshot().dropped, 1);
    assert_nested_json_boundary(&observer, &database, &overflow, id);
    drop(observer);
    assert_disabled_record_is_not_retained(&overflow, id);
    drop(overflow);
    drop(database);
    assert!(owner.upgrade().is_none());
    assert!(bytes.upgrade().is_none());
    assert_eq!(accepted.events[0]["reason"], "accepted");
    Ok(())
}

fn captured_real_event(database: &Database, id: ID) -> Result<serde_json::Value, String> {
    let observer = Scope::enter();
    admission_result(database, Some(id), Some(id));
    let snapshot = observer.snapshot();
    assert_eq!(snapshot.dropped, 0);
    snapshot
        .events
        .into_iter()
        .next()
        .ok_or_else(|| "actual face event missing".into())
}

fn sized_real_database(limit: usize) -> Result<RealDatabase, String> {
    let (original, id, bytes) = real_database()?;
    let mut face = original.face(id).ok_or("actual face missing")?.clone();
    let family = face.families.first_mut().ok_or("actual family missing")?;
    family.0.clear();
    let mut baseline = Database::new();
    let baseline_id = baseline.push_face_info(face.clone());
    let size = captured_real_event(&baseline, baseline_id)?
        .to_string()
        .len();
    /* WHY: serializer の実サイズから容量を決め、escape や ID 表記を推測しない。 */
    face.families.first_mut().ok_or("actual family missing")?.0 =
        "x".repeat(limit.checked_sub(size).ok_or("event exceeds test limit")?);
    let mut database = Database::new();
    let id = database.push_face_info(face);
    assert_eq!(
        database.with_face_data(id, |data, _| data == BUNDLED_SANS_SERIF_FONT),
        Some(true)
    );
    Ok((Arc::new(database), id, bytes))
}

fn overflow_database(database: &Database, id: ID) -> Result<Arc<Database>, String> {
    let mut face = database.face(id).ok_or("actual face missing")?.clone();
    face.families
        .first_mut()
        .ok_or("actual family missing")?
        .0
        .push('x');
    let mut overflow = Database::new();
    assert_eq!(overflow.push_face_info(face), id);
    Ok(Arc::new(overflow))
}

fn assert_overflow_event_size(
    accepted: &serde_json::Value,
    database: &Database,
    id: ID,
) -> Result<(), String> {
    let face = database.face(id).ok_or("overflow face missing")?;
    let family = &face.families.first().ok_or("overflow family missing")?.0;
    let mut enlarged = accepted.clone();
    /* WHY: 実 admission の JSON を基に、実 DB の差分一文字だけを対照する。 */
    enlarged["expectedFace"]["families"][0] = family.clone().into();
    assert_eq!(enlarged.to_string().len(), super::MAX_EVENT_JSON_BYTES + 1);
    assert_eq!(enlarged["expectedFace"]["sourceKind"], "Binary");
    assert_eq!(
        database.with_face_data(id, |data, _| data == BUNDLED_SANS_SERIF_FONT),
        Some(true)
    );
    Ok(())
}

fn assert_nested_json_boundary(observer: &Scope, database: &Database, overflow: &Database, id: ID) {
    let outer = observer.snapshot();
    let inner = Scope::enter();
    admission_result(overflow, Some(id), Some(id));
    assert!(inner.snapshot().events.is_empty());
    assert_eq!(inner.snapshot().dropped, 1);
    admission_result(database, Some(id), Some(id));
    assert_eq!(inner.snapshot().events.len(), 1);
    assert_eq!(inner.snapshot().dropped, 1);
    drop(inner);
    assert_eq!(observer.snapshot().events, outer.events);
    assert_eq!(observer.snapshot().dropped, outer.dropped);
}

#[test]
fn event_serialization_records_parse_and_admission_boundary_events() -> Result<(), String> {
    let observer = Scope::enter();
    let valid = std::sync::atomic::AtomicBool::new(true);
    let selected = std::sync::atomic::AtomicBool::new(false);
    let timing = TreeParseTiming::default();

    known_result(false, &valid, &selected, &timing);
    eager_reparse(&valid, &selected);
    known_attempt();
    eager_attempt();
    known_return();
    eager_result(true);
    prepare_unavailable();
    prepare_ascii_unavailable();
    prepare_known_unavailable();
    prepare_ready();
    reject_family();
    reject_stretch();
    reject_style();

    let events = observer.snapshot().events;
    assert_eq!(events.len(), 13);
    assert_eq!(events[0]["stage"], "known_parse_result");
    assert_eq!(events[1]["route"], "eager_reparse");
    assert_eq!(events[5]["success"], true);
    assert_eq!(events[12]["reason"], "style_not_normal_or_italic");
    Ok(())
}

#[test]
fn raster_observer_records_real_known_fallback_and_eager_fallback() -> Result<(), String> {
    let (mut database, id, _bytes) = real_database()?;
    let observer = Scope::enter();
    known_fallback('A', &[id], &database, None);

    let mut options = resvg::usvg::Options::default();
    install_eager(&mut options);
    let _ = (options.font_resolver.select_fallback)('A', &[id], &mut database);

    let events = observer.snapshot().events;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["route"], "known_probe");
    assert_eq!(events[1]["route"], "eager");
    Ok(())
}
