use super::super::test_support::{
    TempDirectory, physical_font_fixture, shared_physical_font_fixture,
};
use super::{face_identity, physical_file, supported_font_path, unique_file_face};
use resvg::usvg::fontdb::{Database, Source};
use std::fs;

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
const MEMORY_ONLY_ERROR: &str = "font face is memory-only, not physically attributable";
const NON_FILE_ERROR: &str = "font source is not a regular file";
const SHARED_FILE_CASES: [(&str, bool, Option<&str>); 2] = [
    ("matching bytes", false, None),
    (
        "different backing bytes",
        true,
        Some("shared font bytes differ from the physical file"),
    ),
];
const FONT_PATH_CASES: [(&str, bool); 6] = [
    ("font.ttf", true),
    ("font.OTF", true),
    ("font.ttc", true),
    ("font.otc", true),
    ("font.woff2", false),
    ("font", false),
];

#[test]
fn supported_font_path_matches_supported_extensions_case_insensitively() {
    for (path, expected) in FONT_PATH_CASES {
        assert_eq!(
            supported_font_path(std::path::Path::new(path)),
            expected,
            "{path}"
        );
    }
}

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
fn physical_file_rejects_a_missing_path_before_metadata() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let missing = temp.path().join("missing-font.ttf");
    let error = physical_file(&missing)
        .err()
        .ok_or("missing physical font unexpectedly succeeded")?;
    assert!(error.starts_with("canonicalize font:"));
    Ok(())
}

#[test]
fn physical_font_face_is_bound_to_the_real_file_bytes() -> Result<(), String> {
    let (temp, database, _) = physical_font_fixture()?;
    let path = temp.path().join("physical-font.ttf");
    let face = database
        .faces()
        .next()
        .ok_or("fontdb did not load the font")?;

    let identity = face_identity(&database, face)?;
    let (canonical, digest, _) = physical_file(&path)?;
    assert_eq!(identity.path, canonical);
    assert_eq!(identity.file_sha256, digest);
    assert_eq!(identity.face_index, 0);
    Ok(())
}

#[test]
fn shared_file_identity_requires_matching_physical_bytes() -> Result<(), String> {
    for (case, append_byte, expected_error) in SHARED_FILE_CASES {
        let (temp, database, source_bytes) = shared_physical_font_fixture(append_byte)?;
        assert_shared_file_identity_case(&temp, &database, &source_bytes, case, expected_error)?;
    }
    Ok(())
}

fn assert_shared_file_identity_case(
    temp: &TempDirectory,
    database: &Database,
    source_bytes: &[u8],
    case: &str,
    expected_error: Option<&str>,
) -> Result<(), String> {
    let path = temp.path().join("physical-font.ttf");
    let face = database
        .faces()
        .next()
        .ok_or("SharedFile face is missing")?;
    let Source::SharedFile(source_path, backing_bytes) = &face.source else {
        return Err("fontdb did not retain the SharedFile source".to_string());
    };
    assert_eq!(source_path, &path, "{case}");
    assert_eq!(backing_bytes.as_ref().as_ref(), source_bytes, "{case}");

    let result = face_identity(database, face);
    if let Some(expected_error) = expected_error {
        assert_eq!(result.err().as_deref(), Some(expected_error), "{case}");
    } else {
        let identity = result?;
        let (canonical, digest, _) = physical_file(&path)?;
        assert_eq!(identity.path, canonical, "{case}");
        assert_eq!(identity.file_sha256, digest, "{case}");
        assert_eq!(identity.face_index, 0, "{case}");
    }
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

#[test]
fn fontdb_face_with_unsupported_extension_is_not_a_unique_file_face() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let path = temp.path().join("physical-font.bin");
    fs::write(&path, FONT_BYTES).map_err(|error| error.to_string())?;
    let mut database = Database::new();
    database
        .load_font_file(&path)
        .map_err(|error| error.to_string())?;

    assert_eq!(database.faces().count(), 1);
    assert!(unique_file_face(&database).is_none());
    Ok(())
}
