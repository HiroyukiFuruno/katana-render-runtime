use super::super::super::test_support::physical_font_fixture;
use super::super::face_identity;
use super::{descriptor_path_text, descriptors_from_url, verify_candidate_after_lookup};
use std::{fs, io::Write, path::Path};

#[test]
fn coretext_discards_descriptors_if_the_font_changes_after_lookup()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, database, _) = physical_font_fixture()?;
    let face = database.faces().next().ok_or("fixture face missing")?;
    let identity = face_identity(&database, face)?;
    let descriptors = descriptors_from_url(&identity.path)?;
    assert!(!descriptors.is_empty());
    verify_candidate_after_lookup(&identity.path, &identity)?;

    fs::OpenOptions::new()
        .append(true)
        .open(&identity.path)?
        .write_all(&[0])?;
    assert_eq!(
        verify_candidate_after_lookup(&identity.path, &identity)
            .err()
            .as_deref(),
        Some("physical font changed during CoreText descriptor lookup")
    );
    Ok(())
}

#[test]
fn coretext_does_not_encode_a_descriptor_path_lossily() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    let path = Path::new(OsStr::from_bytes(b"/font-\xff.ttf"));
    assert_eq!(
        descriptor_path_text(path).err().as_deref(),
        Some("exact CoreText descriptor URL is not valid UTF-8")
    );
}
