use super::*;
use resvg::usvg::fontdb::{Database, Source};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const BUNDLED_SANS_SERIF_FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
type TestResult<T> = Result<T, Box<dyn std::error::Error>>;
static TEMP_FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct FontFileFixture {
    database: Database,
    face_id: resvg::usvg::fontdb::ID,
    path: PathBuf,
}

impl FontFileFixture {
    fn create() -> TestResult<Self> {
        let path = temp_path("font", "ttf");
        std::fs::write(&path, BUNDLED_SANS_SERIF_FONT)?;
        let mut database = Database::new();
        database.load_font_source(Source::File(path.clone()));
        let face_id = database
            .faces()
            .next()
            .ok_or("fixture font did not load")?
            .id;
        Ok(Self {
            database,
            face_id,
            path,
        })
    }
}

impl Drop for FontFileFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn create() -> TestResult<Self> {
        let path = temp_path("batch", "dir");
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_path(prefix: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "krr-stamp-batch-{prefix}-{}-{}.{}",
        std::process::id(),
        TEMP_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed),
        extension
    ))
}

fn fixture_stamp(fixture: &FontFileFixture) -> Option<FileStamp> {
    super::super::file_generation::file_source_stamp(&fixture.database, fixture.face_id).1
}

#[test]
fn changed_durable_file_invalidates_batch_and_retry_uses_new_stamp() -> TestResult<()> {
    let fixture = FontFileFixture::create()?;
    let fixture_is_durable = super::super::file_stamp_durable_reusable(&fixture.path);
    let mut calls = 0;
    let mut write_result = Ok(());
    let result = with_validated_stamp_batch(|| {
        calls += 1;
        let first = fixture_stamp(&fixture);
        if calls == 1 && fixture_is_durable {
            write_result = std::fs::write(&fixture.path, b"replacement font payload");
        }
        first
    });
    write_result?;
    assert_eq!(calls, if fixture_is_durable { 2 } else { 1 });
    assert_eq!(result, fixture_stamp(&fixture));
    Ok(())
}

#[test]
fn deleted_file_retries_durable_batch_and_keeps_weak_batch_uncached() -> TestResult<()> {
    let fixture = FontFileFixture::create()?;
    let fixture_is_durable = super::super::file_stamp_durable_reusable(&fixture.path);
    let observation = delete_file_during_batch(&fixture)?;
    assert_eq!(fixture_stamp(&fixture), None);
    assert_deleted_file_result(observation, fixture_is_durable);
    Ok(())
}

struct DeletedFileObservation {
    result: Option<FileStamp>,
    first_stamp: Option<FileStamp>,
    first_cache: Option<FileStamp>,
    first_memo_usable: bool,
    calls: usize,
}

fn delete_file_during_batch(fixture: &FontFileFixture) -> TestResult<DeletedFileObservation> {
    let mut calls = 0;
    let mut remove_result = Ok(());
    let mut first_stamp = None;
    let mut first_cache = None;
    let mut first_memo_usable = true;
    let result = with_validated_stamp_batch(|| {
        calls += 1;
        let stamp = fixture_stamp(fixture);
        if calls == 1 {
            first_stamp = stamp.clone();
            first_cache = cached_stamp(&fixture.path);
            first_memo_usable = memo_usable();
            remove_result = std::fs::remove_file(&fixture.path);
        }
        stamp
    });
    remove_result?;
    Ok(DeletedFileObservation {
        result,
        first_stamp,
        first_cache,
        first_memo_usable,
        calls,
    })
}

fn assert_deleted_file_result(observation: DeletedFileObservation, fixture_is_durable: bool) {
    assert_eq!(observation.calls, if fixture_is_durable { 2 } else { 1 });
    if fixture_is_durable {
        assert_eq!(observation.result, None);
    } else {
        assert!(observation.result.is_some());
        assert_eq!(observation.result, observation.first_stamp);
        assert!(observation.first_cache.is_none());
        assert!(!observation.first_memo_usable);
    }
}

#[test]
fn file_change_between_scopes_uses_the_new_stamp() -> TestResult<()> {
    let fixture = FontFileFixture::create()?;
    let before = with_validated_stamp_batch(|| fixture_stamp(&fixture));
    std::fs::write(&fixture.path, b"new generation bytes")?;
    let after = with_validated_stamp_batch(|| fixture_stamp(&fixture));
    assert_ne!(before, after);
    assert_eq!(after, fixture_stamp(&fixture));
    Ok(())
}

