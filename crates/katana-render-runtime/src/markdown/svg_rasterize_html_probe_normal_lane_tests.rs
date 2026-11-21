use super::{TestResult, run_exact_probe, sha256};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const FRAME_MODULE: &str = "markdown::svg_rasterize::font::html::lazy_diagnostic::final_glyphs::css_equivalence_tests::frame_diagnostic";
const BASELINE_TEST: &str = "markdown::svg_rasterize::html_glyph::typeface_tests::helvetica_tests::font_metrics_tests::font_baseline_tests::baseline_cases_write_png_and_metadata";
const STAGES_TEST: &str = "markdown::svg_rasterize::tests::font_memory_tests::stages::macos_font_initialization_stages_report_time_and_rss";
const HTML: &str = "<!doctype html><html><body><h1 style='font-family:sans-serif;font-weight:700'>Example heading</h1><p style='font-family:sans-serif'>Example body</p></body></html>";
const NO_TEXT_HTML: &str = "<!doctype html><html><body><div style='width:80px;height:40px;background:red'></div></body></html>";

#[test]
fn html_frame_and_glyph_audit_run_in_normal_lane() -> TestResult<()> {
    check_frame("ignored_html_frame_and_font_audit_are_saved_together", true)
}

#[test]
fn html_frame_without_glyph_requirement_runs_in_normal_lane() -> TestResult<()> {
    check_frame(
        "ignored_html_frame_is_saved_without_glyph_requirement",
        false,
    )
}

fn check_frame(case: &str, require_glyphs: bool) -> TestResult<()> {
    let directory =
        crate::markdown::svg_rasterize::probe_test_support::TempProbeDirectory::create()?;
    let input = directory.file("frame-input.html");
    let output = directory.file("frame-output");
    let request = directory.file("frame-request.json");
    let html = if require_glyphs { HTML } else { NO_TEXT_HTML };
    std::fs::write(&input, html)?;
    let source_sha = sha256(html.as_bytes());
    write_frame_request(&request, &input, &output, &source_sha)?;
    run_exact_probe(
        &format!("{FRAME_MODULE}::{case}"),
        &[("KRR_HTML_FONT_AUDIT_REQUEST", &request)],
    )?;
    validate_frame(&output, &source_sha, require_glyphs)
}

fn write_frame_request(request: &Path, input: &Path, output: &Path, sha: &str) -> TestResult<()> {
    let value = json!({"input": input, "output": output, "width": 320, "height": 180,
                       "expected_source_sha256": sha});
    std::fs::write(request, serde_json::to_vec(&value)?)?;
    Ok(())
}

fn validate_frame(output: &Path, source_sha: &str, require_glyphs: bool) -> TestResult<()> {
    let report: Value = serde_json::from_slice(&std::fs::read(output.join("report.json"))?)?;
    assert_eq!(report["sourceSha256"], source_sha);
    assert_eq!(report["formalAcceptance"], false);
    assert_eq!(
        (
            report["viewport"]["width"].as_u64(),
            report["viewport"]["height"].as_u64()
        ),
        (Some(320), Some(180))
    );
    let png = image::load_from_memory(&std::fs::read(output.join("frame.png"))?)?.to_rgba8();
    assert_eq!((png.width(), png.height()), (320, 180));
    assert!(png.pixels().any(|pixel| pixel[3] != 0));
    assert_eq!(report["rgbaSha256"], sha256(png.as_raw()));
    check_svg_sha(output, "render-input.svg", &report["renderInputSvgSha256"])?;
    check_svg_sha(output, "resolved-svg.svg", &report["resolvedSvgSha256"])?;
    validate_glyph_audit(&report["audit"], require_glyphs)
}

fn check_svg_sha(output: &Path, name: &str, expected: &Value) -> TestResult<()> {
    let bytes = std::fs::read(output.join(name))?;
    assert_eq!(*expected, sha256(&bytes));
    Ok(())
}

fn validate_glyph_audit(audit: &Value, require_glyphs: bool) -> TestResult<()> {
    assert!(
        audit["finalGlyphPaintCalls"]
            .as_u64()
            .is_some_and(|count| count > 0)
    );
    let trees = audit["finalGlyphTrees"]
        .as_array()
        .ok_or("glyph trees missing")?;
    assert!(!trees.is_empty());
    let glyphs = trees
        .iter()
        .flat_map(|tree| tree["faces"].as_array().into_iter().flatten())
        .filter_map(|face| face["glyphCount"].as_u64())
        .sum::<u64>();
    assert_eq!(glyphs > 0, require_glyphs);
    Ok(())
}

#[test]
fn glyph_baseline_files_run_in_normal_lane() -> TestResult<()> {
    let directory =
        crate::markdown::svg_rasterize::probe_test_support::TempProbeDirectory::create()?;
    let output = directory.file("baseline");
    run_exact_probe(
        BASELINE_TEST,
        &[("KRR_HTML_GLYPH_BASELINE_DIAGNOSTIC_OUTPUT", &output)],
    )?;
    let report: Value = serde_json::from_slice(&std::fs::read(output.join("metadata.json"))?)?;
    let cases = report["cases"].as_array().ok_or("baseline cases missing")?;
    assert_eq!(cases.len(), 14);
    for row in cases {
        check_baseline_row(&output, row)?;
    }
    Ok(())
}

fn check_baseline_row(output: &Path, row: &Value) -> TestResult<()> {
    let name = row["case"].as_str().ok_or("case name missing")?;
    assert!(
        name.bytes()
            .all(|value| value.is_ascii_alphanumeric() || value == b'-' || value == b'_')
    );
    assert!(row["glyphCount"].as_u64().is_some_and(|count| count > 0));
    let pixels =
        image::load_from_memory(&std::fs::read(output.join(format!("{name}.png")))?)?.to_rgba8();
    assert_eq!((pixels.width(), pixels.height()), (160, 80));
    assert!(pixels.pixels().any(|pixel| pixel.0[..3] != [255, 255, 255]));
    let svg = std::fs::read(output.join(format!("{name}.svg")))?;
    assert_eq!(row["sourceSvgSha256"], sha256(&svg));
    Ok(())
}

#[test]
fn font_memory_stages_run_in_normal_lane() -> TestResult<()> {
    let marker = PathBuf::from("1");
    let output = run_exact_probe(STAGES_TEST, &[("KRR_TEST_FONT_STAGES_CHILD", &marker)])?;
    let records = String::from_utf8(output.stderr)?;
    let rows: Vec<Value> = records
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| row["probe"] == "html-font-stage-rss")
        .collect();
    let expected = [
        "process-start",
        "font-database",
        "font-database-memory-snapshot",
        "selected-physical-font-bytes",
        "skia-font-manager-default",
        "skia-data-copy",
        "skia-new-from-data",
        "physical-source-proof",
        "first-glyph-draw",
    ];
    let names: Vec<_> = rows.iter().map(|row| row["stage"].as_str()).collect();
    assert_eq!(names, expected.map(Some));
    for row in rows {
        assert!(row["elapsed_ns"].as_u64().is_some());
        assert!(row["rss_kib"].as_u64().is_some());
    }
    Ok(())
}
