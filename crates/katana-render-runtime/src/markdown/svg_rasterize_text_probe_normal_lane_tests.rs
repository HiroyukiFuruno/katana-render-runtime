use serde_json::Value;
#[cfg(target_os = "macos")]
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
type TestResult<T> = Result<T, Box<dyn std::error::Error>>;
const ROOT_FQN: &str = "markdown::svg_rasterize::text";
const FIXTURES: &str = "assets/test-fixtures/font-probes";
const BOLD_FONT_WEIGHT: u16 = 700;
const NORMAL_FONT_WEIGHT: u16 = 400;
const RECURSIVE_PROBE: (&str, &str, &str) = (
    "bbox_probe::recursive_probe::records_public_recursive_in_isolated_font_options",
    "KRR_RECURSIVE_PROBE_SFNT_PATH",
    "KRR_RECURSIVE_PROBE_JSON_PATH",
);
const HTML_PNG_PROBE: (&str, &str, &str) = (
    "html_png_probe::records_html_raster_route_for_verified_native_svg",
    "KRR_HTML_RASTER_PROBE_SVG_PATH",
    "KRR_HTML_RASTER_PROBE_PNG_PATH",
);

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_probe_normal_lane_tests.rs"]
mod html_probe_normal_lane_tests;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_tree_probe_normal_lane_tests.rs"]
mod tree_probe_normal_lane_tests;

#[cfg(target_os = "macos")]
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn public_bbox_probe_runs_in_normal_lane() -> TestResult<()> {
    let directory = super::super::super::probe_test_support::TempProbeDirectory::create()?;
    let output = directory.file("bbox.json");
    run_probe(
        "bbox_probe::records_library_bbox_for_live_font_declaration",
        &[("KRR_TEXT_BBOX_PROBE_OUTPUT", &output)],
    )?;
    let record = read_record(&output, "usvg-text-bbox-probe-r1")?;
    validate_rows(&record, false)?;
    assert_eq!(record["actual_live_font_proven"], false);
    assert_eq!(record["score_accepted"], false);
    Ok(())
}

#[test]
fn public_recursive_probe_runs_in_normal_lane() -> TestResult<()> {
    let directory = super::super::super::probe_test_support::TempProbeDirectory::create()?;
    let input = fixture("recursive-latin-crsv-normal.ttf")?;
    let output = directory.file("recursive.json");
    let (suffix, input_key, output_key) = RECURSIVE_PROBE;
    run_probe(suffix, &[(input_key, &input), (output_key, &output)])?;
    let record = read_record(&output, "isolated-public-recursive-probe-r1")?;
    assert_eq!(
        record["sfnt_sha256"],
        "f0b8414e5c903579bfd9dbca230a49f9063d92c6d622c6700f121a50a61e06ba"
    );
    validate_rows(&record, true)?;
    assert_eq!(record["production_font_database_modified"], false);
    Ok(())
}

#[test]
fn public_native_svg_probe_runs_in_normal_lane() -> TestResult<()> {
    let directory = super::super::super::probe_test_support::TempProbeDirectory::create()?;
    let input = fixture("native-class-ja-probe.svg")?;
    let output = directory.file("native.png");
    let (suffix, input_key, output_key) = HTML_PNG_PROBE;
    run_probe(suffix, &[(input_key, &input), (output_key, &output)])?;
    let svg = std::fs::read_to_string(input)?;
    assert!(svg.contains("<text") && !svg.contains("<foreignObject"));
    let pixels = image::load_from_memory(&std::fs::read(output)?)?.to_rgba8();
    assert!(pixels.width() > 0 && pixels.height() > 0);
    assert!(pixels.pixels().any(|pixel| pixel[3] != 0));
    assert!(
        pixels
            .pixels()
            .any(|pixel| pixel.0 != pixels.get_pixel(0, 0).0)
    );
    Ok(())
}

