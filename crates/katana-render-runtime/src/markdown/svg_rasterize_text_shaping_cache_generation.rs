use super::{CachedShapedRun, FontSourceGeneration, ShapedRunCacheEntry, ShapedRunKey};
use resvg::usvg::fontdb::ID;

pub(super) struct FaceGeneration {
    generation: FontSourceGeneration,
    widths: usize,
}

pub(super) fn cached_run(
    entry: &mut ShapedRunCacheEntry,
    key: &ShapedRunKey,
    generation: &FontSourceGeneration,
) -> Option<CachedShapedRun> {
    if entry
        .generations
        .get(&key.face_id)
        .is_some_and(|face| face.generation == *generation)
    {
        return entry.values.get(key).copied();
    }
    synchronize(entry, key.face_id, generation);
    None
}

pub(super) fn synchronize(
    entry: &mut ShapedRunCacheEntry,
    face_id: ID,
    generation: &FontSourceGeneration,
) {
    if entry
        .generations
        .get(&face_id)
        .is_some_and(|face| face.generation != *generation)
    {
        /* WHY: 新世代の印で旧世代の他の文字列幅を再利用しない。 */
        let mut removed_bytes = 0usize;
        entry.values.retain(|key, _| {
            if key.face_id == face_id {
                removed_bytes = removed_bytes.saturating_add(key.retained_bytes());
                return false;
            }
            true
        });
        entry.retained_bytes = entry.retained_bytes.saturating_sub(removed_bytes);
        entry.generations.remove(&face_id);
    }
}

pub(super) fn remove_width(entry: &mut ShapedRunCacheEntry, key: &ShapedRunKey) {
    if entry.values.remove(key).is_none() {
        return;
    }
    entry.retained_bytes = entry.retained_bytes.saturating_sub(key.retained_bytes());
    let remove_generation = entry.generations.get_mut(&key.face_id).is_some_and(|face| {
        face.widths = face.widths.saturating_sub(1);
        face.widths == 0
    });
    if remove_generation {
        /* WHY: 幅が残らない face の世代だけがキャッシュに蓄積するのを防ぐ。 */
        entry.generations.remove(&key.face_id);
    }
}

pub(super) fn insert_run(
    entry: &mut ShapedRunCacheEntry,
    key: ShapedRunKey,
    run: CachedShapedRun,
    generation: FontSourceGeneration,
) {
    entry.retained_bytes += key.retained_bytes();
    entry
        .generations
        .entry(key.face_id)
        .or_insert(FaceGeneration {
            generation,
            widths: 0,
        })
        .widths += 1;
    entry.values.insert(key, run);
}
