use super::super::super::super::test_support::physical_font_fixture;
use super::candidate_files;
use std::fs;

const MISSING_FAMILY_ERROR: &str =
    "selected family has no uniquely indexed physical font file in fontdb";

#[test]
fn coretext_candidate_files_match_a_real_fontdb_family() -> Result<(), String> {
    let (temp, database, selected) = physical_font_fixture()?;
    let candidates = candidate_files(&database, &selected)?;
    let expected_path =
        fs::canonicalize(temp.path().join("physical-font.ttf")).map_err(|e| e.to_string())?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates.keys().next(), Some(&expected_path));
    Ok(())
}

#[test]
fn coretext_candidate_files_reject_an_unselected_family() -> Result<(), String> {
    let (_temp, database, mut selected) = physical_font_fixture()?;
    selected.family_name = "family absent from this font file".to_string();

    assert_eq!(
        candidate_files(&database, &selected).err().as_deref(),
        Some(MISSING_FAMILY_ERROR)
    );
    Ok(())
}

#[test]
fn coretext_single_font_validation_rejects_a_nonzero_real_face_index() {
    let error = super::verify_single_font_data(
        include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf"),
        1,
    )
    .err();
    assert_eq!(
        error.as_deref(),
        Some("CoreText URL descriptors cannot prove a nonzero collection face index")
    );
}
