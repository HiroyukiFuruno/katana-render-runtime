pub(super) use super::entries::SelectorKey;
use super::entries::{self, CachedLookup};
use resvg::usvg::fontdb;
use std::sync::Arc;

pub(super) enum Lookup {
    Bypass,
    Hit(Option<fontdb::ID>),
    Miss(SelectorKey),
}

pub(super) fn lookup(
    database: &Arc<fontdb::Database>,
    character: char,
    excluded_ids: &[fontdb::ID],
) -> Lookup {
    if !super::super::font::memo_usable()
        || entries::selector_key_size(excluded_ids.len()) > entries::MAX_KEY_BYTES
    {
        return Lookup::Bypass;
    }
    let key = entries::selector_key(character, excluded_ids);
    let bytes = entries::key_bytes(&key);
    lookup_key(database, key, bytes)
}

fn lookup_key(database: &Arc<fontdb::Database>, key: SelectorKey, bytes: usize) -> Lookup {
    match entries::cached_lookup(database, &key, bytes) {
        CachedLookup::Hit(selection) => return Lookup::Hit(selection),
        CachedLookup::Miss => return Lookup::Miss(key),
        CachedLookup::Bypass => return Lookup::Bypass,
        CachedLookup::ObserveDatabase => {}
    }
    if !observe_database(database) || !super::super::font::memo_usable() {
        return Lookup::Bypass;
    }
    entries::add_database(database);
    /* WHY: 観測中はcallbackを呼ばないため、同threadの新規DBに選択結果はまだない。 */
    Lookup::Miss(key)
}

fn observe_database(database: &Arc<fontdb::Database>) -> bool {
    /* WHY: fallbackより前のface変更も選択結果へ影響するため全faceを観測する。 */
    for face in database.faces() {
        if !super::super::font::observe_file_face_generation(database, face.id)
            || !super::super::font::memo_usable()
        {
            return false;
        }
    }
    true
}
