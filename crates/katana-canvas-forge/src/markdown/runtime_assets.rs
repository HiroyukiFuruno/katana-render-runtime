use std::path::PathBuf;

pub const MERMAID_JS_VERSION: &str = "3.3.1";
pub const MERMAID_JS_CHECKSUM: &str =
    "217b66ef4279c33c141b4afe22effad10a91c02558dc70917be2c0981e78ed87";
pub const MERMAID_DOWNLOAD_URL: &str =
    "https://cdn.jsdelivr.net/npm/mermaid@3.3.1/dist/mermaid.min.js";

pub const MERMAID_ZENUML_JS_VERSION: &str = "0.2.2";
pub const MERMAID_ZENUML_JS_CHECKSUM: &str =
    "39143c3cb4e7a1dc53938de0c85e5fd2aee1533ec4e07d7d95a6ef639956ff1f";
pub const MERMAID_ZENUML_DOWNLOAD_URL: &str =
    "https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid-zenuml@0.2.2/dist/mermaid-zenuml.min.js";

pub const DRAWIO_JS_VERSION: &str = "29.7.10";
pub const DRAWIO_JS_CHECKSUM: &str =
    "a8b7897de995a4e7dd3a541a5e7250d64a295440f728f0ddae72179cdf5a83d5";
pub const DRAWIO_DOWNLOAD_URL: &str = "https://github.com/jgraph/drawio/releases";

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
            bytes: include_bytes!("../../vendor/mermaid/3.3.1/mermaid.min.js"),
        }
    }

    pub(crate) fn drawio() -> Self {
        Self {
            kind: "drawio",
            version: DRAWIO_JS_VERSION,
            filename: "drawio.min.js",
            bytes: include_bytes!("../../vendor/drawio/29.7.10/drawio.min.js"),
        }
    }

    pub(crate) fn mermaid_zenuml() -> Self {
        Self {
            kind: "mermaid-zenuml",
            version: MERMAID_ZENUML_JS_VERSION,
            filename: "mermaid-zenuml.min.js",
            bytes: include_bytes!("../../vendor/mermaid-zenuml/0.2.2/mermaid-zenuml.min.js"),
        }
    }

    pub(crate) fn materialized_path(&self) -> PathBuf {
        std::env::temp_dir()
            .join("katana-canvas-forge")
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
        std::fs::write(&path, self.bytes).map_err(runtime_asset_error)?;
        Ok(path)
    }

    fn exists_with_same_bytes(&self, path: &std::path::Path) -> Result<bool, String> {
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

#[cfg(test)]
mod tests {
    use super::{
        DRAWIO_JS_CHECKSUM, MERMAID_JS_CHECKSUM, MERMAID_ZENUML_JS_CHECKSUM, RuntimeAsset,
    };

    #[test]
    fn materialized_paths_are_versioned() {
        let mermaid = RuntimeAsset::mermaid().materialized_path();
        let zenuml = RuntimeAsset::mermaid_zenuml().materialized_path();
        let drawio = RuntimeAsset::drawio().materialized_path();

        assert!(mermaid.ends_with("vendor/mermaid/3.3.1/mermaid.min.js"));
        assert!(zenuml.ends_with("vendor/mermaid-zenuml/0.2.2/mermaid-zenuml.min.js"));
        assert!(drawio.ends_with("vendor/drawio/29.7.10/drawio.min.js"));
    }

    #[test]
    fn pinned_checksums_are_sha256_hex() {
        assert_eq!(MERMAID_JS_CHECKSUM.len(), 64);
        assert_eq!(MERMAID_ZENUML_JS_CHECKSUM.len(), 64);
        assert_eq!(DRAWIO_JS_CHECKSUM.len(), 64);
        assert!(MERMAID_JS_CHECKSUM.chars().all(|it| it.is_ascii_hexdigit()));
        assert!(
            MERMAID_ZENUML_JS_CHECKSUM
                .chars()
                .all(|it| it.is_ascii_hexdigit())
        );
        assert!(DRAWIO_JS_CHECKSUM.chars().all(|it| it.is_ascii_hexdigit()));
    }

    #[test]
    fn materialize_writes_missing_asset_file() {
        let path = test_path("missing-mermaid.min.js");
        remove_parent(&path);

        let result = RuntimeAsset::mermaid().materialize_at(path.clone());

        assert!(matches!(result, Ok(written) if written == path));
        assert!(path.exists());
        remove_parent(&path);
    }

    #[test]
    fn materialize_reports_empty_path_and_read_errors() {
        let empty_path = RuntimeAsset::mermaid().materialize_at(std::path::PathBuf::new());
        assert!(matches!(empty_path, Err(error) if error.contains("parent")));

        let path = test_path("runtime-directory");
        let _ = std::fs::remove_dir_all(&path);
        let create_result = std::fs::create_dir_all(&path);
        assert!(create_result.is_ok());

        let read_error = RuntimeAsset::mermaid().materialize_at(path.clone());
        assert!(read_error.is_err());
        let _ = std::fs::remove_dir_all(&path);
        remove_parent(&path);
    }

    #[test]
    fn materialize_replaces_different_existing_asset_file() {
        let path = test_path("stale-mermaid.min.js");
        remove_parent(&path);
        let parent = path.parent();
        assert!(matches!(parent, Some(it) if std::fs::create_dir_all(it).is_ok()));
        let write_result = std::fs::write(&path, b"stale");
        assert!(write_result.is_ok());

        let result = RuntimeAsset::mermaid().materialize_at(path.clone());

        assert!(result.is_ok());
        let stored = std::fs::read(path.clone());
        assert!(matches!(stored, Ok(bytes) if bytes != b"stale"));
        remove_parent(&path);
    }

    #[test]
    fn materialize_keeps_same_existing_asset_file() {
        let path = test_path("current-mermaid.min.js");
        remove_parent(&path);
        let first = RuntimeAsset::mermaid().materialize_at(path.clone());
        assert!(matches!(first, Ok(written) if written == path));

        let second = RuntimeAsset::mermaid().materialize_at(path.clone());

        assert!(matches!(second, Ok(written) if written == path));
        remove_parent(&path);
    }

    #[test]
    fn runtime_asset_error_keeps_io_error_message() {
        let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");

        let message = super::runtime_asset_error(error);

        assert_eq!(message, "denied");
    }

    #[test]
    fn remove_parent_accepts_path_without_parent() {
        remove_parent(std::path::Path::new(""));
    }

    fn test_path(name: &str) -> std::path::PathBuf {
        let slug = name.replace(['.', '/'], "-");
        std::env::temp_dir()
            .join(format!(
                "kcf-runtime-assets-test-{}-{slug}",
                std::process::id()
            ))
            .join(name)
    }

    fn remove_parent(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}
