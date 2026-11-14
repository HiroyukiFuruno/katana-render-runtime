use std::{io::Read, sync::OnceLock};

const BROTLI_BUFFER_SIZE: usize = 4096;

pub(super) static MERMAID_RUNTIME_ASSET_BYTES: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
pub(super) static DRAWIO_RUNTIME_ASSET_BYTES: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
#[cfg(test)]
pub(super) static ZENUML_RUNTIME_ASSET_BYTES: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();

#[cfg(test)]
pub(super) const ZENUML_RUNTIME_ASSET_ARCHIVE: &[u8] =
    include_bytes!("generated/zenuml-runtime-assets.bin.br");
#[cfg(test)]
include!("generated/zenuml-runtime-assets-index.rs");

#[derive(Clone, Copy)]
pub(crate) enum RuntimeAssetSource {
    Brotli {
        bytes: &'static [u8],
        cache: &'static OnceLock<Result<Vec<u8>, String>>,
    },
    #[cfg(test)]
    BrotliRange {
        bytes: &'static [u8],
        cache: &'static OnceLock<Result<Vec<u8>, String>>,
        start: usize,
        length: usize,
        archive_length: usize,
    },
}

impl RuntimeAssetSource {
    pub(crate) fn bytes(&self) -> Result<&'static [u8], String> {
        match *self {
            Self::Brotli { bytes, cache } => cache
                .get_or_init(|| RuntimeAssetArchive::brotli(bytes))
                .as_ref()
                .map(Vec::as_slice)
                .map_err(Clone::clone),
            #[cfg(test)]
            Self::BrotliRange {
                bytes,
                cache,
                start,
                length,
                archive_length,
            } => RuntimeAssetArchive::range(bytes, cache, start, length, archive_length),
        }
    }
}

pub(crate) struct RuntimeAssetArchive;

impl RuntimeAssetArchive {
    pub(crate) fn brotli(compressed: &[u8]) -> Result<Vec<u8>, String> {
        let mut decompressor =
            brotli_decompressor::Decompressor::new(compressed, BROTLI_BUFFER_SIZE);
        let mut bytes = Vec::new();
        decompressor
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
    }

    #[cfg(test)]
    pub(crate) fn range(
        compressed: &'static [u8],
        cache: &'static OnceLock<Result<Vec<u8>, String>>,
        start: usize,
        length: usize,
        archive_length: usize,
    ) -> Result<&'static [u8], String> {
        let archive = cache
            .get_or_init(|| {
                let bytes = Self::brotli(compressed)?;
                if bytes.len() != archive_length {
                    return Err(format!(
                        "ZenUML runtime asset archive length mismatch: expected {archive_length}, got {}",
                        bytes.len()
                    ));
                }
                Ok(bytes)
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let end = start
            .checked_add(length)
            .ok_or_else(|| "ZenUML runtime asset archive offset overflow".to_string())?;
        archive
            .get(start..end)
            .ok_or_else(|| "ZenUML runtime asset archive range is out of bounds".to_string())
    }
}
