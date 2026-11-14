use super::{Mode, compare_reference, extra_mode, inventory};
use std::path::{Path, PathBuf};

const EXPECTED_CASES: usize = 1_757;

#[test]
fn upstream_all_render_differences_are_diagnostic_only() -> Result<(), String> {
    let explicit = std::env::var_os("KRR_SVG_RENDER_DIAGNOSTIC_ROOT").map(PathBuf::from);
    let owner = if explicit.is_none() {
        Some(temporary_root()?)
    } else {
        None
    };
    let root = explicit
        .as_deref()
        .or_else(|| owner.as_ref().map(tempfile::TempDir::path))
        .ok_or("diagnostic root is unavailable")?;
    if !root.is_absolute() {
        return Err("diagnostic root must be absolute".into());
    }
    let results = collect_results()?;
    write_report(root, results)
}

fn temporary_root() -> Result<tempfile::TempDir, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root missing")?;
    let tmp = workspace.join("tmp");
    std::fs::create_dir_all(&tmp).map_err(|error| error.to_string())?;
    tempfile::Builder::new()
        .prefix("svg-render-survey-")
        .tempdir_in(tmp)
        .map_err(|error| error.to_string())
}

fn collect_results() -> Result<Vec<serde_json::Value>, String> {
    let cases = inventory::diagnostic_cases()?;
    let mut results = Vec::with_capacity(cases.len());
    for (input, extra) in cases {
        let mode = if extra {
            extra_mode(&input)?
        } else {
            Mode::Normal
        };
        let (_, differences) = compare_reference(&input, mode)
            .map_err(|error| format!("{}: {error}", input.display()))?;
        results.push(serde_json::json!({
            "input": input.display().to_string(),
            "differentPixels": differences,
        }));
    }
    Ok(results)
}

fn write_report(root: &Path, results: Vec<serde_json::Value>) -> Result<(), String> {
    let different_cases = results
        .iter()
        .filter(|result| result["differentPixels"] != 0)
        .count();
    let report = serde_json::json!({
        "diagnosticOnly": true,
        "formalAcceptance": false,
        "differentCases": different_cases,
        "cases": results,
    });
    let output = tempfile::Builder::new()
        .prefix("svg-render-diagnostic-")
        .tempdir_in(root)
        .map_err(|error| error.to_string())?
        .keep();
    let bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    let path = output.join("report.json");
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    let saved = std::fs::read(path).map_err(|error| error.to_string())?;
    let saved: serde_json::Value =
        serde_json::from_slice(&saved).map_err(|error| error.to_string())?;
    validate_report(&saved)?;
    eprintln!(
        "diagnostic only: {different_cases} different cases; {}",
        output.display()
    );
    Ok(())
}

fn validate_report(report: &serde_json::Value) -> Result<(), String> {
    let cases = report["cases"]
        .as_array()
        .ok_or("diagnostic cases missing")?;
    if report["diagnosticOnly"] != true
        || report["formalAcceptance"] != false
        || cases.len() != EXPECTED_CASES
    {
        return Err("diagnostic report contract differs".into());
    }
    let mut different = 0_usize;
    for case in cases {
        if case["input"].as_str().is_none_or(str::is_empty) {
            return Err("diagnostic case input missing".into());
        }
        let count = case["differentPixels"]
            .as_u64()
            .ok_or("diagnostic difference missing")?;
        if count != 0 {
            different += 1;
        }
    }
    if report["differentCases"].as_u64() != Some(different as u64) {
        return Err("diagnostic difference count differs".into());
    }
    Ok(())
}
