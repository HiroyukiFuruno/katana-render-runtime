use super::{ResourceAuditScope, record_failure, record_success};
use crate::renderer::backends::html_subresources::tests::support::{
    LocalFixture, one_response_server,
};
use crate::renderer::backends::html_subresources::transport;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::system::file_size_limit_test_support::FileSizeLimitTest;
use std::path::PathBuf;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn resource_body_write_failure_is_reported() -> Result<(), Box<dyn std::error::Error>> {
    FileSizeLimitTest::run_in_child(
        module_path!(),
        "resource_body_write_failure_is_reported",
        assert_resource_write_limit_error,
    )
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn assert_resource_write_limit_error() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = LocalFixture::new()?;
    let source = fixture.root.join("source.bin");
    let output = fixture.root.join("bodies");
    std::fs::write(&source, b"body")?;
    let url = url::Url::from_file_path(&source).map_err(|()| "invalid fixture path")?;
    let scope = ResourceAuditScope::start();
    transport::load_text(&url)?;
    let error = FileSizeLimitTest::with_zero_limit(|| scope.finish(&output))?
        .err()
        .ok_or("resource body write unexpectedly succeeded")?;
    let expected = FileSizeLimitTest::limit_error();
    assert_eq!(
        error,
        format!("could not persist resource body: {expected}")
    );
    let body = output.join("resource-0000.bin");
    assert_eq!(std::fs::metadata(body)?.len(), 0);
    Ok(())
}

#[test]
fn existing_resource_body_is_rejected_without_replacement() -> Result<(), String> {
    let fixture = LocalFixture::new()?;
    let path = fixture.root.join("existing-body.bin");
    let original = b"original body";
    std::fs::write(&path, original).map_err(|error| error.to_string())?;

    let error = super::persist_body(&path, b"replacement")
        .err()
        .ok_or("existing resource body was accepted")?;
    assert!(error.contains("could not create fresh resource body"));
    assert_eq!(
        std::fs::read(&path)
            .map_err(|error| error.to_string())?
            .as_slice(),
        original.as_slice()
    );
    Ok(())
}

#[test]
fn file_load_persists_the_actual_bytes_and_url() -> Result<(), String> {
    let fixture = LocalFixture::new()?;
    let source = fixture.root.join("source.bin");
    let output = fixture.root.join("bodies");
    let bytes = b"actual file body\0";
    std::fs::write(&source, bytes).map_err(|error| error.to_string())?;
    let url = url::Url::from_file_path(&source).map_err(|()| "invalid fixture path")?;
    let scope = ResourceAuditScope::start();

    assert_eq!(transport::load_text(&url)?.as_bytes(), bytes);
    let inventory = scope.finish(&output)?;

    assert!(inventory.complete);
    assert_eq!(inventory.requests.as_array().map(Vec::len), Some(1));
    let row = &inventory.requests[0];
    assert_eq!(row["url"], url.as_str());
    assert_eq!(row["loaded"], true);
    let body_path = PathBuf::from(row["bodyPath"].as_str().ok_or("body path missing")?);
    assert_eq!(
        std::fs::read(body_path).map_err(|error| error.to_string())?,
        bytes
    );
    assert_eq!(
        row["bytesSha256"],
        "16cbaea3379939638f3865c2098a1c0ad95631e1b774c4e9667dd9f2f91e0c3a"
    );
    Ok(())
}

#[test]
fn http_image_body_is_observed_from_the_actual_response() -> Result<(), String> {
    let fixture = LocalFixture::new()?;
    let output = fixture.root.join("bodies");
    let bytes = b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
    let (address, server) = one_response_server("image/svg+xml", bytes)?;
    let url = url::Url::parse(&address).map_err(|error| error.to_string())?;
    let scope = ResourceAuditScope::start();

    let loaded = super::super::transport::load_image_data_url(&url)?;
    assert!(loaded.starts_with("data:image/svg+xml;base64,"));
    let inventory = scope.finish(&output)?;

    server
        .join()
        .map_err(|_| "HTTP fixture server panicked")?
        .map_err(|error| error.to_string())?;
    assert!(inventory.complete);
    assert_eq!(inventory.requests.as_array().map(Vec::len), Some(1));
    let row = &inventory.requests[0];
    assert_eq!(row["url"], url.as_str());
    let body_path = PathBuf::from(row["bodyPath"].as_str().ok_or("body path missing")?);
    assert_eq!(
        std::fs::read(body_path).map_err(|error| error.to_string())?,
        bytes
    );
    assert_eq!(row["bytesSha256"], super::sha256(bytes));
    Ok(())
}

