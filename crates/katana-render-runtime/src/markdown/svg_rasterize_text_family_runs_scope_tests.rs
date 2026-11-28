use super::per_character_oracle;
use super::{
    TEST_NORMAL_WEIGHT, family_scope_cache, matching_font_face, resolved_font_runs,
    with_render_scoped_stamp_batch,
};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

const TEST_BOLD_WEIGHT: u16 = 700;
const CACHE_STATE_COUNT: usize = 7;

#[test]
fn render_scope_reuses_final_selection_and_separates_configuration() -> Result<(), String> {
    let probe = render_scope_probe()?;
    assert_scope_probe(&probe);
    assert!(
        family_scope_cache::lookup_runs(
            &probe.database,
            probe.base,
            probe.family,
            TEST_NORMAL_WEIGHT,
            false,
            "AV"
        )
        .is_none()
    );
    Ok(())
}

struct ScopeProbe {
    database: Arc<Database>,
    base: ID,
    family: &'static str,
    first: Vec<(ID, String)>,
    second: Vec<(ID, String)>,
    expected: Vec<(ID, String)>,
    nested_runs: Vec<(ID, String)>,
    cache_states: [bool; CACHE_STATE_COUNT],
}

fn render_scope_probe() -> Result<ScopeProbe, String> {
    let family = "Helvetica Neue, Hiragino Kaku Gothic ProN";
    let (database, base) = macos_font_database(family, "public macOS author family")?;
    let (other_database, other_base) =
        macos_font_database(family, "second public macOS author family")?;
    let (runs, cache_states) = with_render_scoped_stamp_batch(|| {
        let runs = collect_scope_runs(&database, base, family);
        let states = scope_cache_states(&database, base, &other_database, other_base, family);
        (runs, states)
    });
    Ok(ScopeProbe {
        database,
        base,
        family,
        first: runs.first,
        second: runs.second,
        expected: runs.expected,
        nested_runs: runs.nested_runs,
        cache_states,
    })
}

struct ScopeRuns {
    first: Vec<(ID, String)>,
    second: Vec<(ID, String)>,
    expected: Vec<(ID, String)>,
    nested_runs: Vec<(ID, String)>,
}

fn collect_scope_runs(database: &Arc<Database>, base: ID, family: &str) -> ScopeRuns {
    let text = "AV日本AV日本";
    let first = resolved_font_runs(database, base, family, text, TEST_NORMAL_WEIGHT, false);
    let second = resolved_font_runs(database, base, family, text, TEST_NORMAL_WEIGHT, false);
    let expected = per_character_oracle(database, base, family, text, TEST_NORMAL_WEIGHT, false);
    let nested_runs = with_render_scoped_stamp_batch(|| {
        resolved_font_runs(database, base, family, text, TEST_NORMAL_WEIGHT, false)
    });
    ScopeRuns {
        first,
        second,
        expected,
        nested_runs,
    }
}

fn scope_cache_states(
    database: &Arc<Database>,
    base: ID,
    other_database: &Arc<Database>,
    other_base: ID,
    family: &str,
) -> [bool; CACHE_STATE_COUNT] {
    [
        cache_has_run(database, base, family, TEST_NORMAL_WEIGHT, false),
        !cache_has_run(database, base, family, TEST_BOLD_WEIGHT, false),
        !cache_has_run(database, base, "Helvetica Neue", TEST_NORMAL_WEIGHT, false),
        !cache_has_run(database, base, family, TEST_NORMAL_WEIGHT, true),
        !cache_has_run(database, ID::dummy(), family, TEST_NORMAL_WEIGHT, false),
        !cache_has_run(
            other_database,
            other_base,
            family,
            TEST_NORMAL_WEIGHT,
            false,
        ),
        cache_has_run(database, base, family, TEST_NORMAL_WEIGHT, false),
    ]
}

fn cache_has_run(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
) -> bool {
    family_scope_cache::lookup_runs(database, base, family, weight, italic, "AV").is_some()
}

fn macos_font_database(family: &str, label: &str) -> Result<(Arc<Database>, ID), String> {
    let mut database = Database::new();
    database.load_system_fonts();
    let database = Arc::new(database);
    let base = matching_font_face(&database, family, TEST_NORMAL_WEIGHT, false)
        .ok_or_else(|| format!("{label} must be available"))?;
    Ok((database, base))
}

fn assert_scope_probe(probe: &ScopeProbe) {
    assert!(probe.cache_states.iter().all(|state| *state));
    assert_eq!(probe.first, probe.second);
    assert_eq!(probe.second, probe.expected);
    assert_eq!(probe.first, probe.nested_runs);
}
