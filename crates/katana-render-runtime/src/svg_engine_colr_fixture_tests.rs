use std::{fs, path::Path, sync::Arc};

use super::{render_tree, usvg, writer_options};
use rstest::rstest;
use serde_json::Value;
use sha2::{Digest, Sha256};
use skrifa::{FontRef, MetadataProvider};

#[path = "svg_engine_colr_fixture_cases.rs"]
mod svg_engine_colr_fixture_cases;

const FIXTURE_DIRECTORY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/colr-public-fonts"
);
const FIXTURE_MANIFEST: &str = "provenance.json";
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const STATIC_FONT: &str = "colr_1.ttf";
const VARIABLE_FONT: &str = "colr_1_variable.ttf";
const LINEAR_GLYPH_ID: u32 = 9;
const RADIAL_GLYPH_ID: u32 = 93;
const LINEAR_GRADIENT: &str = "linearGradient";
const RADIAL_GRADIENT: &str = "radialGradient";

const FIXTURE_FILES: [(&str, &str, &str, u64); 3] = [
    (
        "colr_1.ttf",
        "tests/fonts/colr_1.ttf",
        "0441303144c314d46ee12cf9742f1dd3fcbcb42700e7c51dba7b1b689c1dd1d7",
        21_568,
    ),
    (
        "colr_1_variable.ttf",
        "tests/fonts/colr_1_variable.ttf",
        "1e861480ba54c411af09d600e9b0ecf8deb15b8c2929589ac5a84f2038332c18",
        53_096,
    ),
    (
        "LICENSE",
        "tests/fonts/colr_1_LICENSE",
        "58d1e17ffe5109a7ae296caafcadfdbe6a7d176f0bc4ab01e12a689b0499d8bd",
        11_357,
    ),
];

const MANIFEST_METADATA: [(&str, &str); 7] = [
    ("schema", "krr-svg-colr-test-fonts-v1"),
    ("sourcePackage", "ttf-parser"),
    ("sourceVersion", "0.25.1"),
    ("sourceRepository", "https://github.com/harfbuzz/ttf-parser"),
    ("sourceCommit", "56c33b910b03ca152f78363ec471c5dfd97c3069"),
    (
        "originalProject",
        "https://github.com/googlefonts/color-fonts",
    ),
    ("license", "Apache-2.0"),
];
#[rstest]
#[case(
    STATIC_FONT,
    "linear-gradient-repeat-clip",
    LINEAR_GLYPH_ID,
    LINEAR_GRADIENT,
    true
)]
#[case(
    STATIC_FONT,
    "radial-gradient",
    RADIAL_GLYPH_ID,
    RADIAL_GRADIENT,
    false
)]
#[case(
    VARIABLE_FONT,
    "linear-gradient-repeat-clip",
    LINEAR_GLYPH_ID,
    LINEAR_GRADIENT,
    true
)]
#[case(
    VARIABLE_FONT,
    "radial-gradient",
    RADIAL_GLYPH_ID,
    RADIAL_GRADIENT,
    false
)]
fn public_colr_glyphs_preserve_supported_paint_semantics_through_writer(
    #[case] font_name: &str,
    #[case] case: &str,
    #[case] glyph_id: u32,
    #[case] semantic: &str,
    #[case] clipped: bool,
) -> Result<(), String> {
    let manifest = read_and_verify_manifest()?;
    verify_glyph_roundtrip(&manifest, font_name, case, glyph_id, semantic, clipped)
}

fn verify_glyph_roundtrip(
    manifest: &Value,
    font_name: &str,
    case: &str,
    glyph_id: u32,
    semantic: &str,
    clipped: bool,
) -> Result<(), String> {
    let font_path = fixture_path(font_name);
    verify_manifest_file(manifest, font_name)?;
    let bytes = fs::read(&font_path).map_err(|error| error.to_string())?;
    let character = unicode_for_glyph(&bytes, glyph_id)?;
    let database = fixture_font_database(&font_path)?;
    let named_case = format!("{font_name}-{case}");
    let svg = glyph_svg(&database, character, &named_case)?;
    let tree = parse_fixture_svg(&svg, database.clone(), &named_case)?;
    assert_tree_semantic(&tree, semantic, clipped, &named_case)?;
    let serialized = tree.to_string(&writer_options(case));
    assert_svg_semantic(&serialized, semantic, clipped, &named_case)?;
    let reparsed = parse_fixture_svg(&serialized, database, &named_case)?;
    assert_tree_semantic(&reparsed, semantic, clipped, &named_case)?;
    assert_render_roundtrip(&tree, &reparsed, &named_case)
}

fn read_and_verify_manifest() -> Result<Value, String> {
    let path = fixture_path(FIXTURE_MANIFEST);
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let manifest: Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    for (field, expected) in MANIFEST_METADATA {
        if manifest[field].as_str() != Some(expected) {
            return Err(format!(
                "COLR fixture manifest metadata mismatch for {field}"
            ));
        }
    }
    for (name, _, _, _) in FIXTURE_FILES {
        verify_manifest_file(&manifest, name)?;
    }
    Ok(manifest)
}

