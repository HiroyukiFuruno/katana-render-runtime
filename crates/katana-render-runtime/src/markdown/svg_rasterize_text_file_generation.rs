use resvg::usvg::fontdb::{Database, ID, Source};

pub(in super::super) fn observe_file_face_generation(database: &Database, id: ID) -> bool {
    /* WHY: 未選択の候補ファイルの変更も描画の選択結果へ影響する。 */
    let (is_file, stamp, _) = file_source_stamp(database, id);
    !is_file || stamp.is_some()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileStamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct FontSourceGeneration(Generation);

#[derive(Clone, Debug, Eq, PartialEq)]
enum Generation {
    Immutable,
    File(FileStamp),
    Unavailable,
}

impl FontSourceGeneration {
    pub(in super::super) fn reusable(&self) -> bool {
        !matches!(self.0, Generation::Unavailable)
    }

    pub(in super::super) fn is_file(&self) -> bool {
        matches!(self.0, Generation::File(_))
    }
}

pub(super) fn file_source_stamp(
    database: &Database,
    id: ID,
) -> (bool, Option<FileStamp>, FontSourceGeneration) {
    match database.face_source(id) {
        Some((Source::File(path), _)) => match stamp_path(&path) {
            Ok(stamp) => (
                true,
                Some(stamp.clone()),
                FontSourceGeneration(Generation::File(stamp)),
            ),
            Err(()) => (true, None, FontSourceGeneration(Generation::Unavailable)),
        },
        Some(_) => (false, None, FontSourceGeneration(Generation::Immutable)),
        None => (false, None, FontSourceGeneration(Generation::Unavailable)),
    }
}

pub(in super::super) fn font_source_generation(
    database: &Database,
    id: ID,
) -> FontSourceGeneration {
    file_source_stamp(database, id).2
}

fn stamp_path(path: &std::path::Path) -> Result<FileStamp, ()> {
    if let Some(stamp) = super::stamp_batch::cached_stamp(path) {
        return Ok(stamp);
    }
    let result = stamp_path_uncached(path);
    match &result {
        Ok(stamp) => super::stamp_batch::remember_stamp(path, stamp.clone()),
        Err(()) => super::stamp_batch::mark_unavailable(),
    }
    result
}

pub(super) fn stamp_path_uncached(path: &std::path::Path) -> Result<FileStamp, ()> {
    let metadata = std::fs::metadata(path).map_err(|_| ())?;
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(FileStamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        created: metadata.created().ok(),
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        #[cfg(unix)]
        changed_seconds: metadata.ctime(),
        #[cfg(unix)]
        changed_nanoseconds: metadata.ctime_nsec(),
    })
}
