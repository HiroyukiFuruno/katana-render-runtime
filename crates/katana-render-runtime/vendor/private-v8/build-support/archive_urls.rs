use std::env;
use std::path::Path;

use super::BuildResult;
use super::env_bool;
use super::prebuilt_artifact::{prebuilt_features_suffix, prebuilt_profile};

pub(super) const DEFAULT_ARTIFACT_BASE: &str =
  "https://github.com/denoland/rusty_v8/releases/download";

/// Values substituted into artifact URLs and URL templates.
pub(super) struct UrlVars<'a> {
  /// Release tag path segment, e.g. `v152.0.0` (see [`resolved_tag`]).
  pub(super) tag: &'a str,
  /// Raw `CARGO_PKG_VERSION`, without the `v` prefix.
  pub(super) version: &'a str,
  /// The `TARGET` triple.
  pub(super) target: &'a str,
  /// `release` or `debug`, from [`prebuilt_profile`].
  pub(super) profile: &'a str,
  /// Feature suffix from [`prebuilt_features_suffix`].
  pub(super) features: &'a str,
  /// The full artifact filename.
  pub(super) file: &'a str,
}

/// The tag path segment under which prebuilt artifacts are looked up.
/// `RUSTY_V8_MIRROR_TAG` overrides the default `v{version}` verbatim (no `v`
/// is prepended), so mirrors can serve artifacts from e.g. the last published
/// tag while `Cargo.toml` carries an unpublished version.
pub(super) fn resolved_tag(
  tag_override: Option<&str>,
  version: &str,
) -> String {
  match tag_override {
    Some(tag) => tag.to_string(),
    None => format!("v{version}"),
  }
}

/// Expand `{tag}`, `{version}`, `{target}`, `{profile}`, `{features}` and
/// `{file}` placeholders in a `RUSTY_V8_MIRROR` template.
pub(super) fn expand_url_template(template: &str, vars: &UrlVars) -> String {
  template
    .replace("{tag}", vars.tag)
    .replace("{version}", vars.version)
    .replace("{target}", vars.target)
    .replace("{profile}", vars.profile)
    .replace("{features}", vars.features)
    .replace("{file}", vars.file)
}

pub(super) fn is_remote_url(url: &str) -> bool {
  url.starts_with("http:") || url.starts_with("https:")
}

/// The ordered list of URLs an artifact is fetched from:
///
/// 1. The mirror, if `RUSTY_V8_MIRROR` is set. A value containing a `{`
///    placeholder is treated as a full URL template; otherwise it is a base
///    and the artifact is looked up at `{base}/{tag}/{file}`.
/// 2. For plain (non-template) filesystem mirrors only: the flat layout
///    `{base}/{file}`, so a directory of downloaded artifacts works as a
///    cache without tag subdirectories.
/// 3. The upstream GitHub release. When a mirror is configured this entry is
///    only present with `fallback` (`RUSTY_V8_MIRROR_FALLBACK=1`): a mirror
///    fails closed by default and never silently reaches the network.
pub(super) fn candidate_urls(
  mirror: Option<&str>,
  fallback: bool,
  vars: &UrlVars,
) -> Vec<String> {
  let mut urls = Vec::new();
  if let Some(mirror) = mirror {
    if mirror.contains('{') {
      urls.push(expand_url_template(mirror, vars));
    } else {
      urls.push(format!("{mirror}/{}/{}", vars.tag, vars.file));
      if !is_remote_url(mirror) {
        urls.push(format!("{mirror}/{}", vars.file));
      }
    }
  }
  if mirror.is_none() || fallback {
    urls.push(format!(
      "{DEFAULT_ARTIFACT_BASE}/{}/{}",
      vars.tag, vars.file
    ));
  }
  urls
}

/// [`candidate_urls`] with all inputs read from the environment.
pub(super) fn artifact_url_candidates(file: &str) -> BuildResult<Vec<String>> {
  let version = String::from("152.2.0");
  let tag_override = env::var("RUSTY_V8_MIRROR_TAG").ok();
  let tag = resolved_tag(tag_override.as_deref(), &version);
  let target = env::var("TARGET")?;
  let features = prebuilt_features_suffix();
  let profile = prebuilt_profile()?;
  let vars = UrlVars {
    tag: &tag,
    version: &version,
    target: &target,
    profile,
    features: &features,
    file,
  };
  let mirror = env::var("RUSTY_V8_MIRROR").ok();
  let fallback = env_bool("RUSTY_V8_MIRROR_FALLBACK");
  if mirror.is_some() && !fallback {
    println!(
      "RUSTY_V8_MIRROR is set; not falling back to {DEFAULT_ARTIFACT_BASE} \
       (set RUSTY_V8_MIRROR_FALLBACK=1 to enable)"
    );
  }
  Ok(candidate_urls(mirror.as_deref(), fallback, &vars))
}

/// Candidate locations for a `RUSTY_V8_ARCHIVE` value. A value naming an
/// existing directory is searched for the expected artifact filename
/// (gzipped first, then plain); anything else is used verbatim as a URL or
/// file path, exactly as before.
pub(super) fn archive_urls(
  archive: &str,
  gz_name: &str,
  plain_name: &str,
) -> Vec<String> {
  let path = Path::new(archive);
  if path.is_dir() {
    vec![
      path.join(gz_name).to_string_lossy().into_owned(),
      path.join(plain_name).to_string_lossy().into_owned(),
    ]
  } else {
    vec![archive.to_string()]
  }
}

/// How the prebuilt src binding file is obtained.
/// `RUSTY_V8_SRC_BINDING_PATH` names a local file that is used directly (no
/// download, no copy) and wins over `RUSTY_V8_SRC_BINDING_URL`, which
/// short-circuits the mirror/upstream candidates with exactly one URL or
/// path to fetch from -- mirroring what `RUSTY_V8_ARCHIVE` does for the
/// static lib. With neither set the usual candidate list is tried.
#[derive(Debug, PartialEq)]
pub(super) enum BindingSource {
  Path(String),
  Url(String),
  Candidates,
}

pub(super) fn resolve_binding_source(
  path: Option<String>,
  url: Option<String>,
) -> BindingSource {
  /* A set-but-empty URL counts as unset, like the boolean toggles. */
  let url = url.filter(|url| !url.is_empty());
  match (path, url) {
    (Some(path), _) => BindingSource::Path(path),
    (None, Some(url)) => BindingSource::Url(url),
    (None, None) => BindingSource::Candidates,
  }
}

/// Cache key under `~/.cargo/.rusty_v8`: `<tag>/<artifact filename>`,
/// escaped with [`replace_non_alphanumeric`] to form the file name. Keying
/// on the tag and filename (rather than the full source URL) lets the cache
/// survive switching mirrors without re-downloading identical bytes.
pub(super) fn artifact_cache_key(file: &str) -> String {
  let version = String::from("152.2.0");
  let tag =
    resolved_tag(env::var("RUSTY_V8_MIRROR_TAG").ok().as_deref(), &version);
  format!("{tag}/{file}")
}

pub(super) fn replace_non_alphanumeric(url: &str) -> String {
  url
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
    .collect()
}
