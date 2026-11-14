use super::super::{html_font_db_for_text, with_render_scoped_stamp_batch};
use super::{
    MAX_SELECTED_FONT_MEMO_ENTRIES, append_font_run, cached_font_has_char_with_generation,
    family_candidates, family_scope_cache, matching_font_face, resolved_font_runs,
};
use resvg::usvg::fontdb::Source;
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

const TEST_NORMAL_WEIGHT: u16 = 400;
const LARGE_SCRIPT_START: u32 = 0x0100;
const LARGE_SCRIPT_END: u32 = 0x0240;
const PRIME_TEXT: &str = "A日本";
const KNOWN_AND_NEW_TEXT: &str = "A日本Ж😀";
type FontRuns = Vec<(ID, String)>;
type RunTriplet = (FontRuns, FontRuns, FontRuns);

fn per_character_oracle(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> FontRuns {
    let candidates = family_candidates(database, family, weight, italic);
    super::with_reusable_stamp_batch(|| {
        let mut result = Vec::new();
        for (fallback, run) in super::html_font_runs(database, base, text, weight, italic) {
            for character in run.chars() {
                let selected = candidates
                    .iter()
                    .copied()
                    .find(|id| {
                        cached_font_has_char_with_generation(database, *id, character).0
                            == Some(true)
                    })
                    .unwrap_or(fallback);
                append_font_run(&mut result, selected, character);
            }
        }
        result
    })
}

#[test]
fn repeated_mixed_script_selection_matches_per_character_oracle() -> Result<(), String> {
    let text = "A日本A日本";
    let family = "Noto Sans, serif";
    let weight = 400;
    let italic = false;
    let database = html_font_db_for_text(family, text);
    let base = matching_font_face(&database, family, weight, italic)
        .ok_or_else(|| "public Noto Sans face must be available".to_string())?;
    if !matches!(
        database.face(base).map(|face| &face.source),
        Some(Source::Binary(_))
    ) {
        return Err("the bundled Noto Sans test face must use binary source".to_string());
    }
    if family_candidates(&database, family, weight, italic).len() < 2 {
        return Err("public author family candidates must include two faces".to_string());
    }

    let actual = resolved_font_runs(&database, base, family, text, weight, italic);
    let expected = per_character_oracle(&database, base, family, text, weight, italic);

    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn selected_font_memo_cap_preserves_large_mixed_script_concatenation() -> Result<(), String> {
    let (family, text, weight, italic) = large_mixed_script_fixture();
    if text.chars().count() <= MAX_SELECTED_FONT_MEMO_ENTRIES {
        return Err("test input must exceed the selected-font memo cap".to_string());
    }
    let database = html_font_db_for_text(family, &text);
    let base = matching_font_face(&database, family, weight, italic)
        .ok_or_else(|| "public Noto Sans face must be available".to_string())?;
    let (first, second, expected) = with_render_scoped_stamp_batch(|| {
        let first = resolved_font_runs(&database, base, family, &text, weight, italic);
        let second = resolved_font_runs(&database, base, family, &text, weight, italic);
        let expected = per_character_oracle(&database, base, family, &text, weight, italic);
        (first, second, expected)
    });
    assert_eq!(reconstruct_runs(&first), text);
    assert_eq!(first, second);
    assert_eq!(second, expected);
    Ok(())
}

#[test]
fn cached_selection_reconstructs_known_and_new_unicode_chars() -> Result<(), String> {
    let family = "Noto Sans, serif";
    let database = html_font_db_for_text(family, KNOWN_AND_NEW_TEXT);
    let base = matching_font_face(&database, family, TEST_NORMAL_WEIGHT, false)
        .ok_or_else(|| "public Noto Sans face must be available".to_string())?;
    let (primed, actual, expected) = prime_then_resolve_new_text(&database, base, family);
    assert_known_new_result(&database, base, family, primed, actual, expected);
    Ok(())
}

fn assert_known_new_result(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    primed: Vec<(ID, String)>,
    actual: Vec<(ID, String)>,
    expected: Vec<(ID, String)>,
) {
    assert_eq!(primed, prime_oracle(database, base, family));
    assert_eq!(actual, expected);
    assert_eq!(reconstruct_runs(&actual), KNOWN_AND_NEW_TEXT);
    assert!(
        family_scope_cache::lookup_runs(
            database,
            base,
            family,
            TEST_NORMAL_WEIGHT,
            false,
            KNOWN_AND_NEW_TEXT,
        )
        .is_none()
    );
}

fn prime_oracle(database: &Arc<Database>, base: ID, family: &str) -> Vec<(ID, String)> {
    per_character_oracle(
        database,
        base,
        family,
        PRIME_TEXT,
        TEST_NORMAL_WEIGHT,
        false,
    )
}

fn prime_then_resolve_new_text(database: &Arc<Database>, base: ID, family: &str) -> RunTriplet {
    with_render_scoped_stamp_batch(|| {
        let primed = resolve_test_runs(database, base, family, PRIME_TEXT);
        let actual = resolve_test_runs(database, base, family, KNOWN_AND_NEW_TEXT);
        let expected = per_character_oracle(
            database,
            base,
            family,
            KNOWN_AND_NEW_TEXT,
            TEST_NORMAL_WEIGHT,
            false,
        );
        (primed, actual, expected)
    })
}

fn resolve_test_runs(database: &Arc<Database>, base: ID, family: &str, text: &str) -> FontRuns {
    resolved_font_runs(database, base, family, text, TEST_NORMAL_WEIGHT, false)
}

fn reconstruct_runs(runs: &[(ID, String)]) -> String {
    runs.iter().map(|(_, run)| run.as_str()).collect()
}

fn large_mixed_script_fixture() -> (&'static str, String, u16, bool) {
    let mut text = String::from("A日本A日本");
    for codepoint in LARGE_SCRIPT_START..LARGE_SCRIPT_END {
        if let Some(character) = char::from_u32(codepoint) {
            text.push(character);
        }
    }
    ("Noto Sans, serif", text, TEST_NORMAL_WEIGHT, false)
}

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_text_family_runs_scope_tests.rs"]
mod scope_tests;
