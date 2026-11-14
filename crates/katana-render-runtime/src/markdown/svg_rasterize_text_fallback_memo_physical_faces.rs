use super::super::super::font::FontSourceGeneration;
use resvg::usvg::fontdb;

pub(in super::super::super) fn physical_face_lookup(
    database: &fontdb::Database,
    id: fontdb::ID,
    generation: &FontSourceGeneration,
) -> Option<bool> {
    super::super::scope::ACTIVE_MEMO.with(|active| {
        let active = active.borrow();
        let memo = active.as_ref()?;
        let entry = memo.databases.iter().find(|entry| {
            entry
                .database
                .upgrade()
                .is_some_and(|db| std::ptr::eq(db.as_ref(), database))
        })?;
        let (cached_generation, result) = entry.physical_faces.get(&id)?;
        (cached_generation == generation).then_some(*result)
    })
}

pub(in super::super::super) fn physical_face_store(
    database: &fontdb::Database,
    id: fontdb::ID,
    generation: FontSourceGeneration,
    result: bool,
) {
    if !super::super::super::font::memo_usable() {
        return;
    }
    super::super::scope::ACTIVE_MEMO.with(|active| {
        let mut active = active.borrow_mut();
        let Some(memo) = active.as_mut() else { return };
        let Some(entry) = memo.databases.iter_mut().find(|entry| {
            entry
                .database
                .upgrade()
                .is_some_and(|db| std::ptr::eq(db.as_ref(), database))
        }) else {
            return;
        };
        if entry.physical_faces.len() < super::MAX_ENTRIES {
            entry.physical_faces.insert(id, (generation, result));
        }
    });
}
