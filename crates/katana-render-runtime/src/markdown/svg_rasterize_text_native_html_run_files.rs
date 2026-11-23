use super::super::super::font::platform_fallback::actual_fallback::{
    ActualCoreTextFace, ActualCoreTextFontMetadata, actual_face_for, face_index, name_table,
};
use super::super::super::font::{FileStamp, observe_path_stamp};
use super::super::super::harfrust::{MappedFontBytes, eligible_immutable_system_path};
use super::MAX_PHYSICAL_FONTS;
use objc2_core_text::CTFont;
use resvg::usvg::fontdb::{Database, ID, Source};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

#[path = "svg_rasterize_text_native_html_mapping_pool.rs"]
mod mapping_pool;

const SHA256_DIGEST_BYTES: usize = 32;
const MAPPED_FONT_SESSION_BYTES: u64 = 64 * 1024 * 1024;
type NameTableDigest = [u8; SHA256_DIGEST_BYTES];

#[derive(Clone)]
pub(super) struct FileFace {
    pub(super) path: PathBuf,
    pub(super) name: String,
    pub(super) stamp: FileStamp,
    pub(super) database: Arc<Database>,
    pub(super) id: ID,
    pub(super) name_table: Option<NameTableDigest>,
}

impl FileFace {
    fn from_actual(
        actual: ActualCoreTextFace,
        stamp: FileStamp,
        name_table: Option<NameTableDigest>,
        mapped: Option<MappedFontBytes>,
    ) -> Option<Self> {
        let (database, id) = verified_database(&actual, &stamp, mapped)?;
        Some(Self {
            path: actual.canonical_path,
            name: actual.post_script_name,
            stamp,
            database,
            id,
            name_table,
        })
    }

    pub(super) fn matches(&self, metadata: &ActualCoreTextFontMetadata, stamp: &FileStamp) -> bool {
        self.path == metadata.canonical_path
            && self.name == metadata.post_script_name
            && &self.stamp == stamp
    }

    pub(super) fn with_face_data<T>(&self, inspect: impl FnOnce(&[u8], u32) -> T) -> Option<T> {
        self.database.with_face_data(self.id, inspect)
    }

    fn mapped_bytes(&self) -> u64 {
        match self.database.face_source(self.id) {
            Some((Source::SharedFile(_, data), _)) => data.as_ref().as_ref().len() as u64,
            _ => 0,
        }
    }
}

#[derive(Clone, Default)]
pub(in crate::markdown::svg_rasterize::text_shaping) struct NativeFontFileCache(
    pub(super) Rc<RefCell<Vec<FileFace>>>,
);

impl NativeFontFileCache {
    pub(super) fn resolve(
        &self,
        font: &CTFont,
        metadata: &ActualCoreTextFontMetadata,
        stamp: FileStamp,
    ) -> Option<FileFace> {
        /* WHY: 選択されたname tableも照合し、同じ物理書体の解析結果だけを再利用する。 */
        let name_table = stamp
            .durable_reusable()
            .then(|| name_table_digest(font))
            .flatten();
        if let Some(face) = name_table.and_then(|digest| self.find(metadata, &stamp, digest)) {
            return Some(face);
        }
        let mapped = name_table.and_then(|_| self.map_face(metadata, &stamp));
        let actual = actual_face(font, metadata, mapped.as_ref())?;
        let face = FileFace::from_actual(actual, stamp, name_table, mapped)?;
        if name_table.is_some() {
            self.store(face.clone());
        }
        Some(face)
    }

    fn map_face(
        &self,
        metadata: &ActualCoreTextFontMetadata,
        stamp: &FileStamp,
    ) -> Option<MappedFontBytes> {
        let resident = self
            .0
            .borrow()
            .iter()
            .map(FileFace::mapped_bytes)
            .sum::<u64>();
        map_source(
            &metadata.canonical_path,
            stamp,
            MAPPED_FONT_SESSION_BYTES.saturating_sub(resident),
        )
    }

    pub(super) fn find(
        &self,
        metadata: &ActualCoreTextFontMetadata,
        stamp: &FileStamp,
        name_table: NameTableDigest,
    ) -> Option<FileFace> {
        self.0
            .borrow()
            .iter()
            .find(|face| face.matches(metadata, stamp) && face.name_table == Some(name_table))
            .cloned()
    }

    pub(super) fn store(&self, face: FileFace) {
        let mut faces = self.0.borrow_mut();
        faces.retain(|old| old.path != face.path || old.name != face.name);
        if faces.len() >= MAX_PHYSICAL_FONTS {
            faces.remove(0);
        }
        faces.push(face);
    }
}

fn map_source(path: &Path, stamp: &FileStamp, remaining: u64) -> Option<MappedFontBytes> {
    let length = std::fs::metadata(path).ok()?.len();
    if length > remaining || !eligible_immutable_system_path(path, length).ok()? {
        return None;
    }
    let bytes = mapping_pool::acquire(path, stamp, length)?;
    (observe_path_stamp(path).as_ref() == Ok(stamp)).then_some(bytes)
}

fn actual_face(
    font: &CTFont,
    metadata: &ActualCoreTextFontMetadata,
    mapped: Option<&MappedFontBytes>,
) -> Option<ActualCoreTextFace> {
    let Some(bytes) = mapped else {
        return actual_face_for(font);
    };
    let face_index = face_index(bytes.as_ref().as_ref(), &metadata.post_script_name, font)?;
    Some(ActualCoreTextFace {
        canonical_path: metadata.canonical_path.clone(),
        face_index,
        post_script_name: metadata.post_script_name.clone(),
    })
}

pub(super) fn name_table_digest(font: &CTFont) -> Option<NameTableDigest> {
    Some(Sha256::digest(name_table(font)?).into())
}

pub(super) fn verified_database(
    actual: &ActualCoreTextFace,
    stamp: &FileStamp,
    mapped: Option<MappedFontBytes>,
) -> Option<(Arc<Database>, ID)> {
    let mut database = Database::new();
    let path = actual.canonical_path.clone();
    /* WHY: 書体の検証で取得した読み取り専用mappingを文字計測とラスタライズでも使う。 */
    let source = match mapped {
        Some(data) => Source::SharedFile(path, data),
        None => Source::File(path),
    };
    database.load_font_source(source);
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
    Some((Arc::new(database), id))
}

#[cfg(test)]
#[path = "svg_rasterize_text_native_html_run_files_tests.rs"]
mod tests;
