use super::super::{family_runs::resolved_font_runs, html_font_db_for_text, matching_font_face};
use super::{Scope, lookup_known};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

const CONTEXT_CAPACITY: usize = 16;
const CHARACTER_CAPACITY: usize = 256;
const TEST_WEIGHT: u16 = 400;
const TEST_FAMILY: &str = "Noto Sans, serif";
const CONTEXT_WEIGHTS: [u16; 4] = [400, 500, 600, 700];
const FIRST_TEST_CODEPOINT: u32 = 0x0100;
const NEW_TEST_CODEPOINT: u32 = 0x0240;
type FontRuns = Vec<(ID, String)>;
type ContextSpec = (&'static str, u16, bool);

#[test]
fn fifo_context_bound_evicts_oldest_and_keeps_latest_real_font_entry() -> Result<(), String> {
    super::super::font::with_validated_stamp_batch(context_capacity_case)
}

fn context_capacity_case() -> Result<(), String> {
    let text = "A";
    let database = html_font_db_for_text(TEST_FAMILY, text);
    let base = matching_font_face(&database, TEST_FAMILY, TEST_WEIGHT, false)
        .ok_or_else(|| "public Noto Sans face must be available".to_string())?;
    let expected = per_character_oracle(&database, base, TEST_FAMILY, text, TEST_WEIGHT, false);
    let _scope = Scope::enter();
    resolve_context(&database, base, TEST_FAMILY, TEST_WEIGHT, false, text);
    assert_known(&database, base, TEST_FAMILY, TEST_WEIGHT, false, text)?;
    for (family, weight, italic) in distinct_contexts()
        .into_iter()
        .skip(1)
        .take(CONTEXT_CAPACITY)
    {
        resolve_context(&database, base, family, weight, italic, text);
    }
    assert_eq!(context_count(), CONTEXT_CAPACITY);
    assert_cache_bounds()?;
    assert!(lookup_known(&database, base, TEST_FAMILY, TEST_WEIGHT, false, text).is_none());
    let (family, weight, italic) = latest_context()?;
    assert_known(&database, base, family, weight, italic, text)?;
    assert_eq!(
        resolve_context(&database, base, TEST_FAMILY, TEST_WEIGHT, false, text),
        expected
    );
    Ok(())
}

fn latest_context() -> Result<ContextSpec, String> {
    distinct_contexts()
        .into_iter()
        .skip(1)
        .nth(CONTEXT_CAPACITY - 1)
        .ok_or_else(|| "latest context missing".to_string())
}

#[test]
fn fifo_character_bound_replaces_oldest_real_font_selection() -> Result<(), String> {
    super::super::font::with_validated_stamp_batch(character_capacity_case)
}

fn character_capacity_case() -> Result<(), String> {
    let first_text = bounded_text(CHARACTER_CAPACITY);
    let new_character = char::from_u32(NEW_TEST_CODEPOINT).ok_or("new character missing")?;
    let second_text = format!("{first_text}{new_character}");
    let database = html_font_db_for_text(TEST_FAMILY, &second_text);
    let base = matching_font_face(&database, TEST_FAMILY, TEST_WEIGHT, false)
        .ok_or_else(|| "public Noto Sans face must be available".to_string())?;
    let expected = expected_runs(&database, base, &second_text);
    let _scope = Scope::enter();
    seed_character_context(&database, base, &first_text)?;
    let actual = resolve_context(
        &database,
        base,
        TEST_FAMILY,
        TEST_WEIGHT,
        false,
        &second_text,
    );
    assert_selection(&database, base, new_character)?;
    assert_cache_bounds()?;
    assert_eq!(reconstruct_runs(&actual), second_text);
    assert_eq!(actual, expected);
    Ok(())
}

fn seed_character_context(
    database: &Arc<Database>,
    base: ID,
    first_text: &str,
) -> Result<(), String> {
    let first_character = first_text.chars().next().ok_or("first character missing")?;
    let new_character = char::from_u32(NEW_TEST_CODEPOINT).ok_or("new character missing")?;
    resolve_context(database, base, TEST_FAMILY, TEST_WEIGHT, false, first_text);
    assert_selection(database, base, first_character)?;
    super::store(
        database,
        base,
        TEST_FAMILY,
        TEST_WEIGHT,
        false,
        [(new_character, base)],
    );
    assert_oldest_evicted(database, base, first_character)?;
    assert_selection(database, base, new_character)?;
    Ok(())
}

fn assert_oldest_evicted(
    database: &Arc<Database>,
    base: ID,
    first_character: char,
) -> Result<(), String> {
    let (_, missing) = lookup_known(
        database,
        base,
        TEST_FAMILY,
        TEST_WEIGHT,
        false,
        &first_character.to_string(),
    )
    .ok_or_else(|| "character cache context disappeared".to_string())?;
    if !missing.contains(first_character) {
        return Err("oldest cached character was not evicted".to_string());
    }
    Ok(())
}

fn expected_runs(database: &Arc<Database>, base: ID, text: &str) -> FontRuns {
    per_character_oracle(database, base, TEST_FAMILY, text, TEST_WEIGHT, false)
}

fn assert_selection(database: &Arc<Database>, base: ID, character: char) -> Result<(), String> {
    let text = character.to_string();
    assert_known(database, base, TEST_FAMILY, TEST_WEIGHT, false, &text)
}

fn resolve_context(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
    text: &str,
) -> FontRuns {
    resolved_font_runs(database, base, family, text, weight, italic)
}

fn assert_known(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
    text: &str,
) -> Result<(), String> {
    let (known, missing) = lookup_known(database, base, family, weight, italic, text)
        .ok_or_else(|| "expected cached real-font selection".to_string())?;
    if !missing.is_empty() {
        return Err("cached selection still has missing characters".to_string());
    }
    if known.len() != text.chars().count() {
        return Err("cached selection character count mismatch".to_string());
    }
    Ok(())
}

fn context_count() -> usize {
    super::ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .map_or(0, |cache| cache.contexts.len())
    })
}

