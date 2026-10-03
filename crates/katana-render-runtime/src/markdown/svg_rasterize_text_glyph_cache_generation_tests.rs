#[cfg(windows)]
use super::cached_font_has_char;
use super::{FixtureResult, TestId, file_recovery_fixture};
use resvg::usvg::fontdb::Database;
use std::sync::Arc;

#[cfg(unix)]
#[test]
fn unix_file_generation_remains_durable_reusable() -> FixtureResult<()> {
    let (database, id, _path, _guard, _) = file_recovery_fixture()?;
    let generation = file_generation(&database, id);
    assert!(generation.reusable());
    assert!(generation.durable_reusable());
    Ok(())
}

#[test]
fn unavailable_generation_is_not_durably_reusable() -> FixtureResult<()> {
    let (database, id, path, _guard, _) = file_recovery_fixture()?;
    std::fs::remove_file(path)?;
    let generation = file_generation(&database, id);
    assert!(!generation.reusable());
    assert!(!generation.durable_reusable());
    Ok(())
}

#[cfg(windows)]
#[test]
fn same_length_same_modified_rewrite_does_not_reuse_glyph() -> FixtureResult<()> {
    let (database, id, path, _guard, replacement) = file_recovery_fixture()?;
    let before = file_generation(&database, id);
    assert!(before.reusable());
    assert!(!before.durable_reusable());
    assert_eq!(cached_font_has_char(&database, id, 'A'), Some(true));
    rewrite_with_same_modified_time(&path, &replacement)?;
    assert_eq!(before, file_generation(&database, id));
    assert_eq!(cached_font_has_char(&database, id, 'A'), Some(false));
    Ok(())
}

fn file_generation(
    database: &Arc<Database>,
    id: TestId,
) -> super::super::super::super::file_generation::FontSourceGeneration {
    super::super::super::super::file_generation::font_source_generation(database, id)
}

#[cfg(windows)]
fn rewrite_with_same_modified_time(
    path: &std::path::Path,
    replacement: &[u8],
) -> FixtureResult<()> {
    let modified = std::fs::metadata(path)?.modified()?;
    std::fs::write(path, replacement)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_times(std::fs::FileTimes::new().set_modified(modified))?;
    Ok(())
}
