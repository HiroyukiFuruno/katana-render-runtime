use super::{FileStamp, SharedBytes, SharedSources};
use crate::markdown::svg_rasterize::text_shaping::{
    eligible_immutable_system_path, stamp_path_uncached,
};
use std::path::Path;

pub(super) struct SharedSource {
    pub(super) data: SharedBytes,
    pub(super) length: usize,
    pub(super) newly_mapped: bool,
}

pub(super) fn shared_source(
    path: &Path,
    stamp: Option<&FileStamp>,
    sources: &mut SharedSources,
    remaining: usize,
) -> Result<Option<SharedSource>, ()> {
    let Some(stamp) = stamp.filter(|stamp| stamp.durable_reusable()) else {
        return Ok(None);
    };
    let has_seed = sources.seeded(path, stamp).is_some();
    if !has_seed && remaining == 0 {
        return Ok(None);
    }
    let Some(length) = eligible_file_length(path)? else {
        return Ok(None);
    };
    if let Some(data) = seeded_source(path, stamp, sources, length)? {
        return Ok(Some(SharedSource {
            data,
            length,
            newly_mapped: false,
        }));
    }
    new_source(path, stamp, sources, length, remaining)
}

fn eligible_file_length(path: &Path) -> Result<Option<usize>, ()> {
    let length = usize::try_from(std::fs::metadata(path).map_err(|_| ())?.len()).map_err(|_| ())?;
    let file_length = u64::try_from(length).map_err(|_| ())?;
    eligible_immutable_system_path(path, file_length)
        .map(|eligible| eligible.then_some(length))
        .map_err(|_| ())
}

fn seeded_source(
    path: &Path,
    stamp: &FileStamp,
    sources: &SharedSources,
    length: usize,
) -> Result<Option<SharedBytes>, ()> {
    let Some(data) = sources.seeded(path, stamp) else {
        return Ok(None);
    };
    if data.as_ref().as_ref().len() != length || stamp_path_uncached(path).as_ref() != Ok(stamp) {
        return Err(());
    }
    Ok(Some(std::sync::Arc::clone(data)))
}

fn new_source(
    path: &Path,
    stamp: &FileStamp,
    sources: &mut SharedSources,
    length: usize,
    remaining: usize,
) -> Result<Option<SharedSource>, ()> {
    if length > remaining {
        return Ok(None);
    }
    let data = sources.get(path, stamp).ok_or(())?;
    if data.as_ref().as_ref().len() != length {
        return Err(());
    }
    Ok(Some(SharedSource {
        data,
        length,
        newly_mapped: true,
    }))
}
