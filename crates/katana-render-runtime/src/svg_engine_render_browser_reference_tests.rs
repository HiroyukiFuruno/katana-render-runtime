use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MANIFEST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/browser-corrections/morphology/manifest.json"
);
const CORRECTION_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/browser-corrections/morphology"
);
const MORPHOLOGY_CASES: [&str; 6] = [
    "filters/feMorphology/empty-radius.svg",
    "filters/feMorphology/negative-radius.svg",
    "filters/feMorphology/no-radius.svg",
    "filters/feMorphology/radius-with-too-many-values.svg",
    "filters/feMorphology/radius=10-0.svg",
    "filters/feMorphology/zero-radius.svg",
];
const ONE_AXIS_CASE: &str = "filters/feMorphology/radius=10-0.svg";
const EXPECTED_BROWSER_SHA256: &str =
    "031c93c5e822ffd8d22121bb41ed139fb6fcc4e96f6aa1f43ea31ec36fb9242c";
const EXPECTED_BROWSER_NAME: &str = "Chromium font observer";
const EXPECTED_BROWSER_VERSION: &str = "155.0.8059.85";
const EXPECTED_RENDER_WIDTH: u32 = 300;
const EXPECTED_RENDER_HEIGHT: u32 = 300;
const EXPECTED_DEVICE_SCALE_FACTOR: u32 = 1;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    browser: Browser,
    render: Render,
    cases: Vec<Case>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Browser {
    name: String,
    version: String,
    sha256: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Render {
    width: u32,
    height: u32,
    device_scale_factor: u32,
    transparent: bool,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Comparison {
    BrowserPixels,
    OneAxisContract,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    input: String,
    input_sha256: String,
    upstream_png_sha256: String,
    reference: String,
    reference_sha256: String,
    comparison: Comparison,
}

pub(super) fn verify_manifest() -> Result<(), String> {
    load_verified_manifest().map(|_| ())
}

pub(super) fn reference_png_path(input: &Path) -> Result<PathBuf, String> {
    let relative = canonical_fixture_relative_path(input)?;
    let Some(relative) = relative else {
        return Ok(input.with_extension("png"));
    };
    let relative = relative
        .to_str()
        .ok_or_else(|| "fixture path is not UTF-8".to_owned())?
        .replace('\\', "/");
    if !MORPHOLOGY_CASES.contains(&relative.as_str()) {
        return Ok(input.with_extension("png"));
    }
    let manifest = load_verified_manifest()?;
    let record = manifest
        .cases
        .iter()
        .find(|case| case.input == relative)
        .ok_or_else(|| format!("browser correction missing for {relative}"))?;
    Ok(Path::new(CORRECTION_ROOT).join(&record.reference))
}

pub(super) fn is_one_axis_contract(input: &Path) -> Result<bool, String> {
    let relative = canonical_fixture_relative_path(input)?;
    let Some(relative) = relative else {
        return Ok(false);
    };
    let relative = relative
        .to_str()
        .ok_or_else(|| "fixture path is not UTF-8".to_owned())?
        .replace('\\', "/");
    if !MORPHOLOGY_CASES.contains(&relative.as_str()) {
        return Ok(false);
    }
    let manifest = load_verified_manifest()?;
    let record = manifest
        .cases
        .iter()
        .find(|case| case.input == relative)
        .ok_or_else(|| format!("browser correction missing for {relative}"))?;
    Ok(record.comparison == Comparison::OneAxisContract)
}

fn canonical_fixture_relative_path(input: &Path) -> Result<Option<PathBuf>, String> {
    let root = Path::new(super::inventory::FIXTURE_ROOT)
        .join("tests")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let input = input.canonicalize().map_err(|error| error.to_string())?;
    Ok(input.strip_prefix(root).ok().map(Path::to_path_buf))
}

fn load_verified_manifest() -> Result<Manifest, String> {
    let bytes = std::fs::read(MANIFEST).map_err(|error| error.to_string())?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &Manifest) -> Result<(), String> {
    validate_manifest_at(
        manifest,
        Path::new(super::inventory::FIXTURE_ROOT)
            .join("tests")
            .as_path(),
        Path::new(CORRECTION_ROOT),
    )
}

fn validate_manifest_at(
    manifest: &Manifest,
    upstream_root: &Path,
    correction_root: &Path,
) -> Result<(), String> {
    validate_metadata(manifest)?;
    if manifest.cases.len() != MORPHOLOGY_CASES.len() {
        return Err("browser correction case count mismatch".to_owned());
    }
    let mut inputs = BTreeSet::new();
    for case in &manifest.cases {
        validate_case_at(case, &mut inputs, upstream_root, correction_root)?;
    }
    if inputs.len() != MORPHOLOGY_CASES.len()
        || MORPHOLOGY_CASES
            .iter()
            .any(|input| !inputs.contains(*input))
    {
        return Err("browser correction case inventory mismatch".to_owned());
    }
    Ok(())
}

fn validate_metadata(manifest: &Manifest) -> Result<(), String> {
    let browser = &manifest.browser;
    let render = &manifest.render;
    if manifest.schema != "krr-svg-browser-corrections-v1"
        || browser.name != EXPECTED_BROWSER_NAME
        || browser.version != EXPECTED_BROWSER_VERSION
        || browser.sha256 != EXPECTED_BROWSER_SHA256
        || render.width != EXPECTED_RENDER_WIDTH
        || render.height != EXPECTED_RENDER_HEIGHT
        || render.device_scale_factor != EXPECTED_DEVICE_SCALE_FACTOR
        || !render.transparent
    {
        return Err("browser correction metadata mismatch".to_owned());
    }
    Ok(())
}

fn validate_case_at(
    case: &Case,
    inputs: &mut BTreeSet<String>,
    upstream_root: &Path,
    correction_root: &Path,
) -> Result<(), String> {
    if !MORPHOLOGY_CASES.contains(&case.input.as_str()) || !inputs.insert(case.input.clone()) {
        return Err("invalid or duplicate browser correction input".to_owned());
    }
    if case.comparison != expected_comparison(&case.input)? {
        return Err("browser correction comparison role mismatch".to_owned());
    }
    let input = Path::new(&case.input);
    let expected_reference = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| format!("{stem}.png"))
        .ok_or_else(|| "invalid browser correction input name".to_owned())?;
    if case.reference != expected_reference {
        return Err("browser correction reference path mismatch".to_owned());
    }
    verify_case_files(case, input, upstream_root, correction_root)
}

fn expected_comparison(input: &str) -> Result<Comparison, String> {
    if input == ONE_AXIS_CASE {
        Ok(Comparison::OneAxisContract)
    } else if MORPHOLOGY_CASES.contains(&input) {
        Ok(Comparison::BrowserPixels)
    } else {
        Err("invalid browser correction input".to_owned())
    }
}

fn verify_case_files(
    case: &Case,
    input: &Path,
    upstream_root: &Path,
    correction_root: &Path,
) -> Result<(), String> {
    verify_file_sha256(&upstream_root.join(input), &case.input_sha256)?;
    verify_file_sha256(
        &upstream_root.join(input.with_extension("png")),
        &case.upstream_png_sha256,
    )?;
    let reference = correction_root.join(&case.reference);
    verify_file_sha256(&reference, &case.reference_sha256)?;
    verify_reference_dimensions(&reference)
}

fn verify_file_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if super::inventory::hex_digest(&bytes) != expected {
        return Err(format!(
            "browser correction SHA mismatch: {}",
            path.display()
        ));
    }
    Ok(())
}

fn verify_reference_dimensions(path: &Path) -> Result<(), String> {
    let image = image::load_from_memory(&std::fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    if image.width() != EXPECTED_RENDER_WIDTH || image.height() != EXPECTED_RENDER_HEIGHT {
        return Err(format!(
            "browser correction dimensions mismatch: {}",
            path.display()
        ));
    }
    Ok(())
}

#[path = "svg_engine_render_browser_reference_unit_tests.rs"]
mod unit_tests;
