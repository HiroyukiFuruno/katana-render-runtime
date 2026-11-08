use super::{TestResult, run_exact_probe, sha256};
use serde_json::{Value, json};
use std::path::Path;

const TREE_MODULE: &str = "markdown::svg_rasterize::text_shaping::harfrust::tree_diagnostic";
const INPUT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="64"><text x="0" y="48" font-family="trebuchet ms,verdana,arial,sans-serif" font-size="16" font-weight="400">Root Runtime V8 DOM shim Output SVG Rasterize</text><text x="0" y="32" font-family="trebuchet ms,verdana,arial,sans-serif" font-size="16" font-weight="700">/</text></svg>"#;
const CASES: [(&str, u64); 8] = [
    ("/", 700),
    ("Root", 400),
    ("Runtime", 400),
    ("V8", 400),
    ("DOM shim", 400),
    ("Output", 400),
    ("SVG", 400),
    ("Rasterize", 400),
];

#[test]
fn tree_harfrust_diagnostic_runs_in_normal_lane() -> TestResult<()> {
    let (directory, report) = run_tree_probe(
        "records_tree_trebuchet_harfrust_and_usvg_metrics",
        "krr-tree-trebuchet-harfrust-usvg-layout-v2",
    )?;
    let rows = checked_rows(&report)?;
    for (row, (text, weight)) in rows.iter().zip(CASES) {
        assert_eq!(row["text"], text);
        assert_eq!(row["weight"], weight);
        assert_eq!(row["glyph_count_equal"], true);
        assert!(
            !row["harfrust"]
                .as_array()
                .ok_or("glyph records missing")?
                .is_empty()
        );
        assert!(row["usvg_metrics"].is_object());
    }
    validate_tree_svgs(&directory.file("output"))?;
    Ok(())
}

fn validate_tree_svgs(output: &Path) -> TestResult<()> {
    assert_eq!(
        std::fs::read(output.join("text.svg"))?,
        INPUT_SVG.as_bytes()
    );
    let outline = std::fs::read_to_string(output.join("outline.svg"))?;
    let document = resvg::usvg::roxmltree::Document::parse(&outline)?;
    assert_eq!(document.root_element().tag_name().name(), "svg");
    assert!(!document.descendants().any(|node| node.has_tag_name("text")));
    assert!(document.descendants().any(|node| node.has_tag_name("path")));
    Ok(())
}

#[test]
fn tree_coretext_raster_diagnostic_runs_in_normal_lane() -> TestResult<()> {
    let (directory, report) = run_tree_probe(
        "coretext_diagnostic::records_coretext_tree_glyph_raster_and_krr_reference",
        "krr-tree-coretext-glyph-raster-diagnostic-v1",
    )?;
    for (index, (row, (text, weight))) in checked_rows(&report)?.iter().zip(CASES).enumerate() {
        assert_eq!(row["text"], text);
        assert_eq!(row["weight"], weight);
        assert_eq!(row["face_index"], 0);
        assert!(
            !row["glyphs"]
                .as_array()
                .ok_or("glyph records missing")?
                .is_empty()
        );
        for (key, prefix) in [("coretext", "coretext"), ("krr_reference", "krr")] {
            check_png(
                &directory
                    .file("output")
                    .join(format!("{prefix}-{index:02}.png")),
                &row[key],
            )?;
        }
    }
    Ok(())
}

fn run_tree_probe(
    case: &str,
    schema: &str,
) -> TestResult<(
    crate::markdown::svg_rasterize::probe_test_support::TempProbeDirectory,
    Value,
)> {
    let directory =
        crate::markdown::svg_rasterize::probe_test_support::TempProbeDirectory::create()?;
    let input = directory.file("tree-input.html");
    let output = directory.file("output");
    let request = directory.file("tree-request.json");
    let html = format!("<!doctype html><html><body>{INPUT_SVG}</body></html>");
    std::fs::write(&input, &html)?;
    let source_sha = sha256(html.as_bytes());
    write_tree_request(&request, &input, &output, &source_sha)?;
    run_exact_probe(
        &format!("{TREE_MODULE}::{case}"),
        &[("KRR_HTML_FONT_AUDIT_REQUEST", &request)],
    )?;
    let report: Value = serde_json::from_slice(&std::fs::read(output.join("report.json"))?)?;
    assert_eq!(report["schema"], schema);
    assert_eq!(report["input_sha256"], source_sha);
    assert_eq!(report["formal_acceptance"], false);
    Ok((directory, report))
}

fn write_tree_request(request: &Path, input: &Path, output: &Path, sha: &str) -> TestResult<()> {
    let payload = json!({
        "input": input, "output": output, "expected_source_sha256": sha,
    });
    std::fs::write(request, serde_json::to_vec(&payload)?)?;
    Ok(())
}

fn checked_rows(report: &Value) -> TestResult<&[Value]> {
    let rows = report["rows"].as_array().ok_or("diagnostic rows missing")?;
    assert_eq!(rows.len(), CASES.len());
    Ok(rows.as_slice())
}

fn check_png(path: &Path, record: &Value) -> TestResult<()> {
    let data = std::fs::read(path)?;
    assert_eq!(record["png_sha256"], sha256(&data));
    let image = image::load_from_memory(&data)?.to_rgba8();
    assert_eq!((image.width(), image.height()), (512, 64));
    assert!(image.pixels().any(|pixel| pixel[3] != 0));
    Ok(())
}
