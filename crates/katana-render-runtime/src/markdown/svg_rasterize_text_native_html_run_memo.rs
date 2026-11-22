use super::super::font::platform_fallback::actual_fallback::{
    ActualCoreTextFace, NativeHtmlFallback, actual_face_for, actual_font_metadata_for,
    native_html_fallback,
};
use super::super::font::{FileStamp, observe_path_stamp};
use super::SelectionQuery;
use crate::markdown::svg_rasterize::native_html_faces;
use resvg::usvg::{
    FontVariation,
    fontdb::{Database, ID},
};
use std::{cell::RefCell, collections::HashMap, path::PathBuf, sync::Arc};

#[path = "svg_rasterize_text_native_html_run_axes.rs"]
mod axes;

const MAX_STYLE_QUERIES: usize = 32;
const MAX_GLYPH_QUERIES: usize = 2048;
const MAX_PHYSICAL_FONTS: usize = 16;

type GlyphSelections = HashMap<char, Option<Arc<SelectedFace>>>;

#[derive(Default)]
pub(super) struct Memo {
    glyphs: HashMap<SelectionKey, GlyphSelections>,
    faces: Vec<FileFace>,
}

thread_local! {
    static MEMO: RefCell<Option<Memo>> = const { RefCell::new(None) };
}

pub(in crate::markdown::svg_rasterize::text_shaping) struct Scope(Option<Memo>);

impl Scope {
    pub(in crate::markdown::svg_rasterize::text_shaping) fn enter() -> Self {
        Self(MEMO.with(|cache| cache.replace(Some(Memo::default()))))
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        MEMO.with(|cache| {
            let _ = cache.replace(self.0.take());
        });
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct SelectionKey {
    pub(super) base: String,
    pub(super) size: u32,
    pub(super) weight: u16,
    pub(super) language: Option<String>,
}

struct FileFace {
    path: PathBuf,
    name: String,
    stamp: FileStamp,
    database: Arc<Database>,
    id: ID,
}

pub(super) struct SelectedFace {
    pub(super) database: Arc<Database>,
    pub(super) id: ID,
    pub(super) alias: String,
    pub(super) variations: Vec<FontVariation>,
}

pub(super) fn with_current<T>(operation: impl FnOnce(&mut Memo) -> T) -> Option<T> {
    MEMO.with(|memo| memo.borrow_mut().as_mut().map(operation))
}

impl Memo {
    pub(super) fn selection(
        &mut self,
        query: &SelectionQuery<'_>,
        ch: char,
    ) -> Option<Arc<SelectedFace>> {
        let key = &query.key;
        if let Some(value) = self.glyphs.get(key).and_then(|glyphs| glyphs.get(&ch)) {
            return value.clone();
        }
        let selected = self.lookup(query, ch).map(Arc::new);
        if self.glyphs.contains_key(key) || self.glyphs.len() < MAX_STYLE_QUERIES {
            let glyphs = self.glyphs.entry(key.clone()).or_default();
            if glyphs.len() < MAX_GLYPH_QUERIES {
                glyphs.insert(ch, selected.clone());
            }
        }
        selected
    }

    fn lookup(&mut self, query: &SelectionQuery<'_>, ch: char) -> Option<SelectedFace> {
        let result = native_html_fallback(
            query.native_base(),
            ch,
            f64::from(f32::from_bits(query.key.size)),
            query.key.weight,
            query.key.language.as_deref(),
        )?;
        let (database, id) = self.physical_face(&result.font)?;
        selected_face(database, id, result, ch)
    }

    fn physical_face(&mut self, font: &objc2_core_text::CTFont) -> Option<(Arc<Database>, ID)> {
        let metadata = actual_font_metadata_for(font)?;
        let stamp = observe_path_stamp(&metadata.canonical_path).ok()?;
        let file = self.faces.iter().find(|file| {
            file.path == metadata.canonical_path
                && file.name == metadata.post_script_name
                && file.stamp == stamp
        });
        match file {
            Some(face) => Some((Arc::clone(&face.database), face.id)),
            None => self.load_face(font, stamp),
        }
    }

    fn load_face(
        &mut self,
        font: &objc2_core_text::CTFont,
        stamp: FileStamp,
    ) -> Option<(Arc<Database>, ID)> {
        if self.faces.len() >= MAX_PHYSICAL_FONTS {
            return None;
        }
        let actual = actual_face_for(font)?;
        let (database, id) = verified_database(&actual, &stamp)?;
        self.faces.push(FileFace {
            path: actual.canonical_path,
            name: actual.post_script_name,
            stamp,
            database: Arc::clone(&database),
            id,
        });
        Some((database, id))
    }
}

fn selected_face(
    database: Arc<Database>,
    id: ID,
    result: NativeHtmlFallback,
    ch: char,
) -> Option<SelectedFace> {
    let face = database.face(id)?;
    let (variations, outline) = database.with_face_data(id, |bytes, index| {
        axes::selected_font_axes(result, face, bytes, index, ch)
    })??;
    let alias = native_html_faces::register(face, outline)?;
    Some(SelectedFace {
        database,
        id,
        alias,
        variations,
    })
}

fn verified_database(
    actual: &ActualCoreTextFace,
    stamp: &FileStamp,
) -> Option<(Arc<Database>, ID)> {
    let mut database = Database::new();
    database.load_font_file(&actual.canonical_path).ok()?;
    let id = database
        .faces()
        .find(|face| face.index == actual.face_index)?
        .id;
    if &observe_path_stamp(&actual.canonical_path).ok()? != stamp {
        return None;
    }
    let mut face = database.face(id)?.clone();
    database.remove_face(id);
    face.post_script_name.clone_from(&actual.post_script_name);
    let id = database.push_face_info(face);
    let database = Arc::new(database);
    Some((database, id))
}

#[cfg(test)]
#[path = "svg_rasterize_text_native_html_run_memo_tests.rs"]
mod tests;
