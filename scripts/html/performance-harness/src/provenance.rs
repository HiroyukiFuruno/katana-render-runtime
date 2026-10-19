use sha2::{Digest, Sha256};
use std::{env, error::Error, fs};

pub(super) fn provenance() -> Result<serde_json::Value, Box<dyn Error>> {
    let required = |name: &str| -> Result<String, Box<dyn Error>> {
        Ok(env::var(name).map_err(|_| format!("{name} is required"))?)
    };
    let lock_path = required("KRR_PERF_LOCK_PATH")?;
    let source_snapshot_path = required("KRR_PERF_SOURCE_SNAPSHOT_PATH")?;
    Ok(serde_json::json!({
        "binary_sha256": hex_digest(&fs::read(env::current_exe()?)?),
        "rustc": required("KRR_PERF_RUSTC")?,
        "cargo": required("KRR_PERF_CARGO")?,
        "environment_sha256": required("KRR_PERF_ENVIRONMENT_SHA256")?,
        "deps_lock_path": lock_path,
        "deps_lock_sha256": hex_digest(&fs::read(&lock_path)?),
        "source_snapshot_path": source_snapshot_path,
        "source_snapshot_sha256": hex_digest(&fs::read(&source_snapshot_path)?),
    }))
}

pub(super) fn arm_name() -> &'static str {
    if cfg!(feature = "baseline") {
        "baseline-0.4.23"
    } else {
        "candidate"
    }
}

pub(super) fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
