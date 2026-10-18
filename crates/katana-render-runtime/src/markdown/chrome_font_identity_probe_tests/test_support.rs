use super::{
    CaptureRequest, CaptureResult, ProbeRequest, ProbeResponse, REQUEST_SCHEMA, RESPONSE_SCHEMA,
    ResourceFile, SelectedFont,
};
use crate::system::ProcessService;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_TEMP_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(super) const PROBE_TEST: &str =
    "markdown::chrome_font_identity_probe_tests::chrome_font_identity_probe_cli";

pub(super) struct TempDirectory(PathBuf);

impl TempDirectory {
    pub(super) fn new() -> Result<Self, String> {
        let serial = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "krr-chrome-font-identity-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).map_err(|error| format!("create temp directory: {error}"))?;
        Ok(Self(path))
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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
    fs::write(&request_path, &request_bytes).map_err(|error| format!("write request: {error}"))?;
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
    if response.captures.len() != 1 {
        return Err("font probe response must contain one capture".to_string());
    }
    let capture = response
        .captures
        .into_iter()
        .next()
        .ok_or("font probe capture response is missing")?;
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
