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
#[path = "resource_audit_tests.rs"]
mod tests;
