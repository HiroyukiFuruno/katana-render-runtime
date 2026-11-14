use super::super::super::HtmlFontRequest;
use super::super::super::font::{FontSourceGeneration, font_source_generation};
use resvg::usvg::fontdb::{Database, Source};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const MAX_STORED_TOKENS: usize = 8;
const OVERSIZED_FAMILY_COUNT: usize = 3000;
const OVERSIZED_FAMILY_WIDTH: usize = 100;
const AGGREGATE_FAMILY_COUNT: usize = 500;
const AGGREGATE_FAMILY_WIDTH: usize = 80;

type TestResult<T> = Result<T, String>;
type FontGenerations = Vec<(resvg::usvg::fontdb::ID, FontSourceGeneration)>;
type Fixture = (Arc<Database>, FontGenerations, TempFont);

struct TempFont(PathBuf);

impl Drop for TempFont {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn database_with_file() -> TestResult<Fixture> {
    let index = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "krr-token-store-{}-{index}.ttf",
        std::process::id()
    ));
    std::fs::write(&path, FONT_BYTES).map_err(|error| error.to_string())?;
    let mut database = Database::new();
    database.load_font_source(Source::File(path.clone()));
    let database = Arc::new(database);
    let generations = database
        .faces()
        .map(|face| (face.id, font_source_generation(&database, face.id)))
        .collect();
    Ok((database, generations, TempFont(path)))
}

fn request(label: &str) -> HtmlFontRequest {
    HtmlFontRequest::from_text(label, "A")
}

