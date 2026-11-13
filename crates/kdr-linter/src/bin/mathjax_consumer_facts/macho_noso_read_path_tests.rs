use super::super::{inspect_file, validate_read_at_path};
use super::write_fixture;
use std::fs;

#[test]
fn read_validation_preserves_the_io_error_when_the_file_disappears()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = write_fixture()?;
    let before = fs::metadata(&fixture.path)?;
    let bytes = fs::read(&fixture.path)?;
    validate_read_at_path(&fixture.path, &before, bytes.len(), before.len())?;
    fs::remove_file(&fixture.path)?;
    let error = fs::metadata(&fixture.path)
        .err()
        .ok_or("file still exists")?;
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        validate_read_at_path(&fixture.path, &before, bytes.len(), before.len()),
        Err(error.to_string())
    );
    Ok(())
}

#[test]
fn inspect_file_rejects_a_valid_macho_reached_via_a_noncanonical_path()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = write_fixture()?;
    let parent = fixture.path.parent().ok_or("missing fixture directory")?;
    let child = parent.join("subdirectory");
    fs::create_dir(&child)?;
    let alias = child
        .join("..")
        .join(fixture.path.file_name().ok_or("missing fixture name")?);
    assert_eq!(alias.canonicalize()?, fixture.path);
    assert_eq!(
        inspect_file(alias, fixture.bytes.len() as u64, &fixture.digest)
            .err()
            .as_deref(),
        Some("path is not a canonical regular file")
    );
    Ok(())
}
