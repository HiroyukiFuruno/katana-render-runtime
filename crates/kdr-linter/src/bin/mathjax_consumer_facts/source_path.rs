use super::super::canonical_path::CanonicalPath;
use std::fs;
use std::path::Path;

pub(super) struct SourcePath;

impl SourcePath {
    pub(super) fn opaque_bytes(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
        let candidate = root.join(relative);
        let path = candidate
            .canonicalize()
            .map_err(|error| format!("missing-source:{relative}:{error}"))?;
        let canonical_root = root
            .canonicalize()
            .map_err(|error| format!("missing-source-root:{error}"))?;
        if !path.starts_with(&canonical_root)
            || CanonicalPath::for_comparison(&path) != CanonicalPath::for_comparison(&candidate)
        {
            return Err(format!("unsafe-source-path:{relative}"));
        }
        fs::read(&path).map_err(|error| format!("read-source:{relative}:{error}"))
    }
}
