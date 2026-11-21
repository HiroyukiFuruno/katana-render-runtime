use super::{Cache, Context, ContextKey, cacheable_family, purge_dead_contexts};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

pub(super) fn store(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
    selections: impl IntoIterator<Item = (char, ID)>,
) {
    if !super::super::font::memo_usable() || !cacheable_family(family) {
        return;
    }
    super::ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(cache) = active.as_mut() else {
            return;
        };
        purge_dead_contexts(cache);
        let context_index = context_index(cache, database, base, family, weight, italic);
        store_pending(cache, context_index, selections);
    });
}

fn store_pending(
    cache: &mut Cache,
    context_index: usize,
    selections: impl IntoIterator<Item = (char, ID)>,
) {
    let pending = selections
        .into_iter()
        /* WHY: 部分検索では、上限内の既知文字に新規文字が1つ加わるため、満杯の文脈でも最古要素を置換できる分だけ読む。 */
        .take(super::MAX_CONTEXT_CHARS + 1)
        .collect::<Vec<_>>();
    let known = cache.contexts[context_index]
        .selections
        .keys()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut new = pending
        .iter()
        .filter(|(character, _)| !known.contains(character))
        .copied()
        .collect::<Vec<_>>();
    new.sort_unstable_by_key(|(character, _)| *character);
    for (character, selected) in new {
        if !store_selection(cache, context_index, character, selected) {
            return;
        }
    }
}

fn store_selection(cache: &mut Cache, context_index: usize, character: char, selected: ID) -> bool {
    if !super::super::font::memo_usable() {
        return false;
    }
    let context = &mut cache.contexts[context_index];
    if context.selections.contains_key(&character) {
        return true;
    }
    while context.selections.len() >= super::MAX_CONTEXT_CHARS {
        let Some(oldest) = context.insertion_order.pop_front() else {
            return false;
        };
        if context.selections.remove(&oldest).is_some() {
            cache.total_entries -= 1;
        }
    }
    if cache.total_entries >= super::MAX_TOTAL_ENTRIES {
        return false;
    }
    context.selections.insert(character, selected);
    context.insertion_order.push_back(character);
    cache.total_entries += 1;
    true
}

fn context_index(
    cache: &mut Cache,
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
) -> usize {
    if let Some(index) = existing_context_index(cache, database, base, family, weight, italic) {
        return index;
    }
    while cache.contexts.len() >= super::MAX_CONTEXTS
        || cache.total_family_bytes.saturating_add(family.len()) > super::MAX_TOTAL_FAMILY_BYTES
    {
        let context = cache.contexts.remove(0);
        cache.total_entries -= context.selections.len();
        cache.total_family_bytes -= context.key.family.len();
    }
    cache
        .contexts
        .push(new_context(database, base, family, weight, italic));
    cache.total_family_bytes += family.len();
    cache.contexts.len() - 1
}

fn existing_context_index(
    cache: &Cache,
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
) -> Option<usize> {
    cache.contexts.iter().position(|context| {
        super::same_context(&context.key, database, base, family, weight, italic)
    })
}

fn new_context(
    database: &Arc<Database>,
    base: ID,
    family: &str,
    weight: u16,
    italic: bool,
) -> Context {
    Context {
        key: ContextKey {
            database: Arc::downgrade(database),
            base,
            family: family.into(),
            weight,
            italic,
        },
        selections: std::collections::HashMap::new(),
        insertion_order: std::collections::VecDeque::new(),
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::{Arc, Cache, Database, ID, new_context, store_pending, store_selection};

    fn real_cache() -> Result<(Arc<Database>, ID, Cache), String> {
        let mut database = Database::new();
        database
            .load_font_data(include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf").to_vec());
        let base = database.faces().next().ok_or("real font missing")?.id;
        let database = Arc::new(database);
        let context = new_context(&database, base, "Noto Sans", 400, false);
        let cache = Cache {
            contexts: vec![context],
            ..Cache::default()
        };
        Ok((database, base, cache))
    }

    #[test]
    fn selection_preserves_known_character_and_refuses_exhausted_global_budget()
    -> Result<(), String> {
        let (_database, base, mut cache) = real_cache()?;
        super::super::super::font::with_validated_stamp_batch(|| {
            assert!(store_selection(&mut cache, 0, 'A', base));
            assert!(store_selection(&mut cache, 0, 'A', base));
            assert_eq!(cache.total_entries, 1);
            cache.total_entries = super::super::MAX_TOTAL_ENTRIES;
            assert!(!store_selection(&mut cache, 0, 'B', base));
            assert!(!cache.contexts[0].selections.contains_key(&'B'));
        });
        Ok(())
    }

    #[test]
    fn full_context_without_eviction_order_rejects_new_selection() -> Result<(), String> {
        let (_database, base, mut cache) = real_cache()?;
        cache.contexts[0].selections = (0..super::super::MAX_CONTEXT_CHARS)
            .filter_map(|offset| char::from_u32(0x100 + offset as u32))
            .map(|character| (character, base))
            .collect();
        cache.total_entries = cache.contexts[0].selections.len();
        super::super::super::font::with_validated_stamp_batch(|| {
            assert!(!store_selection(&mut cache, 0, 'A', base));
            assert_eq!(cache.total_entries, super::super::MAX_CONTEXT_CHARS);
        });
        Ok(())
    }

    #[test]
    fn selection_without_valid_stamp_scope_leaves_cache_unchanged() -> Result<(), String> {
        let (_database, base, mut cache) = real_cache()?;
        assert!(!store_selection(&mut cache, 0, 'A', base));
        assert_eq!(cache.total_entries, 0);
        assert!(cache.contexts[0].selections.is_empty());
        Ok(())
    }

    #[test]
    fn pending_selection_stops_at_global_limit_without_partial_insert() -> Result<(), String> {
        let (_database, base, mut cache) = real_cache()?;
        cache.total_entries = super::super::MAX_TOTAL_ENTRIES;
        super::super::super::font::with_validated_stamp_batch(|| {
            store_pending(&mut cache, 0, [('A', base), ('B', base)]);
            assert!(cache.contexts[0].selections.is_empty());
            assert_eq!(cache.total_entries, super::super::MAX_TOTAL_ENTRIES);
        });
        Ok(())
    }
}
