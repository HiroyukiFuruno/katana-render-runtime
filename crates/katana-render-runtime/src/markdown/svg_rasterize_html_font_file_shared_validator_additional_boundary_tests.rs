use super::super::support::{
    validate_face_sources, validate_selector_events, validate_timing_stages,
};
use super::*;

const TIMING_ERROR: &str = "required timing stage order differs";
const CLOCK_ERROR: &str = "audit mode clock/drop contract failed";
const OWNER_ERROR: &str =
    "SharedFile transient database or byte owner missing or survived close/drop";

fn reject_timing_mutations(actual: &Arm) -> Result<(), String> {
    let mut measured = cloned_arm(actual);
    measured.capture.stages = vec![
        ("file_fixture_database_build", 0),
        ("shared_transient_proof", 0),
        ("lazy_parse_inclusive", 0),
        ("render_tree", 0),
    ];
    validate_timing_stages(&measured)?;
    for edit in [0, 1, 2] {
        let mut changed = cloned_arm(&measured);
        match edit {
            0 => {
                changed.capture.stages.pop();
            }
            1 => changed.capture.stages.swap(1, 2),
            _ => changed.capture.stages[1].0 = "file_cached_db_proof",
        }
        assert_eq!(validate_timing_stages(&changed), Err(TIMING_ERROR.into()));
    }
    Ok(())
}

fn reject_selector_mutations(actual: &Arm) -> Result<(), String> {
    let cases = [
        (
            "selector",
            "route",
            "unexpected",
            "known-only selector admission failed",
        ),
        (
            "admission_result",
            "reason",
            "rejected",
            "known-only selector admission failed",
        ),
    ];
    for (stage, field, value, message) in cases {
        reject_event_field(actual, stage, field, value.into(), message)?;
    }
    let mut changed = actual.events.clone();
    let index = changed
        .iter()
        .position(|event| event["stage"] == "selector")
        .ok_or("selector missing")?;
    changed.remove(index);
    assert_eq!(
        validate_selector_events(&changed),
        Err("selected-face event count differs from pinned interaction fixture".into())
    );
    Ok(())
}

fn reject_observer_order_and_missing_stage(actual: &Arm) -> Result<(), String> {
    let mut reordered = actual.events.clone();
    reordered.swap(0, 1);
    let observer_error = "existing raster-observer stage coverage/order differs from exact fixture";
    assert_eq!(validate_observer(&reordered), Err(observer_error.into()));
    let mut missing = actual.events.clone();
    missing.pop();
    assert_eq!(validate_observer(&missing), Err(observer_error.into()));
    Ok(())
}

fn reject_clock_and_owner_mutations(actual: &Arm) -> Result<(), String> {
    let edits: &[fn(&mut Arm)] = &[
        |arm| arm.session_ns = 1,
        |arm| arm.close_ns = 1,
        |arm| arm.drop_ns = 1,
        |arm| arm.capture.stages.push(("unexpected", 0)),
    ];
    for &edit in edits {
        reject_arm_field(actual, edit, validate_audit, CLOCK_ERROR)?;
    }
    let mut changed = cloned_arm(actual);
    changed.capture.shared_byte_owners.clear();
    assert_eq!(validate_shared_drop(&changed), Err(OWNER_ERROR.into()));
    Ok(())
}

fn reject_face_identity_mutations(actual: &Arm) -> Result<(), String> {
    let original = actual.capture.faces.first().ok_or("audited face missing")?;
    let edits: &[fn(&mut FaceSource)] = &[
        |face| face.path = None,
        |face| face.index = face.index.wrapping_add(1),
        |face| face.post_script_name.push_str("-different"),
    ];
    for edit in edits {
        let mut changed = original.clone();
        edit(&mut changed);
        assert_eq!(
            audit::find_audited_face(&actual.capture, &changed).err(),
            Some("measured face missing from untimed audit".into())
        );
    }
    let mut changed = cloned_arm(actual);
    changed.capture.faces[0].kind = "File".into();
    assert!(validate_face_sources(&changed).is_err());
    Ok(())
}

fn reject_live_database_owners(actual: &Arm) -> Result<(), String> {
    let database = Arc::new(Database::new());
    for selected in [false, true] {
        let mut changed = cloned_arm(actual);
        let owners = if selected {
            &mut changed.capture.selected_database_owners
        } else {
            &mut changed.capture.database_owners
        };
        *owners.first_mut().ok_or("database owner missing")? = Arc::downgrade(&database);
        assert_eq!(validate_shared_drop(&changed), Err(OWNER_ERROR.into()));
    }
    Ok(())
}

fn reject_missing_file_stamp(actual: &Arm) -> Result<(), String> {
    let face = actual.capture.faces.first().ok_or("audited face missing")?;
    let file = OwnedFile::copy_from(face.path.as_deref().ok_or("audited path missing")?)?;
    let capture = owned_file_audit(&file)?;
    let expected = capture.faces.first().ok_or("owned face missing")?;
    std::fs::remove_file(&file.0).map_err(|error| error.to_string())?;
    assert_eq!(
        audit::audit_file_source(&file.0, expected),
        Err("font stamp failed after sample".into())
    );
    Ok(())
}

fn reject_missing_binary_sha(actual: &Arm) -> Result<(), String> {
    let face = actual.capture.faces.first().ok_or("audited face missing")?;
    let mut binary = face.clone();
    binary.path = None;
    binary.bytes_sha256 = None;
    let mut audit_capture = actual.capture.clone();
    audit_capture.faces = vec![binary.clone()];
    assert_eq!(
        audit::audit_one_source(&audit_capture, &binary),
        Err("binary source SHA missing from audit".into())
    );
    Ok(())
}

#[test]
fn actual_shared_audit_rejects_additional_boundary_mutations() -> Result<(), String> {
    let actual = session(SourceRoute::SharedFile, Mode::Audit)?;
    validate_audit(&actual)?;
    reject_timing_mutations(&actual)?;
    reject_selector_mutations(&actual)?;
    reject_observer_order_and_missing_stage(&actual)?;
    reject_clock_and_owner_mutations(&actual)?;
    reject_live_database_owners(&actual)?;
    reject_face_identity_mutations(&actual)?;
    reject_missing_file_stamp(&actual)?;
    reject_missing_binary_sha(&actual)?;
    validate_audit(&actual)
}
