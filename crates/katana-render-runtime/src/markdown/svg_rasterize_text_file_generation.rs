use resvg::usvg::fontdb::{Database, ID, Source};

pub(in super::super) fn observe_file_face_generation(database: &Database, id: ID) -> bool {
    /* WHY: 未選択の候補ファイルの変更も描画の選択結果へ影響する。 */
    let (is_file, stamp, _) = file_source_stamp(database, id);
    !is_file || stamp.is_some()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::markdown::svg_rasterize) struct FileStamp {
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
    pub(super) durable_reusable: bool,
}

impl FileStamp {
    pub(in crate::markdown::svg_rasterize) fn durable_reusable(&self) -> bool {
        self.durable_reusable
    }
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
    #[cfg(all(test, target_os = "macos"))]
    pub(in crate::markdown::svg_rasterize) fn diagnostic_matches_stamp(
        &self,
        stamp: &FileStamp,
    ) -> bool {
        stamp.durable_reusable() && matches!(&self.0, Generation::File(current) if current == stamp)
    }

    pub(in super::super) fn reusable(&self) -> bool {
        !matches!(self.0, Generation::Unavailable)
    }

    pub(in super::super) fn durable_reusable(&self) -> bool {
        match &self.0 {
            Generation::Immutable => true,
            Generation::File(stamp) => stamp.durable_reusable,
            Generation::Unavailable => false,
        }
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
        Some((Source::File(path) | Source::SharedFile(path, _), _)) => match stamp_path(&path) {
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
        #[cfg(all(test, target_os = "macos"))]
        super::super::super::font::observation::count(
            super::super::super::font::observation::Metric::StampMemoHit,
        );
        return Ok(stamp);
    }
    #[cfg(all(test, target_os = "macos"))]
    super::super::super::font::observation::count(
        super::super::super::font::observation::Metric::StampMemoMiss,
    );
    let result = stamp_path_uncached(path);
    match &result {
        Ok(stamp) => super::stamp_batch::remember_stamp(path, stamp.clone()),
        Err(()) => super::stamp_batch::mark_unavailable(),
    }
    result
}

#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize) fn observe_path_stamp(
    path: &std::path::Path,
) -> Result<FileStamp, ()> {
    #[cfg(test)]
    super::super::super::font::observation::count(
        super::super::super::font::observation::Metric::PositiveObserve,
    );
    stamp_path(path)
}

pub(in super::super) fn font_source_generation_uncached(
    database: &Database,
    id: ID,
) -> FontSourceGeneration {
    match database.face_source(id) {
        Some((Source::File(path) | Source::SharedFile(path, _), _)) => FontSourceGeneration(
            stamp_path_uncached(&path)
                .map(Generation::File)
                .unwrap_or(Generation::Unavailable),
        ),
        Some(_) => FontSourceGeneration(Generation::Immutable),
        None => FontSourceGeneration(Generation::Unavailable),
    }
}

pub(in crate::markdown::svg_rasterize) fn stamp_path_uncached(
    path: &std::path::Path,
) -> Result<FileStamp, ()> {
    #[cfg(all(test, target_os = "macos"))]
    super::super::super::font::observation::count(
        super::super::super::font::observation::Metric::StampMetadata,
    );
    let metadata = std::fs::metadata(path).map_err(|_| ())?;
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let durable_reusable = super::filesystem_policy::durable_reusable(path, metadata.ctime_nsec());
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let durable_reusable = false;
    Ok(FileStamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        created: metadata.created().ok(),
        durable_reusable,
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

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_text_sharedfile_generation_tests.rs"]
mod sharedfile_tests;
