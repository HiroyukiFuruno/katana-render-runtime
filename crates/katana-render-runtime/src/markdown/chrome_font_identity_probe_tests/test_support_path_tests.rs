use super::super::{
    CaptureRequest, ProbeRequest, ProbeResponse, REQUEST_SCHEMA, RESPONSE_SCHEMA, ResourceFile,
};
use super::{TempDirectory, request_sha256, run_probe_child, verified_single_capture};
use std::ffi::OsString;

const CONDITION: &str = "non-utf8-path";

#[cfg(unix)]
fn non_utf8_name() -> OsString {
    use std::os::unix::ffi::OsStringExt;
    OsString::from_vec(b"font-\xff.ttf".to_vec())
}

#[cfg(windows)]
fn non_utf8_name() -> OsString {
    use std::os::windows::ffi::OsStringExt;
    const NAME: [u16; 6] = [0x0066, 0xd800, 0x002e, 0x0074, 0x0074, 0x0066];
    OsString::from_wide(&NAME)
}

fn request_with_resource(resource_path: std::path::PathBuf) -> ProbeRequest {
    ProbeRequest {
        schema: REQUEST_SCHEMA.to_owned(),
        captures: vec![CaptureRequest {
            condition_id: CONDITION.to_owned(),
            selected_fonts: Vec::new(),
            resource_files: vec![ResourceFile {
                url: "https://example.test/font".to_owned(),
                body_path: resource_path,
                bytes_sha256: super::super::identity::sha256(b"font response"),
            }],
        }],
    }
}

#[test]
fn non_utf8_resource_path_rejects_request_json_without_starting_probe() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let resource_path = temp.path().join(non_utf8_name());
    assert!(resource_path.to_str().is_none());
    let request = request_with_resource(resource_path);
    assert!(request_sha256(&request).is_err_and(|error| error.starts_with("serialize request:")));
    let response_path = temp.path().join("response.json");
    let error = run_probe_child(&request, &response_path, super::PROBE_TEST)
        .err()
        .ok_or("non-UTF-8 path was serialized")?;
    assert!(error.starts_with("serialize request:"));
    assert!(!response_path.exists());
    assert!(!response_path.with_extension("request.json").exists());
    let response = ProbeResponse {
        schema: RESPONSE_SCHEMA.to_owned(),
        request_sha256: String::new(),
        inventory_method: String::new(),
        inventory_limit: String::new(),
        captures: Vec::new(),
    };
    assert!(
        verified_single_capture(&request, response, CONDITION)
            .err()
            .is_some_and(|error| error.starts_with("serialize request:"))
    );
    Ok(())
}
