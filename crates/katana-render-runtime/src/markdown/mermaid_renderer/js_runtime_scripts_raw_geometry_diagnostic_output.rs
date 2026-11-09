use std::{
    ffi::OsString,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

pub(super) struct DiagnosticOutput {
    path: PathBuf,
    _owner: Option<tempfile::TempDir>,
}

impl DiagnosticOutput {
    pub(super) fn new(output_env: &str) -> Result<Self, String> {
        Self::from_override(std::env::var_os(output_env))
    }

    fn from_override(output: Option<OsString>) -> Result<Self, String> {
        if let Some(output) = output {
            let path = PathBuf::from(output);
            if !path.is_absolute() {
                return Err("diagnostic output path must be absolute".to_owned());
            }
            return Ok(Self { path, _owner: None });
        }
        let tmp = workspace_tmp()?;
        std::fs::create_dir_all(&tmp).map_err(|error| error.to_string())?;
        let owner = tempfile::Builder::new()
            .prefix("raw-geometry-diagnostic-")
            .tempdir_in(tmp)
            .map_err(|error| error.to_string())?;
        let path = owner.path().join("diagnostic.json");
        Ok(Self {
            path,
            _owner: Some(owner),
        })
    }

    pub(super) fn write(&self, bytes: &[u8]) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or("diagnostic output parent is missing")?
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let tmp = workspace_tmp()?
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !parent.starts_with(&tmp) {
            return Err("diagnostic output must be under workspace tmp".to_owned());
        }
        let path = parent.join(
            self.path
                .file_name()
                .ok_or("diagnostic output filename is missing")?,
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())
    }
}

fn workspace_tmp() -> Result<PathBuf, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root missing")?;
    Ok(workspace.join("tmp"))
}

#[cfg(test)]
#[path = "js_runtime_scripts_raw_geometry_diagnostic_output_tests.rs"]
mod tests;
