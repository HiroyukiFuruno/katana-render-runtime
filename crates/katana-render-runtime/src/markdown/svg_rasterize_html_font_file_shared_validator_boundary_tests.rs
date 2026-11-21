use super::*;
use crate::markdown::svg_rasterize::font::html::proof::CreationProof;
use resvg::usvg::fontdb::{Database, ID};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
type ArmValidator = fn(&Arm) -> Result<(), String>;
type LibraryBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

#[path = "svg_rasterize_html_font_file_shared_validator_additional_boundary_tests.rs"]
mod additional_boundaries;

fn cloned_arm(actual: &Arm) -> Arm {
    Arm {
        route: actual.route,
        pixels: actual.pixels.clone(),
        pixel_sha256: actual.pixel_sha256.clone(),
        events: actual.events.clone(),
        observer_dropped: actual.observer_dropped,
        session_ns: actual.session_ns,
        close_ns: actual.close_ns,
        drop_ns: actual.drop_ns,
        capture: actual.capture.clone(),
    }
}

fn reject_arm_field(
    actual: &Arm,
    edit: impl FnOnce(&mut Arm),
    validator: ArmValidator,
    message: &str,
) -> Result<(), String> {
    let mut changed = cloned_arm(actual);
    edit(&mut changed);
    assert_eq!(validator(&changed), Err(message.to_owned()));
    validate_audit(actual)
}

fn reject_capture_field(
    actual: &Arm,
    edit: impl FnOnce(&mut Capture),
    message: &str,
) -> Result<(), String> {
    let mut changed = actual.capture.clone();
    edit(&mut changed);
    assert_eq!(
        validate_capture(Mode::Audit, &changed),
        Err(message.to_owned())
    );
    validate_capture(Mode::Audit, &actual.capture)
}

fn reject_capture_provenance(actual: &Arm) -> Result<(), String> {
    reject_arm_field(
        actual,
        |arm| arm.observer_dropped = 1,
        validate_audit,
        "audit mode clock/drop contract failed",
    )?;
    reject_arm_field(
        actual,
        |arm| arm.capture.faces[0].stamp = None,
        validate_audit,
        "audit omitted stable font stamp or bytes SHA",
    )?;
    reject_arm_field(
        actual,
        |arm| arm.capture.faces[0].bytes_sha256 = None,
        validate_audit,
        "audit omitted stable font stamp or bytes SHA",
    )?;
    reject_capture_field(
        actual,
        |capture| capture.faces.clear(),
        "diagnostic capture incomplete: []",
    )?;
    reject_capture_field(
        actual,
        |capture| capture.dropped = 1,
        "diagnostic capture incomplete: []",
    )
}

fn reject_event_field(
    actual: &Arm,
    stage: &str,
    field: &str,
    value: serde_json::Value,
    message: &str,
) -> Result<(), String> {
    let mut changed = cloned_arm(actual);
    let event = changed
        .events
        .iter_mut()
        .find(|event| event["stage"] == stage)
        .ok_or("accepted actual event missing")?;
    event[field] = value;
    assert_eq!(validate_audit(&changed), Err(message.to_owned()));
    validate_audit(actual)
}

fn reject_runtime_evidence(actual: &Arm) -> Result<(), String> {
    reject_event_field(
        actual,
        "prepare",
        "success",
        false.into(),
        "known-only preparation/return evidence missing",
    )?;
    reject_event_field(
        actual,
        "known_parse_result",
        "valid",
        false.into(),
        "expected one successful valid known parse",
    )?;
    reject_event_field(
        actual,
        "selector",
        "admitted",
        false.into(),
        "known-only selector admission failed",
    )?;
    reject_event_field(
        actual,
        "known_parse_result",
        "route",
        "eager_reparse".into(),
        "eager fallback/reparse observed",
    )
}

fn reject_observer_envelope(actual: &Arm) -> Result<(), String> {
    let message = "existing raster-observer stage coverage/order differs from exact fixture";
    let mut reordered = actual.events.clone();
    reordered
        .first_mut()
        .ok_or("actual prepare event missing")?["stage"] = "missing_prepare".into();
    assert_eq!(validate_observer(&reordered), Err(message.to_owned()));
    let mut oversized = actual.events.clone();
    oversized
        .first_mut()
        .ok_or("actual prepare event missing")?["extra"] =
        "x".repeat(MAX_SERIALIZED_EVENT_BYTES).into();
    assert_eq!(validate_observer(&oversized), Err(message.to_owned()));
    validate_observer(&actual.events)
}

