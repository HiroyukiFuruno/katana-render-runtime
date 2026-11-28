use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

const HASH_READ_BUFFER_BYTES: usize = 64 * 1024;

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
  bytes
    .as_ref()
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

#[cfg(test)]
pub(super) fn sha256_hex(data: &[u8]) -> String {
  hex_digest(Sha256::digest(data))
}

pub(super) fn sha256_hex_of_file(path: &Path) -> Result<String, String> {
  let mut file = fs::File::open(path)
    .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
  let mut hasher = Sha256::new();
  let mut buffer = [0_u8; HASH_READ_BUFFER_BYTES];
  loop {
    let bytes_read = file
      .read(&mut buffer)
      .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if bytes_read == 0 {
      break;
    }
    hasher.update(&buffer[..bytes_read]);
  }
  Ok(hex_digest(hasher.finalize()))
}
