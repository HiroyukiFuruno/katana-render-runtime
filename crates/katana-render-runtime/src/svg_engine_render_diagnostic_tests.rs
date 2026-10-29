use super::{Mode, compare_reference, extra_mode, inventory};
use std::path::PathBuf;

#[test]
#[ignore = "diagnostic only; not a release acceptance gate"]
fn upstream_all_render_differences_are_diagnostic_only() -> Result<(), String> {
    let root = std::env::var_os("KRR_SVG_RENDER_DIAGNOSTIC_ROOT")
        .map(PathBuf::from)
        .ok_or("KRR_SVG_RENDER_DIAGNOSTIC_ROOT is required")?;
    if !root.is_absolute() {
        return Err("diagnostic root must be absolute".into());
    }
    let results = collect_results()?;
    write_report(root, results)
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

fn write_report(root: PathBuf, results: Vec<serde_json::Value>) -> Result<(), String> {
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
    std::fs::write(output.join("report.json"), bytes).map_err(|error| error.to_string())?;
    eprintln!(
        "diagnostic only: {different_cases} different cases; {}",
        output.display()
    );
    Ok(())
}
