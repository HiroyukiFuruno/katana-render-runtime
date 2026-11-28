use super::super::font::platform_fallback::actual_fallback::{
    ActualCoreTextFontMetadata, NativeHtmlFallback, actual_font_metadata_for,
};
use super::super::font::{FileStamp, observe_path_stamp};
use super::SelectionQuery;
use crate::markdown::svg_rasterize::native_html_faces;
use resvg::usvg::{
    FontVariation,
    fontdb::{Database, ID},
};
use std::{cell::RefCell, collections::HashMap, sync::Arc};

#[path = "svg_rasterize_text_native_html_run_axes.rs"]
mod axes;
#[path = "svg_rasterize_text_native_html_run_files.rs"]
mod files;
use files::FileFace;
pub(in crate::markdown::svg_rasterize::text_shaping) use files::NativeFontFileCache;

const MAX_STYLE_QUERIES: usize = 32;
const MAX_GLYPH_QUERIES: usize = 2048;
const MAX_PHYSICAL_FONTS: usize = 16;

type GlyphSelections = HashMap<char, Option<Arc<SelectedFace>>>;

#[derive(Default)]
pub(super) struct Memo {
    glyphs: HashMap<SelectionKey, GlyphSelections>,
    faces: Vec<FileFace>,
    files: NativeFontFileCache,
}

thread_local! {
    static MEMO: RefCell<Option<Memo>> = const { RefCell::new(None) };
}

pub(in crate::markdown::svg_rasterize::text_shaping) struct Scope(Option<Memo>);

impl Scope {
    pub(in crate::markdown::svg_rasterize::text_shaping) fn enter(
        files: &NativeFontFileCache,
    ) -> Self {
        let frame = Memo {
            glyphs: HashMap::new(),
            faces: Vec::new(),
            files: files.clone(),
        };
        Self(MEMO.with(|cache| cache.replace(Some(frame))))
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
        let result = query.native_query()?.select(ch)?;
        let file = self.physical_face(&result.font)?;
        selected_face(file, result, ch)
    }

    fn physical_face(&mut self, font: &objc2_core_text::CTFont) -> Option<&FileFace> {
        let metadata = actual_font_metadata_for(font)?;
        let stamp = observe_path_stamp(&metadata.canonical_path).ok()?;
        if let Some(index) = self
            .faces
            .iter()
            .position(|file| file.matches(&metadata, &stamp))
        {
            return self.faces.get(index);
        }
        self.load_face(font, &metadata, stamp)
    }

    fn load_face(
        &mut self,
        font: &objc2_core_text::CTFont,
        metadata: &ActualCoreTextFontMetadata,
        stamp: FileStamp,
    ) -> Option<&FileFace> {
        if self.faces.len() >= MAX_PHYSICAL_FONTS {
            return None;
        }
        let face = self.files.resolve(font, metadata, stamp, &self.faces)?;
        self.faces.push(face);
        self.faces.last()
    }
}

fn selected_face(file: &FileFace, result: NativeHtmlFallback, ch: char) -> Option<SelectedFace> {
    let face = file.database.face(file.id)?;
    let (variations, outline) = file.with_face_data(|bytes, index| {
        axes::selected_font_axes(result, face, bytes, index, ch)
    })??;
    let alias = native_html_faces::register(face, outline)?;
    Some(SelectedFace {
        database: Arc::clone(&file.database),
        id: file.id,
        alias,
        variations,
    })
}

#[cfg(test)]
#[path = "svg_rasterize_text_native_html_run_memo_tests.rs"]
mod tests;
