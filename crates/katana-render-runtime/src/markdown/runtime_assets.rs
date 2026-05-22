use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static RUNTIME_ASSET_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub const MERMAID_JS_VERSION: &str = "11.15.0";
pub const MERMAID_JS_CHECKSUM: &str =
    "70137e77bb273bb2ef972b86e8b0400cca8be53cb25bfc45911a186dc98665de";
pub const MERMAID_DOWNLOAD_URL: &str =
    "https://cdn.jsdelivr.net/npm/mermaid@11.15.0/dist/mermaid.min.js";

pub const MERMAID_ZENUML_JS_VERSION: &str = "0.2.3";
pub const MERMAID_ZENUML_JS_CHECKSUM: &str =
    "28eeec88021d9e9728df4d005ff723a3d71da29a21dbcfa2a628232c35ef2ab6";
pub const MERMAID_ZENUML_DOWNLOAD_URL: &str =
    "https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid-zenuml@0.2.3/dist/mermaid-zenuml.min.js";

pub const ZENUML_CORE_JS_VERSION: &str = "3.47.9";
pub const ZENUML_CORE_JS_CHECKSUM: &str =
    "ece11a311907401113f965e110c25c04c6a9b3dcbbb234bf2cd593a3f3ebe3df";
pub const ZENUML_CORE_DOWNLOAD_URL: &str =
    "https://cdn.jsdelivr.net/npm/@zenuml/core@3.47.9/dist/zenuml.js";

pub const DRAWIO_JS_VERSION: &str = "30.0.2";
pub const DRAWIO_JS_CHECKSUM: &str =
    "0435d7a829549490482d576a37556224fa190d538610c96908632e5cda7c601f";
pub const DRAWIO_DOWNLOAD_URL: &str = "https://github.com/jgraph/drawio/releases/tag/v30.0.2";

pub const MATHJAX_JS_VERSION: &str = "4.1.2";
pub const MATHJAX_JS_CHECKSUM: &str =
    "e201dba4a20191563337e7f95ebeef6724bd2fbdc079c431b4bb8ecdfc059c33";
pub const MATHJAX_DOWNLOAD_URL: &str = "https://cdn.jsdelivr.net/npm/mathjax@4.1.2/tex-svg.js";

pub(crate) struct RuntimeAsset {
    kind: &'static str,
    version: &'static str,
    filename: &'static str,
    bytes: &'static [u8],
}

impl RuntimeAsset {
    pub(crate) fn mermaid() -> Self {
        Self {
            kind: "mermaid",
            version: MERMAID_JS_VERSION,
            filename: "mermaid.min.js",
            bytes: include_bytes!("../../vendor/mermaid/11.15.0/mermaid.min.js"),
        }
    }

    pub(crate) fn drawio() -> Self {
        Self {
            kind: "drawio",
            version: DRAWIO_JS_VERSION,
            filename: "drawio.min.js",
            bytes: include_bytes!("../../vendor/drawio/30.0.2/drawio.min.js"),
        }
    }

    #[cfg(test)]
    pub(crate) fn zenuml_core() -> Self {
        Self {
            kind: "zenuml-core",
            version: ZENUML_CORE_JS_VERSION,
            filename: "zenuml.js",
            bytes: include_bytes!("../../vendor/zenuml-core/3.47.9/zenuml.js"),
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

    fn write_atomically(&self, path: &Path, parent: &Path) -> Result<(), String> {
        let temp_path = self.temporary_write_path(parent);
        std::fs::write(&temp_path, self.bytes).map_err(runtime_asset_error)?;
        match std::fs::rename(&temp_path, path) {
            Ok(()) => Ok(()),
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
            Ok(existing) => Ok(existing == self.bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(runtime_asset_error(error)),
        }
    }
}

fn runtime_asset_error(error: std::io::Error) -> String {
    error.to_string()
}

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
