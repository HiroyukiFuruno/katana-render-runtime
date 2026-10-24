use super::super::super::font::font_source_generation_uncached;
use super::entry::{CacheKey, FontHandle};
use fontique::{SourceCache, SourceId, SourceInfo, SourceKind};
use harfrust::font::Font;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::{os::unix::fs::MetadataExt, path::Path, sync::Arc};

const SYSTEM_FONT_ROOT: &str = "/System/Library/Fonts/";
const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn try_load(
    database: &Database,
    face_id: ID,
    path: &Path,
    face_index: u32,
    key: CacheKey,
    generation: super::super::super::font::FontSourceGeneration,
    length: u64,
) -> Result<Option<FontHandle>, String> {
    if !eligible_source(database, face_id, path, face_index, length)? {
        return Ok(None);
    }
    super::admit_source_budget(length as usize);
    let source = SourceInfo::new(SourceId::new(), SourceKind::Path(Arc::from(path)));
    let mut cache = SourceCache::default();
    let blob = cache.get(&source).ok_or("immutable font mapping missing")?;
    let bytes = blob.into_raw_parts().0;
    if bytes.as_ref().as_ref().len() as u64 != length {
        return Err("immutable font mapping length changed".to_owned());
    }
    let font = Font::new(bytes, face_index).ok_or("immutable font mapping invalid")?;
    if font_source_generation_uncached(database, face_id) != generation {
        return Err("immutable font generation changed".to_owned());
    }
    Ok(Some(mapped_handle(font, key, length)))
}

fn mapped_handle(font: Font, key: CacheKey, length: u64) -> FontHandle {
    FontHandle {
        font,
        key: Some(key),
        poisoned: None,
        retained_source_bytes: length as usize,
    }
}

pub(super) fn eligible_source(
    database: &Database,
    face_id: ID,
    path: &Path,
    face_index: u32,
    length: u64,
) -> Result<bool, String> {
    let Some((Source::File(source), source_index)) = database.face_source(face_id) else {
        return Ok(false);
    };
    if source != path
        || source_index != face_index
        || length > MAX_SOURCE_BYTES
        || !path.starts_with(SYSTEM_FONT_ROOT)
    {
        return Ok(false);
    }
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    if canonical != path {
        return Ok(false);
    }
    let metadata = std::fs::metadata(&canonical).map_err(|error| error.to_string())?;
    let root = std::fs::metadata("/").map_err(|error| error.to_string())?;
    let mount = nix::sys::statfs::statfs(&canonical).map_err(|error| error.to_string())?;
    Ok(eligible_mount(
        mount.filesystem_type_name(),
        mount.flags(),
        metadata.dev() == root.dev(),
    ))
}

fn eligible_mount(filesystem: &str, flags: nix::mount::MntFlags, same_device: bool) -> bool {
    let required = nix::mount::MntFlags::MNT_RDONLY
        .union(nix::mount::MntFlags::MNT_ROOTFS)
        .union(nix::mount::MntFlags::MNT_SNAPSHOT);
    filesystem == "apfs" && flags.contains(required) && same_device
}

#[cfg(test)]
mod tests {
    use super::{eligible_mount, eligible_source};
    use nix::mount::MntFlags;
    use resvg::usvg::fontdb::{Database, Source};
    use std::io::Write;

    const ALL_FLAGS: MntFlags = MntFlags::MNT_RDONLY
        .union(MntFlags::MNT_ROOTFS)
        .union(MntFlags::MNT_SNAPSHOT);

    #[test]
    fn immutable_mapping_requires_apfs_all_flags_and_root_device() {
        assert!(eligible_mount("apfs", ALL_FLAGS, true));
        assert!(!eligible_mount("apfs", ALL_FLAGS, false));
        for missing in [
            MntFlags::MNT_RDONLY,
            MntFlags::MNT_ROOTFS,
            MntFlags::MNT_SNAPSHOT,
        ] {
            assert!(!eligible_mount("apfs", ALL_FLAGS.difference(missing), true));
        }
        assert!(!eligible_mount("hfs", ALL_FLAGS, true));
    }

    #[test]
    fn ordinary_mutable_temp_file_is_not_eligible() -> Result<(), String> {
        let mut temp = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
        temp.write_all(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"))
            .map_err(|error| error.to_string())?;
        let path = temp.path().to_path_buf();
        let mut database = Database::new();
        database.load_font_source(Source::File(path.clone()));
        let face = database
            .faces()
            .next()
            .ok_or("temporary font face missing")?
            .id;
        let length = std::fs::metadata(&path)
            .map_err(|error| error.to_string())?
            .len();
        let result = eligible_source(&database, face, &path, 0, length);
        assert!(!result?);
        let mut permissions = temp
            .as_file()
            .metadata()
            .map_err(|error| error.to_string())?
            .permissions();
        permissions.set_readonly(true);
        temp.as_file()
            .set_permissions(permissions)
            .map_err(|error| error.to_string())?;
        assert!(!eligible_source(&database, face, &path, 0, length)?);
        Ok(())
    }
}