#[test]
fn nested_scope_skips_memo_and_preserves_outer_snapshot() -> TestResult<()> {
    let outer = FontFileFixture::create()?;
    let inner = FontFileFixture::create()?;
    let outer_is_durable = super::super::file_stamp_durable_reusable(&outer.path);
    let mut calls = 0;
    let result = with_validated_stamp_batch(|| {
        calls += 1;
        nested_scope_snapshot(&outer, &inner)
    });
    assert_eq!(calls, 1);
    assert_nested_scope_expectations(&result, outer_is_durable);
    assert!(cached_stamp(&inner.path).is_none());
    Ok(())
}

type NestedScopeSnapshot = (
    Option<FileStamp>,
    (Option<FileStamp>, bool, bool),
    Option<FileStamp>,
);

fn nested_scope_snapshot(outer: &FontFileFixture, inner: &FontFileFixture) -> NestedScopeSnapshot {
    let outer_before = fixture_stamp(outer);
    let inner_state = with_validated_stamp_batch(|| {
        (
            fixture_stamp(inner),
            cached_stamp(&outer.path).is_some(),
            cached_stamp(&inner.path).is_some(),
        )
    });
    (outer_before, inner_state, cached_stamp(&outer.path))
}

fn assert_nested_scope_expectations(result: &NestedScopeSnapshot, outer_is_durable: bool) {
    assert!(result.0.is_some() && result.1.0.is_some());
    assert!(!result.1.1);
    assert!(!result.1.2);
    assert_eq!(result.2.is_some(), outer_is_durable);
    if outer_is_durable {
        assert_eq!(result.2, result.0);
    } else {
        assert!(result.2.is_none());
    }
}

#[test]
fn panic_unwind_restores_batch_tls_for_nested_and_top_level_scopes() -> TestResult<()> {
    let fixture = FontFileFixture::create()?;
    let fixture_is_durable = super::super::file_stamp_durable_reusable(&fixture.path);
    let mut calls = 0;
    let completed = with_validated_stamp_batch(|| {
        calls += 1;
        let stamp = fixture_stamp(&fixture);
        let nested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_validated_stamp_batch(|| std::panic::resume_unwind(Box::new(())))
        }));
        (
            nested.is_err(),
            stamp.is_some(),
            cached_stamp(&fixture.path).is_some(),
            memo_usable(),
        )
    });
    assert_eq!(calls, 1);
    assert!(completed.0 && completed.1);
    assert_eq!(completed.2, fixture_is_durable);
    assert_eq!(completed.3, fixture_is_durable);
    let top_level = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_validated_stamp_batch(|| std::panic::resume_unwind(Box::new(())))
    }));
    assert!(top_level.is_err());
    assert!(ACTIVE_BATCH.with(|active| active.borrow().is_none()));
    Ok(())
}

#[test]
fn unavailable_file_within_scope_retries_without_memo() -> TestResult<()> {
    let fixture = FontFileFixture::create()?;
    std::fs::remove_file(&fixture.path)?;
    let mut calls = 0;
    let result = with_validated_stamp_batch(|| {
        calls += 1;
        fixture_stamp(&fixture)
    });
    assert_eq!(calls, 2);
    assert!(result.is_none());
    assert!(ACTIVE_BATCH.with(|active| active.borrow().is_none()));
    Ok(())
}

#[test]
fn missing_batch_state_never_accepts_memoized_result() {
    let mut calls = 0;
    let result = with_validated_stamp_batch(|| {
        calls += 1;
        if calls == 1 {
            ACTIVE_BATCH.with(|active| active.replace(None));
        }
        calls
    });
    assert_eq!(result, 2);
    assert!(ACTIVE_BATCH.with(|active| active.borrow().is_none()));
}

#[test]
fn duplicate_path_retains_stamp_only_on_durable_filesystems() -> TestResult<()> {
    let fixture = FontFileFixture::create()?;
    let mut calls = 0;
    let result = with_validated_stamp_batch(|| {
        calls += 1;
        let stamp = fixture_stamp(&fixture);
        if let Some(observed) = stamp.clone() {
            remember_stamp(&fixture.path, observed);
        }
        stamp
    });
    assert_eq!(calls, 1);
    assert_eq!(result, fixture_stamp(&fixture));
    Ok(())
}

#[path = "svg_rasterize_text_stamp_batch_weak_tests.rs"]
mod weak_tests;
