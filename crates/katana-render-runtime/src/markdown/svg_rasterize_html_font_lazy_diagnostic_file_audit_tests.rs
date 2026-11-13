use super::super::super::super::super::font_file_fixture::FONT_BYTES;
use super::*;
use crate::markdown::svg_rasterize::font::html::proof::CreationProof;
use resvg::usvg::fontdb::Source;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
const AUDITED_SELECTIONS: usize = 2;
type FileInput = (Arc<Database>, Arc<CreationProof>, ID);

struct AuditFile(PathBuf);

impl AuditFile {
    fn new() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "krr-lazy-file-audit-{}-{}.ttf",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        let file = Self(path);
        std::io::Write::write_all(&mut output, FONT_BYTES).map_err(|error| error.to_string())?;
        Ok(file)
    }

    fn input(&self) -> Result<FileInput, String> {
        let mut database = Database::new();
        let mut proof = CreationProof::default();
        proof.load(
            &mut database,
            &[self.0.to_str().ok_or("audit path is not UTF-8")?],
        );
        assert_eq!(database.len(), 1);
        let face = database.faces().next().ok_or("actual File face missing")?;
        assert!(matches!(&face.source, Source::File(path) if path == &self.0));
        let id = face.id;
        proof.capture_prefix(&database);
        assert!(proof.current(&database));
        Ok((Arc::new(database), Arc::new(proof), id))
    }
}

impl Drop for AuditFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn assert_empty_missing_ids(database: &Arc<Database>) {
    assert!(database.face(ID::dummy()).is_none());
    let (_, missing) = with_route(SourceRoute::File, Mode::Audit, || {
        record_selected_face(database, None);
        record_selected_face(database, Some(ID::dummy()));
    });
    assert_no_captured_records(&missing);
    assert!(missing.errors.is_empty());
    assert_audit_route_restored();
}

fn assert_no_captured_records(capture: &Capture) {
    assert!(capture.faces.is_empty());
    assert!(capture.database_owners.is_empty());
    assert!(capture.selected_database_owners.is_empty());
    assert!(capture.shared_byte_owners.is_empty());
    assert!(capture.stages.is_empty());
    assert_eq!(capture.dropped, 0);
}

fn assert_audit_route_restored() {
    assert!(active());
    assert!(matches!(
        super::super::MODE.with(std::cell::Cell::get),
        Mode::Audit
    ));
}

fn audited_selection(
    database: &Arc<Database>,
    proof: &Arc<CreationProof>,
    id: ID,
) -> Result<Arc<Database>, String> {
    let (selected, selected_proof) = super::super::select_database(database, Arc::clone(proof))
        .ok_or("valid File audit route rejected")?;
    assert!(Arc::ptr_eq(database, &selected));
    assert!(Arc::ptr_eq(proof, &selected_proof));
    record_selected_face(&selected, Some(id));
    assert_empty_missing_ids(&selected);
    record_selected_face(&selected, Some(id));
    Ok(selected)
}

fn actual_file_digest(file: &AuditFile) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(&file.0).map_err(|error| error.to_string())?;
    assert_eq!(bytes, FONT_BYTES);
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn assert_file_audit_records(
    capture: &Capture,
    file: &AuditFile,
    database: &Database,
    id: ID,
) -> Result<(), String> {
    let face = database.face(id).ok_or("audited metadata missing")?;
    let digest = actual_file_digest(file)?;
    let stamp = crate::markdown::svg_rasterize::text_shaping::stamp_path_uncached(&file.0)
        .map_err(|()| "actual audit File stamp unavailable".to_owned())?;
    assert_eq!(capture.faces.len(), AUDITED_SELECTIONS);
    for record in &capture.faces {
        assert_eq!(record.kind, "File");
        assert_eq!(record.path.as_ref(), Some(&file.0));
        assert_eq!(record.bytes_sha256.as_ref(), Some(&digest));
        assert_eq!(record.stamp.as_ref(), Some(&format!("{stamp:?}")));
        assert_file_metadata(record, face);
    }
    assert!(capture.stages.is_empty() && capture.errors.is_empty());
    assert!(capture.shared_byte_owners.is_empty());
    assert_eq!(capture.dropped, 0);
    Ok(())
}

