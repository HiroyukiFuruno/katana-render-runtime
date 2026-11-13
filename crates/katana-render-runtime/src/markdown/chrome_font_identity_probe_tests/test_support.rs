use super::{
    CaptureRequest, CaptureResult, ProbeRequest, ProbeResponse, REQUEST_SCHEMA, RESPONSE_SCHEMA,
    ResourceFile, SelectedFont,
};
use crate::system::ProcessService;
use resvg::usvg::fontdb::{Database, Source};
use std::{fs, io::Write, path::Path, process::Output, sync::Arc};

pub(super) const PROBE_TEST: &str =
    "markdown::chrome_font_identity_probe_tests::chrome_font_identity_probe_cli";

pub(super) struct TempDirectory(tempfile::TempDir);

const PHYSICAL_FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");

pub(super) fn physical_font_fixture() -> Result<(TempDirectory, Database, SelectedFont), String> {
    let temp = TempDirectory::new()?;
    let (database, selected) =
        write_physical_font(&temp.path().join("physical-font.ttf"), PHYSICAL_FONT_BYTES)?;
    Ok((temp, database, selected))
}

fn write_physical_font(path: &Path, bytes: &[u8]) -> Result<(Database, SelectedFont), String> {
    fs::write(path, bytes).map_err(|error| error.to_string())?;
    load_physical_font(path)
}

fn load_physical_font(path: &Path) -> Result<(Database, SelectedFont), String> {
    let mut database = Database::new();
    database
        .load_font_file(path)
        .map_err(|error| error.to_string())?;
    /* WHY: fontdbはファミリー名を読み取れたfaceだけを登録する。 */
    let (face, family_name) = database
        .faces()
        .next()
        .and_then(|face| face.families.first().map(|(name, _)| (face, name)))
        .ok_or("fontdb did not load the font")?;
    let selected = SelectedFont {
        family_name: family_name.clone(),
        post_script_name: face.post_script_name.clone(),
        glyph_count: 1,
        is_custom_font: false,
    };
    Ok((database, selected))
}

pub(super) fn shared_physical_font_fixture(
    append_byte: bool,
) -> Result<(TempDirectory, Database, Vec<u8>), String> {
    let (temp, _file_database, _) = physical_font_fixture()?;
    let path = temp.path().join("physical-font.ttf");
    let mut source_bytes = PHYSICAL_FONT_BYTES.to_vec();
    if append_byte {
        source_bytes.push(0);
    }
    let mut database = Database::new();
    database.load_font_source(Source::SharedFile(path, Arc::new(source_bytes.clone())));
    Ok((temp, database, source_bytes))
}

impl TempDirectory {
    pub(super) fn new() -> Result<Self, String> {
        Self::new_in(&std::env::temp_dir())
    }

    fn new_in(parent: &Path) -> Result<Self, String> {
        tempfile::Builder::new()
            .prefix("krr-chrome-font-identity-")
            .tempdir_in(parent)
            .map(Self)
            .map_err(|error| format!("create temp directory: {error}"))
    }

    pub(super) fn path(&self) -> &Path {
        self.0.path()
    }
}

pub(super) fn run_probe_child(
    request: &ProbeRequest,
    response_path: &Path,
    test_name: &str,
) -> Result<ProbeResponse, String> {
    let request_path = response_path.with_extension("request.json");
    if response_path.exists() || request_path.exists() {
        return Err("probe request and response paths must be fresh".to_string());
    }
    let request_bytes =
        serde_json::to_vec(request).map_err(|error| format!("serialize request: {error}"))?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&request_path)
        .and_then(|mut request_file| request_file.write_all(&request_bytes))
        .map_err(|error| format!("write request: {error}"))?;
    let output = run_ignored_test(&request_path, response_path, test_name)?;
    if !output.status.success() {
        return Err(format!(
            "ignored font probe failed with status {:?}",
            output.status.code()
        ));
    }
    read_response(response_path)
}

pub(super) fn request_sha256(request: &ProbeRequest) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(request).map_err(|error| format!("serialize request: {error}"))?;
    Ok(super::identity::sha256(&bytes))
}

pub(super) fn probe_single_capture(
    condition_id: &str,
    selected: SelectedFont,
    resources: Vec<ResourceFile>,
) -> Result<CaptureResult, String> {
    let request = ProbeRequest {
        schema: REQUEST_SCHEMA.to_string(),
        captures: vec![CaptureRequest {
            condition_id: condition_id.to_string(),
            selected_fonts: vec![selected],
            resource_files: resources,
        }],
    };
    let temp = TempDirectory::new()?;
    let response = run_probe_child(&request, &temp.path().join("response.json"), PROBE_TEST)?;
    verified_single_capture(&request, response, condition_id)
}

fn verified_single_capture(
    request: &ProbeRequest,
    response: ProbeResponse,
    condition_id: &str,
) -> Result<CaptureResult, String> {
    if response.schema != RESPONSE_SCHEMA || response.request_sha256 != request_sha256(request)? {
        return Err("font probe response schema or request binding differs".to_string());
    }
    let [capture]: [CaptureResult; 1] = response
        .captures
        .try_into()
        .map_err(|_| "font probe response must contain one capture".to_string())?;
    if capture.condition_id != condition_id {
        return Err("font probe response condition differs".to_string());
    }
    Ok(capture)
}

pub(super) fn run_ignored_test(
    request_path: &Path,
    response_path: &Path,
    test_name: &str,
) -> Result<Output, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("resolve current test executable: {error}"))?;
    ProcessService::create_command(executable)
        .args(["--ignored", "--exact", test_name, "--nocapture"])
        .env("KRR_CHROME_FONT_IDENTITY_REQUEST", request_path)
        .env("KRR_CHROME_FONT_IDENTITY_OUTPUT", response_path)
        .output()
        .map_err(|error| format!("run ignored font probe: {error}"))
}

fn read_response(response_path: &Path) -> Result<ProbeResponse, String> {
    let bytes = fs::read(response_path).map_err(|error| format!("read response: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse response: {error}"))
}

#[cfg(test)]
#[path = "test_support_contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
#[path = "test_support_path_tests.rs"]
mod path_tests;
