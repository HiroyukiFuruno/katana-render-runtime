use crate::markdown::runtime_asset_archive::RuntimeAssetSource;
use std::{
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

static RUNTIME_ASSET_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static MERMAID_RUNTIME_ASSET_BYTES: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
static DRAWIO_RUNTIME_ASSET_BYTES: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
#[cfg(test)]
static ZENUML_RUNTIME_ASSET_BYTES: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();

#[cfg(test)]
const ZENUML_RUNTIME_ASSET_ARCHIVE: &[u8] =
    include_bytes!("generated/zenuml-runtime-assets.bin.br");
#[cfg(test)]
include!("generated/zenuml-runtime-assets-index.rs");

pub const MERMAID_JS_VERSION: &str = "11.17.2";
pub const MERMAID_JS_CHECKSUM: &str =
    "581ed7d74bd9048d0e3a91363927d72ef22942d7722546b27f7cc29e35390eb8";
pub const MERMAID_DOWNLOAD_URL: &str =
    "https://cdn.jsdelivr.net/npm/mermaid@11.17.2/dist/mermaid.min.js";

pub const MERMAID_ZENUML_JS_VERSION: &str = "0.2.3";
pub const MERMAID_ZENUML_JS_CHECKSUM: &str =
    "28eeec88021d9e9728df4d005ff723a3d71da29a21dbcfa2a628232c35ef2ab6";
pub const MERMAID_ZENUML_DOWNLOAD_URL: &str =
    "https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid-zenuml@0.2.3/dist/mermaid-zenuml.min.js";

pub const ZENUML_CORE_JS_VERSION: &str = "4.2.1";
pub const ZENUML_CORE_JS_CHECKSUM: &str =
    "554828af031325ae41887e9ec26468f2fa89cb29e8bf096d51e4909d72e69cdf";
pub const ZENUML_CORE_DOWNLOAD_URL: &str =
    "https://cdn.jsdelivr.net/npm/@zenuml/core@4.2.1/dist/zenuml.js";

pub const DRAWIO_JS_VERSION: &str = "31.4.2";
pub const DRAWIO_JS_CHECKSUM: &str =
    "3810b944463d342daa438e0bf7793addbcc96531269c4aea26907142b1a38d0a";
pub const DRAWIO_DOWNLOAD_URL: &str = "https://github.com/jgraph/drawio/releases/tag/v31.4.2";

pub const MATHJAX_JS_VERSION: &str = "4.1.3";
pub const MATHJAX_JS_CHECKSUM: &str =
    "23c036deccc0f2374834a47e4032e452419f3ac027bf17e17c104e2746b19f4c";
pub const MATHJAX_DOWNLOAD_URL: &str = "https://cdn.jsdelivr.net/npm/mathjax@4.1.3/tex-svg.js";

pub(crate) struct RuntimeAsset {
    kind: &'static str,
    version: &'static str,
    filename: &'static str,
    source: RuntimeAssetSource,
}

impl RuntimeAsset {
    pub(crate) fn mermaid() -> Self {
        Self {
            kind: "mermaid",
            version: MERMAID_JS_VERSION,
            filename: "mermaid.min.js",
            source: RuntimeAssetSource::Brotli {
                bytes: include_bytes!("../../vendor/mermaid/11.17.2/mermaid.min.js.br"),
                cache: &MERMAID_RUNTIME_ASSET_BYTES,
            },
        }
    }

    pub(crate) fn drawio() -> Self {
        Self {
            kind: "drawio",
            version: DRAWIO_JS_VERSION,
            filename: "drawio.min.js",
            source: RuntimeAssetSource::Brotli {
                bytes: include_bytes!("../../vendor/drawio/31.4.2/drawio.min.js.br"),
                cache: &DRAWIO_RUNTIME_ASSET_BYTES,
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn mermaid_zenuml() -> Self {
        Self {
            kind: "mermaid-zenuml",
            version: MERMAID_ZENUML_JS_VERSION,
            filename: "mermaid-zenuml.min.js",
            source: RuntimeAssetSource::BrotliRange {
                bytes: ZENUML_RUNTIME_ASSET_ARCHIVE,
                cache: &ZENUML_RUNTIME_ASSET_BYTES,
                start: MERMAID_ZENUML_ASSET_OFFSET,
                length: MERMAID_ZENUML_ASSET_LENGTH,
                archive_length: ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH,
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn zenuml_core() -> Self {
        Self {
            kind: "zenuml-core",
            version: ZENUML_CORE_JS_VERSION,
            filename: "zenuml.js",
            source: RuntimeAssetSource::BrotliRange {
                bytes: ZENUML_RUNTIME_ASSET_ARCHIVE,
                cache: &ZENUML_RUNTIME_ASSET_BYTES,
                start: ZENUML_CORE_ASSET_OFFSET,
                length: ZENUML_CORE_ASSET_LENGTH,
                archive_length: ZENUML_RUNTIME_ASSETS_UNCOMPRESSED_LENGTH,
            },
        }
    }

    pub(crate) fn materialized_path(&self) -> PathBuf {
        std::env::temp_dir()
            .join("katana-render-runtime")
            .join("vendor")
            .join(self.kind)
            .join(self.version)
            .join(self.filename)
    }

    pub(crate) fn materialize_at(&self, path: PathBuf) -> Result<PathBuf, String> {
        if self.exists_with_same_bytes(&path)? {
            return Ok(path);
        }
        let Some(parent) = path.parent() else {
            return Err(format!("{} runtime asset path has no parent", self.kind));
        };
        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;
        self.write_atomically(&path, parent)?;
        Ok(path)
    }

    pub(crate) fn bytes(&self) -> Result<&'static [u8], String> {
        self.source.bytes()
    }

    fn write_atomically(&self, path: &Path, parent: &Path) -> Result<(), String> {
        let temp_path = self.temporary_write_path(parent);
        std::fs::write(&temp_path, self.bytes()?).map_err(runtime_asset_error)?;
        match std::fs::rename(&temp_path, path) {
            Ok(()) => Ok(()),
            #[cfg(windows)]
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.handle_existing_destination(path, &temp_path)
            }
            Err(error) => Self::cleanup_temp_and_report(temp_path, error),
        }
    }

    fn temporary_write_path(&self, parent: &Path) -> PathBuf {
        let sequence = RUNTIME_ASSET_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        parent.join(format!(
            ".{}.{}.{}.tmp",
            self.filename,
            std::process::id(),
            sequence
        ))
    }

    #[cfg(windows)]
    fn handle_existing_destination(&self, path: &Path, temp_path: &Path) -> Result<(), String> {
        if self.exists_with_same_bytes(path)? {
            std::fs::remove_file(temp_path).map_err(runtime_asset_error)?;
            return Ok(());
        }
        remove_existing_destination(path)?;
        std::fs::rename(temp_path, path).map_err(runtime_asset_error)
    }

    fn cleanup_temp_and_report(temp_path: PathBuf, error: std::io::Error) -> Result<(), String> {
        let _ = std::fs::remove_file(temp_path);
        Err(runtime_asset_error(error))
    }

    fn exists_with_same_bytes(&self, path: &Path) -> Result<bool, String> {
        match std::fs::read(path) {
            Ok(existing) => Ok(existing == self.bytes()?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(runtime_asset_error(error)),
        }
    }
}

fn runtime_asset_error(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(windows)]
fn remove_existing_destination(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(runtime_asset_error(error)),
    }
}

#[cfg(test)]
#[path = "runtime_assets_tests.rs"]
mod tests;
