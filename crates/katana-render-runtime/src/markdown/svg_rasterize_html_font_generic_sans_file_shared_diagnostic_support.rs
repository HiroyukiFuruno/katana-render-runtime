use super::{
    Arm, Capture, EXPECTED_SELECTED_FACES, MAX_OBSERVER_EVENTS, MAX_SERIALIZED_EVENT_BYTES, Mode,
    SourceRoute, validate_shared_drop,
};

const EXPECTED_STAGES: [&str; 10] = [
    "prepare",
    "known_parse_attempt",
    "admission_result",
    "selector",
    "admission_result",
    "selector",
    "admission_result",
    "selector",
    "known_parse_result",
    "parse_route",
];
pub(super) fn validate_observer(events: &[serde_json::Value]) -> Result<(), String> {
    let stages = events
        .iter()
        .map(|event| event["stage"].as_str())
        .collect::<Vec<_>>();
    let expected = EXPECTED_STAGES
        .iter()
        .copied()
        .map(Some)
        .collect::<Vec<_>>();
    if events.len() > MAX_OBSERVER_EVENTS
        || stages != expected
        || events.iter().any(|event| {
            !serde_json::to_vec(event).is_ok_and(|bytes| bytes.len() <= MAX_SERIALIZED_EVENT_BYTES)
        })
    {
        return Err(
            "existing raster-observer stage coverage/order differs from exact fixture".into(),
        );
    }
    Ok(())
}
pub(super) fn validate_capture(mode: Mode, capture: &Capture) -> Result<(), String> {
    if !capture.errors.is_empty()
        || capture.dropped != 0
        || (matches!(mode, Mode::Measure) && capture.stages.is_empty())
        || capture.faces.is_empty()
    {
        return Err(format!(
            "diagnostic capture incomplete: {:?}",
            capture.errors
        ));
    }
    Ok(())
}
pub(super) fn validate_arm(arm: &Arm) -> Result<(), String> {
    if arm.observer_dropped != 0 {
        return Err("existing raster observer dropped events".into());
    }
    validate_known_only_route(&arm.events)?;
    validate_timing_stages(arm)?;
    validate_face_sources(arm)?;
    validate_shared_drop(arm)
}
pub(super) fn validate_timing_stages(arm: &Arm) -> Result<(), String> {
    let names = arm
        .capture
        .stages
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    let setup = if arm.route == SourceRoute::File {
        "file_cached_db_proof"
    } else {
        "shared_transient_proof"
    };
    if names
        != [
            "known_database_lookup",
            setup,
            "lazy_parse_inclusive",
            "render_tree",
        ]
    {
        return Err("required timing stage order differs".into());
    }
    Ok(())
}
pub(super) fn validate_face_sources(arm: &Arm) -> Result<(), String> {
    let wanted = if arm.route == SourceRoute::File {
        "File"
    } else {
        "SharedFile"
    };
    let count = arm
        .capture
        .faces
        .iter()
        .filter(|face| face.kind == wanted)
        .count();
    if count != EXPECTED_SELECTED_FACES
        || arm.capture.faces.len() != EXPECTED_SELECTED_FACES
        || arm.capture.selected_database_owners.len() != EXPECTED_SELECTED_FACES
        || arm.capture.database_owners.len() != 1
    {
        return Err(format!(
            "expected {EXPECTED_SELECTED_FACES} selected {wanted} faces, saw {count}"
        ));
    }
    Ok(())
}
pub(super) fn validate_known_only_route(events: &[serde_json::Value]) -> Result<(), String> {
    if !events
        .iter()
        .any(|event| event["stage"] == "prepare" && event["success"] == true)
        || !events
            .iter()
            .any(|event| event["stage"] == "parse_route" && event["route"] == "known_return")
    {
        return Err("known-only preparation/return evidence missing".into());
    }
    validate_known_parse(events)?;
    validate_selector_events(events)?;
    if events.iter().any(|event| {
        event["stage"] == "fallback"
            || event["route"] == "eager_reparse"
            || event["stage"] == "eager_parse_attempt"
            || event["stage"] == "eager_parse_result"
    }) {
        return Err("eager fallback/reparse observed".into());
    }
    Ok(())
}
pub(super) fn validate_known_parse(events: &[serde_json::Value]) -> Result<(), String> {
    let known = events
        .iter()
        .filter(|event| event["stage"] == "known_parse_result")
        .collect::<Vec<_>>();
    if known.len() != 1
        || known[0]["success"] != true
        || known[0]["valid"] != true
        || known[0]["selected"] != true
    {
        return Err("expected one successful valid known parse".into());
    }
    Ok(())
}
pub(super) fn validate_selector_events(events: &[serde_json::Value]) -> Result<(), String> {
    let selectors = events
        .iter()
        .filter(|event| event["stage"] == "selector")
        .collect::<Vec<_>>();
    let admissions = events
        .iter()
        .filter(|event| event["stage"] == "admission_result")
        .collect::<Vec<_>>();
    if selectors.len() != EXPECTED_SELECTED_FACES || admissions.len() != EXPECTED_SELECTED_FACES {
        return Err("selected-face event count differs from pinned interaction fixture".into());
    }
    if selectors
        .iter()
        .any(|event| event["route"] != "known_probe" || event["admitted"] != true)
        || admissions.iter().any(|event| event["reason"] != "accepted")
    {
        return Err("known-only selector admission failed".into());
    }
    Ok(())
}

pub(super) fn tree_parse_samples(events: &[serde_json::Value]) -> Result<Vec<u64>, String> {
    events
        .iter()
        .filter(|event| event["stage"] == "known_parse_result")
        .map(|event| {
            event["treeParseNs"]
                .as_u64()
                .ok_or_else(|| "treeParseNs is not a nonnegative integer".into())
        })
        .collect()
}

pub(super) fn validate_audit(arm: &Arm) -> Result<(), String> {
    if arm.observer_dropped != 0
        || !arm.capture.stages.is_empty()
        || arm.session_ns != 0
        || arm.close_ns != 0
        || arm.drop_ns != 0
    {
        return Err("audit mode clock/drop contract failed".into());
    }
    validate_known_only_route(&arm.events)?;
    validate_face_sources(arm)?;
    validate_shared_drop(arm)?;
    if arm
        .capture
        .faces
        .iter()
        .any(|face| face.stamp.is_none() || face.bytes_sha256.is_none())
    {
        return Err("audit omitted stable font stamp or bytes SHA".into());
    }
    Ok(())
}
