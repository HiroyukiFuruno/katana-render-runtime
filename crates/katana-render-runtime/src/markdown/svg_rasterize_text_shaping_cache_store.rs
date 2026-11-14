use super::{
    Arc, CachedShapedRun, Digest, FontShape, FontSourceGeneration, HashMap,
    MAX_CACHED_SHAPED_RUN_BYTES, MAX_CACHED_SHAPED_RUN_DATABASES, MAX_CACHED_SHAPED_RUNS,
    SHAPED_RUN_CACHE, Sha256, ShapedRunCacheEntry, ShapedRunKey, generation, usvg,
};

pub(super) fn cached_shaped_run_record_for_generation(
    database: &Arc<usvg::fontdb::Database>,
    key: &ShapedRunKey,
    generation: &FontSourceGeneration,
) -> Option<CachedShapedRun> {
    if !generation.reusable() {
        return None;
    }
    SHAPED_RUN_CACHE.with(|cache| {
        let mut entries = cache.borrow_mut();
        entries.retain(|entry| entry.database.upgrade().is_some());
        entries
            .iter_mut()
            .find(|entry| {
                entry
                    .database
                    .upgrade()
                    .is_some_and(|cached| Arc::ptr_eq(&cached, database))
            })
            .and_then(|entry| generation::cached_run(entry, key, generation))
    })
}

pub(super) fn cache_shaped_run_width(
    database: &Arc<usvg::fontdb::Database>,
    key: ShapedRunKey,
    width: f32,
    validated_generation: FontSourceGeneration,
) {
    cache_shaped_run_record(
        database,
        key,
        CachedShapedRun {
            advance: width,
            svg_endpoints: None,
            ink_bounds: None,
        },
        validated_generation,
    );
}

pub(super) fn cache_shaped_run_record(
    database: &Arc<usvg::fontdb::Database>,
    key: ShapedRunKey,
    run: CachedShapedRun,
    validated_generation: FontSourceGeneration,
) {
    SHAPED_RUN_CACHE.with(|cache| {
        let mut entries = cache.borrow_mut();
        entries.retain(|entry| entry.database.upgrade().is_some());
        let entry = shaped_run_cache_entry(&mut entries, database);
        let key_bytes = key.retained_bytes();
        if key_bytes > MAX_CACHED_SHAPED_RUN_BYTES {
            return;
        }
        /* WHY: 検証後の置換が古い測定値を新世代へ昇格させないよう、検証済み世代で保存する。 */
        generation::synchronize(entry, key.face_id, &validated_generation);
        generation::remove_width(entry, &key);
        evict_shaped_runs_for_insertion(entry, key_bytes);
        generation::insert_run(entry, key, run, validated_generation);
    });
}

pub(super) fn evict_shaped_runs_for_insertion(entry: &mut ShapedRunCacheEntry, key_bytes: usize) {
    while entry.retained_bytes + key_bytes > MAX_CACHED_SHAPED_RUN_BYTES
        || entry.values.len() >= MAX_CACHED_SHAPED_RUNS
    {
        let Some(evicted_key) = entry.values.keys().next().cloned() else {
            break;
        };
        generation::remove_width(entry, &evicted_key);
    }
}

impl ShapedRunKey {
    pub(super) fn new(
        face_id: usvg::fontdb::ID,
        text: &str,
        font: FontShape,
        features: Option<&str>,
    ) -> Self {
        /* WHY: session close後も生きる幅cacheへ文書本文を保持しない。 */
        Self {
            face_id,
            text_sha256: Sha256::digest(text.as_bytes()).into(),
            source_bytes: text.len().saturating_add(features.map_or(0, str::len)),
            font_size_bits: font.size.to_bits(),
            weight: font.weight,
            italic: font.italic,
            font_feature_sha256: features.map(|value| Sha256::digest(value.as_bytes()).into()),
            trim: font.trim,
        }
    }

    pub(super) fn retained_bytes(&self) -> usize {
        self.source_bytes
            .saturating_add(std::mem::size_of::<CachedShapedRun>())
    }
}

pub(super) fn shaped_run_cache_entry<'a>(
    entries: &'a mut Vec<ShapedRunCacheEntry>,
    database: &Arc<usvg::fontdb::Database>,
) -> &'a mut ShapedRunCacheEntry {
    if let Some(index) = entries.iter().position(|entry| {
        entry
            .database
            .upgrade()
            .is_some_and(|cached| Arc::ptr_eq(&cached, database))
    }) {
        return &mut entries[index];
    }
    if entries.len() == MAX_CACHED_SHAPED_RUN_DATABASES {
        entries.remove(0);
    }
    entries.push(ShapedRunCacheEntry {
        database: Arc::downgrade(database),
        values: HashMap::new(),
        generations: HashMap::new(),
        retained_bytes: 0,
    });
    let inserted_index = entries.len() - 1;
    &mut entries[inserted_index]
}
