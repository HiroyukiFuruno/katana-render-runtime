use super::sha256::sha256_hex_of_file;
use std::env;
use std::fs;
use std::io;
use std::path::Path;

use super::BuildResult;

const SHA256_HEX_LENGTH: usize = 64;

/// What the `.sum` file next to a downloaded artifact records. Older builds
/// stored the bare source URL; current builds additionally record the
/// SHA-256 of the fetched archive bytes (as served, before decompression)
/// and of the artifact as written to disk, so a cached artifact can be
/// verified instead of trusted on the URL string alone.
pub(super) struct ArtifactChecksum {
  pub(super) url: String,
  pub(super) archive_sha256: Option<String>,
  pub(super) sha256: Option<String>,
}

pub(super) fn read_checksum(path: &Path) -> Option<ArtifactChecksum> {
  let content = fs::read_to_string(path).ok()?;
  if !content.starts_with("url ") {
    /* Legacy format: the file holds nothing but the source URL. */
    return Some(ArtifactChecksum {
      url: content,
      archive_sha256: None,
      sha256: None,
    });
  }
  let mut url = None;
  let mut archive_sha256 = None;
  let mut sha256 = None;
  for line in content.lines() {
    match parse_checksum_field(line) {
      Some(("url", value)) => url = Some(value.to_string()),
      Some(("archive-sha256", value)) => {
        archive_sha256 = Some(value.to_string())
      }
      Some(("sha256", value)) => sha256 = Some(value.to_string()),
      _ => {}
    }
  }
  Some(ArtifactChecksum {
    url: url?,
    archive_sha256,
    sha256,
  })
}

pub(super) fn parse_checksum_field(line: &str) -> Option<(&str, &str)> {
  ["url", "archive-sha256", "sha256"]
    .into_iter()
    .find_map(|name| {
      line
        .strip_prefix(name)
        .and_then(|v| v.strip_prefix(' '))
        .map(|v| (name, v))
    })
}

pub(super) fn write_checksum(
  path: &Path,
  checksum: &ArtifactChecksum,
) -> io::Result<()> {
  let mut content = format!("url {}\n", checksum.url);
  if let Some(hash) = &checksum.archive_sha256 {
    content.push_str(&format!("archive-sha256 {hash}\n"));
  }
  if let Some(hash) = &checksum.sha256 {
    content.push_str(&format!("sha256 {hash}\n"));
  }
  fs::write(path, content).map_err(|error| {
    io::Error::new(
      error.kind(),
      format!("failed to write {}: {error}", path.display()),
    )
  })
}

/// The `RUSTY_V8_ARCHIVE_SHA256` pin for the static lib archive, if set.
/// The value is the SHA-256 of the archive bytes as fetched or copied,
/// before decompression -- i.e. what `sha256sum` reports on the `.gz`
/// release asset, or on the plain file when the source is not gzipped.
/// A set-but-empty variable counts as unset, like the boolean toggles.
pub(super) fn pinned_archive_sha256() -> BuildResult<Option<String>> {
  let Ok(value) = env::var("RUSTY_V8_ARCHIVE_SHA256") else {
    return Ok(None);
  };
  if value.is_empty() {
    return Ok(None);
  }
  if value.len() != SHA256_HEX_LENGTH
    || !value.bytes().all(|b| b.is_ascii_hexdigit())
  {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      format!(
        "RUSTY_V8_ARCHIVE_SHA256 must be 64 hexadecimal characters, got {value:?}"
      ),
    )
    .into());
  }
  Ok(Some(value))
}

pub(super) fn krr_release_archive_sha256(target: &str) -> Option<&'static str> {
  match target {
    "aarch64-apple-darwin" => {
      Some("a4a5484444ad4b5f10dc7bed141236dd9dbeabf2006d14520f19b935d4db4227")
    }
    "x86_64-unknown-linux-gnu" => {
      Some("b6683e9afcb77fbd8cb2c3acf45d34d52029ab216b332522c95179c12f0fed3c")
    }
    "x86_64-pc-windows-msvc" => {
      Some("0b17ca072bae37dd4ff00e6014d2b413becb031c9342ee11cb8226a5881f62b2")
    }
    _ => None,
  }
}

pub(super) fn krr_release_binding_sha256(target: &str) -> Option<&'static str> {
  match target {
    "aarch64-apple-darwin" => {
      Some("b9d5652ebca515eb2bf4c2ee4a6551b2567117b5aa720f430d2032616dd7e3b5")
    }
    "x86_64-unknown-linux-gnu" => {
      Some("3e4d5fecdbe0f98a6017e69d7ca82519a77fb032c5eb5e4cdbecdd3e2ec519d5")
    }
    "x86_64-pc-windows-msvc" => {
      Some("4fbdc0fbb831ff7b26962f0273d77a66fbc3bbb596fbe9e9022c0128f8c88099")
    }
    _ => None,
  }
}

/// With a hash pinned, check a local source archive against it before use.
pub(super) fn verify_pinned_source(
  path: &Path,
  expected_sha256: Option<&str>,
) -> Result<(), String> {
  let Some(expected) = expected_sha256 else {
    return Ok(());
  };
  let actual = sha256_hex_of_file(path)?;
  if actual.eq_ignore_ascii_case(expected) {
    Ok(())
  } else {
    Err(format!(
      "SHA-256 mismatch for {}: expected {expected}, got {actual}",
      path.display()
    ))
  }
}

/// Decide whether an artifact left behind by a previous build can be reused,
/// by checking its recorded SHA-256 against the file on disk. Returns false
/// to force a re-download. Records made before hashes were kept (by older
/// versions of this script) also force a re-download: adopting the on-disk
/// bytes as truth would grandfather in pre-existing corruption forever,
/// while one fresh download records a hash actually tied to the source.
pub(super) fn verify_recorded_artifact(
  filename: &Path,
  recorded: &ArtifactChecksum,
  expected_archive_sha256: Option<&str>,
) -> bool {
  /* A pinned archive hash can only be checked against a recorded one; */
  /* records without one force a re-download so the pin is actually enforced. */
  if let Some(expected) = expected_archive_sha256 {
    match &recorded.archive_sha256 {
      Some(actual) if actual.eq_ignore_ascii_case(expected) => {}
      _ => return false,
    }
  }
  verify_recorded_file_hash(filename, recorded.sha256.as_deref())
}

pub(super) fn verify_recorded_file_hash(
  filename: &Path,
  recorded_hash: Option<&str>,
) -> bool {
  let Some(recorded_hash) = recorded_hash else {
    println!(
      "No SHA-256 recorded for {}; re-fetching",
      filename.display()
    );
    return false;
  };
  let actual = sha256_hex_of_file(filename).unwrap_or_default();
  if actual.eq_ignore_ascii_case(recorded_hash) {
    return true;
  }
  println!(
    "SHA-256 of {} does not match the recorded value \
     (expected {recorded_hash}, got {actual}); re-fetching",
    filename.display()
  );
  false
}
