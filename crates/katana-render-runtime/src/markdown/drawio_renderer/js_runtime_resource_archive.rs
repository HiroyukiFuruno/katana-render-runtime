use super::{DrawioResource, drawio_resource, selector::DrawioResourceSelector};
use crate::markdown::runtime_asset_archive::RuntimeAssetArchive;

const DRAWIO_RESOURCE_ARCHIVE: &[u8] = include_bytes!("generated/drawio-resources.bin.br");

pub(super) type DrawioResourceArchiveEntry = (&'static str, usize, usize);
pub(super) type DrawioResourceArchiveIndex = &'static [DrawioResourceArchiveEntry];

pub(super) struct DrawioResourceArchiveGroup {
    pub(super) compressed_start: usize,
    pub(super) compressed_length: usize,
    pub(super) uncompressed_length: usize,
    pub(super) index: DrawioResourceArchiveIndex,
}

include!("generated/drawio-resources-index.rs");

pub(super) struct DrawioResourceArchive;

impl DrawioResourceArchive {
    pub(super) fn collect(
        selector: &DrawioResourceSelector<'_>,
        resources: &mut Vec<DrawioResource>,
    ) -> Result<(), String> {
        for group in DRAWIO_RESOURCE_ARCHIVE_GROUPS {
            if !group_is_selected(group, selector) {
                continue;
            }
            let archive = Self::group_bytes(group)?;
            Self::collect_index(&archive, group.index, selector, resources)?;
        }
        Ok(())
    }

    fn collect_index(
        archive: &[u8],
        index: DrawioResourceArchiveIndex,
        selector: &DrawioResourceSelector<'_>,
        resources: &mut Vec<DrawioResource>,
    ) -> Result<(), String> {
        for &(path, start, length) in index {
            if selector.includes(path) {
                resources.push(drawio_resource(
                    path.to_string(),
                    resource_contents(archive, path, start, length)?,
                ));
            }
        }
        Ok(())
    }

    fn group_bytes(group: &DrawioResourceArchiveGroup) -> Result<Vec<u8>, String> {
        let compressed = compressed_group_contents(
            DRAWIO_RESOURCE_ARCHIVE,
            group.compressed_start,
            group.compressed_length,
        )?;
        validate_resource_archive(
            RuntimeAssetArchive::brotli(compressed)?,
            group.uncompressed_length,
        )
    }
}

#[cfg(test)]
pub(super) fn group_bytes_for_test(group: &DrawioResourceArchiveGroup) -> Result<Vec<u8>, String> {
    DrawioResourceArchive::group_bytes(group)
}

fn group_is_selected(
    group: &DrawioResourceArchiveGroup,
    selector: &DrawioResourceSelector<'_>,
) -> bool {
    group
        .index
        .iter()
        .any(|&(path, _, _)| selector.includes(path))
}

#[cfg(test)]
pub(super) fn selected_groups(
    selector: &DrawioResourceSelector<'_>,
) -> Vec<&'static DrawioResourceArchiveGroup> {
    DRAWIO_RESOURCE_ARCHIVE_GROUPS
        .iter()
        .filter(|group| group_is_selected(group, selector))
        .collect()
}

pub(super) fn validate_resource_archive(
    bytes: Vec<u8>,
    expected_length: usize,
) -> Result<Vec<u8>, String> {
    if bytes.len() != expected_length {
        return Err(format!(
            "Draw.io resource archive length mismatch: expected {expected_length}, got {}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

pub(super) fn compressed_group_contents(
    archive: &[u8],
    start: usize,
    length: usize,
) -> Result<&[u8], String> {
    let Some(end) = start.checked_add(length) else {
        return Err("Draw.io resource archive group offset overflow".to_string());
    };
    archive
        .get(start..end)
        .ok_or_else(|| "Draw.io resource archive group is out of bounds".to_string())
}

pub(super) fn resource_contents<'a>(
    archive: &'a [u8],
    path: &str,
    start: usize,
    length: usize,
) -> Result<&'a [u8], String> {
    let Some(end) = start.checked_add(length) else {
        return Err(format!("Draw.io resource archive offset overflow: {path}"));
    };
    archive
        .get(start..end)
        .ok_or_else(|| format!("Draw.io resource archive entry is out of bounds: {path}"))
}
