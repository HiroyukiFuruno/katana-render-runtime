use super::*;
use std::fs::{self, FileTimes, OpenOptions};

const BYTE_INVERSION_MASK: u8 = 0xff;

#[test]
fn weak_file_stamp_is_not_memoized_and_disables_memo_without_retry() -> TestResult<()> {
    let fixture = FontFileFixture::create()?;
    let original = stamp_path_uncached(&fixture.path).map_err(|_| "fixture stamp unavailable")?;
    let original_contents = fs::read(&fixture.path)?;
    let mut weak = original.clone();
    weak.durable_reusable = false;
    let mut calls = 0;
    let result = with_validated_stamp_batch(|| -> TestResult<bool> {
        calls += 1;
        remember_stamp(&fixture.path, weak.clone());
        assert!(cached_stamp(&fixture.path).is_none());
        assert!(!memo_usable());
        replace_and_validate_changed_contents(&fixture, &original_contents)?;
        assert!(cached_stamp(&fixture.path).is_none());
        Ok(true)
    });
    assert_eq!(calls, 1);
    assert!(result?);
    let next_scope = with_validated_stamp_batch(memo_usable);
    assert!(next_scope);
    assert!(cached_stamp(&fixture.path).is_none());
    Ok(())
}

fn replace_and_validate_changed_contents(
    fixture: &FontFileFixture,
    original_contents: &[u8],
) -> TestResult<()> {
    let original_metadata = fs::metadata(&fixture.path)?;
    let original_len = original_metadata.len();
    let original_modified = original_metadata.modified()?;
    replace_file_preserving_size_and_mtime(&fixture.path)?;
    let current = fixture_stamp(fixture).ok_or("replacement stamp unavailable")?;
    let current_contents = fs::read(&fixture.path)?;
    let current_metadata = fs::metadata(&fixture.path)?;
    assert_ne!(current_contents, original_contents);
    assert_eq!(current_metadata.len(), original_len);
    assert_eq!(current_metadata.modified()?, original_modified);
    assert_eq!(
        current,
        stamp_path_uncached(&fixture.path).map_err(|_| "uncached stamp unavailable")?
    );
    Ok(())
}

#[test]
fn durable_stamp_collected_before_weak_stamp_is_still_validated() -> TestResult<()> {
    let durable = FontFileFixture::create()?;
    let weak = FontFileFixture::create()?;
    if !super::super::super::file_stamp_durable_reusable(&durable.path) {
        return Ok(());
    }
    let expected = fixture_stamp(&durable).ok_or("durable fixture stamp unavailable")?;
    let mut calls = 0;
    let result = with_validated_stamp_batch(|| -> TestResult<Option<FileStamp>> {
        calls += 1;
        let durable_stamp = fixture_stamp(&durable).ok_or("durable stamp unavailable")?;
        let mut weak_stamp =
            stamp_path_uncached(&weak.path).map_err(|_| "weak stamp unavailable")?;
        weak_stamp.durable_reusable = false;
        remember_stamp(&weak.path, weak_stamp);
        if calls == 1 {
            fs::write(&durable.path, b"replacement after weak observation")?;
        }
        Ok(Some(durable_stamp))
    });
    let result = result?;
    assert_eq!(calls, 2);
    assert_ne!(result, Some(expected));
    assert_eq!(result, fixture_stamp(&durable));
    Ok(())
}

#[test]
fn overflow_at_257_paths_retries_durable_batch_and_keeps_weak_batch_uncached() -> TestResult<()> {
    let directory = TempDirectory::create()?;
    let paths = (0..=MAX_BATCH_PATHS)
        .map(|index| directory.0.join(format!("fixture-{index}.dat")))
        .collect::<Vec<_>>();
    for path in &paths {
        fs::write(path, b"fixture")?;
    }
    let filesystem_is_durable = super::super::super::file_stamp_durable_reusable(&paths[0]);
    let mut calls = 0;
    let mut first_cached_stamp = None;
    let mut first_memo_usable = None;
    let complete = with_validated_stamp_batch(|| {
        calls += 1;
        stamp_all_paths(
            &paths,
            calls == 1,
            &mut first_cached_stamp,
            &mut first_memo_usable,
        )
    });
    assert!(complete);
    assert_eq!(calls, if filesystem_is_durable { 2 } else { 1 });
    assert_eq!(first_cached_stamp.is_some(), filesystem_is_durable);
    assert_eq!(first_memo_usable, Some(filesystem_is_durable));
    Ok(())
}

fn stamp_all_paths(
    paths: &[PathBuf],
    record_first_observation: bool,
    first_cached_stamp: &mut Option<FileStamp>,
    first_memo_usable: &mut Option<bool>,
) -> bool {
    for (index, path) in paths.iter().enumerate() {
        let Ok(stamp) = stamp_path_uncached(path) else {
            return false;
        };
        remember_stamp(path, stamp);
        if record_first_observation && index == 0 {
            *first_cached_stamp = cached_stamp(path);
            *first_memo_usable = Some(memo_usable());
        }
    }
    true
}

fn replace_file_preserving_size_and_mtime(path: &std::path::Path) -> TestResult<()> {
    let modified = fs::metadata(path)?.modified()?;
    let mut replacement = fs::read(path)?;
    replacement[0] ^= BYTE_INVERSION_MASK;
    let replacement_path = path.with_extension("replacement.ttf");
    fs::write(&replacement_path, replacement)?;
    let replacement_file = OpenOptions::new().write(true).open(&replacement_path)?;
    replacement_file.set_times(FileTimes::new().set_modified(modified))?;
    replacement_file.sync_all()?;
    drop(replacement_file);
    fs::rename(replacement_path, path)?;
    Ok(())
}