#[test]
fn failed_load_and_policy_failure_leave_inventory_incomplete() -> Result<(), String> {
    let fixture = LocalFixture::new()?;
    let missing = fixture.root.join("missing.bin");
    let url = url::Url::from_file_path(missing).map_err(|()| "invalid fixture path")?;
    let scope = ResourceAuditScope::start();
    assert!(transport::load_text(&url).is_err());
    let source = crate::renderer::backends::HtmlBrowserSource::new(
        "<p>fixture</p>",
        "https://example.test/index.html",
    )
    .map_err(|error| error.to_string())?;
    let loader = crate::renderer::backends::html_subresources::HtmlSubresourceLoader::new(&source);
    assert!(loader.load_text("ftp://example.test/blocked.txt").is_err());
    let inventory = scope.finish(&fixture.root.join("bodies"))?;
    assert!(!inventory.complete);
    assert!(inventory.reason.contains("could not be read"));
    assert!(inventory.reason.contains("unsupported resource scheme"));
    Ok(())
}

#[test]
fn nested_scope_restores_outer_capture_and_fresh_write_failure_is_reported() -> Result<(), String> {
    let fixture = LocalFixture::new()?;
    let outer = ResourceAuditScope::start();
    record_success("data:text/plain,outer", b"outer");
    {
        let inner = ResourceAuditScope::start();
        record_success("data:text/plain,inner", b"inner");
        drop(inner);
    }
    record_success("data:text/plain,after", b"after");
    let inventory = outer.finish(&fixture.root.join("outer-bodies"))?;
    assert_eq!(inventory.requests.as_array().map(Vec::len), Some(2));

    let collision = fixture.root.join("collision");
    std::fs::create_dir(&collision).map_err(|error| error.to_string())?;
    let scope = ResourceAuditScope::start();
    record_success("data:text/plain,body", b"body");
    assert!(scope.finish(&collision).is_err());
    Ok(())
}

#[test]
fn inactive_capture_fails_closed_when_reported() -> Result<(), String> {
    let fixture = LocalFixture::new()?;
    let scope = ResourceAuditScope::start();
    super::ACTIVE_AUDIT.with(|active| {
        drop(active.replace(None));
    });
    let inventory = scope.finish(&fixture.root.join("bodies"))?;
    assert!(!inventory.complete);
    assert_eq!(
        inventory.reason,
        "resource audit scope was not active at report time"
    );
    assert_eq!(inventory.requests.as_array().map(Vec::len), Some(0));
    Ok(())
}

#[test]
fn relative_body_directory_is_rejected_before_writing() -> Result<(), String> {
    let scope = ResourceAuditScope::start();
    let result = scope.finish(std::path::Path::new("relative-resource-audit-bodies"));
    assert_eq!(
        result.err().as_deref(),
        Some("resource body directory must be an absolute path")
    );
    Ok(())
}

#[test]
fn failure_overflow_cannot_produce_a_complete_inventory() -> Result<(), String> {
    let fixture = LocalFixture::new()?;
    let scope = ResourceAuditScope::start();
    for index in 0..=super::MAX_AUDITED_REQUESTS {
        record_failure(&format!("fixture:{index}"), "load failed");
    }
    let inventory = scope.finish(&fixture.root.join("bodies"))?;
    assert!(!inventory.complete);
    assert!(inventory.reason.contains("bounded capture capacity"));
    assert_eq!(inventory.requests.as_array().map(Vec::len), Some(0));
    Ok(())
}

#[test]
fn request_overflow_cannot_produce_a_complete_inventory() -> Result<(), String> {
    let fixture = LocalFixture::new()?;
    let scope = ResourceAuditScope::start();
    for index in 0..=super::MAX_AUDITED_REQUESTS {
        record_success(&format!("data:text/plain,{index}"), b"");
    }
    let inventory = scope.finish(&fixture.root.join("bodies"))?;
    assert!(!inventory.complete);
    assert!(inventory.reason.contains("bounded capture capacity"));
    Ok(())
}