#[test]
fn public_font_and_svg_probes_reject_relative_input_without_output() -> TestResult<()> {
    let directory = super::super::super::probe_test_support::TempProbeDirectory::create()?;
    let input = PathBuf::from("relative-probe-input");
    let destination = directory.file("must-not-be-created");
    for (suffix, input_key, output_key) in [RECURSIVE_PROBE, HTML_PNG_PROBE] {
        let fqn = format!("{ROOT_FQN}::{suffix}");
        let result = execute_probe(&fqn, &[(input_key, &input), (output_key, &destination)])?;
        let stdout = std::str::from_utf8(&result.stdout)?;
        let stderr = std::str::from_utf8(&result.stderr)?;
        assert!(!result.status.success(), "{fqn}");
        assert!(stdout.contains("0 passed; 1 failed; 0 ignored"), "{fqn}");
        assert!(stderr.contains("absolute probe path required"), "{fqn}");
        assert!(!destination.exists(), "{fqn}");
    }
    Ok(())
}

fn fixture(name: &str) -> TestResult<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURES)
        .join(name)
        .canonicalize()?)
}

fn read_record(path: &Path, schema: &str) -> TestResult<Value> {
    let record: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    assert_eq!(record["schema"], schema);
    assert_eq!(record["score_accepted"], false);
    Ok(record)
}

fn validate_rows(record: &Value, recursive: bool) -> TestResult<()> {
    let rows = record["rows"].as_array().ok_or("rows missing")?;
    let expected = [
        ("動物", BOLD_FONT_WEIGHT),
        ("+int 年齢", NORMAL_FONT_WEIGHT),
        ("Animal", BOLD_FONT_WEIGHT),
        ("+int age", NORMAL_FONT_WEIGHT),
    ];
    assert_eq!(rows.len(), expected.len());
    for (row, (text, weight)) in rows.iter().zip(expected) {
        assert_eq!(row["text"], text);
        assert_eq!(row["requested_weight"], weight);
        assert_eq!(
            row["requested_family"],
            "Recursive Variable, arial, sans-serif"
        );
        validate_row(row, recursive)?;
    }
    Ok(())
}

fn validate_row(row: &Value, recursive: bool) -> TestResult<()> {
    for key in ["width", "height"] {
        let value = row["bbox"][key].as_f64().ok_or("bbox missing")?;
        assert!(value.is_finite() && value > 0.0);
    }
    let spans = row["spans"].as_array().ok_or("spans missing")?;
    assert!(!spans.is_empty());
    let glyph_count = spans
        .iter()
        .filter_map(|span| span["glyphs"].as_array())
        .map(Vec::len)
        .sum::<usize>();
    assert!(glyph_count > 0);
    if recursive {
        assert_eq!(row["missing_glyph_id_zero_count"], 0);
        assert_eq!(row["base_database_isolated"], true);
        assert!(
            row["alias_selector_calls"]
                .as_u64()
                .ok_or("calls missing")?
                > 0
        );
    }
    Ok(())
}

fn run_probe(suffix: &str, environment: &[(&str, &std::path::PathBuf)]) -> TestResult<()> {
    let fqn = format!("{ROOT_FQN}::{suffix}");
    run_exact_probe(&fqn, environment).map(|_| ())
}

fn run_exact_probe(
    fqn: &str,
    environment: &[(&str, &PathBuf)],
) -> TestResult<std::process::Output> {
    let output = execute_probe(fqn, environment)?;
    validate_probe_output(fqn, &output)?;
    Ok(output)
}

fn execute_probe(fqn: &str, environment: &[(&str, &PathBuf)]) -> TestResult<std::process::Output> {
    let mut command = crate::system::ProcessService::create_command(std::env::current_exe()?);
    command.args([
        "--ignored",
        "--exact",
        fqn,
        "--test-threads=1",
        "--quiet",
        "--nocapture",
    ]);
    for (key, value) in environment {
        command.env(key, value);
    }
    Ok(command.output()?)
}

fn validate_probe_output(fqn: &str, output: &std::process::Output) -> TestResult<()> {
    let stdout = std::str::from_utf8(&output.stdout)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "isolated probe {fqn} failed: {stdout}\n{stderr}"
    );
    let summaries: Vec<_> = stdout
        .lines()
        .filter(|line| line.starts_with("test result:"))
        .collect();
    assert_eq!(
        summaries.len(),
        1,
        "unexpected summaries for {fqn}: {stdout}\n{stderr}"
    );
    assert!(
        summaries[0].starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "),
        "isolated probe {fqn} did not run exactly one test: {stdout}\n{stderr}"
    );
    Ok(())
}
