use super::*;
use std::fs::{self, File, FileTimes, OpenOptions};

const BYTE_INVERSION_MASK: u8 = 0xff;

#[test]
fn weak_file_stamp_is_not_memoized_and_disables_memo_without_retry() -> TestResult<()> {
    let fixture = FontFileFixture::create()?;
    let original = stamp_path_uncached(&fixture.path).map_err(|_| "fixture stamp unavailable")?;
    let mut weak = original.clone();
    weak.durable_reusable = false;
    let mut calls = 0;
    let result = with_validated_stamp_batch(|| -> TestResult<bool> {
        calls += 1;
        remember_stamp(&fixture.path, weak.clone());
        assert!(cached_stamp(&fixture.path).is_none());
        assert!(!memo_usable());
        replace_file_preserving_size_and_mtime(&fixture.path)?;
        let current = fixture_stamp(&fixture).ok_or("replacement stamp unavailable")?;
        assert_ne!(current, original);
        assert_eq!(
            current,
            stamp_path_uncached(&fixture.path).map_err(|_| "uncached stamp unavailable")?
        );
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

fn replace_file_preserving_size_and_mtime(path: &std::path::Path) -> TestResult<()> {
    let modified = fs::metadata(path)?.modified()?;
    let mut replacement = fs::read(path)?;
    replacement[0] ^= BYTE_INVERSION_MASK;
    let replacement_path = path.with_extension("replacement.ttf");
    fs::write(&replacement_path, replacement)?;
    OpenOptions::new()
        .write(true)
        .open(&replacement_path)?
        .set_times(FileTimes::new().set_modified(modified))?;
    File::open(&replacement_path)?.sync_all()?;
    fs::rename(replacement_path, path)?;
    Ok(())
}