fn aggregate_request(index: usize) -> HtmlFontRequest {
    let family = (0..AGGREGATE_FAMILY_COUNT)
        .map(|name| {
            format!(
                "aggregate-{index}-{name:0width$}",
                width = AGGREGATE_FAMILY_WIDTH
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    HtmlFontRequest::from_text(&family, "A")
}

fn build(
    request: &HtmlFontRequest,
    database: Arc<Database>,
    generations: FontGenerations,
) -> TestResult<(super::super::HtmlTextMeasurementToken, usize)> {
    super::TokenStore::build_token(request, database, generations)
        .ok_or_else(|| "real token construction unexpectedly rejected".to_string())
}

#[test]
fn weak_database_expiration_rejects_stored_token() -> TestResult<()> {
    let (database, generations, _path) = database_with_file()?;
    let request = request("Noto Sans");
    let (token, bytes) = build(&request, Arc::clone(&database), generations.clone())?;
    let mut store = super::TokenStore::new();
    store.insert(request.clone(), token, bytes);
    drop(database);
    let (replacement, _, _replacement_path) = database_with_file()?;
    super::super::super::with_render_scoped_stamp_batch(|| {
        assert!(store.get(&request, &replacement, &generations).is_none());
    });
    Ok(())
}

#[test]
fn distinct_database_identity_rejects_same_generation_shape() -> TestResult<()> {
    let (first, generations, _first_path) = database_with_file()?;
    let (second, _, _second_path) = database_with_file()?;
    let request = request("Noto Sans");
    let (token, bytes) = build(&request, Arc::clone(&first), generations.clone())?;
    let mut store = super::TokenStore::new();
    store.insert(request.clone(), token, bytes);
    super::super::super::with_render_scoped_stamp_batch(|| {
        assert!(store.get(&request, &second, &generations).is_none());
    });
    Ok(())
}

#[test]
#[cfg(unix)]
fn changed_file_generation_rejects_stored_token_for_same_database() -> TestResult<()> {
    let (database, _, path) = database_with_file()?;
    let request = request("Noto Sans");
    let generation_a: FontGenerations = super::super::super::with_render_scoped_stamp_batch(|| {
        database
            .faces()
            .map(|face| (face.id, font_source_generation(&database, face.id)))
            .collect()
    });
    let (token, bytes) = build(&request, Arc::clone(&database), generation_a.clone())?;
    let mut store = super::TokenStore::new();
    store.insert(request.clone(), token, bytes);
    super::super::super::with_render_scoped_stamp_batch(|| {
        assert!(store.get(&request, &database, &generation_a).is_some());
    });
    let mut changed_font = FONT_BYTES.to_vec();
    changed_font.push(0);
    std::fs::write(&path.0, changed_font).map_err(|error| error.to_string())?;
    let generation_b: FontGenerations = super::super::super::with_render_scoped_stamp_batch(|| {
        database
            .faces()
            .map(|face| (face.id, font_source_generation(&database, face.id)))
            .collect()
    });
    assert_ne!(generation_a, generation_b);
    super::super::super::with_render_scoped_stamp_batch(|| {
        assert!(store.get(&request, &database, &generation_b).is_none());
    });
    Ok(())
}

#[test]
fn ninth_real_request_evicts_oldest_entry() -> TestResult<()> {
    let (store, fixtures) = real_entries()?;
    assert_eviction(&store, &fixtures);
    Ok(())
}

#[test]
fn aggregate_budget_evicts_oldest_real_entry() -> TestResult<()> {
    let mut fixtures = Vec::with_capacity(MAX_STORED_TOKENS);
    let mut store = super::TokenStore::new();
    for index in 0..MAX_STORED_TOKENS {
        let (database, generations, path) = database_with_file()?;
        let request = aggregate_request(index);
        let (token, bytes) = build(&request, Arc::clone(&database), generations.clone())?;
        store.insert(request, token, bytes);
        fixtures.push((database, generations, path));
    }
    assert_aggregate_eviction(&store, &fixtures);
    Ok(())
}

fn assert_aggregate_eviction(store: &super::TokenStore, fixtures: &[Fixture]) {
    let first_request = aggregate_request(0);
    let last_index = MAX_STORED_TOKENS - 1;
    let last_request = aggregate_request(last_index);
    super::super::super::with_render_scoped_stamp_batch(|| {
        assert!(
            store
                .get(&first_request, &fixtures[0].0, &fixtures[0].1)
                .is_none()
        );
        assert!(
            store
                .get(
                    &last_request,
                    &fixtures[last_index].0,
                    &fixtures[last_index].1
                )
                .is_some()
        );
    });
}

fn real_entries() -> TestResult<(super::TokenStore, Vec<Fixture>)> {
    let mut fixtures = Vec::with_capacity(MAX_STORED_TOKENS + 1);
    let mut store = super::TokenStore::new();
    for index in 0..=MAX_STORED_TOKENS {
        let (database, generations, path) = database_with_file()?;
        let request = request(&format!("Noto Sans, missing-{index}"));
        let (token, bytes) = build(&request, Arc::clone(&database), generations.clone())?;
        store.insert(request, token, bytes);
        fixtures.push((database, generations, path));
    }
    Ok((store, fixtures))
}

fn assert_eviction(store: &super::TokenStore, fixtures: &[Fixture]) {
    let (first_database, first_generations, _) = &fixtures[0];
    let first_request = request("Noto Sans, missing-0");
    let (last_database, last_generations, _) = &fixtures[MAX_STORED_TOKENS];
    let last_request = request(&format!("Noto Sans, missing-{MAX_STORED_TOKENS}"));
    super::super::super::with_render_scoped_stamp_batch(|| {
        assert!(
            store
                .get(&first_request, first_database, first_generations)
                .is_none()
        );
        assert!(
            store
                .get(&last_request, last_database, last_generations)
                .is_some()
        );
    });
}

#[test]
fn oversized_normalized_request_is_rejected_by_real_accounting() -> TestResult<()> {
    let (database, generations, _path) = database_with_file()?;
    let family = (0..OVERSIZED_FAMILY_COUNT)
        .map(|index| format!("family-{index:0width$}", width = OVERSIZED_FAMILY_WIDTH))
        .collect::<Vec<_>>()
        .join(",");
    let request = HtmlFontRequest::from_text(&family, "A");
    assert!(super::TokenStore::build_token(&request, database, generations).is_none());
    Ok(())
}

#[test]
fn direct_generation_match_rejects_an_expired_database() -> TestResult<()> {
    let (database, generations, _path) = database_with_file()?;
    let request = request("Noto Sans");
    let (token, _) = build(&request, Arc::clone(&database), generations.clone())?;
    drop(database);
    let (replacement, _, _replacement_path) = database_with_file()?;
    assert!(!super::same_database_and_generations(
        &token,
        &replacement,
        &generations
    ));
    Ok(())
}
