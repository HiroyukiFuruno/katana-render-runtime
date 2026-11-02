use super::{Mode, NORMAL_WIDTH, compare, parse_source, render, usvg};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[path = "svg_engine_render_marker_legacy_aa_clone_tests.rs"]
mod clone_tree;
#[path = "svg_engine_render_painting_partition_tests.rs"]
mod partition;

const INVENTORY: &str = include_str!(
    "../../../tests/fixtures/svg-engine/browser-corrections/marker/capture-cases.json"
);
const CASE_COUNT: usize = 48;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Inventory {
    schema: String,
    requested_width: u32,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    input: String,
    input_sha256: String,
    upstream_png_sha256: String,
    requested_width: u32,
    requested_height: u32,
}

pub(super) struct MarkerLegacyAa;

impl MarkerLegacyAa {
    pub(super) fn verify_if_member(input: &Path) -> Result<bool, String> {
        let inventory = read_inventory()?;
        let root = fixture_root();
        let canonical = input.canonicalize().map_err(|error| error.to_string())?;
        for case in &inventory.cases {
            let candidate = root
                .join(&case.input)
                .canonicalize()
                .map_err(|error| error.to_string())?;
            if candidate == canonical {
                let mut errors = Vec::new();
                record_case(&root, case, &mut errors);
                if !errors.is_empty() {
                    return Err(errors.join("\n"));
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/svg-engine/upstream/resvg/tests")
}

fn read_inventory() -> Result<Inventory, String> {
    let inventory: Inventory =
        serde_json::from_str(INVENTORY).map_err(|error| error.to_string())?;
    if inventory.schema != "krr-svg-marker-capture-cases-v1"
        || inventory.requested_width != NORMAL_WIDTH
        || inventory.cases.len() != CASE_COUNT
    {
        return Err("marker inventory contract differs".into());
    }
    Ok(inventory)
}

#[test]
fn exact_48_marker_cases_compare_legacy_aa_on_cloned_tree() -> Result<(), String> {
    let inventory = read_inventory()?;
    let root = fixture_root();
    let mut seen = HashSet::new();
    let mut errors = Vec::new();
    let mut total = 0;
    for case in &inventory.cases {
        if !seen.insert(&case.input) {
            errors.push(format!("{}: duplicate case", case.input));
            continue;
        }
        total += record_case(&root, case, &mut errors);
    }
    println!("current total={total}");
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn record_case(root: &Path, case: &Case, errors: &mut Vec<String>) -> usize {
    match compare_case(root, case) {
        Ok((current, legacy, converted)) => {
            println!(
                "{} current={} legacy={} converted={}",
                case.input, current, legacy, converted
            );
            if legacy != 0 {
                errors.push(format!("{}: legacy difference {legacy}", case.input));
            }
            if current == 0 {
                errors.push(format!(
                    "{}: clipping control had no observed effect",
                    case.input
                ));
            }
            current
        }
        Err(error) => {
            errors.push(format!("{}: {error}", case.input));
            0
        }
    }
}

type LoadedCase = (PathBuf, Vec<u8>, Vec<u8>);

fn load_case(root: &Path, case: &Case) -> Result<LoadedCase, String> {
    let relative = Path::new(&case.input);
    if !case.input.starts_with("tests/painting/")
        || !case.input.ends_with(".svg")
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("invalid case path".into());
    }
    let input = root.join(relative);
    let bytes = std::fs::read(&input).map_err(|error| error.to_string())?;
    assert_source(root, &bytes, &case.input_sha256)?;
    let reference =
        std::fs::read(input.with_extension("png")).map_err(|error| error.to_string())?;
    assert_sha(&reference, &case.upstream_png_sha256)?;
    let png = image::load_from_memory(&reference).map_err(|error| error.to_string())?;
    if png.width() != case.requested_width
        || png.height() != case.requested_height
        || case.requested_width != NORMAL_WIDTH
    {
        return Err("reference dimensions differ".into());
    }
    Ok((input, bytes, reference))
}

fn compare_case(root: &Path, case: &Case) -> Result<(usize, usize, usize), String> {
    let (input, bytes, reference) = load_case(root, case)?;
    let tree = parse_source(&input, &bytes)?;
    let original = render(&tree, Mode::Normal)?;
    let masks = super::marker_clip_mask_tests::MarkerClipMask::verify(
        &tree,
        original.width(),
        original.height(),
    )?;
    println!("{} masks={} mask_difference=0", case.input, masks);
    let mut legacy = tree.clone();
    let converted = clone_tree::change_marker_candidates(&mut legacy)?;
    if converted == 0 {
        return Err("no marker candidates converted".into());
    }
    let legacy_image = render(&legacy, Mode::Normal)?;
    let again = render(&tree, Mode::Normal)?;
    if original.data() != again.data() {
        return Err("original tree changed".into());
    }
    Ok((
        compare::pixel_differences(&original, &reference)?,
        compare::pixel_differences(&legacy_image, &reference)?,
        converted,
    ))
}

fn assert_sha(data: &[u8], expected: &str) -> Result<(), String> {
    let actual = Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected {
        return Err("fixture SHA differs".into());
    }
    Ok(())
}

fn assert_source(root: &Path, bytes: &[u8], expected: &str) -> Result<(), String> {
    assert_sha(bytes, expected)?;
    let source = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let lower = source.to_ascii_lowercase();
    if ["clippath", "clip-path", "shape-rendering"]
        .iter()
        .any(|value| lower.contains(value))
    {
        return Err("explicit clip or shape rendering is unsupported".into());
    }
    let document = usvg::roxmltree::Document::parse(source).map_err(|error| error.to_string())?;
    for node in document.descendants().filter(|node| node.is_element()) {
        for attr in node.attributes().filter(|attr| attr.name() == "href") {
            if !attr.value().starts_with('#') && attr.value() != "../../../resources/image.svg" {
                return Err("unrecognized external resource".into());
            }
        }
    }
    if lower.contains("<image") {
        assert_external_image(root)?;
    }
    Ok(())
}

fn assert_external_image(root: &Path) -> Result<(), String> {
    let bytes =
        std::fs::read(root.join("resources/image.svg")).map_err(|error| error.to_string())?;
    if bytes
        != include_bytes!(
            "../../../tests/fixtures/svg-engine/upstream/resvg/tests/resources/image.svg"
        )
    {
        return Err("external image source changed".into());
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| error.to_string())?
        .to_ascii_lowercase();
    if ["clippath", "clip-path", "shape-rendering", "href="]
        .iter()
        .any(|value| text.contains(value))
    {
        return Err("external image has unsupported nested content".into());
    }
    Ok(())
}