fn verify_manifest_file(manifest: &Value, name: &str) -> Result<(), String> {
    let files = manifest["files"]
        .as_array()
        .ok_or("COLR fixture manifest files must be an array")?;
    let record = files
        .iter()
        .find(|entry| entry["path"].as_str() == Some(name))
        .ok_or_else(|| format!("COLR fixture manifest lacks {name}"))?;
    let (source_path, expected, expected_bytes) = FIXTURE_FILES
        .iter()
        .find(|(path, _, _, _)| *path == name)
        .map(|(_, source, digest, bytes)| (*source, *digest, *bytes))
        .ok_or_else(|| format!("unrecognized COLR fixture file {name}"))?;
    verify_manifest_file_record(record, name, source_path, expected, expected_bytes)?;
    verify_fixture_bytes(name, expected, expected_bytes)
}

fn verify_manifest_file_record(
    record: &Value,
    name: &str,
    source_path: &str,
    expected: &str,
    expected_bytes: u64,
) -> Result<(), String> {
    if record["sourcePath"].as_str() != Some(source_path) {
        return Err(format!("COLR fixture source path mismatch for {name}"));
    }
    let recorded_sha = record["sha256"]
        .as_str()
        .ok_or_else(|| format!("COLR fixture manifest SHA missing for {name}"))?;
    if recorded_sha != expected || record["bytes"].as_u64() != Some(expected_bytes) {
        return Err(format!("COLR fixture manifest record mismatch for {name}"));
    }
    Ok(())
}

fn verify_fixture_bytes(name: &str, expected: &str, expected_bytes: u64) -> Result<(), String> {
    let path = fixture_path(name);
    if fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .len()
        != expected_bytes
    {
        return Err(format!("COLR fixture byte count mismatch for {name}"));
    }
    let actual = Sha256::digest(fs::read(&path).map_err(|error| error.to_string())?);
    let actual = actual
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected {
        return Err(format!("COLR fixture SHA mismatch for {name}"));
    }
    Ok(())
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(FIXTURE_DIRECTORY).join(name)
}

fn unicode_for_glyph(bytes: &[u8], target: u32) -> Result<char, String> {
    let font = FontRef::new(bytes).map_err(|error| format!("invalid COLR font: {error}"))?;
    font.charmap()
        .mappings()
        .find_map(|(codepoint, glyph)| {
            (glyph.to_u32() == target)
                .then(|| char::from_u32(codepoint))
                .flatten()
        })
        .ok_or_else(|| format!("COLR glyph {target} has no Unicode mapping"))
}

fn fixture_font_database(path: &Path) -> Result<Arc<usvg::fontdb::Database>, String> {
    let mut database = usvg::fontdb::Database::new();
    database
        .load_font_file(path)
        .map_err(|error| format!("failed loading COLR fixture font: {error}"))?;
    if database.faces().next().is_none() {
        return Err("COLR fixture font database has no face".to_owned());
    }
    Ok(Arc::new(database))
}

fn glyph_svg(
    database: &usvg::fontdb::Database,
    character: char,
    case: &str,
) -> Result<String, String> {
    let family = database
        .faces()
        .next()
        .and_then(|face| face.families.first())
        .map(|(name, _)| name)
        .ok_or("COLR fixture font family is missing")?;
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"512\" height=\"512\"><text x=\"8\" y=\"440\" font-family=\"{family}\" font-size=\"500\">{character}</text></svg><!-- {case} -->"
    ))
}

fn parse_fixture_svg(
    svg: &str,
    database: Arc<usvg::fontdb::Database>,
    case: &str,
) -> Result<usvg::Tree, String> {
    let options = usvg::Options {
        fontdb: database,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(svg, &options)
        .map_err(|error| format!("failed parsing COLR fixture {case}: {error}"))
}

fn assert_tree_semantic(
    tree: &usvg::Tree,
    semantic: &str,
    clipped: bool,
    case: &str,
) -> Result<(), String> {
    let serialized = tree.to_string(&writer_options(case));
    assert_svg_semantic(&serialized, semantic, clipped, case)
}

fn assert_svg_semantic(svg: &str, semantic: &str, clipped: bool, case: &str) -> Result<(), String> {
    if !svg.contains(semantic) {
        return Err(format!("{case} lost COLR paint semantic {semantic}"));
    }
    if clipped && (!svg.contains("spreadMethod=\"repeat\"") || !svg.contains("clipPath")) {
        return Err("COLR linear gradient lost repeat or clipping semantics".to_owned());
    }
    Ok(())
}

fn assert_render_roundtrip(
    original: &usvg::Tree,
    reparsed: &usvg::Tree,
    case: &str,
) -> Result<(), String> {
    let before = render_tree(original)?;
    let after = render_tree(reparsed)?;
    assert!(
        has_opaque_pixels(before.data()),
        "empty COLR render for {case}"
    );
    assert!(
        before.data() == after.data(),
        "COLR writer pixels differ for {case}"
    );
    Ok(())
}

fn has_opaque_pixels(data: &[u8]) -> bool {
    data.chunks_exact(RGBA_CHANNELS)
        .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
}
