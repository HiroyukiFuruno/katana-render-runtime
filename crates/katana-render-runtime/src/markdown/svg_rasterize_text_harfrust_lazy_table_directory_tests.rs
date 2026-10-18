use super::{FontFaceCacheScope, font_for_face};
use resvg::usvg::fontdb::{Database, Source};
use std::path::{Path, PathBuf};

const EMBEDDED_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

fn temp_path(label: &str, extension: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "krr-lazy-directory-{label}-{}-{stamp}.{extension}",
        std::process::id()
    ))
}

fn database_with_replaced_file(
    valid_source: Source,
    source_index: u32,
    replacement: &Path,
    requested_index: u32,
) -> Result<(Database, resvg::usvg::fontdb::ID), String> {
    let mut source_database = Database::new();
    match valid_source {
        Source::File(path) => {
            source_database.load_font_source(Source::File(path));
        }
        Source::Binary(data) => {
            source_database.load_font_data(data.as_ref().as_ref().to_vec());
        }
        Source::SharedFile(_, _) => return Err("shared source is not a draft input".to_string()),
    }
    let mut info = source_database
        .faces()
        .find(|face| face.index == source_index)
        .cloned()
        .ok_or_else(|| format!("source face {source_index} missing"))?;
    info.source = Source::File(replacement.to_path_buf());
    info.index = requested_index;
    let mut database = Database::new();
    let face = database.push_face_info(info);
    Ok((database, face))
}

fn assert_rejected(database: &Database, face: resvg::usvg::fontdb::ID) {
    let _scope = FontFaceCacheScope::enter();
    assert!(font_for_face(database, face).is_none());
    assert!(super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
}

#[cfg(target_os = "macos")]
fn real_ttc_bytes() -> Result<Vec<u8>, String> {
    let source = Path::new("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc");
    if !source.exists() {
        return Err("expected Hiragino W3 TTC path".to_string());
    }
    std::fs::read(source).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn ttc_database(
    path: &Path,
    requested_index: u32,
) -> Result<(Database, resvg::usvg::fontdb::ID), String> {
    database_with_replaced_file(
        Source::File(Path::new("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc").to_path_buf()),
        0,
        path,
        requested_index,
    )
}

#[cfg(target_os = "macos")]
fn assert_rejected_ttc_directory_case(
    label: &str,
    range: std::ops::Range<usize>,
    replacement: &[u8],
    requested_index: u32,
) -> Result<(), String> {
    let path = temp_path(label, "ttc");
    let mut bytes = real_ttc_bytes()?;
    bytes[range].copy_from_slice(replacement);
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    let (database, face) = ttc_database(&path, requested_index)?;
    assert_rejected(&database, face);
    std::fs::remove_file(path).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
#[test]
fn unsupported_ttc_version_is_rejected_without_cache_entry() -> Result<(), String> {
    assert_rejected_ttc_directory_case("bad-version", 4..8, &[0, 3, 0, 0], 0)
}

#[cfg(target_os = "macos")]
#[test]
fn zero_ttc_face_count_is_rejected_without_cache_entry() -> Result<(), String> {
    assert_rejected_ttc_directory_case("zero-count", 8..12, &[0, 0, 0, 0], 0)
}

#[cfg(target_os = "macos")]
#[test]
fn out_of_range_ttc_face_index_is_rejected_without_cache_entry() -> Result<(), String> {
    assert_rejected_ttc_directory_case("bad-index", 8..12, &[0, 0, 0, 1], 1)
}

fn embedded_sfnt_database(path: &Path) -> Result<(Database, resvg::usvg::fontdb::ID), String> {
    database_with_replaced_file(
        Source::Binary(std::sync::Arc::new(EMBEDDED_FONT_BYTES.to_vec())),
        0,
        path,
        0,
    )
}

#[test]
fn empty_sfnt_directory_is_rejected_without_cache_entry() -> Result<(), String> {
    let path = temp_path("empty-sfnt", "ttf");
    std::fs::write(&path, [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        .map_err(|error| error.to_string())?;
    let (database, face) = embedded_sfnt_database(&path)?;
    assert_rejected(&database, face);
    std::fs::remove_file(path).map_err(|error| error.to_string())
}

#[test]
fn truncated_sfnt_table_records_are_rejected_without_cache_entry() -> Result<(), String> {
    let path = temp_path("truncated-record", "ttf");
    std::fs::write(
        &path,
        [0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    )
    .map_err(|error| error.to_string())?;
    let (database, face) = embedded_sfnt_database(&path)?;
    assert_rejected(&database, face);
    std::fs::remove_file(path).map_err(|error| error.to_string())
}
