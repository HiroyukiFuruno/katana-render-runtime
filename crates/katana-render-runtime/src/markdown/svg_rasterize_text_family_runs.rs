use super::fallback::html_font_runs;
use super::family_scope_cache;
use super::font::{
    append_font_run, cached_font_has_char_with_generation, matching_font_face,
    with_reusable_stamp_batch,
};
use resvg::usvg::fontdb::{Database, ID};
use std::{collections::HashMap, sync::Arc};

const MAX_SELECTED_FONT_MEMO_ENTRIES: usize = 256;

pub(super) fn resolved_font_runs(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Vec<(ID, String)> {
    with_reusable_stamp_batch(|| {
        resolve_font_runs_once(database, base, family, text, weight, italic)
    })
}

fn resolve_font_runs_once(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Vec<(ID, String)> {
    if let Some(result) = cached_font_runs(database, base, family, text, weight, italic) {
        return result;
    }
    if let Some(result) = try_scoped_font_runs(database, base, family, text, (weight, italic)) {
        return result;
    }
    resolve_uncached_font_runs(database, base, family, text, weight, italic)
}

fn try_scoped_font_runs(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
    style: (u16, bool),
) -> Option<Vec<(ID, String)>> {
    let (known, missing) =
        family_scope_cache::lookup_known(database, base, family, style.0, style.1, text)?;
    if missing.is_empty() {
        return rebuild_font_runs(text, &known);
    }
    if known.len() + missing.chars().count() > MAX_SELECTED_FONT_MEMO_ENTRIES {
        return None;
    }
    resolve_partial_font_runs(database, base, family, text, style, known, &missing)
}

fn resolve_partial_font_runs(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
    style: (u16, bool),
    mut selections: HashMap<char, ID>,
    missing: &str,
) -> Option<Vec<(ID, String)>> {
    let (weight, italic) = style;
    let candidates = family_candidates(database, family, weight, italic);
    let mut selected_font_memo = HashMap::new();
    for (fallback, run) in html_font_runs(database, base, missing, weight, italic) {
        for character in run.chars() {
            let selected = select_font_face(
                database,
                &candidates,
                character,
                fallback,
                &mut selected_font_memo,
            );
            selections.insert(character, selected);
        }
    }
    if !super::font::memo_usable() {
        return None;
    }
    family_scope_cache::store(database, base, family, weight, italic, selections.clone());
    rebuild_font_runs(text, &selections)
}

fn rebuild_font_runs(text: &str, selections: &HashMap<char, ID>) -> Option<Vec<(ID, String)>> {
    let mut result = Vec::new();
    for character in text.chars() {
        append_font_run(&mut result, *selections.get(&character)?, character);
    }
    Some(result)
}

fn resolve_uncached_font_runs(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Vec<(ID, String)> {
    let candidates = family_candidates(database, family, weight, italic);
    let mut result = Vec::new();
    let mut selected_font_memo = HashMap::new();
    let mut selections = HashMap::new();
    for (fallback, run) in html_font_runs(database, base, text, weight, italic) {
        for character in run.chars() {
            let selected = select_font_face(
                database,
                &candidates,
                character,
                fallback,
                &mut selected_font_memo,
            );
            if selections.len() < MAX_SELECTED_FONT_MEMO_ENTRIES {
                selections.entry(character).or_insert(selected);
            }
            append_font_run(&mut result, selected, character);
        }
    }
    family_scope_cache::store(database, base, family, weight, italic, selections);
    result
}

fn select_font_face(
    database: &Arc<Database>,
    candidates: &[ID],
    character: char,
    fallback: ID,
    memo: &mut HashMap<char, ID>,
) -> ID {
    if !super::font::memo_usable() {
        return ordered_font_face(database, candidates, character).unwrap_or(fallback);
    }
    let selected = memo
        .get(&character)
        .copied()
        .or_else(|| ordered_font_face(database, candidates, character));
    if super::font::memo_usable()
        && memo.len() < MAX_SELECTED_FONT_MEMO_ENTRIES
        && let Some(selected) = selected
    {
        memo.insert(character, selected);
    }
    selected.unwrap_or(fallback)
}

fn ordered_font_face(database: &Arc<Database>, candidates: &[ID], character: char) -> Option<ID> {
    candidates
        .iter()
        .copied()
        .find(|id| cached_font_has_char_with_generation(database, *id, character).0 == Some(true))
}

fn cached_font_runs(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Option<Vec<(ID, String)>> {
    family_scope_cache::lookup_runs(database, base, family, weight, italic, text)
}

fn family_candidates(database: &Database, family: &str, weight: u16, italic: bool) -> Vec<ID> {
    let mut candidates = family
        .split(',')
        .filter_map(|name| matching_font_face(database, name.trim(), weight, italic))
        .collect::<Vec<_>>();
    if family.split(',').any(is_system_family) {
        /* WHY: 日本語のsystem fallbackは、明示されたauthor familyをすべて試した後に使う。 */
        candidates.extend(matching_font_face(
            database,
            ".Hiragino Kaku Gothic Interface",
            weight,
            italic,
        ));
    }
    candidates
}

fn is_system_family(name: &str) -> bool {
    matches!(
        name.trim()
            .trim_matches(['\'', '"'])
            .to_ascii_lowercase()
            .as_str(),
        "system-ui" | "-apple-system" | "blinkmacsystemfont"
    )
}

#[cfg(test)]
#[path = "svg_rasterize_text_family_runs_tests.rs"]
mod tests;

#[cfg(test)]
mod boundary_tests {
    use super::{
        family_scope_cache, resolve_partial_font_runs, select_font_face, try_scoped_font_runs,
    };
    use resvg::usvg::fontdb::{Database, ID};
    use std::{collections::HashMap, sync::Arc};

    const WEIGHT: u16 = 400;
    const FAMILY: &str = "Noto Sans";

    fn real_font_database() -> Result<(Arc<Database>, ID), String> {
        let mut database = Database::new();
        database.load_font_data(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec());
        let base = database
            .faces()
            .next()
            .ok_or("bundled font did not load")?
            .id;
        Ok((Arc::new(database), base))
    }

    #[test]
    fn complete_scoped_selection_reconstructs_actual_text() -> Result<(), String> {
        let (database, base) = real_font_database()?;
        super::super::font::with_validated_stamp_batch(|| {
            let _scope = family_scope_cache::Scope::enter();
            family_scope_cache::store(
                &database,
                base,
                FAMILY,
                WEIGHT,
                false,
                [('A', base), ('V', base)],
            );
            assert_eq!(
                try_scoped_font_runs(&database, base, FAMILY, "AVA", (WEIGHT, false)),
                Some(vec![(base, "AVA".to_owned())])
            );
        });
        Ok(())
    }

    #[test]
    fn disabled_memo_selects_real_face_without_retaining_a_character() -> Result<(), String> {
        let (database, base) = real_font_database()?;
        assert!(!super::super::font::memo_usable());
        let mut memo = HashMap::new();
        assert_eq!(
            select_font_face(&database, &[base], 'A', ID::dummy(), &mut memo),
            base
        );
        assert!(memo.is_empty());
        Ok(())
    }

    #[test]
    fn partial_selection_without_stamp_scope_is_not_committed() -> Result<(), String> {
        let (database, base) = real_font_database()?;
        assert!(!super::super::font::memo_usable());
        let _scope = family_scope_cache::Scope::enter();
        let known = HashMap::from([('A', base)]);
        assert!(
            resolve_partial_font_runs(&database, base, FAMILY, "AB", (WEIGHT, false), known, "B")
                .is_none()
        );
        assert!(
            family_scope_cache::lookup_runs(&database, base, FAMILY, WEIGHT, false, "AB").is_none()
        );
        Ok(())
    }
}
