use super::super::file_generation::FontSourceGeneration;
use resvg::usvg::fontdb::{Database, ID};
use std::collections::VecDeque;
use std::sync::{Arc, Weak};

pub(super) const MAX_CACHE_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_ENTRY_BYTES: usize = 1024 * 1024;
pub(super) const MAX_CACHE_ENTRIES: usize = 1024;

pub(super) struct Entry {
    database: Weak<Database>,
    face_id: ID,
    face_index: u32,
    generation: FontSourceGeneration,
    bytes: Arc<[u8]>,
}

#[derive(Default)]
pub(super) struct Cache {
    entries: VecDeque<Entry>,
    retained_bytes: usize,
}

impl Cache {
    pub(super) fn lookup(
        &mut self,
        database: &Arc<Database>,
        face_id: ID,
        face_index: u32,
        generation: &FontSourceGeneration,
    ) -> Option<Arc<[u8]>> {
        self.purge_dead_databases();
        let index = self
            .entries
            .iter()
            .position(|entry| same_face(entry, database, face_id, face_index));
        let index = index?;
        if self.entries[index].generation == *generation {
            return Some(Arc::clone(&self.entries[index].bytes));
        }
        self.remove_at(index);
        None
    }

    pub(super) fn insert(
        &mut self,
        database: &Arc<Database>,
        face_id: ID,
        face_index: u32,
        generation: FontSourceGeneration,
        bytes: Arc<[u8]>,
    ) {
        let byte_len = bytes.len();
        if byte_len > MAX_ENTRY_BYTES || byte_len > MAX_CACHE_BYTES {
            return;
        }
        self.purge_dead_databases();
        self.remove_matching_face(database, face_id, face_index);
        while self.entries.len() >= MAX_CACHE_ENTRIES
            || self.retained_bytes.saturating_add(byte_len) > MAX_CACHE_BYTES
        {
            self.remove_at(0);
        }
        self.retained_bytes += byte_len;
        self.entries.push_back(Entry {
            database: Arc::downgrade(database),
            face_id,
            face_index,
            generation,
            bytes,
        });
    }

    pub(super) fn remove_face(&mut self, database: &Arc<Database>, face_id: ID) {
        self.purge_dead_databases();
        self.entries.retain(|entry| {
            !(entry.face_id == face_id
                && entry
                    .database
                    .upgrade()
                    .is_some_and(|cached| Arc::ptr_eq(&cached, database)))
        });
        self.recount_bytes();
    }

    fn remove_matching_face(&mut self, database: &Arc<Database>, face_id: ID, face_index: u32) {
        self.entries
            .retain(|entry| !same_face(entry, database, face_id, face_index));
        self.recount_bytes();
    }

    fn purge_dead_databases(&mut self) {
        self.entries
            .retain(|entry| entry.database.upgrade().is_some());
        self.recount_bytes();
    }

    fn remove_at(&mut self, index: usize) {
        self.entries.remove(index);
        self.recount_bytes();
    }

    fn recount_bytes(&mut self) {
        self.retained_bytes = self.entries.iter().map(|entry| entry.bytes.len()).sum();
    }
}

fn same_face(entry: &Entry, database: &Arc<Database>, face_id: ID, face_index: u32) -> bool {
    entry.face_id == face_id
        && entry.face_index == face_index
        && entry
            .database
            .upgrade()
            .is_some_and(|cached| Arc::ptr_eq(&cached, database))
}

#[cfg(test)]
#[path = "svg_rasterize_text_cmap_storage_tests.rs"]
mod tests;
