use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn mutate_file(path: &std::path::Path, delete: bool) -> std::io::Result<()> {
    if delete {
        std::fs::remove_file(path)
    } else {
        std::fs::write(path, b"new font generation")
    }
}

fn default_selector_matches_uncached_result() -> TestResult<()> {
    let database = binary_database();
    let excluded = [first_face(&database)?];
    let mut expected_database = Arc::clone(&database);
    let expected =
        (usvg::FontResolver::default_fallback_selector())('あ', &excluded, &mut expected_database);
    let mut options = usvg::Options::default();
    super::super::install(&mut options);
    let mut actual_database = Arc::clone(&database);
    let actual = with_attempt(|| {
        (options.font_resolver.select_fallback)('あ', &excluded, &mut actual_database)
    });
    assert_eq!(actual, expected);
    Ok(())
}

fn assert_file_change_retries(delete: bool) -> TestResult<()> {
    let file = FontFile::create()?;
    let database = file_database(&file.0);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let attempts = AtomicUsize::new(0);
    let mut mutation = Ok(());
    let result = super::super::super::with_validated_tree_parse(|| {
        attempts.fetch_add(1, Ordering::Relaxed);
        let first = select(&mut options, &database, 'x', &[]);
        let repeated = select(&mut options, &database, 'x', &[]);
        if attempts.load(Ordering::Relaxed) == 1 {
            mutation = mutate_file(&file.0, delete);
        }
        (first, repeated)
    });
    mutation?;
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert_eq!(result.0, result.1);
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    Ok(())
}

#[test]
fn changed_file_retries_whole_parse_uncached() -> TestResult<()> {
    assert_file_change_retries(false)
}

#[test]
fn missing_file_retries_whole_parse_uncached() -> TestResult<()> {
    assert_file_change_retries(true)
}

#[test]
fn nested_scope_and_panic_restore_outer_memo() -> TestResult<()> {
    default_selector_matches_uncached_result()?;
    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let database = binary_database();
    let expected = first_face(&database)?;
    super::super::super::with_validated_tree_parse(|| {
        let first = select(&mut options, &database, 'x', &[]);
        let _ = super::super::super::with_validated_tree_parse(|| {
            select(&mut options, &database, 'x', &[])
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::super::super::with_validated_tree_parse(|| {
                std::panic::resume_unwind(Box::new("nested parse"));
            });
        }));
        assert!(panic.is_err());
        assert_eq!(first, Some(expected));
        assert_eq!(select(&mut options, &database, 'x', &[]), first);
    });
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    Ok(())
}

#[test]
fn entry_limit_bypasses_new_keys_and_keeps_earlier_entry() -> TestResult<()> {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = counted_options(Arc::clone(&calls));
    let database = binary_database();
    first_face(&database)?;
    let total = super::super::MAX_SELECTOR_ENTRIES + 1;
    super::super::super::with_validated_tree_parse(|| {
        for character in '\u{1000}'..='\u{1fff}' {
            select(&mut options, &database, character, &[]);
        }
        select(&mut options, &database, '\u{2000}', &[]);
        select(&mut options, &database, '\u{1000}', &[]);
        select(&mut options, &database, '\u{2000}', &[]);
    });
    assert_eq!(calls.load(Ordering::Relaxed), total + 1);
    Ok(())
}

#[path = "svg_rasterize_text_fallback_memo_boundary_tests.rs"]
mod boundary_tests;
