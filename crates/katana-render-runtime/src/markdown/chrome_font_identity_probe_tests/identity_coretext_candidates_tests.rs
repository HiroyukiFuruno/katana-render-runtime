use super::super::super::super::test_support::{TempDirectory, physical_font_fixture};
use super::{PhysicalFace, candidate_files, insert_physical_identity, unique_candidate_face};
use std::{collections::BTreeMap, fs};

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

fn physical_candidate() -> Result<(TempDirectory, PhysicalFace), String> {
    let (temp, database, selected) = physical_font_fixture()?;
    let candidate = candidate_files(&database, &selected)?
        .into_values()
        .next()
        .ok_or("physical font candidate missing")?;
    Ok((temp, candidate))
}

#[test]
fn coretext_candidate_index_requires_one_zero_index() -> Result<(), String> {
    let (_temp, candidate) = physical_candidate()?;
    for (indices, expected_error) in [
        (&[0_u32][..], None),
        (
            &[0_u32, 1][..],
            Some("selected family has ambiguous physical face indices in a font file"),
        ),
        (
            &[1_u32][..],
            Some("CoreText URL descriptors cannot prove nonzero face index"),
        ),
    ] {
        let faces = indices
            .iter()
            .map(|index| {
                let mut face = candidate.clone();
                face.face_index = *index;
                (*index, face)
            })
            .collect::<BTreeMap<_, _>>();
        let result = unique_candidate_face(faces);
        assert_eq!(result.as_ref().err().map(String::as_str), expected_error);
        if expected_error.is_none() {
            assert_eq!(result.ok(), Some(candidate.clone()));
        }
    }
    Ok(())
}

#[test]
fn coretext_candidate_rejects_changed_physical_font_identity() -> Result<(), String> {
    let (_temp, candidate) = physical_candidate()?;
    super::verify_single_font_file(&candidate.path, &candidate)?;
    let mut grouped = BTreeMap::new();
    insert_physical_identity(&mut grouped, candidate.clone())?;
    let mut bytes = fs::read(&candidate.path).map_err(|error| error.to_string())?;
    bytes.push(0);
    fs::write(&candidate.path, bytes).map_err(|error| error.to_string())?;
    let (_, new_digest, _) = super::physical_file(&candidate.path)?;
    assert_ne!(new_digest, candidate.file_sha256);
    let mut updated = candidate.clone();
    updated.file_sha256 = new_digest;
    assert_eq!(
        insert_physical_identity(&mut grouped, updated)
            .err()
            .as_deref(),
        Some("font file changed while indexing physical candidates")
    );
    assert_eq!(
        super::verify_single_font_file(&candidate.path, &candidate)
            .err()
            .as_deref(),
        Some("physical candidate changed before single-font verification")
    );
    Ok(())
}
