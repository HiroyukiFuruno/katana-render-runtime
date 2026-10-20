pub(crate) struct TestSupport;

impl TestSupport {
    pub(crate) fn catalog_test_plan() -> serde_json::Value {
        serde_json::json!({
            "profile": "catalog-plan84",
            "conditionId": "async/expected-state/1280x900@1",
            "caseId": "async",
            "state": "expected-state",
            "operationKind": "script-await-timer-microtask",
            "targetSelector": "#state",
            "fixtureSha256": "a".repeat(64),
            "viewport": {"width": 1280, "height": 900, "scale": 1.0},
            "operationEvidencePath": "operation.json",
            "operationEvidenceSha256": "b".repeat(64),
            "evidenceBinarySha256": "c".repeat(64),
            "operationStream": [],
        })
    }
}