fn assert_file_metadata(record: &super::super::FaceSource, face: &resvg::usvg::fontdb::FaceInfo) {
    assert_eq!(record.index, face.index);
    assert_eq!(record.post_script_name, face.post_script_name);
    assert_eq!(
        record.families,
        face.families
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(record.style, format!("{:?}", face.style));
    assert_eq!(record.weight, face.weight.0);
    assert_eq!(record.stretch, format!("{:?}", face.stretch));
    assert_eq!(record.monospaced, face.monospaced);
}

fn assert_file_audit_owners(capture: &Capture, owner: &Weak<Database>) {
    assert_eq!(capture.database_owners.len(), 1);
    assert_eq!(capture.selected_database_owners.len(), AUDITED_SELECTIONS);
    for retained in capture
        .database_owners
        .iter()
        .chain(&capture.selected_database_owners)
    {
        assert!(Weak::ptr_eq(retained, owner));
        assert!(retained.upgrade().is_some());
    }
}

#[test]
fn measured_routes_record_the_stage_for_the_selected_font_source() -> Result<(), String> {
    let file = AuditFile::new()?;
    let (database, proof, _) = file.input()?;
    for (route, expected_stage) in [
        (SourceRoute::File, "file_cached_db_proof"),
        (SourceRoute::SharedFile, "shared_transient_proof"),
    ] {
        let (selection, capture) = with_route(route, Mode::Measure, || {
            super::super::select_database(&database, Arc::clone(&proof))
        });
        let (selected, selected_proof) = selection.ok_or("font route setup failed")?;
        assert!(selected_proof.current(&selected));
        assert_eq!(
            capture
                .stages
                .iter()
                .map(|stage| stage.0)
                .collect::<Vec<_>>(),
            [expected_stage]
        );
        assert!(capture.errors.is_empty());
    }
    Ok(())
}

#[test]
fn lazy_audit_records_real_file_digest_and_ignores_missing_local_id() -> Result<(), String> {
    let file = AuditFile::new()?;
    let (database, proof, id) = file.input()?;
    let owner = Arc::downgrade(&database);
    assert!(!active());
    let (selected, capture) = with_route(SourceRoute::File, Mode::Audit, || {
        audited_selection(&database, &proof, id)
    });
    let selected = selected?;
    assert!(!active());
    assert_file_audit_records(&capture, &file, &database, id)?;
    assert_file_audit_owners(&capture, &owner);
    drop(selected);
    assert_eq!(Arc::strong_count(&database), 1);
    drop(database);
    assert!(owner.upgrade().is_none());
    assert!(
        capture
            .database_owners
            .iter()
            .chain(&capture.selected_database_owners)
            .all(|retained| retained.upgrade().is_none())
    );
    Ok(())
}

#[test]
fn lazy_audit_stale_file_rejects_route_before_setup() -> Result<(), String> {
    let file = AuditFile::new()?;
    let (database, proof, _) = file.input()?;
    let owner = Arc::downgrade(&database);
    /* WHY: 実ファイルの世代を直列に変え、raceや架空stampで失効を模擬しない。 */
    std::fs::write(&file.0, b"invalid font generation").map_err(|error| error.to_string())?;
    assert!(!proof.current(&database));
    let (selected, capture) = with_route(SourceRoute::File, Mode::Audit, || {
        super::super::select_database(&database, proof)
    });
    assert!(selected.is_none());
    assert_eq!(capture.errors, ["CreationProof invalid before route setup"]);
    assert_no_captured_records(&capture);
    assert!(!active());
    assert_eq!(Arc::strong_count(&database), 1);
    drop(database);
    assert!(owner.upgrade().is_none());
    Ok(())
}

#[test]
fn lazy_audit_does_not_record_a_selection_that_became_stale() -> Result<(), String> {
    let file = AuditFile::new()?;
    let (database, proof, _) = file.input()?;
    assert!(proof.current(&database));
    std::fs::write(&file.0, b"replacement after selection").map_err(|error| error.to_string())?;

    let (selected, capture) = with_route(SourceRoute::File, Mode::Audit, || {
        super::super::routes::record_database_selection((database, proof))
    });
    assert!(selected.is_none());
    assert_eq!(capture.errors, ["CreationProof invalid after route setup"]);
    assert_no_captured_records(&capture);
    Ok(())
}

#[test]
fn diagnostic_file_fixture_rejects_replaced_font_without_retaining_database() -> Result<(), String>
{
    let file = AuditFile::new()?;
    let (database, proof, _) = file.input()?;
    let validate = super::super::routes::validate_file_fixture_database;
    let owner = Arc::downgrade(&database);
    let valid = validate(Arc::clone(&database), Arc::clone(&proof))
        .ok_or("unchanged diagnostic database rejected")?;
    assert!(Arc::ptr_eq(&valid.database, &database));
    drop(valid);
    std::fs::write(&file.0, b"replaced font").map_err(|error| error.to_string())?;
    assert!(validate(database, proof).is_none());
    assert!(owner.upgrade().is_none());
    Ok(())
}

#[test]
fn lazy_audit_rejects_a_file_removed_after_route_selection() -> Result<(), String> {
    let file = AuditFile::new()?;
    let (database, proof, id) = file.input()?;
    let (selected, capture) = with_route(SourceRoute::File, Mode::Audit, || {
        let (selected, _) = super::super::select_database(&database, proof)
            .ok_or("valid File audit route rejected")?;
        std::fs::remove_file(&file.0).map_err(|error| error.to_string())?;
        record_selected_face(&selected, Some(id));
        Ok::<_, String>(selected)
    });
    assert!(Arc::ptr_eq(&selected?, &database));
    assert_eq!(
        capture.errors,
        ["selected font source could not be audited"]
    );
    assert_eq!(capture.database_owners.len(), 1);
    assert!(capture.faces.is_empty());
    assert!(capture.selected_database_owners.is_empty());
    assert!(capture.shared_byte_owners.is_empty());
    assert_eq!(capture.dropped, 0);
    assert!(!active());
    Ok(())
}