fn real_mapping(actual: &Arm) -> Result<LibraryBytes, String> {
    let path = actual
        .capture
        .faces
        .first()
        .and_then(|face| face.path.as_ref())
        .ok_or("actual audited SharedFile path missing")?;
    let source = fontique::SourceInfo::new(
        fontique::SourceId::new(),
        fontique::SourceKind::Path(Arc::from(path.as_path())),
    );
    fontique::SourceCache::default()
        .get(&source)
        .map(|blob| blob.into_raw_parts().0)
        .ok_or_else(|| "actual library mapping unavailable".into())
}

fn reject_live_owner_then_accept_drop(actual: &Arm) -> Result<(), String> {
    let data = real_mapping(actual)?;
    let owner = Arc::downgrade(&data);
    let mut changed = cloned_arm(actual);
    *changed
        .capture
        .shared_byte_owners
        .first_mut()
        .ok_or("actual mapping Weak missing")? = owner.clone();
    assert!(owner.upgrade().is_some());
    assert_eq!(
        validate_shared_drop(&changed),
        Err("SharedFile transient database or byte owner missing or survived close/drop".into())
    );
    drop(data);
    assert!(owner.upgrade().is_none());
    validate_shared_drop(&changed)?;
    validate_audit(actual)
}

struct OwnedFile(PathBuf);
impl OwnedFile {
    fn copy_from(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let path = std::env::temp_dir().join(format!(
            "krr-validator-source-{}-{}.ttf",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        let owned = Self(path);
        let written = std::io::Write::write_all(&mut output, &bytes);
        drop(output);
        written.map_err(|error| error.to_string())?;
        Ok(owned)
    }
}
impl Drop for OwnedFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn owned_file_audit(file: &OwnedFile) -> Result<Capture, String> {
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    proof.load(
        &mut database,
        &[file.0.to_str().ok_or("owned path is not UTF-8")?],
    );
    let id: ID = database
        .faces()
        .next()
        .ok_or("owned real File face missing")?
        .id;
    proof.capture_prefix(&database);
    let database = Arc::new(database);
    let (selected, capture) = lazy_diagnostic::with_route(SourceRoute::File, Mode::Audit, || {
        let (selected, _) = lazy_diagnostic::select_database(&database, Arc::new(proof))
            .ok_or("owned real File audit route rejected")?;
        lazy_diagnostic::record_selected_face(&selected, Some(id));
        Ok::<_, String>(selected)
    });
    drop(selected?);
    drop(database);
    validate_capture(Mode::Audit, &capture)?;
    let face = capture
        .faces
        .first()
        .ok_or("actual owned File audit missing")?;
    audit::audit_one_source(&capture, face)?;
    Ok(capture)
}

fn reject_actual_file_update(actual: &Arm) -> Result<(), String> {
    let path = actual
        .capture
        .faces
        .first()
        .and_then(|face| face.path.as_ref())
        .ok_or("actual audited File path missing")?;
    let file = OwnedFile::copy_from(path)?;
    let capture = owned_file_audit(&file)?;
    let expected = capture
        .faces
        .first()
        .ok_or("actual owned File record missing")?;
    std::fs::write(&file.0, b"changed actual owned font generation")
        .map_err(|error| error.to_string())?;
    assert_eq!(
        audit::audit_file_source(&file.0, expected),
        Err(format!("font stamp changed: {}", file.0.display()))
    );
    let stamp = crate::markdown::svg_rasterize::text_shaping::stamp_path_uncached(&file.0)
        .map_err(|()| "changed owned File stamp unavailable".to_owned())?;
    /* WHY: 新しい実stampと古い実digestの組合せはconsumer負例であり、有効proofにしない。 */
    let mut changed = expected.clone();
    changed.stamp = Some(format!("{stamp:?}"));
    assert_eq!(
        audit::audit_file_source(&file.0, &changed),
        Err(format!("font bytes changed: {}", file.0.display()))
    );
    Ok(())
}

#[test]
fn actual_file_shared_capture_validators_reject_missing_provenance_and_live_owners()
-> Result<(), String> {
    let file = session(SourceRoute::File, Mode::Audit)?;
    let shared = session(SourceRoute::SharedFile, Mode::Audit)?;
    for actual in [&file, &shared] {
        validate_audit(actual)?;
        validate_observer(&actual.events)?;
        validate_capture(Mode::Audit, &actual.capture)?;
        reject_capture_provenance(actual)?;
        reject_runtime_evidence(actual)?;
        reject_observer_envelope(actual)?;
    }
    reject_live_owner_then_accept_drop(&shared)?;
    reject_actual_file_update(&file)?;
    validate_audit(&file)?;
    validate_audit(&shared)?;
    Ok(())
}
