pub(super) use super::super::font::FontSourceGeneration;
use super::super::font::{cached_font_has_char_with_generation, font_source_generation};
use std::sync::Arc;

pub(super) const MAX_FILE_DEPENDENCIES: usize = 128;

pub(super) struct FileGenerationDependencies {
    pub(super) entries: Vec<(resvg::usvg::fontdb::ID, FontSourceGeneration)>,
    pub(super) reusable: bool,
}

impl FileGenerationDependencies {
    pub(super) fn new() -> Self {
        Self {
            entries: Vec::new(),
            reusable: true,
        }
    }

    pub(super) fn observe(
        &mut self,
        face_id: resvg::usvg::fontdb::ID,
        before: FontSourceGeneration,
        after: FontSourceGeneration,
    ) {
        if !before.reusable() || before != after {
            self.reusable = false;
        } else if before.is_file() {
            if self.entries.len() == MAX_FILE_DEPENDENCIES {
                self.reusable = false;
            } else {
                self.entries.push((face_id, before));
            }
        }
    }
}

pub(super) fn probe_with_generation(
    database: &Arc<resvg::usvg::fontdb::Database>,
    face_id: resvg::usvg::fontdb::ID,
    character: char,
    dependencies: &mut FileGenerationDependencies,
) -> Option<bool> {
    let (support, before) = cached_font_has_char_with_generation(database, face_id, character);
    let after = font_source_generation(database, face_id);
    dependencies.observe(face_id, before, after);
    support
}

#[derive(Clone)]
pub(super) struct HtmlFallbackSelection {
    pub(super) face_id: resvg::usvg::fontdb::ID,
    pub(super) dependencies: Arc<[(resvg::usvg::fontdb::ID, FontSourceGeneration)]>,
}

pub(super) struct FallbackResolution {
    pub(super) face_id: resvg::usvg::fontdb::ID,
    pub(super) cacheable: bool,
    pub(super) dependencies: FileGenerationDependencies,
}

impl FallbackResolution {
    pub(super) fn new(
        face_id: resvg::usvg::fontdb::ID,
        cacheable: bool,
        dependencies: FileGenerationDependencies,
    ) -> Self {
        Self {
            face_id,
            cacheable,
            dependencies,
        }
    }
}

pub(super) fn selection_for_cache(resolution: FallbackResolution) -> Option<HtmlFallbackSelection> {
    (resolution.cacheable && resolution.dependencies.reusable).then(|| HtmlFallbackSelection {
        face_id: resolution.face_id,
        dependencies: Arc::from(resolution.dependencies.entries),
    })
}

pub(super) fn selection_is_current(
    database: &resvg::usvg::fontdb::Database,
    selection: &HtmlFallbackSelection,
) -> bool {
    selection.dependencies.iter().all(|(face_id, expected)| {
        let current = font_source_generation(database, *face_id);
        current.reusable() && current == *expected
    })
}
