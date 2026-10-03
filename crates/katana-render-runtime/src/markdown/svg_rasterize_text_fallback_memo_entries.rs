use resvg::usvg::fontdb;
use std::{
    collections::HashMap,
    mem,
    sync::{Arc, Weak},
};

pub(super) const MAX_DATABASES: usize = 8;
pub(super) const MAX_ENTRIES: usize = 4096;
pub(super) const MAX_KEY_BYTES: usize = 1024 * 1024;

#[derive(Hash, Eq, PartialEq)]
pub(super) struct SelectorKey {
    character: char,
    excluded_ids: Box<[fontdb::ID]>,
}

struct DatabaseMemo {
    database: Weak<fontdb::Database>,
    selections: HashMap<SelectorKey, Option<fontdb::ID>>,
}

#[derive(Default)]
pub(super) struct SelectorMemo {
    databases: Vec<DatabaseMemo>,
    entries: usize,
    key_bytes: usize,
}

pub(super) enum CachedLookup {
    Bypass,
    Hit(Option<fontdb::ID>),
    Miss,
    ObserveDatabase,
}

pub(super) fn cached_lookup(
    database: &Arc<fontdb::Database>,
    key: &SelectorKey,
    bytes: usize,
) -> CachedLookup {
    super::scope::ACTIVE_MEMO
        .with(|active| cached_lookup_in_memo(active.borrow_mut().as_mut(), database, key, bytes))
}

fn cached_lookup_in_memo(
    memo: Option<&mut SelectorMemo>,
    database: &Arc<fontdb::Database>,
    key: &SelectorKey,
    bytes: usize,
) -> CachedLookup {
    let Some(memo) = memo else {
        return CachedLookup::Bypass;
    };
    let index = memo.databases.iter().position(|entry| {
        entry
            .database
            .upgrade()
            .is_some_and(|cached| Arc::ptr_eq(&cached, database))
    });
    match index {
        Some(index) => cached_entry_lookup(memo, index, key, bytes),
        None if memo.databases.len() < MAX_DATABASES && can_store(memo, bytes) => {
            CachedLookup::ObserveDatabase
        }
        None => CachedLookup::Bypass,
    }
}

fn cached_entry_lookup(
    memo: &SelectorMemo,
    index: usize,
    key: &SelectorKey,
    bytes: usize,
) -> CachedLookup {
    match memo.databases[index].selections.get(key) {
        Some(selection) => CachedLookup::Hit(*selection),
        None if can_store(memo, bytes) => CachedLookup::Miss,
        None => CachedLookup::Bypass,
    }
}

pub(super) fn add_database(database: &Arc<fontdb::Database>) {
    super::scope::ACTIVE_MEMO.with(|active| {
        let mut active = active.borrow_mut();
        let Some(memo) = active.as_mut() else {
            return;
        };
        if memo.databases.len() < MAX_DATABASES {
            memo.databases.push(DatabaseMemo {
                database: Arc::downgrade(database),
                selections: HashMap::new(),
            });
        }
    });
}

pub(super) fn store(
    database: &Arc<fontdb::Database>,
    key: SelectorKey,
    selection: Option<fontdb::ID>,
) {
    if !super::super::font::memo_usable() {
        return;
    }
    super::scope::ACTIVE_MEMO.with(|active| {
        let mut active = active.borrow_mut();
        let Some(memo) = active.as_mut() else {
            return;
        };
        insert_selection(memo, database, key, selection);
    });
}

fn insert_selection(
    memo: &mut SelectorMemo,
    database: &Arc<fontdb::Database>,
    key: SelectorKey,
    selection: Option<fontdb::ID>,
) {
    let bytes = key_bytes(&key);
    if !can_store(memo, bytes) {
        return;
    }
    let Some(entry) = memo.databases.iter_mut().find(|entry| {
        entry
            .database
            .upgrade()
            .is_some_and(|cached| Arc::ptr_eq(&cached, database))
    }) else {
        return;
    };
    entry.selections.insert(key, selection);
    memo.entries += 1;
    memo.key_bytes += bytes;
}

fn can_store(memo: &SelectorMemo, bytes: usize) -> bool {
    memo.entries < MAX_ENTRIES && memo.key_bytes.saturating_add(bytes) <= MAX_KEY_BYTES
}

pub(super) fn key_bytes(key: &SelectorKey) -> usize {
    mem::size_of::<char>()
        + key
            .excluded_ids
            .len()
            .saturating_mul(mem::size_of::<fontdb::ID>())
}

pub(super) fn selector_key_size(excluded_count: usize) -> usize {
    mem::size_of::<char>() + excluded_count.saturating_mul(mem::size_of::<fontdb::ID>())
}

pub(super) fn selector_key(character: char, excluded_ids: &[fontdb::ID]) -> SelectorKey {
    SelectorKey {
        character,
        excluded_ids: excluded_ids.into(),
    }
}
