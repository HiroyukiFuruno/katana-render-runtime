use super::archive_urls::{
  archive_urls, artifact_cache_key, artifact_url_candidates,
};
use super::artifact_download::download_artifact;
use super::checksum::{krr_release_archive_sha256, pinned_archive_sha256};
use super::env_bool;
use super::BuildResult;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const OUT_DIR_TO_PROFILE_DIRECTORY_LEVELS: usize = 3;

pub(super) fn prebuilt_profile() -> BuildResult<&'static str> {
  let target_os = env::var("CARGO_CFG_TARGET_OS")?;
  /* Use v8 in release mode unless $V8_FORCE_DEBUG=true */
  /* Note: we always use the release build on windows. */
  if target_os != "windows" && env_bool("V8_FORCE_DEBUG") {
    Ok("debug")
  } else {
    Ok("release")
  }
}

pub(super) fn prebuilt_features_suffix() -> String {
  let mut features = String::new();
  if env::var("CARGO_FEATURE_V8_ENABLE_POINTER_COMPRESSION").is_ok() {
    features.push_str("_ptrcomp");
  }
  if env::var("CARGO_FEATURE_V8_ENABLE_SANDBOX").is_ok() {
    features.push_str("_sandbox");
  }
  features
}

pub(super) fn static_lib_name(suffix: &str) -> BuildResult<String> {
  let target_os = env::var("CARGO_CFG_TARGET_OS")?;
  if target_os == "windows" {
    Ok(format!("rusty_v8{suffix}.lib"))
  } else {
    Ok(format!("librusty_v8{suffix}.a"))
  }
}

pub(super) fn static_lib_path() -> BuildResult<PathBuf> {
  Ok(static_lib_dir()?.join(static_lib_name("")?))
}

pub(super) fn static_checksum_path(path: &Path) -> PathBuf {
  let mut path = path.to_path_buf();
  path.set_extension("sum");
  path
}

pub(super) fn static_lib_dir() -> BuildResult<PathBuf> {
  Ok(build_dir()?.join("gn_out").join("obj"))
}

pub(super) fn build_dir() -> BuildResult<PathBuf> {
  /* target/debug//build/rusty_v8-d9e5a424d4f96994/out/ */
  let out_dir = env::var_os("OUT_DIR").ok_or_else(|| {
    io::Error::other(
      "The 'OUT_DIR' environment is not set (it should be something like \
       'target/debug/rusty_v8-{hash}').",
    )
  })?;
  let out_dir_abs = env::current_dir()?.join(out_dir);

  /* This would be `target/debug` or `target/release` */
  out_dir_abs
    .ancestors()
    .nth(OUT_DIR_TO_PROFILE_DIRECTORY_LEVELS)
    .map(Path::to_path_buf)
    .ok_or_else(|| {
      io::Error::other(format!(
        "OUT_DIR path has fewer than three parent directories: {}",
        out_dir_abs.display()
      ))
      .into()
    })
}

pub(super) fn download_static_lib_binaries() -> BuildResult<()> {
  let dir = static_lib_dir()?;
  fs::create_dir_all(&dir).map_err(|error| {
    io::Error::new(
      error.kind(),
      format!("failed to create {}: {error}", dir.display()),
    )
  })?;
  println!("cargo:rustc-link-search={}", dir.display());
  if report_skipped_static_library()? {
    return Ok(());
  }

  let (gz_name, lib_name, expected_sha256) = static_archive_identity()?;
  let (urls, cache_key) =
    static_archive_download_candidates(&gz_name, &lib_name)?;
  println!("static lib URLs: {urls:?}");
  download_artifact(
    &urls,
    &static_lib_path()?,
    cache_key.as_deref(),
    expected_sha256.as_deref(),
  )?;
  Ok(())
}

pub(super) fn report_skipped_static_library() -> BuildResult<bool> {
  if !env_bool("RUSTY_V8_SKIP_DOWNLOAD") {
    return Ok(false);
  }
  /* A previously built library remains usable in skip-download mode. */
  let static_lib_path = static_lib_path()?;
  if static_lib_path.exists() {
    println!(
      "cargo:warning=RUSTY_V8_SKIP_DOWNLOAD is set; using the existing {} \
       as-is (it may be stale)",
      static_lib_path.display()
    );
    if pinned_archive_sha256()?.is_some() {
      println!(
        "cargo:warning=RUSTY_V8_ARCHIVE_SHA256 is not verified when \
         RUSTY_V8_SKIP_DOWNLOAD is set"
      );
    }
  } else {
    println!(
      "cargo:warning=RUSTY_V8_SKIP_DOWNLOAD is set; the V8 static library \
       was not downloaded. `cargo check` will work, but linking will fail \
       with 'could not find native static library `rusty_v8`' until this \
       is built again without RUSTY_V8_SKIP_DOWNLOAD"
    );
  }
  Ok(true)
}

pub(super) fn static_archive_identity(
) -> BuildResult<(String, String, Option<String>)> {
  let target = env::var("TARGET")?;
  let profile = prebuilt_profile()?;
  let features = prebuilt_features_suffix();
  let lib_name = static_lib_name(&format!("{features}_{profile}_{target}"))?;
  let gz_name = format!("{lib_name}.gz");
  let official_hash = (profile == "release" && features.is_empty())
    .then(|| krr_release_archive_sha256(&target))
    .flatten();
  let expected_sha256 = pinned_archive_sha256()?
    .or_else(|| official_hash.map(str::to_owned));
  Ok((gz_name, lib_name, expected_sha256))
}

pub(super) fn static_archive_download_candidates(
  gz_name: &str,
  lib_name: &str,
) -> BuildResult<(Vec<String>, Option<String>)> {
  /* RUSTY_V8_ARCHIVE short-circuits the mirror/upstream candidates entirely: */
  /* it points at exactly one archive, or at a directory to look up the */
  /* expected artifact filename in. The cargo-home cache is only keyed for */
  /* candidate URLs, not for explicit overrides. */
  match env::var("RUSTY_V8_ARCHIVE") {
    Ok(custom_archive) => {
      Ok((archive_urls(&custom_archive, gz_name, lib_name), None))
    }
    Err(_) => Ok((
        artifact_url_candidates(gz_name)?,
        Some(artifact_cache_key(gz_name)),
      )),
  }
}