fn assert_cache_bounds() -> Result<(), String> {
    super::ACTIVE.with(|active| {
        let cache = active.borrow();
        let Some(cache) = cache.as_ref() else {
            return Err("active cache scope missing".to_string());
        };
        if cache.contexts.len() > super::MAX_CONTEXTS
            || cache.total_entries > super::MAX_TOTAL_ENTRIES
            || cache.total_family_bytes > super::MAX_TOTAL_FAMILY_BYTES
        {
            return Err("cache aggregate bound exceeded".to_string());
        }
        for context in &cache.contexts {
            if context.selections.len() > super::MAX_CONTEXT_CHARS
                || context.selections.len() != context.insertion_order.len()
            {
                return Err("cache context bound exceeded".to_string());
            }
        }
        Ok(())
    })
}

fn distinct_contexts() -> Vec<ContextSpec> {
    [
        "Noto Sans, serif",
        "Noto Sans,  serif",
        "Noto Sans , serif",
        "Noto Sans, serif ",
    ]
    .into_iter()
    .flat_map(|family| {
        CONTEXT_WEIGHTS.into_iter().flat_map(move |weight| {
            [false, true]
                .into_iter()
                .map(move |italic| (family, weight, italic))
        })
    })
    .collect()
}

fn bounded_text(count: usize) -> String {
    let mut text = String::new();
    let mut codepoint = FIRST_TEST_CODEPOINT;
    while text.chars().count() < count {
        if let Some(character) = char::from_u32(codepoint) {
            text.push(character);
        }
        codepoint += 1;
    }
    text
}

fn per_character_oracle(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> FontRuns {
    let mut result = Vec::new();
    for character in text.chars() {
        let character_text = character.to_string();
        for (face, run) in
            resolved_font_runs(database, base, family, &character_text, weight, italic)
        {
            for selected_character in run.chars() {
                super::super::font::append_font_run(&mut result, face, selected_character);
            }
        }
    }
    result
}

fn reconstruct_runs(runs: &[(ID, String)]) -> String {
    runs.iter().map(|(_, run)| run.as_str()).collect()
}
