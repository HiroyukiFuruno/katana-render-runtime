use std::fs;
use std::path::{Path, PathBuf};

use super::archive_urls::{is_remote_url, replace_non_alphanumeric};
use super::checksum::{ArtifactChecksum, verify_pinned_source, write_checksum};
use super::install_archive::copy_archive;
use super::prebuilt_artifact::static_checksum_path;
use super::sha256::sha256_hex_of_file;

#[path = "downloaders.rs"]
mod downloaders;

pub(super) fn download_file(
  url: &str,
  filename: &Path,
  cache_key: Option<&str>,
  expected_archive_sha256: Option<&str>,
) -> Result<(), String> {
  if !is_remote_url(url) {
    return copy_local_archive(url, filename, expected_archive_sha256);
  }
  if copy_cached_archive(url, filename, cache_key, expected_archive_sha256) {
    return Ok(());
  }
  let tmpfile = prepare_download_path(filename)?;
  downloaders::download(url, &tmpfile)?;
  let archive_sha256 = verify_download(&tmpfile, url, expected_archive_sha256)?;
  install_download(url, filename, &tmpfile, archive_sha256)
}

fn copy_local_archive(
  url: &str,
  filename: &Path,
  expected_archive_sha256: Option<&str>,
) -> Result<(), String> {
  verify_pinned_source(Path::new(url), expected_archive_sha256)?;
  copy_archive(url, filename)?;
  /* Local copies are not recorded in the checksum file; remove any stale */
  /* record of a previous http(s) download so it cannot mask this copy. */
  let _ = fs::remove_file(static_checksum_path(filename));
  Ok(())
}

fn copy_cached_archive(
  url: &str,
  filename: &Path,
  cache_key: Option<&str>,
  expected_archive_sha256: Option<&str>,
) -> bool {
  let Ok(cargo_home) = home::cargo_home() else {
    return false;
  };
  let cache_dir = cargo_home.join(".rusty_v8");
  let cache_paths = cached_archive_paths(&cache_dir, url, cache_key);
  for path in cache_paths {
    if copy_cached_path(&path, filename, expected_archive_sha256) {
      return true;
    }
  }
  false
}

fn cached_archive_paths(
  cache_dir: &Path,
  url: &str,
  cache_key: Option<&str>,
) -> Vec<PathBuf> {
  let mut paths = Vec::new();
  if let Some(key) = cache_key {
    paths.push(cache_dir.join(replace_non_alphanumeric(key)));
  }
  /* Keep recognizing the escaped URL key so existing Cargo caches remain usable. */
  paths.push(cache_dir.join(replace_non_alphanumeric(url)));
  paths
}

fn copy_cached_path(
  path: &Path,
  filename: &Path,
  expected_archive_sha256: Option<&str>,
) -> bool {
  println!("Looking for download in '{path:?}'");
  if !path.exists() {
    return false;
  }
  if let Err(error) = verify_pinned_source(path, expected_archive_sha256) {
    /* A bad cache entry should not disable this URL; fall through and */
    /* download it. */
    println!("Ignoring cached {path:?}: {error}");
    return false;
  }
  match copy_archive(&path.to_string_lossy(), filename) {
    Ok(()) => {
      /* Local copies are not recorded in the checksum file; remove any */
      /* stale record of a previous http(s) download. */
      let _ = fs::remove_file(static_checksum_path(filename));
      true
    }
    Err(error) => {
      println!("Failed to copy {path:?}: {error}");
      false
    }
  }
}

fn prepare_download_path(filename: &Path) -> Result<PathBuf, String> {
  let tmpfile = filename.with_extension("tmp");
  if tmpfile.exists() {
    println!("Deleting old tmpfile {}", tmpfile.display());
    fs::remove_file(&tmpfile).map_err(|error| {
      format!("failed to delete {}: {error}", tmpfile.display())
    })?;
  }
  Ok(tmpfile)
}

fn verify_download(
  tmpfile: &Path,
  url: &str,
  expected_archive_sha256: Option<&str>,
) -> Result<String, String> {
  if !tmpfile.exists() {
    return Err("downloader reported success but produced no file".to_string());
  }
  /* Hash the downloaded bytes (before decompression) and verify them against */
  /* a pinned hash, if any, before the artifact is put in place. */
  let archive_sha256 = match sha256_hex_of_file(tmpfile) {
    Ok(hash) => hash,
    Err(error) => {
      let _ = fs::remove_file(tmpfile);
      return Err(error);
    }
  };
  if let Some(expected) = expected_archive_sha256
    && !archive_sha256.eq_ignore_ascii_case(expected)
  {
    let _ = fs::remove_file(tmpfile);
    return Err(format!(
      "SHA-256 mismatch for {url}: expected {expected}, got {archive_sha256}"
    ));
  }
  Ok(archive_sha256)
}

fn install_download(
  url: &str,
  filename: &Path,
  tmpfile: &Path,
  archive_sha256: String,
) -> Result<(), String> {
  /* Move file & record the source URL and content hashes. */
  if let Err(error) = copy_archive(&tmpfile.to_string_lossy(), filename) {
    let _ = fs::remove_file(tmpfile);
    return Err(error);
  }
  fs::remove_file(tmpfile).map_err(|error| {
    format!("failed to delete {}: {error}", tmpfile.display())
  })?;
  write_checksum(
    &static_checksum_path(filename),
    &ArtifactChecksum {
      url: url.to_string(),
      archive_sha256: Some(archive_sha256),
      sha256: sha256_hex_of_file(filename).ok(),
    },
  )
  .map_err(|error| error.to_string())?;
  assert!(filename.exists());
  assert!(static_checksum_path(filename).exists());
  assert!(!tmpfile.exists());
  Ok(())
}
