use super::super::test_support::TempDirectory;
use super::{face_identity, physical_file, unique_file_face};
use resvg::usvg::fontdb::Database;
use std::fs;

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
const MEMORY_ONLY_ERROR: &str = "font face is memory-only, not physically attributable";
const NON_FILE_ERROR: &str = "font source is not a regular file";

#[test]
fn physical_file_rejects_an_existing_directory() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    assert_eq!(
        physical_file(temp.path()).err().as_deref(),
        Some(NON_FILE_ERROR)
    );
    Ok(())
}

#[test]
fn binary_font_has_no_physical_identity_or_unique_file_face() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let face = database
        .faces()
        .next()
        .ok_or("bundled font has no parsed face")?;

    assert_eq!(
        face_identity(&database, face).err().as_deref(),
        Some(MEMORY_ONLY_ERROR)
    );
    assert!(unique_file_face(&database).is_none());
    Ok(())
}

#[test]
fn repeated_postscript_name_is_not_a_unique_physical_face() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let first_path = temp.path().join("first-copy.ttf");
    let second_path = temp.path().join("second-copy.ttf");
    fs::write(&first_path, FONT_BYTES).map_err(|error| error.to_string())?;
    fs::write(&second_path, FONT_BYTES).map_err(|error| error.to_string())?;
    let mut database = Database::new();
    database
        .load_font_file(&first_path)
        .map_err(|error| error.to_string())?;
    database
        .load_font_file(&second_path)
        .map_err(|error| error.to_string())?;

    assert_eq!(database.faces().count(), 2);
    assert!(unique_file_face(&database).is_none());
    Ok(())
}
