#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{fs::Metadata, path::Path, time::SystemTime};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::markdown::svg_rasterize) struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
    pub(in super::super) durable_reusable: bool,
}

impl FileStamp {
    pub(in crate::markdown::svg_rasterize) fn durable_reusable(&self) -> bool {
        self.durable_reusable
    }

    #[cfg(target_os = "macos")]
    pub(in crate::markdown::svg_rasterize) fn file_length(&self) -> u64 {
        self.len
    }
}

#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize) fn stamp_open_font_file(
    path: &Path,
    file: &std::fs::File,
) -> Result<FileStamp, ()> {
    let metadata = file.metadata().map_err(|_| ())?;
    if !metadata.is_file() {
        return Err(());
    }
    Ok(stamp_for_metadata(path, &metadata))
}

#[cfg(windows)]
pub(in super::super) fn non_durable_file_stamp(metadata: &Metadata) -> FileStamp {
    from_metadata(metadata, false)
}

pub(super) fn stamp_for_metadata(path: &Path, metadata: &Metadata) -> FileStamp {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let durable_reusable =
        super::super::filesystem_policy::durable_reusable(path, metadata.ctime_nsec());
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let durable_reusable = {
        let _ = path;
        false
    };
    from_metadata(metadata, durable_reusable)
}

fn from_metadata(metadata: &Metadata, durable_reusable: bool) -> FileStamp {
    FileStamp {
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
    }
}
