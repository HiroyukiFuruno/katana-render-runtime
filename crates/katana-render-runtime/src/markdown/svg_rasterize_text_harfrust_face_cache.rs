use harfrust::font::Font;
use resvg::usvg::fontdb::{Database, Source};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

#[path = "svg_rasterize_text_harfrust_face_cache_entry.rs"]
mod entry;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_text_harfrust_immutable_mapping.rs"]
pub(super) mod immutable_mapping;
#[path = "svg_rasterize_text_harfrust_lazy_table.rs"]
mod lazy_table;
#[path = "svg_rasterize_text_harfrust_face_cache_load.rs"]
mod load;
use entry::{CacheKey, FontHandle};
pub(super) use load::commit_file_font;

const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;

struct Entry {
    key: CacheKey,
    font: FontHandle,
}
struct ActiveCache {
    depth: usize,
    entry: Option<Entry>,
    spare: Option<Entry>,
}

impl ActiveCache {
    fn enter(&mut self) {
        if self.depth == 0 {
            self.entry = None;
            self.spare = None;
        }
        self.depth += 1;
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if self.depth == 0 {
            self.entry = None;
            self.spare = None;
        }
    }
}

thread_local! {
    static ACTIVE_SCOPE: RefCell<ActiveCache> = const { RefCell::new(ActiveCache { depth: 0, entry: None, spare: None }) };
}

pub(crate) struct FontFaceCacheScope(PhantomData<Rc<()>>);

impl FontFaceCacheScope {
    pub(crate) fn enter() -> Self {
        ACTIVE_SCOPE.with(|scope| scope.borrow_mut().enter());
        #[cfg(all(test, target_os = "macos"))]
        observation::publish();
        Self(PhantomData)
    }
}

impl Drop for FontFaceCacheScope {
    fn drop(&mut self) {
        ACTIVE_SCOPE.with(|scope| scope.borrow_mut().leave());
        #[cfg(all(test, target_os = "macos"))]
        observation::publish();
    }
}

fn cached_font(key: &CacheKey) -> Option<FontHandle> {
    #[cfg(all(test, target_os = "macos"))]
    let _observation = observation::Mutation::new(false);
    ACTIVE_SCOPE.with(|scope| {
        let mut scope = scope.borrow_mut();
        if scope.depth == 0 {
            scope.entry = None;
            scope.spare = None;
            #[cfg(all(test, target_os = "macos"))]
            observation::count(observation::Metric::MetricsMiss);
            return None;
        }
        if scope.entry.as_ref().is_some_and(|entry| entry.key == *key) {
            #[cfg(all(test, target_os = "macos"))]
            observation::count(observation::Metric::MetricsPrimaryHit);
            return scope.entry.as_ref().map(|entry| entry.font.clone());
        }
        if scope.spare.as_ref().is_some_and(|entry| entry.key == *key) {
            let ActiveCache { entry, spare, .. } = &mut *scope;
            std::mem::swap(entry, spare);
            #[cfg(all(test, target_os = "macos"))]
            observation::count(observation::Metric::MetricsSpareHit);
            return scope.entry.as_ref().map(|entry| entry.font.clone());
        }
        #[cfg(all(test, target_os = "macos"))]
        observation::count(observation::Metric::MetricsMiss);
        None
    })
}

pub(super) fn admit_source_budget(incoming: usize) {
    #[cfg(all(test, target_os = "macos"))]
    let _observation = observation::Mutation::new(true);
    ACTIVE_SCOPE.with(|scope| {
        let mut scope = scope.borrow_mut();
        if incoming > MAX_SOURCE_BYTES {
            scope.entry = None;
            scope.spare = None;
            return;
        }
        let available = MAX_SOURCE_BYTES - incoming;
        let retained = scope
            .entry
            .iter()
            .chain(scope.spare.iter())
            .map(|entry| entry.font.retained_source_bytes)
            .sum::<usize>();
        if retained > available {
            /* WHY: 次faceのtable読込開始前に古いlibrary backingを解放する。 */
            scope.spare = None;
        }
        if scope
            .entry
            .as_ref()
            .is_some_and(|entry| entry.font.retained_source_bytes > available)
        {
            scope.entry = None;
        }
    });
}

fn cache_font(key: CacheKey, font: FontHandle) {
    #[cfg(all(test, target_os = "macos"))]
    let _observation = observation::Mutation::new(false);
    ACTIVE_SCOPE.with(|scope| {
        let mut scope = scope.borrow_mut();
        if scope.depth == 0 {
            return;
        }
        if font.retained_source_bytes > MAX_SOURCE_BYTES {
            scope.entry = None;
            scope.spare = None;
            return;
        }
        if scope.entry.as_ref().is_some_and(|entry| entry.key == key) {
            scope.spare = scope.spare.take().filter(|entry| {
                entry.font.retained_source_bytes <= MAX_SOURCE_BYTES - font.retained_source_bytes
            });
            scope.entry = Some(Entry { key, font });
            return;
        }
        /* WHY: libraryのtable/metrics共有を2face間で保ち、合計source上限は増やさない。 */
        let previous = scope.entry.take();
        scope.spare = previous.filter(|entry| {
            entry.font.retained_source_bytes <= MAX_SOURCE_BYTES - font.retained_source_bytes
        });
        scope.entry = Some(Entry { key, font });
    });
}

pub(super) fn clear_cache() {
    #[cfg(all(test, target_os = "macos"))]
    let _observation = observation::Mutation::new(false);
    ACTIVE_SCOPE.with(|scope| {
        let mut scope = scope.borrow_mut();
        scope.entry = None;
        scope.spare = None;
    });
}

pub(super) fn font_for_face(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
) -> Option<FontHandle> {
    let (source, face_index) = match database.face_source(face_id) {
        Some(value) => value,
        None => {
            clear_cache();
            return None;
        }
    };
    match source {
        Source::Binary(data) => load::binary_font(data, face_index),
        Source::SharedFile(_, data) => {
            let data_len = data.as_ref().as_ref().len();
            admit_source_budget(data_len);
            Some(FontHandle {
                font: Font::new(data, face_index)?,
                key: None,
                poisoned: None,
                retained_source_bytes: data_len,
            })
        }
        Source::File(path) => load::file_font(database, face_id, path, face_index),
    }
}

#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_lazy_table_directory_tests.rs"]
mod lazy_table_directory_tests;
#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_face_cache_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_combined_metrics_fault_tests.rs"]
mod combined_metrics_fault_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_text_harfrust_face_cache_observation.rs"]
pub(in crate::markdown::svg_rasterize) mod observation;
