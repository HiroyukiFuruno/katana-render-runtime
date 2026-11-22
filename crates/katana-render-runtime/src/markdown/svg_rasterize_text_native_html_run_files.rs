use super::super::super::font::platform_fallback::actual_fallback::{
    ActualCoreTextFace, ActualCoreTextFontMetadata, actual_face_for, name_table,
};
use super::super::super::font::{FileStamp, observe_path_stamp};
use super::MAX_PHYSICAL_FONTS;
use objc2_core_text::CTFont;
use resvg::usvg::fontdb::{Database, ID};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

const SHA256_DIGEST_BYTES: usize = 32;
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
    pub(super) fn matches(&self, metadata: &ActualCoreTextFontMetadata, stamp: &FileStamp) -> bool {
        self.path == metadata.canonical_path
            && self.name == metadata.post_script_name
            && &self.stamp == stamp
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
        let actual = actual_face_for(font)?;
        let (database, id) = verified_database(&actual, &stamp)?;
        let face = FileFace {
            path: actual.canonical_path,
            name: actual.post_script_name,
            stamp,
            database,
            id,
            name_table,
        };
        if name_table.is_some() {
            self.store(face.clone());
        }
        Some(face)
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

pub(super) fn name_table_digest(font: &CTFont) -> Option<NameTableDigest> {
    Some(Sha256::digest(name_table(font)?).into())
}

pub(super) fn verified_database(
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
    Some((Arc::new(database), id))
}
