#![cfg(test)]

use super::{
    catalog_condition, phase_snapshot, HtmlBrowserSession, HtmlBrowserViewport, SessionFrame,
    MILLISECONDS_PER_SECOND,
};
use crate::{options::OperationPlan, test_support::TestSupport};
#[cfg(feature = "baseline")]
use baseline::{HtmlBrowserSource, HtmlRuntime};
#[cfg(feature = "candidate")]
use candidate::{HtmlBrowserSource, HtmlRuntime};
use std::{cell::Cell, time::Instant};

struct ObservedSessionFrame<'session> {
    session: &'session HtmlBrowserSession,
    completed: Cell<Option<Instant>>,
}

impl SessionFrame for ObservedSessionFrame<'_> {
    fn frame_state(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let frame = self.session.frame_state()?;
        self.completed.set(Some(Instant::now()));
        Ok(frame)
    }
}

#[test]
fn phase_snapshot_elapsed_includes_real_frame_acquisition() -> Result<(), Box<dyn std::error::Error>>
{
    let viewport = HtmlBrowserViewport::new(80, 24, 1.0)?;
    let source = HtmlBrowserSource::new(
        "<body style='background:red'>Frame</body>".to_owned(),
        "https://example.test/phase-timing.html",
    )?;
    let mut session = HtmlRuntime.open(source, viewport)?;
    let observed = ObservedSessionFrame {
        session: &session,
        completed: Cell::new(None),
    };
    let start = Instant::now();
    let snapshot = phase_snapshot(start, &observed)?;
    let completed = observed
        .completed
        .get()
        .ok_or("frame completion not observed")?;
    let completed_ms = completed.duration_since(start).as_secs_f64() * MILLISECONDS_PER_SECOND;
    let elapsed_ms = snapshot["elapsed_ms"]
        .as_f64()
        .ok_or("elapsed time missing")?;
    assert!(elapsed_ms >= completed_ms);
    assert_eq!(snapshot["frame"], session.frame_state()?);
    session.close()?;
    Ok(())
}

#[test]
fn catalog_condition_preserves_legacy_null_contract() -> Result<(), Box<dyn std::error::Error>> {
    let plan = serde_json::from_value::<OperationPlan>(serde_json::json!({
        "fixture_sha256": "legacy",
        "operation": "click",
        "x": 1.0,
        "y": 2.0,
        "expected_dom_state_path": "state.json",
        "expected_dom_state_sha256": "state"
    }))?;
    assert_eq!(catalog_condition(&plan), serde_json::Value::Null);
    Ok(())
}

#[test]
fn catalog_condition_preserves_catalog_identity_and_pins() -> Result<(), Box<dyn std::error::Error>>
{
    let plan = serde_json::from_value::<OperationPlan>(TestSupport::catalog_test_plan())?;
    let condition = catalog_condition(&plan);
    assert_eq!(condition["profile"], "catalog-plan84");
    assert_eq!(condition["condition_id"], "async/expected-state/1280x900@1");
    assert_eq!(condition["case_id"], "async");
    assert_eq!(condition["state"], "expected-state");
    assert_eq!(condition["operation_kind"], "script-await-timer-microtask");
    assert_eq!(condition["target_selector"], "#state");
    assert_eq!(condition["operation_evidence_path"], "operation.json");
    assert_eq!(condition["operation_evidence_sha256"], "b".repeat(64));
    assert_eq!(condition["evidence_binary_sha256"], "c".repeat(64));
    Ok(())
}
