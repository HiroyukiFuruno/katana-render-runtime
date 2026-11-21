use super::super::{
    CaptureRequest, ProbeRequest, ProbeResponse, REQUEST_SCHEMA, RESPONSE_SCHEMA, ResourceFile,
};
use super::{
    TempDirectory, physical_font_fixture, probe_single_capture, request_sha256, run_probe_child,
    verified_single_capture,
};
use std::{ffi::OsString, fs};

const CONDITION: &str = "non-utf8-path";

#[test]
fn probe_command_preserves_executable_resolution_failure() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let unresolved = temp.path().join("missing-probe").canonicalize();
    let expected = unresolved
        .as_ref()
        .err()
        .ok_or("missing probe executable resolved")?;
    assert_eq!(expected.kind(), std::io::ErrorKind::NotFound);
    let expected = format!("resolve current test executable: {expected}");
    assert_eq!(
        super::probe_command_for_executable(unresolved, super::PROBE_TEST).err(),
        Some(expected)
    );
    Ok(())
}

#[test]
fn temporary_probe_directory_rejects_a_file_as_parent() -> Result<(), String> {
    let parent = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    let error = TempDirectory::new_in(parent.path())
        .err()
        .ok_or("temporary probe directory accepted a file as parent")?;
    assert!(error.starts_with("create temp directory:"));
    assert!(parent.path().is_file());
    Ok(())
}

#[test]
fn physical_font_fixture_rejects_a_directory_and_an_unparseable_font() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let path = temp.path().join("malformed.ttf");
    let malformed = b"invalid sfnt";
    let expected_io = fs::write(temp.path(), malformed)
        .err()
        .ok_or("directory write unexpectedly succeeded")?;
    assert_eq!(
        super::write_physical_font(temp.path(), malformed).err(),
        Some(expected_io.to_string())
    );
    assert_eq!(
        super::write_physical_font(&path, malformed)
            .err()
            .as_deref(),
        Some("fontdb did not load the font")
    );
    assert_eq!(
        fs::read(path).map_err(|error| error.to_string())?,
        malformed
    );
    Ok(())
}

#[test]
fn physical_font_load_reports_a_missing_file_without_creating_it() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let missing = temp.path().join("missing.ttf");
    let expected = fs::File::open(&missing)
        .err()
        .ok_or("missing font file opened")?;
    assert_eq!(expected.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        super::load_physical_font(&missing).err(),
        Some(expected.to_string())
    );
    assert!(!missing.exists());
    Ok(())
}

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

#[test]
fn probe_child_rejects_a_successful_process_that_selected_no_test() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let resource_path = temp.path().join("captured-font.body");
    fs::write(&resource_path, b"font response").map_err(|error| error.to_string())?;
    let request = request_with_resource(resource_path);
    let output = temp.path().join("missing-response.json");
    let error = run_probe_child(&request, &output, "krr::no_such_probe_test")
        .err()
        .ok_or("a child with no matching test was accepted")?;
    assert!(error.starts_with("read response:"));
    assert!(!output.exists());
    let written =
        fs::read(output.with_extension("request.json")).map_err(|error| error.to_string())?;
    let expected = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    assert_eq!(written, expected);
    Ok(())
}

#[test]
fn probe_child_reports_a_process_launch_error_without_a_response()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDirectory::new()?;
    let resource_path = temp.path().join("captured-font.body");
    fs::write(&resource_path, b"font response")?;
    let request = request_with_resource(resource_path);
    let output = temp.path().join("response.json");
    let error = run_probe_child(&request, &output, "invalid\0test-name")
        .err()
        .ok_or("the process accepted an argument containing NUL")?;
    assert!(error.starts_with("run ignored font probe:"), "{error}");
    assert_eq!(
        fs::read(output.with_extension("request.json"))?,
        serde_json::to_vec(&request)?
    );
    assert!(!output.exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn probe_child_does_not_write_through_a_dangling_request_link() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let output = temp.path().join("response.json");
    let request_path = output.with_extension("request.json");
    let target = temp.path().join("uncreated-request.json");
    std::os::unix::fs::symlink(&target, &request_path).map_err(|error| error.to_string())?;

    let request = request_with_resource(temp.path().join("font.body"));
    let error = run_probe_child(&request, &output, super::PROBE_TEST)
        .err()
        .ok_or("probe accepted a dangling request link")?;
    assert!(error.starts_with("write request:"));
    assert!(!target.exists());
    assert!(!output.exists());
    assert_eq!(
        fs::read_link(request_path).map_err(|error| error.to_string())?,
        target
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn probe_child_does_not_write_through_a_dangling_response_link()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDirectory::new()?;
    let resource = temp.path().join("captured-font.body");
    fs::write(&resource, b"font response")?;
    let request = request_with_resource(resource);
    let output = temp.path().join("response.json");
    let target = temp.path().join("uncreated-response.json");
    std::os::unix::fs::symlink(&target, &output)?;
    let error = run_probe_child(&request, &output, super::PROBE_TEST)
        .err()
        .ok_or("probe accepted a dangling response link")?;
    assert!(error.starts_with("ignored font probe failed with status Some("));
    assert!(output.with_extension("request.json").is_file());
    assert!(!target.exists());
    assert_eq!(fs::read_link(output)?, target);
    Ok(())
}

#[test]
fn single_capture_propagates_a_real_probe_child_validation_failure() -> Result<(), String> {
    let (_temp, _database, selected) = physical_font_fixture()?;
    let error = probe_single_capture("", selected, Vec::new())
        .err()
        .ok_or("empty condition ID was accepted")?;
    assert!(error.starts_with("ignored font probe failed with status Some("));
    Ok(())
}
