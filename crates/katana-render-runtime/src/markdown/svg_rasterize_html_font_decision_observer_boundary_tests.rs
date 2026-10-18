use super::{Decision, MAX_DECISIONS, MeasuredDatabase, Scope, record};
use std::sync::Arc;

const FAMILY: &str = "Noto Sans";
const WEIGHT: u16 = 400;

#[test]
fn decision_observer_real_database_retains_128_and_rejects_129_without_strong_ownership()
-> Result<(), String> {
    let selected = real_database()?;
    let observer = Scope::enter();
    assert!(observer.records().is_empty());
    fill_to_limit(&selected);
    let before = observer.records();
    assert_bounded_records(&before, &selected);
    record(&selected, FAMILY, "rejected-over-cap", WEIGHT, false);
    assert_eq!(observer.records().len(), MAX_DECISIONS);
    assert_bounded_records(&observer.records(), &selected);
    assert_nested_restoration(&observer, &selected);
    assert_eq!(Arc::strong_count(&selected.database), 1);
    drop(observer);
    assert_disabled_record_is_not_retained(&selected);
    drop(selected);
    assert!(
        before
            .iter()
            .all(|decision| decision.database.upgrade().is_none())
    );
    Ok(())
}

fn real_database() -> Result<MeasuredDatabase, String> {
    let mut selected = super::super::measured_database(FAMILY, "observer source", WEIGHT, false);
    /* WHY: 実測用 DB の library clone を専有し、他テストの共有 Arc 増減を所有検証へ混ぜない。 */
    selected.database = Arc::new(selected.database.as_ref().clone());
    let id = selected
        .database
        .faces()
        .next()
        .ok_or("real face missing")?
        .id;
    assert_eq!(
        selected
            .database
            .with_face_data(id, |data, _| !data.is_empty()),
        Some(true)
    );
    assert_eq!(Arc::strong_count(&selected.database), 1);
    Ok(selected)
}

fn fill_to_limit(selected: &MeasuredDatabase) {
    for index in 0..MAX_DECISIONS {
        record(
            selected,
            FAMILY,
            &format!("decision-{index}"),
            WEIGHT,
            false,
        );
    }
}

fn assert_bounded_records(records: &[Decision], selected: &MeasuredDatabase) {
    assert_eq!(records.len(), MAX_DECISIONS);
    assert_eq!(
        records.first().map(|record| record.text.as_str()),
        Some("decision-0")
    );
    assert_eq!(
        records.last().map(|record| record.text.clone()),
        Some(format!("decision-{}", MAX_DECISIONS - 1))
    );
    for (index, decision) in records.iter().enumerate() {
        assert_eq!(decision.text, format!("decision-{index}"));
        assert_eq!(decision.key, selected.key);
        assert_eq!(
            (decision.family.as_str(), decision.weight, decision.italic),
            (FAMILY, WEIGHT, false)
        );
        assert!(
            decision
                .database
                .upgrade()
                .is_some_and(|database| Arc::ptr_eq(&database, &selected.database))
        );
    }
    assert_eq!(Arc::strong_count(&selected.database), 1);
}

fn assert_nested_restoration(observer: &Scope, selected: &MeasuredDatabase) {
    let inner = Scope::enter();
    assert!(inner.records().is_empty());
    record(selected, FAMILY, "inner-only", WEIGHT, false);
    assert_eq!(inner.records().len(), 1);
    assert_eq!(inner.records()[0].text, "inner-only");
    drop(inner);
    let restored = observer.records();
    assert_eq!(restored.len(), MAX_DECISIONS);
    assert_bounded_records(&restored, selected);
    assert!(
        restored
            .iter()
            .all(|decision| decision.text != "inner-only" && decision.text != "rejected-over-cap")
    );
}

fn assert_disabled_record_is_not_retained(selected: &MeasuredDatabase) {
    record(selected, FAMILY, "disabled", WEIGHT, false);
    let next = Scope::enter();
    assert!(next.records().is_empty());
    drop(next);
}
