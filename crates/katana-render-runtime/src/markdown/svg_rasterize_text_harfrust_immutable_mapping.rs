use super::super::super::font::font_source_generation_uncached;
use super::entry::{CacheKey, FontHandle};
use fontique::{SourceCache, SourceId, SourceInfo, SourceKind};
use harfrust::font::Font;
use resvg::usvg::fontdb::{Database, ID, Source};
use std::{os::unix::fs::MetadataExt, path::Path, sync::Arc};

const SYSTEM_FONT_ROOT: &str = "/System/Library/Fonts/";
const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

fn filesystem_error_to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

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

pub(in crate::markdown::svg_rasterize) fn eligible_source(
    database: &Database,
    face_id: ID,
    path: &Path,
    face_index: u32,
    length: u64,
) -> Result<bool, String> {
    let Some((Source::File(source), source_index)) = database.face_source(face_id) else {
        return Ok(false);
    };
    if source != path || source_index != face_index {
        return Ok(false);
    }
    eligible_path(path, length)
}

pub(in crate::markdown::svg_rasterize) fn eligible_path(
    path: &Path,
    length: u64,
) -> Result<bool, String> {
    if length > MAX_SOURCE_BYTES || !path.starts_with(SYSTEM_FONT_ROOT) {
        return Ok(false);
    }
    let canonical = path.canonicalize().map_err(filesystem_error_to_string)?;
    if canonical != path {
        return Ok(false);
    }
    let metadata = std::fs::metadata(&canonical).map_err(filesystem_error_to_string)?;
    let root = std::fs::metadata("/").map_err(filesystem_error_to_string)?;
    let mount = nix::sys::statfs::statfs(&canonical).map_err(filesystem_error_to_string)?;
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
#[path = "svg_rasterize_text_harfrust_immutable_mapping_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_immutable_mapping_tests.rs"]
mod tests;
