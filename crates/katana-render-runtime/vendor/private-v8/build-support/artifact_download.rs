use super::checksum::{read_checksum, verify_recorded_artifact};
use super::download::download_file;
use super::env_bool;
use super::prebuilt_artifact::static_checksum_path;
use super::BuildResult;
use std::env;
use std::io;
use std::path::Path;

/// Fetch an artifact into `filename`, trying each candidate URL in order.
/// Returns the full list of attempted URLs and the escape hatches if every
/// candidate fails.
pub(super) fn download_artifact(
  urls: &[String],
  filename: &Path,
  cache_key: Option<&str>,
  expected_archive_sha256: Option<&str>,
) -> BuildResult<()> {
  try_download_artifact(urls, filename, cache_key, expected_archive_sha256)
    .map_err(io::Error::other)?;
  Ok(())
}

/// Like [`download_artifact`], but returns the diagnostic instead of
/// panicking, so callers can fall back on failure (e.g.
/// `RUSTY_V8_SKIP_DOWNLOAD` reusing an existing binding).
pub(super) fn try_download_artifact(
  urls: &[String],
  filename: &Path,
  cache_key: Option<&str>,
  expected_archive_sha256: Option<&str>,
) -> Result<(), String> {
  if can_reuse_artifact(filename, urls, expected_archive_sha256) {
    return Ok(());
  }

  match fetch_first_available(
    urls,
    filename,
    cache_key,
    expected_archive_sha256,
  ) {
    Ok(()) => Ok(()),
    Err(errors) => Err(artifact_download_error(filename, &errors)),
  }
}

pub(super) fn can_reuse_artifact(
  filename: &Path,
  urls: &[String],
  expected_archive_sha256: Option<&str>,
) -> bool {
  if !filename.exists() {
    return false;
  }
  let Some(recorded) = read_checksum(&static_checksum_path(filename)) else {
    return false;
  };
  urls.contains(&recorded.url)
    && verify_recorded_artifact(filename, &recorded, expected_archive_sha256)
}

pub(super) fn fetch_first_available(
  urls: &[String],
  filename: &Path,
  cache_key: Option<&str>,
  expected_archive_sha256: Option<&str>,
) -> Result<(), Vec<String>> {
  let mut errors = Vec::new();
  for url in urls {
    println!("Trying to fetch from {url}");
    match download_file(url, filename, cache_key, expected_archive_sha256) {
      Ok(()) => return Ok(()),
      Err(error) => {
        println!("Failed to fetch from {url}: {error}");
        errors.push(format!("- {url}: {error}"));
      }
    }
  }
  Err(errors)
}

pub(super) fn artifact_download_error(
  filename: &Path,
  errors: &[String],
) -> String {
  /* A configured mirror fails closed: mention the opt-in that would have */
  /* allowed falling back to the upstream release. */
  let fallback_hint = if env::var("RUSTY_V8_MIRROR").is_ok()
    && !env_bool("RUSTY_V8_MIRROR_FALLBACK")
  {
    " Set RUSTY_V8_MIRROR_FALLBACK=1 to fall back to the upstream GitHub \
     release when the mirror misses."
  } else {
    ""
  };
  format!(
    "Failed to fetch the V8 prebuilt artifact {}. Tried:\n{}\n\
     Source builds are not supported by the packaged V8 inventory. You can \
     point the build at an artifact via RUSTY_V8_ARCHIVE (static lib), a \
     binding via RUSTY_V8_SRC_BINDING_URL or RUSTY_V8_SRC_BINDING_PATH (src \
     binding only), a mirror via RUSTY_V8_MIRROR, or another release tag via \
     RUSTY_V8_MIRROR_TAG.{fallback_hint}",
    filename.display(),
    errors.join("\n")
  )
}
