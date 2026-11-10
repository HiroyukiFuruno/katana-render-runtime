use super::{RESPONSE_SCHEMA, probe::probe};
use std::{env, fs, fs::OpenOptions, io::Write, path::PathBuf};

pub(super) fn run() -> Result<(), String> {
    let request_path = env::var_os("KRR_CHROME_FONT_IDENTITY_REQUEST")
        .ok_or_else(|| "KRR_CHROME_FONT_IDENTITY_REQUEST is required".to_string())?;
    let output_path = env::var_os("KRR_CHROME_FONT_IDENTITY_OUTPUT")
        .ok_or_else(|| "KRR_CHROME_FONT_IDENTITY_OUTPUT is required".to_string())?;
    let request_bytes = fs::read(request_path).map_err(|error| format!("read request: {error}"))?;
    let response = probe(&request_bytes)?;
    write_fresh_response(PathBuf::from(output_path), &response)
}

fn write_fresh_response(
    output_path: PathBuf,
    response: &super::ProbeResponse,
) -> Result<(), String> {
    if !output_path.is_absolute() {
        return Err("response path must be absolute".to_string());
    }
    if response.schema != RESPONSE_SCHEMA {
        return Err("response schema is inconsistent".to_string());
    }
    let response_bytes = serde_json::to_vec_pretty(response)
        .map_err(|error| format!("serialize response: {error}"))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)
        .map_err(|error| format!("create fresh response without overwriting: {error}"))?;
    output
        .write_all(&response_bytes)
        .map_err(|error| format!("write response: {error}"))
}

#[path = "cli_response_contract_tests.rs"]
mod response_contract_tests;
