use super::super::super::super::font::{stamp_open_font_file, stamp_path_uncached};
use super::super::super::super::harfrust::{
    eligible_immutable_system_path, map_immutable_font_data,
};
use super::{FileFace, FileStamp, MAPPED_FONT_SESSION_BYTES, MappedFontBytes, mapping_pool};
use resvg::usvg::fontdb::Source;
use std::{
    fs::{File, OpenOptions},
    io::{Read, copy},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::Arc,
};

const MAX_SELECTED_FILE_BYTES: u64 = 128 * 1024 * 1024;

impl FileFace {
    pub(super) fn mapped_bytes(&self) -> u64 {
        self.mapped_source().map_or(0, |bytes| mapping_len(&bytes))
    }

    pub(super) fn is_large_mapping(&self) -> bool {
        self.mapped_bytes() > MAPPED_FONT_SESSION_BYTES
    }

    fn mapped_source(&self) -> Option<MappedFontBytes> {
        let (Source::SharedFile(_, bytes), _) = self.database.face_source(self.id)? else {
            return None;
        };
        Some(bytes)
    }

    fn large_mapping(&self) -> Option<MappedFontBytes> {
        let bytes = self.mapped_source()?;
        (mapping_len(&bytes) > MAPPED_FONT_SESSION_BYTES).then_some(bytes)
    }
}

fn mapping_len(bytes: &MappedFontBytes) -> u64 {
    bytes.as_ref().as_ref().len() as u64
}

pub(super) fn map_source(
    path: &Path,
    stamp: &FileStamp,
    remaining: u64,
) -> Option<MappedFontBytes> {
    let length = std::fs::metadata(path).ok()?.len();
    if length > remaining || !eligible_immutable_system_path(path, length).ok()? {
        return None;
    }
    let bytes = mapping_pool::acquire(path, stamp, length)?;
    (super::observe_path_stamp(path).as_ref() == Ok(stamp)).then_some(bytes)
}

pub(super) fn map_large_source(
    path: &Path,
    stamp: &FileStamp,
    length: u64,
    frame_faces: &[FileFace],
) -> Option<MappedFontBytes> {
    let frame_mapping = frame_faces
        .iter()
        .find_map(|face| face.large_mapping().map(|bytes| (face, bytes)));
    if let Some((face, bytes)) = frame_mapping {
        /* WHY: 同じTTC内の別の面は共有し、1フレームで保つ大容量ファイルは1件にする。 */
        return (face.path == path && &face.stamp == stamp).then_some(bytes);
    }
    map(path, stamp, length)
}

struct PrivateMapping {
    bytes: MappedFontBytes,
    _directory: tempfile::TempDir,
}

impl AsRef<[u8]> for PrivateMapping {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref().as_ref()
    }
}

fn map(path: &Path, stamp: &FileStamp, length: u64) -> Option<MappedFontBytes> {
    if !stamp.durable_reusable() || length > MAX_SELECTED_FILE_BYTES {
        return None;
    }
    let mut origin = open_origin(path).ok()?;
    if stamp_open_font_file(path, &origin).as_ref() != Ok(stamp) {
        return None;
    }
    let directory = tempfile::tempdir().ok()?;
    let snapshot = directory.path().join("selected-font");
    create_bounded_snapshot(&mut origin, &snapshot, length).ok()?;
    map_verified_snapshot(path, stamp, length, &origin, &snapshot, directory)
}

fn map_verified_snapshot(
    path: &Path,
    stamp: &FileStamp,
    length: u64,
    origin: &File,
    snapshot: &Path,
    directory: tempfile::TempDir,
) -> Option<MappedFontBytes> {
    if stamp_open_font_file(path, origin).as_ref() != Ok(stamp)
        || stamp_path_uncached(path).as_ref() != Ok(stamp)
    {
        return None;
    }
    /* WHY: 所有する一時ファイルだけをmapし、元ファイルの更新でmappingが壊れる経路を閉じる。 */
    let bytes = map_immutable_font_data(snapshot, length).ok()?;
    Some(Arc::new(PrivateMapping {
        bytes,
        _directory: directory,
    }))
}

fn open_origin(path: &Path) -> std::io::Result<File> {
    /* WHY: 選択後のパス差し替えがsymlinkやFIFOでも、別の書体を開いたり待ち続けたりしない。 */
    let flags = nix::fcntl::OFlag::O_NOFOLLOW.union(nix::fcntl::OFlag::O_NONBLOCK);
    OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits())
        .open(path)
}

fn create_bounded_snapshot(origin: &mut File, target: &Path, length: u64) -> std::io::Result<()> {
    if reflink_copy::reflink_file_to_path(origin, target).is_ok() {
        return Ok(());
    }
    copy_bounded(origin, target, length)
}

fn copy_bounded(origin: &mut File, target: &Path, length: u64) -> std::io::Result<()> {
    let mut destination = File::create_new(target)?;
    let copied = copy(&mut origin.take(length + 1), &mut destination)?;
    if copied != length {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
    }
    Ok(())
}

#[cfg(test)]
#[path = "svg_rasterize_text_selected_font_snapshot_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
mod tests {
    use super::{MAX_SELECTED_FILE_BYTES, map, stamp_path_uncached};

    const ORIGINAL: &[u8] = b"original";
    const REPLACEMENT: &[u8] = b"replaced";

    #[test]
    fn private_mapping_survives_the_same_length_rewrite_of_the_original()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("source");
        std::fs::write(&path, ORIGINAL)?;
        let stamp = stamp_path_uncached(&path).map_err(|()| "origin stamp unavailable")?;
        let mapping = map(&path, &stamp, stamp.file_length()).ok_or("snapshot unavailable")?;
        std::fs::write(&path, REPLACEMENT)?;
        assert_eq!(mapping.as_ref().as_ref(), ORIGINAL);
        assert!(map(&path, &stamp, stamp.file_length()).is_none());
        Ok(())
    }

    #[test]
    fn private_mapping_rejects_unbounded_or_missing_origins()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("source");
        std::fs::write(&path, ORIGINAL)?;
        let stamp = stamp_path_uncached(&path).map_err(|()| "origin stamp unavailable")?;
        assert!(map(&path, &stamp, MAX_SELECTED_FILE_BYTES + 1).is_none());
        std::fs::remove_file(&path)?;
        assert!(map(&path, &stamp, stamp.file_length()).is_none());
        Ok(())
    }

    #[test]
    fn private_mapping_refuses_a_symlink_to_the_same_inode()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("source");
        let link = directory.path().join("link");
        std::fs::write(&path, ORIGINAL)?;
        std::os::unix::fs::symlink(&path, &link)?;
        let stamp = stamp_path_uncached(&link).map_err(|()| "origin stamp unavailable")?;
        assert!(map(&link, &stamp, stamp.file_length()).is_none());
        Ok(())
    }
}
