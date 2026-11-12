use super::{Arc, CreationProof, Database, Fixture, Source, font};
use crate::markdown::svg_rasterize::font::html::shared_raster::shared_sources::SharedSources;
use resvg::usvg::fontdb::ID;

#[test]
fn creation_proof_real_directory_and_invalid_font_fail_closed() -> Result<(), String> {
    let fixture = Fixture::new()?;
    assert_rejected_source_path(&fixture.root)?;
    let invalid = fixture.root.join("invalid.ttf");
    std::fs::write(&invalid, b"not a font").map_err(|error| error.to_string())?;
    assert_rejected_source_path(&invalid)?;
    fixture.cleanup()?;
    Ok(())
}

fn assert_rejected_source_path(path: &std::path::Path) -> Result<(), String> {
    let mut database = Database::new();
    let mut proof = CreationProof::default();
    proof.load(
        &mut database,
        &[path.to_str().ok_or("fixture path is not UTF-8")?],
    );
    proof.capture_prefix(&database);
    assert!(proof.failed);
    assert!(database.is_empty());
    assert!(!proof.current(&database));
    assert!(proof.transient(&Arc::new(database)).is_none());
    Ok(())
}

#[test]
fn creation_proof_second_load_rejects_replaced_real_font_generation() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let (mut proof, mut database) = fixture.load()?;
    let old = proof
        .stamps
        .get(&fixture.present)
        .ok_or("initial stamp missing")?
        .clone();
    let replacement = fixture.root.join("replacement.ttf");
    std::fs::write(&replacement, font::BUNDLED_SANS_SERIF_FONT)
        .map_err(|error| error.to_string())?;
    std::fs::rename(replacement, &fixture.present).map_err(|error| error.to_string())?;
    assert!(!proof.current(&database));
    proof.load(
        Arc::make_mut(&mut database),
        &[fixture.present.to_str().ok_or("path not UTF-8")?],
    );
    assert_ne!(proof.stamps.get(&fixture.present), Some(&old));
    assert!(proof.failed);
    assert!(!proof.current(&database));
    assert!(proof.transient(&database).is_none());
    fixture.cleanup()?;
    Ok(())
}

#[test]
fn creation_proof_rejects_font_removed_between_presence_and_identity_checks() -> Result<(), String>
{
    let fixture = Fixture::new()?;
    let mut proof = CreationProof::default();
    assert!(!proof.record_missing(&fixture.present));
    std::fs::remove_file(&fixture.present).map_err(|error| error.to_string())?;
    assert!(!proof.mark_loaded_identity(&fixture.present));
    assert!(proof.failed);
    assert!(proof.absent.is_empty());
    assert!(proof.loaded_paths.is_empty());
    std::fs::write(&fixture.present, font::BUNDLED_SANS_SERIF_FONT)
        .map_err(|error| error.to_string())?;
    let mut database = Database::new();
    proof.load(
        &mut database,
        &[fixture.present.to_str().ok_or("path not UTF-8")?],
    );
    assert!(!database.is_empty());
    assert!(!proof.current(&database));
    assert!(proof.transient(&Arc::new(database)).is_none());
    fixture.cleanup()?;
    Ok(())
}

fn binary_source() -> Source {
    Source::Binary(Arc::new(font::BUNDLED_SANS_SERIF_FONT.to_vec()))
}

type SourceDatabase = (Database, ID);

fn database_for_source(source: Source) -> Result<SourceDatabase, String> {
    let mut parsed = Database::new();
    parsed.load_font_data(font::BUNDLED_SANS_SERIF_FONT.to_vec());
    let mut face = parsed.faces().next().ok_or("bundled face missing")?.clone();
    face.source = source;
    let mut database = Database::new();
    let id = database.push_face_info(face);
    Ok((database, id))
}

#[test]
fn selected_proof_binary_keeps_backing_and_rejects_id_outside_prefix() -> Result<(), String> {
    let source = binary_source();
    let (mut database, id) = database_for_source(source.clone())?;
    let mut proof = CreationProof::default();
    proof.capture_prefix(&database);
    let mut sources = SharedSources::default();
    let selected = proof
        .selected_shared_face(&database, id, &mut sources, None, None)
        .ok_or("real binary face rejected")?;
    assert!(
        database
            .face(id)
            .is_some_and(|face| super::super::same_face(&selected, face))
    );
    assert!(super::super::same_source(&selected.source, &source));
    assert!(!super::super::same_source(
        &selected.source,
        &binary_source()
    ));
    let extra = database.push_face_info(selected);
    assert_ne!(extra, id);
    assert!(!proof.contains(extra));
    assert!(proof.current(&database));
    assert!(
        proof
            .selected_shared_face(&database, extra, &mut sources, None, None)
            .is_none()
    );
    Ok(())
}

#[test]
fn selected_proof_shared_source_input_and_wrong_reuse_kind_fail_closed() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let data = Arc::new(std::fs::read(&fixture.present).map_err(|error| error.to_string())?);
    let (database, id) = database_for_source(Source::SharedFile(fixture.present.clone(), data))?;
    let mut proof = CreationProof::default();
    proof.capture_prefix(&database);
    let mut sources = SharedSources::default();
    assert!(proof.current(&database));
    assert!(
        proof
            .selected_shared_face(&database, id, &mut sources, None, None)
            .is_none()
    );
    assert_wrong_file_reuse(&fixture, &mut sources)?;
    fixture.cleanup()?;
    Ok(())
}

#[test]
fn selected_proof_rejects_file_face_without_creation_stamp() -> Result<(), String> {
    let fixture = Fixture::new()?;
    let mut database = Database::new();
    let id = database
        .load_font_source(Source::File(fixture.present.clone()))
        .first()
        .copied()
        .ok_or("real file face missing")?;
    let mut proof = CreationProof::default();
    proof.capture_prefix(&database);
    assert!(proof.current(&database));
    let mut sources = SharedSources::default();
    assert!(
        proof
            .selected_shared_face(&database, id, &mut sources, None, None)
            .is_none()
    );
    assert!(proof.current(&database));
    fixture.cleanup()?;
    Ok(())
}

fn assert_wrong_file_reuse(fixture: &Fixture, sources: &mut SharedSources) -> Result<(), String> {
    let (proof, database) = fixture.load()?;
    let id = database.faces().next().ok_or("real file face missing")?.id;
    let source = binary_source();
    assert!(
        proof
            .selected_shared_face(&database, id, sources, Some(&source), None)
            .is_none()
    );
    let wrong_path = Source::SharedFile(
        fixture.absent.clone(),
        Arc::new(font::BUNDLED_SANS_SERIF_FONT.to_vec()),
    );
    assert!(
        proof
            .selected_shared_face(&database, id, sources, Some(&wrong_path), None)
            .is_none()
    );
    assert!(proof.current(&database));
    Ok(())
}
