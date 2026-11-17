use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::archive_urls::{
  BindingSource, artifact_cache_key, artifact_url_candidates,
  resolve_binding_source,
};
use super::artifact_download::try_download_artifact;
use super::checksum::{krr_release_binding_sha256, verify_pinned_source};
use super::env_bool;
use super::prebuilt_artifact::{
  build_dir, prebuilt_features_suffix, prebuilt_profile,
};
use super::BuildResult;

pub(super) fn print_prebuilt_src_binding_path() -> BuildResult<()> {
  let source = resolve_binding_source(
    env::var("RUSTY_V8_SRC_BINDING_PATH").ok(),
    env::var("RUSTY_V8_SRC_BINDING_URL").ok(),
  );
  if let BindingSource::Path(binding) = &source {
    println!("cargo:rustc-env=RUSTY_V8_SRC_BINDING_PATH={binding}");
    return Ok(());
  }
  let binding = PrebuiltBinding::new(&source)?;
  binding.refresh_if_needed()?;
  println!(
    "cargo:rustc-env=RUSTY_V8_SRC_BINDING_PATH={}",
    binding.path.display()
  );
  Ok(())
}

struct PrebuiltBinding<'a> {
  source: &'a BindingSource,
  path: PathBuf,
  name: String,
  expected_sha256: Option<&'static str>,
  archive_dir: Option<String>,
}

impl<'a> PrebuiltBinding<'a> {
  fn new(source: &'a BindingSource) -> BuildResult<Self> {
    let target = env::var("TARGET")?;
    let profile = prebuilt_profile()?;
    let features = prebuilt_features_suffix();
    let name = format!("src_binding{features}_{profile}_{target}.rs");
    let path = build_dir()?.join("gn_out").join("src-binding").join(&name);
    let expected_sha256 = (profile == "release" && features.is_empty())
      .then(|| krr_release_binding_sha256(&target))
      .flatten();
    /* RUSTY_V8_ARCHIVE is authoritative and must not silently reach network. */
    let archive_dir = env::var("RUSTY_V8_ARCHIVE")
      .ok()
      .filter(|archive| Path::new(archive).is_dir());
    Ok(Self {
      source,
      path,
      name,
      expected_sha256,
      archive_dir,
    })
  }

  fn refresh_if_needed(&self) -> BuildResult<()> {
    if !self.needs_refresh() {
      return Ok(());
    }
    self.create_parent_directory()?;
    let (urls, cache_key) = self.download_candidates()?;
    let result = try_download_artifact(
      &urls,
      &self.path,
      cache_key.as_deref(),
      self.expected_sha256,
    );
    if let Err(error) = result {
      self.handle_refresh_failure(&error)?;
    }
    Ok(())
  }

  fn needs_refresh(&self) -> bool {
    !matches!(self.source, BindingSource::Candidates)
      || env::var("RUSTY_V8_MIRROR").is_ok()
      || self.archive_dir.is_some()
      || !self.path.exists()
      || self.expected_sha256.is_some()
        && verify_pinned_source(&self.path, self.expected_sha256).is_err()
  }

  fn create_parent_directory(&self) -> BuildResult<()> {
    if let Some(parent) = self.path.parent() {
      fs::create_dir_all(parent).map_err(|error| {
        std::io::Error::new(
          error.kind(),
          format!("failed to create {}: {error}", parent.display()),
        )
      })?;
    }
    Ok(())
  }

  fn download_candidates(&self) -> BuildResult<(Vec<String>, Option<String>)> {
    Ok(match (self.source, &self.archive_dir) {
      (BindingSource::Url(url), _) => (vec![url.clone()], None),
      (_, Some(archive)) => (
        vec![
          Path::new(archive)
            .join(&self.name)
            .to_string_lossy()
            .into_owned(),
        ],
        None,
      ),
      _ => (
        artifact_url_candidates(&self.name)?,
        Some(artifact_cache_key(&self.name)),
      ),
    })
  }

  fn handle_refresh_failure(&self, error: &str) -> BuildResult<()> {
    /* Preserve a verified stale binding when downloads are skipped or the */
    /* configured archive directory lacks its binding. */
    let can_use_existing = (env_bool("RUSTY_V8_SKIP_DOWNLOAD")
      || self.archive_dir.is_some())
      && self.path.exists()
      && verify_pinned_source(&self.path, self.expected_sha256).is_ok();
    if can_use_existing {
      println!(
        "cargo:warning=could not refresh the src binding, using the \
         existing {} as-is (it may be stale)",
        self.path.display()
      );
      return Ok(());
    }
    Err(std::io::Error::other(error.to_owned()).into())
  }
}
