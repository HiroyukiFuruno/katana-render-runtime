use super::{CORRECTION_ROOT, Case, Manifest, validate_metadata, verify_case_files};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const MUTATED_BROWSER_NAME: &str = "other browser";
const MUTATED_BROWSER_VERSION: &str = "0.0.0";
const MUTATED_BROWSER_SHA256: &str = "0";
const TEST_WRONG_IMAGE_WIDTH: u32 = 1;
const TEST_WRONG_IMAGE_HEIGHT: u32 = 1;
const METADATA_INVALID_INCREMENT: u32 = 1;

#[path = "svg_engine_render_browser_reference_role_tests.rs"]
mod role_tests;

#[derive(Clone, Copy, Debug)]
enum MetadataMutation {
    Schema,
    BrowserName,
    BrowserVersion,
    BrowserSha,
    Width,
    Height,
    DeviceScaleFactor,
    Transparent,
}

#[derive(Clone, Copy, Debug)]
enum FileMutation {
    InputSha,
    UpstreamPngSha,
    ReferenceSha,
    MissingInput,
    MissingReference,
    InvalidPng,
    WrongDimensions,
}

struct CaseSandbox {
    _directory: TempDir,
    upstream_root: PathBuf,
    correction_root: PathBuf,
}

#[test]
fn browser_metadata_mutations_are_rejected() -> Result<(), String> {
    assert!(validate_metadata(&valid_manifest()?).is_ok());
    for mutation in [
        MetadataMutation::Schema,
        MetadataMutation::BrowserName,
        MetadataMutation::BrowserVersion,
        MetadataMutation::BrowserSha,
        MetadataMutation::Width,
        MetadataMutation::Height,
        MetadataMutation::DeviceScaleFactor,
        MetadataMutation::Transparent,
    ] {
        let mut manifest = valid_manifest()?;
        mutate_metadata(&mut manifest, mutation);
        assert!(validate_metadata(&manifest).is_err(), "{mutation:?}");
    }
    Ok(())
}

#[test]
fn case_files_reject_mutated_hash_missing_and_invalid_png_inputs() -> Result<(), String> {
    let manifest = valid_manifest()?;
    let case = manifest
        .cases
        .first()
        .ok_or("correction case missing")?
        .clone();
    let sandbox = case_sandbox(&case)?;
    verify_case_files(
        &case,
        Path::new(&case.input),
        &sandbox.upstream_root,
        &sandbox.correction_root,
    )?;
    for mutation in [
        FileMutation::InputSha,
        FileMutation::UpstreamPngSha,
        FileMutation::ReferenceSha,
        FileMutation::MissingInput,
        FileMutation::MissingReference,
        FileMutation::InvalidPng,
        FileMutation::WrongDimensions,
    ] {
        assert_file_mutation_rejected(case.clone(), mutation)?;
    }
    Ok(())
}

fn valid_manifest() -> Result<Manifest, String> {
    let bytes = std::fs::read(super::MANIFEST).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn mutate_metadata(manifest: &mut Manifest, mutation: MetadataMutation) {
    match mutation {
        MetadataMutation::Schema => manifest.schema.push_str("-changed"),
        MetadataMutation::BrowserName => manifest.browser.name = MUTATED_BROWSER_NAME.to_owned(),
        MetadataMutation::BrowserVersion => {
            manifest.browser.version = MUTATED_BROWSER_VERSION.to_owned();
        }
        MetadataMutation::BrowserSha => manifest.browser.sha256 = MUTATED_BROWSER_SHA256.to_owned(),
        MetadataMutation::Width => manifest.render.width += METADATA_INVALID_INCREMENT,
        MetadataMutation::Height => manifest.render.height += METADATA_INVALID_INCREMENT,
        MetadataMutation::DeviceScaleFactor => {
            manifest.render.device_scale_factor += METADATA_INVALID_INCREMENT;
        }
        MetadataMutation::Transparent => manifest.render.transparent = false,
    }
}

fn assert_file_mutation_rejected(case: Case, mutation: FileMutation) -> Result<(), String> {
    let sandbox = case_sandbox(&case)?;
    let mut case = case;
    apply_file_mutation(&mut case, &sandbox, mutation)?;
    assert!(
        verify_case_files(
            &case,
            Path::new(&case.input),
            &sandbox.upstream_root,
            &sandbox.correction_root
        )
        .is_err(),
        "{mutation:?}"
    );
    Ok(())
}

fn case_sandbox(case: &Case) -> Result<CaseSandbox, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let upstream_root = directory.path().join("upstream/tests");
    let correction_root = directory.path().join("corrections");
    copy_fixture(
        &original_input_path(&case.input),
        &upstream_root.join(&case.input),
    )?;
    copy_fixture(
        &original_input_path(&case.input).with_extension("png"),
        &upstream_root.join(Path::new(&case.input).with_extension("png")),
    )?;
    copy_fixture(
        &Path::new(CORRECTION_ROOT).join(&case.reference),
        &correction_root.join(&case.reference),
    )?;
    Ok(CaseSandbox {
        _directory: directory,
        upstream_root,
        correction_root,
    })
}

fn original_input_path(input: &str) -> PathBuf {
    Path::new(super::super::inventory::FIXTURE_ROOT)
        .join("tests")
        .join(input)
}

fn copy_fixture(source: &Path, destination: &Path) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or("fixture destination has no parent")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn apply_file_mutation(
    case: &mut Case,
    sandbox: &CaseSandbox,
    mutation: FileMutation,
) -> Result<(), String> {
    let input = sandbox.upstream_root.join(&case.input);
    let original_png = sandbox
        .upstream_root
        .join(Path::new(&case.input).with_extension("png"));
    let reference = sandbox.correction_root.join(&case.reference);
    match mutation {
        FileMutation::InputSha => {
            std::fs::write(input, b"changed source").map_err(|error| error.to_string())?
        }
        FileMutation::UpstreamPngSha => std::fs::write(original_png, b"changed original png")
            .map_err(|error| error.to_string())?,
        FileMutation::ReferenceSha => std::fs::write(reference, b"changed corrected png")
            .map_err(|error| error.to_string())?,
        FileMutation::MissingInput => {
            std::fs::remove_file(input).map_err(|error| error.to_string())?
        }
        FileMutation::MissingReference => {
            std::fs::remove_file(reference).map_err(|error| error.to_string())?
        }
        FileMutation::InvalidPng => replace_reference(case, &reference, b"not a PNG")?,
        FileMutation::WrongDimensions => replace_reference(case, &reference, &wrong_size_png()?)?,
    }
    Ok(())
}

fn replace_reference(case: &mut Case, path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    case.reference_sha256 = super::super::inventory::hex_digest(bytes);
    Ok(())
}

fn wrong_size_png() -> Result<Vec<u8>, String> {
    let pixmap = tiny_skia::Pixmap::new(TEST_WRONG_IMAGE_WIDTH, TEST_WRONG_IMAGE_HEIGHT)
        .ok_or("failed to allocate test pixmap")?;
    pixmap.encode_png().map_err(|error| error.to_string())
}
