use super::*;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
type ObservedSelections = Arc<Mutex<Vec<(char, Vec<ID>)>>>;
type SelectionResults = (Option<ID>, Option<ID>, Option<ID>, Option<ID>, Option<ID>);
type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

struct FontFile(std::path::PathBuf);

impl FontFile {
    fn create() -> TestResult<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "krr-selector-{}-{}.ttf",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, FONT)?;
        Ok(Self(path))
    }
}

impl Drop for FontFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn binary_database() -> Arc<Database> {
    let mut database = Database::new();
    database.load_font_source(Source::Binary(Arc::new(FONT.to_vec())));
    Arc::new(database)
}

fn file_database(path: &std::path::Path) -> Arc<Database> {
    let mut database = Database::new();
    database.load_font_source(Source::File(path.to_path_buf()));
    Arc::new(database)
}

fn first_face(database: &Database) -> TestResult<ID> {
    database
        .faces()
        .next()
        .map(|face| face.id)
        .ok_or_else(|| "test font has no face".into())
}

fn options_with(
    selector: impl Fn(char, &[ID], &mut Arc<Database>) -> Option<ID> + Send + Sync + 'static,
) -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    options.font_resolver.select_fallback = Box::new(selector);
    install(&mut options);
    options
}

fn select(
    options: &mut usvg::Options<'_>,
    database: &Arc<Database>,
    character: char,
    excluded: &[ID],
) -> Option<ID> {
    let mut database = Arc::clone(database);
    (options.font_resolver.select_fallback)(character, excluded, &mut database)
}

fn with_attempt<T>(mut operation: impl FnMut() -> T) -> T {
    super::super::font::with_validated_stamp_batch(|| super::with_attempt(&mut operation))
}

fn recording_options(
    calls: Arc<AtomicUsize>,
    observed: ObservedSelections,
) -> usvg::Options<'static> {
    options_with(move |character, excluded, database| {
        calls.fetch_add(1, Ordering::Relaxed);
        let Ok(mut entries) = observed.lock() else {
            return None;
        };
        entries.push((character, excluded.to_vec()));
        drop(entries);
        if character == '∅' {
            None
        } else {
            excluded
                .first()
                .copied()
                .or_else(|| database.faces().next().map(|face| face.id))
        }
    })
}

fn selector_answers(
    options: &mut usvg::Options<'_>,
    database: &Arc<Database>,
    first: ID,
    second: ID,
) -> SelectionResults {
    let selected = select(options, database, 'あ', &[first, second]);
    let repeated = select(options, database, 'あ', &[first, second]);
    let reversed = select(options, database, 'あ', &[second, first]);
    let missing = select(options, database, '∅', &[]);
    let missing_again = select(options, database, '∅', &[]);
    (selected, repeated, reversed, missing, missing_again)
}

fn counted_options(calls: Arc<AtomicUsize>) -> usvg::Options<'static> {
    options_with(move |_, _, database| {
        calls.fetch_add(1, Ordering::Relaxed);
        database.faces().next().map(|face| face.id)
    })
}

#[test]
fn preserves_uncached_selector_arguments_order_and_none_results() -> TestResult<()> {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut options = recording_options(Arc::clone(&calls), Arc::clone(&observed));
    let database = binary_database();
    let first = first_face(&database)?;
    let second = ID::dummy();
    let result = with_attempt(|| selector_answers(&mut options, &database, first, second));
    assert_eq!(result, (Some(first), Some(first), Some(second), None, None));
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    let observed = observed
        .lock()
        .map_err(|_| std::io::Error::other("selector observations poisoned"))?;
    assert_eq!(
        observed.as_slice(),
        [
            ('あ', vec![first, second]),
            ('あ', vec![second, first]),
            ('∅', vec![])
        ]
    );
    Ok(())
}

#[test]
fn database_weak_identity_keeps_distinct_cache_partitions() -> TestResult<()> {
    let calls = Arc::new(AtomicUsize::new(0));
    let selector_calls = Arc::clone(&calls);
    let mut options = options_with(move |_, _, database| {
        selector_calls.fetch_add(1, Ordering::Relaxed);
        database.faces().next().map(|face| face.id)
    });
    let populated = binary_database();
    let empty = Arc::new(Database::new());
    let expected = first_face(&populated)?;
    let result = with_attempt(|| {
        let first = select(&mut options, &populated, 'x', &[]);
        let first_again = select(&mut options, &populated, 'x', &[]);
        let second = select(&mut options, &empty, 'x', &[]);
        let second_again = select(&mut options, &empty, 'x', &[]);
        (first, first_again, second, second_again)
    });
    assert_eq!(result, (Some(expected), Some(expected), None, None));
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    Ok(())
}

#[test]
fn arc_make_mut_weak_dissociation_forces_a_fresh_miss() -> TestResult<()> {
    let calls = Arc::new(AtomicUsize::new(0));
    let selector_calls = Arc::clone(&calls);
    let mut options = options_with(move |_, _, database| {
        selector_calls.fetch_add(1, Ordering::Relaxed);
        if database.faces().count() == 1 {
            database.faces().next().map(|face| face.id)
        } else {
            database.faces().last().map(|face| face.id)
        }
    });
    let mut database = binary_database();
    let initial_face = first_face(&database)?;
    let selections = with_attempt(|| {
        let original = select(&mut options, &database, 'x', &[]);
        Arc::make_mut(&mut database).load_font_source(Source::Binary(Arc::new(FONT.to_vec())));
        let changed = select(&mut options, &database, 'x', &[]);
        (original, changed)
    });
    assert_eq!(selections.0, Some(initial_face));
    assert_ne!(selections.0, selections.1);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    Ok(())
}

#[path = "svg_rasterize_text_fallback_memo_scope_tests.rs"]
mod scope_tests;
