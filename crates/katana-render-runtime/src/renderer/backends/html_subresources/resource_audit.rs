use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

const MAX_AUDITED_REQUESTS: usize = 2_048;
const MAX_AUDITED_BYTES: usize = 128 * 1024 * 1024;

thread_local! {
    static ACTIVE_AUDIT: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

#[derive(Default)]
struct Capture {
    requests: Vec<Request>,
    failures: Vec<String>,
    bytes: usize,
    overflowed: bool,
}

struct Request {
    url: String,
    bytes: Vec<u8>,
}

pub(in crate::renderer::backends) struct ResourceAuditScope {
    previous: Option<Capture>,
}

pub(in crate::renderer::backends) struct Inventory {
    pub(in crate::renderer::backends) complete: bool,
    pub(in crate::renderer::backends) requests: Value,
    pub(in crate::renderer::backends) reason: String,
}

impl ResourceAuditScope {
    pub(in crate::renderer::backends) fn start() -> Self {
        let previous = ACTIVE_AUDIT.with(|active| active.replace(Some(Capture::default())));
        Self { previous }
    }

    pub(in crate::renderer::backends) fn finish(
        self,
        body_directory: &Path,
    ) -> Result<Inventory, String> {
        let capture = ACTIVE_AUDIT.with(|active| active.replace(None));
        let Some(capture) = capture else {
            return Ok(Inventory {
                complete: false,
                requests: json!([]),
                reason: "resource audit scope was not active at report time".to_owned(),
            });
        };
        persist_capture(capture, body_directory)
    }
}

impl Drop for ResourceAuditScope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        ACTIVE_AUDIT.with(|active| {
            active.replace(previous);
        });
    }
}

pub(super) fn record_success(url: &str, bytes: &[u8]) {
    ACTIVE_AUDIT.with(|active| {
        let mut active = active.borrow_mut();
        let Some(capture) = active.as_mut() else {
            return;
        };
        if capture.requests.len() >= MAX_AUDITED_REQUESTS
            || capture.bytes.saturating_add(bytes.len()) > MAX_AUDITED_BYTES
        {
            capture.overflowed = true;
            return;
        }
        capture.bytes += bytes.len();
        capture.requests.push(Request {
            url: url.to_owned(),
            bytes: bytes.to_vec(),
        });
    });
}

pub(super) fn record_failure(url: &str, reason: &str) {
    ACTIVE_AUDIT.with(|active| {
        if let Some(capture) = active.borrow_mut().as_mut() {
            if capture.failures.len() >= MAX_AUDITED_REQUESTS {
                capture.overflowed = true;
            } else {
                capture.failures.push(format!("{url}: {reason}"));
            }
        }
    });
}

fn persist_capture(capture: Capture, body_directory: &Path) -> Result<Inventory, String> {
    if !body_directory.is_absolute() {
        return Err("resource body directory must be an absolute path".to_owned());
    }
    let complete = capture.failures.is_empty() && !capture.overflowed;
    let reason = capture_reason(&capture);
    fs::create_dir(body_directory)
        .map_err(|error| format!("could not create fresh resource body directory: {error}"))?;
    let mut requests = Vec::with_capacity(capture.requests.len());
    for (index, request) in capture.requests.into_iter().enumerate() {
        let body_path = body_directory.join(format!("resource-{index:04}.bin"));
        persist_body(&body_path, &request.bytes)?;
        requests.push(json!({
            "url": request.url,
            "bodyPath": body_path,
            "bytesSha256": sha256(&request.bytes),
            "loaded": true
        }));
    }
    Ok(Inventory {
        complete,
        requests: Value::Array(requests),
        reason,
    })
}

fn persist_body(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("could not create fresh resource body: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("could not persist resource body: {error}"))
}

fn capture_reason(capture: &Capture) -> String {
    let mut reasons = capture.failures.clone();
    if capture.overflowed {
        reasons.push("resource audit exceeded its bounded capture capacity".to_owned());
    }
    if reasons.is_empty() {
        String::new()
    } else {
        reasons.join("; ")
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ResourceAuditScope, record_success};
    use crate::renderer::backends::html_subresources::tests::support::{
        LocalFixture, one_response_server,
    };
    use crate::renderer::backends::html_subresources::transport;
    use std::path::PathBuf;

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
        let loader =
            crate::renderer::backends::html_subresources::HtmlSubresourceLoader::new(&source);
        assert!(loader.load_text("ftp://example.test/blocked.txt").is_err());
        let inventory = scope.finish(&fixture.root.join("bodies"))?;
        assert!(!inventory.complete);
        assert!(inventory.reason.contains("could not be read"));
        assert!(inventory.reason.contains("unsupported resource scheme"));
        Ok(())
    }

    #[test]
    fn nested_scope_restores_outer_capture_and_fresh_write_failure_is_reported()
    -> Result<(), String> {
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
}
