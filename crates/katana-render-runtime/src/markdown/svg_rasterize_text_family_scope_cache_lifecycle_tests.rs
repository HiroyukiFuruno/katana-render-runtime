use super::super::{family_runs::resolved_font_runs, html_font_db_for_text, matching_font_face};
use super::{Scope, lookup_known};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

const TEST_FAMILY: &str = "Noto Sans, serif";
const TEST_WEIGHT: u16 = 400;

#[test]
fn dead_database_context_is_purged_before_reuse() -> Result<(), String> {
    super::super::font::with_validated_stamp_batch(dead_database_context_case)
}

fn dead_database_context_case() -> Result<(), String> {
    let text = "A";
    let database = isolated_test_database();
    let base = matching_font_face(&database, TEST_FAMILY, TEST_WEIGHT, false)
        .ok_or_else(|| "public Noto Sans face must be available".to_string())?;
    let weak_database = Arc::downgrade(&database);
    let _scope = Scope::enter();
    resolve_context(&database, base, TEST_FAMILY, text);
    assert_eq!(context_count(), 1);
    drop(database);
    assert!(weak_database.upgrade().is_none());

    let replacement = isolated_test_database();
    let replacement_base = matching_font_face(&replacement, TEST_FAMILY, TEST_WEIGHT, false)
        .ok_or_else(|| "replacement Noto Sans face must be available".to_string())?;
    assert!(
        lookup_known(
            &replacement,
            replacement_base,
            TEST_FAMILY,
            TEST_WEIGHT,
            false,
            text,
        )
        .is_none()
    );
    assert_eq!(context_count(), 0);
    Ok(())
}

#[test]
fn oversized_family_is_not_cached() -> Result<(), String> {
    super::super::font::with_validated_stamp_batch(oversized_family_case)
}

fn oversized_family_case() -> Result<(), String> {
    let text = "A";
    let database = html_font_db_for_text(TEST_FAMILY, text);
    let base = matching_font_face(&database, TEST_FAMILY, TEST_WEIGHT, false)
        .ok_or_else(|| "public Noto Sans face must be available".to_string())?;
    let oversized_family = format!("{}{}", "x".repeat(super::MAX_FAMILY_BYTES + 1), TEST_FAMILY);
    let expected = per_character_oracle(&database, base, &oversized_family, text);
    let _scope = Scope::enter();
    let actual = resolve_context(&database, base, &oversized_family, text);
    assert_eq!(actual, expected);
    assert!(lookup_known(&database, base, &oversized_family, TEST_WEIGHT, false, text,).is_none());
    assert_eq!(context_count(), 0);
    Ok(())
}

#[test]
fn duplicate_selection_is_cached_once_and_preserves_runs() -> Result<(), String> {
    super::super::font::with_validated_stamp_batch(duplicate_selection_case)
}

fn duplicate_selection_case() -> Result<(), String> {
    let text = "A";
    let database = isolated_test_database();
    let base = matching_font_face(&database, TEST_FAMILY, TEST_WEIGHT, false)
        .ok_or_else(|| "public Noto Sans face must be available".to_string())?;
    let expected = per_character_oracle(&database, base, TEST_FAMILY, text);
    let _scope = Scope::enter();
    super::store(
        &database,
        base,
        TEST_FAMILY,
        TEST_WEIGHT,
        false,
        [('A', base), ('A', base)],
    );
    let (known, missing) = lookup_known(&database, base, TEST_FAMILY, TEST_WEIGHT, false, text)
        .ok_or_else(|| "duplicate selection was not cached".to_string())?;
    assert_eq!(known.len(), 1);
    assert!(missing.is_empty());
    let actual = resolve_context(&database, base, TEST_FAMILY, text);
    assert_eq!(actual, expected);
    assert_duplicate_cache_state()
}

fn assert_duplicate_cache_state() -> Result<(), String> {
    super::ACTIVE.with(|active| {
        let cache = active.borrow();
        let cache = cache.as_ref().ok_or("active cache scope missing")?;
        let context = cache.contexts.first().ok_or("cached context missing")?;
        assert_eq!(context.selections.len(), 1);
        assert_eq!(context.insertion_order.len(), 1);
        Ok(())
    })
}

fn isolated_test_database() -> Arc<Database> {
    let mut database = Database::new();
    database.load_font_data(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec());
    Arc::new(database)
}

fn resolve_context(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
) -> Vec<(ID, String)> {
    resolved_font_runs(database, base, family, text, TEST_WEIGHT, false)
}

fn context_count() -> usize {
    super::ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .map_or(0, |cache| cache.contexts.len())
    })
}

fn per_character_oracle(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
) -> Vec<(ID, String)> {
    let mut result = Vec::new();
    for character in text.chars() {
        let character_text = character.to_string();
        for (face, run) in resolve_context(database, base, family, &character_text) {
            for selected_character in run.chars() {
                super::super::font::append_font_run(&mut result, face, selected_character);
            }
        }
    }
    result
}

#[test]
fn finished_scope_does_not_restore_released_previous_cache() {
    let outer = Scope::enter();
    let mut inner = Scope::enter();
    inner.previous = None;
    inner.finished = true;
    drop(inner);
    assert!(super::ACTIVE.with(|active| active.borrow().is_some()));
    drop(outer);
    assert!(super::ACTIVE.with(|active| active.borrow().is_none()));
}
