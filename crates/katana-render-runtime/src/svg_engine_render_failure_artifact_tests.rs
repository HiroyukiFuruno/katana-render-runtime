use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

pub(super) fn save_if_requested(
    input: &Path,
    reference: &Path,
    pixmap: &Pixmap,
    differences: usize,
) -> Result<(), String> {
    let Some(root) = std::env::var_os("KRR_SVG_RENDER_FAILURE_ARTIFACTS") else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    if !root.is_absolute() {
        return Err("artifact root must be absolute".into());
    }
    let directory = tempfile::Builder::new()
        .prefix("svg-render-failure-")
        .tempdir_in(root)
        .map_err(|error| error.to_string())?
        .keep();
    write_artifacts(&directory, input, reference, pixmap, differences)
}

fn write_artifacts(
    directory: &Path,
    input: &Path,
    reference: &Path,
    pixmap: &Pixmap,
    differences: usize,
) -> Result<(), String> {
    let png = pixmap.encode_png().map_err(|error| error.to_string())?;
    std::fs::write(directory.join("actual.png"), png).map_err(|error| error.to_string())?;
    let metadata = serde_json::json!({
        "input": input.display().to_string(),
        "reference": reference.display().to_string(),
        "width": pixmap.width(),
        "height": pixmap.height(),
        "differentPixels": differences,
    });
    let bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| error.to_string())?;
    std::fs::write(directory.join("metadata.json"), bytes).map_err(|error| error.to_string())
}
