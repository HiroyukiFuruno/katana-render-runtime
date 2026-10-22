use super::super::super::test_support::{TempDirectory, physical_font_fixture};
use super::super::PhysicalFace;
use core_foundation::{base::CFType, dictionary::CFDictionary, string::CFString};
use core_text::font_descriptor::new_from_attributes;
use std::fs;

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
const PATH_MISMATCH_ERROR: &str = "exact CoreText descriptor URL differs from its query file";
const IDENTITY_MISMATCH_ERROR: &str =
    "physical candidate changed before CoreText descriptor lookup";
const NAME_MISMATCH_ERROR: &str = "CoreText descriptor PostScript name differs from CDP";

#[test]
fn coretext_descriptor_path_must_match_the_real_query_font() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let query_path = temp.path().join("query.ttf");
    let other_path = temp.path().join("other.ttf");
    fs::write(&query_path, FONT_BYTES).map_err(|error| error.to_string())?;
    fs::write(&other_path, FONT_BYTES).map_err(|error| error.to_string())?;
    let (canonical, file_sha256, _) = super::super::physical_file(&query_path)?;
    let identity = PhysicalFace {
        path: canonical,
        face_index: 0,
        file_sha256,
    };

    assert_eq!(
        super::verify_descriptor_path(&query_path, &identity.path, &identity),
        Ok(())
    );
    assert_eq!(
        super::verify_descriptor_path(&other_path, &identity.path, &identity)
            .err()
            .as_deref(),
        Some(PATH_MISMATCH_ERROR)
    );
    Ok(())
}

#[test]
fn coretext_default_descriptor_url_must_match_query_file() -> Result<(), String> {
    let (_temp, database, selected) = physical_font_fixture()?;
    let face = database
        .faces()
        .next()
        .ok_or("fontdb did not load the real font")?;
    let identity = super::super::face_identity(&database, face)?;
    let attributes: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[]);
    let descriptor = new_from_attributes(&attributes);
    let descriptor_path = descriptor
        .font_path()
        .ok_or("CoreText did not provide its default descriptor URL")?;
    let canonical_descriptor_path =
        fs::canonicalize(&descriptor_path).map_err(|error| error.to_string())?;

    assert_ne!(canonical_descriptor_path.as_path(), identity.path.as_path());

    assert_eq!(
        super::verify_descriptor_binding(
            &descriptor,
            &identity.path,
            &identity,
            &selected.post_script_name,
        )
        .err()
        .as_deref(),
        Some(PATH_MISMATCH_ERROR)
    );
    Ok(())
}

#[test]
fn coretext_rejects_changed_physical_identity_before_lookup() -> Result<(), String> {
    let (_temp, database, selected) = physical_font_fixture()?;
    let face = database
        .faces()
        .next()
        .ok_or("fontdb did not load the real font")?;
    let mut identity = super::super::face_identity(&database, face)?;
    identity.file_sha256 = "0".repeat(64);
    let identity_path = identity.path.clone();
    let error = super::resolve_file(&identity_path, identity, &selected.post_script_name)
        .err()
        .ok_or("changed physical identity must be rejected")?;
    assert_eq!(error, IDENTITY_MISMATCH_ERROR);
    Ok(())
}

#[test]
fn coretext_rejects_descriptor_postscript_name_mismatch() -> Result<(), String> {
    let (temp, _database, _selected) = physical_font_fixture()?;
    let path = temp.path().join("physical-font.ttf");
    let descriptors = super::descriptors_from_url(&path)?;
    let descriptor = descriptors
        .iter()
        .next()
        .ok_or("CoreText returned no descriptor for the fixture font")?;
    let error = super::verify_descriptor_name(&descriptor, "krr-unmatched-postscript-name")
        .err()
        .ok_or("descriptor name mismatch must be rejected")?;
    assert_eq!(error, NAME_MISMATCH_ERROR);
    Ok(())
}
