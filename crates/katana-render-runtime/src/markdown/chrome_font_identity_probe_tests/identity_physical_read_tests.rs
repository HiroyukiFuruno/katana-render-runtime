use super::super::test_support::physical_font_fixture;
use super::{face_identity, verify_read_generation};
use std::fs;
use std::time::{Duration, UNIX_EPOCH};

const FONT_FILENAME: &str = "physical-font.ttf";

#[test]
fn rejects_a_font_that_changes_after_its_initial_stat() -> Result<(), Box<dyn std::error::Error>> {
    let (temp, _, _) = physical_font_fixture()?;
    let path = temp.path().join(FONT_FILENAME);
    let file = fs::OpenOptions::new().write(true).open(&path)?;
    let initial = file.metadata()?;
    verify_read_generation(&path, &initial)?;
    file.set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000_000))?;
    let changed_time = file.metadata()?;
    assert_eq!(initial.len(), changed_time.len());
    assert_ne!(initial.modified()?, changed_time.modified()?);
    assert_changed_font(&path, &initial);
    file.set_len(initial.len() + 1)?;
    assert_changed_font(&path, &changed_time);
    Ok(())
}

fn assert_changed_font(path: &std::path::Path, initial: &fs::Metadata) {
    assert_eq!(
        verify_read_generation(path, initial).err().as_deref(),
        Some("font file changed while hashing")
    );
}

#[test]
fn reports_the_stat_error_when_a_font_disappears_after_reading()
-> Result<(), Box<dyn std::error::Error>> {
    let (temp, database, _) = physical_font_fixture()?;
    let path = temp.path().join(FONT_FILENAME);
    let face = database.faces().next().ok_or("fixture face missing")?;
    let initial = fs::metadata(&path)?;
    assert!(!fs::read(&path)?.is_empty());
    fs::remove_file(&path)?;
    assert!(
        verify_read_generation(&path, &initial)
            .is_err_and(|error| error.starts_with("restat font:"))
    );
    assert!(
        face_identity(&database, face).is_err_and(|error| error.starts_with("canonicalize font:"))
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn reports_the_read_error_when_a_regular_font_is_unreadable()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let (temp, _, _) = physical_font_fixture()?;
    let path = temp.path().join(FONT_FILENAME);
    let original = fs::metadata(&path)?.permissions();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000))?;
    let result = super::physical_file(&path);
    fs::set_permissions(&path, original)?;
    assert!(result.is_err_and(|error| error.starts_with("read font:")));
    Ok(())
}
