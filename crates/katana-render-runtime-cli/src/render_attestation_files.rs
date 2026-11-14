use anyhow::{Context, ensure};
use katana_render_runtime::RenderInput;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const HASH_READ_BUFFER_SIZE: usize = 65_536;
const HEX_NIBBLE_BITS: u32 = 4;
const HEX_NIBBLE_MASK: u8 = 15;

fn validate_existing(path: &Path) -> anyhow::Result<()> {
    ensure!(
        path.is_absolute(),
        "attestation path must be absolute: {}",
        path.display()
    );
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "attestation requires regular files: {}",
        path.display()
    );
    ensure!(
        fs::canonicalize(path)? == path,
        "attestation path must be canonical: {}",
        path.display()
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FileRecord {
    pub(super) path: PathBuf,
    pub(super) sha256: String,
    pub(super) size: u64,
}

impl FileRecord {
    pub(super) fn read(path: &Path) -> anyhow::Result<Self> {
        validate_existing(path)?;
        let mut file = File::open(path)?;
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; HASH_READ_BUFFER_SIZE];
        let mut size = 0_u64;
        loop {
            let length = file.read(&mut buffer)?;
            if length == 0 {
                break;
            }
            digest.update(&buffer[..length]);
            size += u64::try_from(length)?;
        }
        Ok(Self {
            path: path.to_path_buf(),
            sha256: hex(&digest.finalize()),
            size,
        })
    }

    pub(super) fn value(&self) -> Value {
        json!({"path": self.path, "sha256": self.sha256, "size": self.size})
    }
}

pub(super) fn validate_destination(path: &Path) -> anyhow::Result<()> {
    ensure!(
        path.is_absolute(),
        "attestation destination must be absolute: {}",
        path.display()
    );
    let parent = path
        .parent()
        .context("attestation destination has no parent")?;
    ensure!(
        fs::canonicalize(parent)? == parent,
        "attestation destination parent must be canonical"
    );
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => anyhow::bail!("attestation destination already exists: {}", path.display()),
    }
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut file = File::create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub(super) fn create_artifacts(receipt: &Path, raw_source: &[u8]) -> anyhow::Result<PathBuf> {
    let artifacts = receipt.with_extension("artifacts");
    fs::create_dir(&artifacts).with_context(|| {
        format!(
            "attestation artifacts must be fresh: {}",
            artifacts.display()
        )
    })?;
    write_new(&artifacts.join("raw-input.bin"), raw_source)?;
    Ok(artifacts)
}

pub(super) fn input_snapshots(
    artifacts: &Path,
    input: &RenderInput,
    raw_sha256: &str,
) -> anyhow::Result<Value> {
    let input_bytes = serde_json::to_vec(input)?;
    let source = FileRecord::read(&artifacts.join("request-source.mmd"))?;
    let request = FileRecord::read(&artifacts.join("render-input.json"))?;
    let raw = FileRecord::read(&artifacts.join("raw-input.bin"))?;
    ensure!(
        source.sha256 == sha256(input.source.as_bytes())
            && request.sha256 == sha256(&input_bytes)
            && raw.sha256 == raw_sha256,
        "attestation input snapshots changed"
    );
    Ok(json!({"rawInput":raw.value(),"renderSource":source.value(),"renderInput":request.value()}))
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> HEX_NIBBLE_BITS)]));
        result.push(char::from(DIGITS[usize::from(byte & HEX_NIBBLE_MASK)]));
    }
    result
}
