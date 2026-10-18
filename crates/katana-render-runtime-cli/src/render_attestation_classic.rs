use super::files::{FileRecord, sha256, write_new};
use anyhow::{Context, ensure};
use katana_render_runtime::RenderOutput;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[path = "render_attestation_classic_events.rs"]
mod events;
#[path = "render_attestation_classic_protocol.rs"]
mod protocol;
use events::CaptureSubscriber;
use protocol::CaptureState;

const TARGET: &str = "katana_render_runtime::mermaid_classic_capture";

pub(super) struct ClassicCapture {
    records: Vec<FileRecord>,
    value: Value,
}

impl ClassicCapture {
    pub(super) fn verify(&self) -> anyhow::Result<Value> {
        for record in &self.records {
            ensure!(
                FileRecord::read(&record.path)? == *record,
                "classic capture artifact changed"
            );
        }
        Ok(self.value.clone())
    }
}

pub(super) fn capture(
    artifacts: &Path,
    source: &str,
    render: impl FnOnce() -> anyhow::Result<RenderOutput>,
) -> anyhow::Result<(RenderOutput, ClassicCapture)> {
    let state = Arc::new(Mutex::new(CaptureState::new()));
    let subscriber = CaptureSubscriber {
        state: Arc::clone(&state),
    };
    let output = tracing::subscriber::with_default(subscriber, render)?;
    ensure!(
        Arc::strong_count(&state) == 1,
        "classic subscriber escaped render"
    );
    let mut guard = state
        .lock()
        .map_err(|_| anyhow::anyhow!("classic capture lock poisoned"))?;
    let state = std::mem::replace(&mut *guard, CaptureState::new());
    state.validate(source)?;
    Ok((output, write_capture(artifacts, state)?))
}

fn write_record(path: &Path, bytes: &[u8]) -> anyhow::Result<FileRecord> {
    write_new(path, bytes)?;
    let record = FileRecord::read(path)?;
    ensure!(
        record.sha256 == sha256(bytes) && record.size == u64::try_from(bytes.len())?,
        "classic written artifact differs from captured bytes"
    );
    Ok(record)
}

fn write_capture(artifacts: &Path, state: CaptureState) -> anyhow::Result<ClassicCapture> {
    let request = state.request.context("classic request missing")?;
    let request = write_record(&artifacts.join("classic-request.json"), request.as_bytes())?;
    let mut records = vec![request];
    let mut programs = Vec::new();
    for (index, script) in state.scripts.into_iter().enumerate() {
        let path = artifacts.join(format!("classic-script-{index:02}.js"));
        let record = write_record(&path, script.code.as_bytes())?;
        programs
            .push(json!({"index":index,"name":script.name,"completed":true,"file":record.value()}));
        records.push(record);
    }
    let manifest = json!({
        "schema":"katana-mermaid-classic-capture-v1","status":"evaluated",
        "eventTarget":TARGET,"scriptKind":"individual classic programs in one V8 realm",
        "request":records[0].value(),"programs":programs,
        "requiresExternalExitEvidence":true,"acceptedConsumerClosure":false
    });
    let record = write_record(
        &artifacts.join("classic-capture.json"),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    let value = json!({"manifest":record.value(),"capture":manifest});
    records.push(record);
    Ok(ClassicCapture { records, value })
}

#[cfg(test)]
#[path = "render_attestation_classic_tests.rs"]
mod tests;
