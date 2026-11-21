/* Copyright 2018-2019 the Deno authors. All rights reserved. MIT license. */
use fslock::LockFile;
use std::env;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::Path;

#[path = "build-support/archive_urls.rs"]
mod archive_urls;
#[path = "build-support/artifact_download.rs"]
mod artifact_download;
#[path = "build-support/checksum.rs"]
mod checksum;
#[path = "build-support/download.rs"]
mod download;
#[path = "build-support/install_archive.rs"]
mod install_archive;
#[path = "build-support/link.rs"]
mod link;
#[path = "build-support/link_flags.rs"]
mod link_flags;
#[path = "build-support/prebuilt_artifact.rs"]
mod prebuilt_artifact;
#[path = "build-support/sha256.rs"]
mod sha256;
use link::print_prebuilt_src_binding_path;
use link_flags::print_link_flags;
use prebuilt_artifact::download_static_lib_binaries;

type BuildResult<T> = Result<T, Box<dyn std::error::Error>>;

const RERUN_ENVIRONMENT_KEYS: &[&str] = &[
  "CXXSTDLIB",
  "DENO_TRYBUILD",
  "DOCS_RS",
  "GN_ARGS",
  "OUT_DIR",
  "PYTHON",
  "RUSTY_V8_ARCHIVE",
  "RUSTY_V8_ARCHIVE_SHA256",
  "RUSTY_V8_MIRROR",
  "RUSTY_V8_MIRROR_TAG",
  "RUSTY_V8_MIRROR_FALLBACK",
  "RUSTY_V8_SKIP_DOWNLOAD",
  "RUSTY_V8_SRC_BINDING_PATH",
  "RUSTY_V8_SRC_BINDING_URL",
  "V8_FORCE_DEBUG",
  "V8_FROM_SOURCE",
];

pub(super) fn configure() -> BuildResult<()> {
  emit_rerun_directives();
  if env::var_os("DOCS_RS").is_some() {
    return print_prebuilt_src_binding_path();
  }
  print_link_flags()?;
  if env::var_os("DENO_TRYBUILD").is_some() {
    let binding = env::var("RUSTY_V8_SRC_BINDING_PATH")
      .map_err(|error| format!("RUSTY_V8_SRC_BINDING_PATH: {error}"))?;
    println!("cargo:rustc-env=RUSTY_V8_SRC_BINDING_PATH={binding}");
    return Ok(());
  }
  reject_source_build_request()?;
  configure_runtime()
}

fn reject_source_build_request() -> BuildResult<()> {
  if env_bool("V8_FROM_SOURCE") {
    return Err(
      "V8_FROM_SOURCE=1 is unsupported by katana-render-runtime: the package \
        does not include the V8 GN source tree and build tools. Remove this \
        variable to use the normal pinned prebuilt-artifact path."
        .into(),
    );
  }
  Ok(())
}

fn emit_rerun_directives() {
  println!("cargo:rerun-if-changed=vendor/private-v8/PROVENANCE.json");

  /* These are all the environment variables that we check. This is */
  /* probably more than what is needed, but missing an important */
  /* variable can lead to broken links when switching rusty_v8 */
  /* versions. */
  for key in rerun_environment_keys() {
    println!("cargo:rerun-if-env-changed={key}");
  }
}

fn rerun_environment_keys() -> &'static [&'static str] {
  RERUN_ENVIRONMENT_KEYS
}

fn configure_runtime() -> BuildResult<()> {
  /* Cargo likes to run multiple build scripts at once sometimes. */
  /* Nothing that follows is safe to run multiple times at once, */
  /* because we store everything in a parent directory of OUT_DIR. */
  let _lockfile = acquire_lock()?;
  print_prebuilt_src_binding_path()?;
  download_static_lib_binaries()
}

fn acquire_lock() -> BuildResult<LockFile> {
  let root = env::current_dir()?;
  let out_dir = env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?;
  let absolute = root.join(out_dir);
  let parent = absolute
    .ancestors()
    .nth(2)
    .ok_or_else(|| format!("OUT_DIR has no build directory: {}", absolute.display()))?;
  let path = parent.join("v8.fslock");
  let mut lockfile = LockFile::open(&path)
    .map_err(|error| format!("could not open V8 lock {}: {error}", path.display()))?;
  lockfile
    .lock_with_pid()
    .map_err(|error| format!("could not acquire V8 lock {}: {error}", path.display()))?;
  println!("lockfile: {path:?}");
  Ok(lockfile)
}

fn env_bool(key: &str) -> bool {
  matches!(
    env::var(key).unwrap_or_default().as_str(),
    "true" | "1" | "yes"
  )
}

#[cfg(test)]
mod test {
  use std::env;
  use std::fs;
  use std::path::Path;

  use super::archive_urls::{
    BindingSource, DEFAULT_ARTIFACT_BASE, UrlVars, archive_urls,
    candidate_urls, expand_url_template, replace_non_alphanumeric,
    resolve_binding_source, resolved_tag,
  };
  use super::checksum::{
    ArtifactChecksum, read_checksum, verify_pinned_source,
    verify_recorded_artifact, write_checksum,
  };
  use super::prebuilt_artifact::static_checksum_path;
  use super::sha256::{Sha256, sha256_hex};
  use super::BuildResult;

  const TEST_VARS: UrlVars = UrlVars {
    tag: "v139.0.0",
    version: "140.0.0",
    target: "x86_64-unknown-linux-gnu",
    profile: "release",
    features: "_ptrcomp",
    file: "librusty_v8_ptrcomp_release_x86_64-unknown-linux-gnu.a.gz",
  };

  #[test]
  fn test_resolved_tag() {
    assert_eq!(resolved_tag(None, "140.0.0"), "v140.0.0");
    /* Overrides are used verbatim: no `v` is prepended. */
    assert_eq!(resolved_tag(Some("nightly"), "140.0.0"), "nightly");
    assert_eq!(resolved_tag(Some("v139.0.0"), "140.0.0"), "v139.0.0");
  }

  #[test]
  fn test_expand_url_template() {
    assert_eq!(
      expand_url_template(
        "https://ex.com/{tag}/{version}/{target}/{profile}{features}/{file}",
        &TEST_VARS
      ),
      "https://ex.com/v139.0.0/140.0.0/x86_64-unknown-linux-gnu/\
       release_ptrcomp/librusty_v8_ptrcomp_release_x86_64-unknown-linux-gnu.a.gz"
    );
  }

  #[test]
  fn test_candidate_urls_no_mirror() {
    assert_eq!(
      candidate_urls(None, false, &TEST_VARS),
      vec![format!(
        "{DEFAULT_ARTIFACT_BASE}/v139.0.0/{}",
        TEST_VARS.file
      )]
    );
  }

  #[test]
  fn test_candidate_urls_http_mirror() {
    /* A remote base mirror keeps the `{base}/{tag}/{file}` layout and fails */
    /* closed: no flat layout for remote mirrors, no upstream fallback. */
    assert_eq!(
      candidate_urls(Some("https://mirror.example"), false, &TEST_VARS),
      vec![format!(
        "https://mirror.example/v139.0.0/{}",
        TEST_VARS.file
      )]
    );
  }

  #[test]
  fn test_candidate_urls_filesystem_mirror() {
    /* Filesystem mirrors additionally try the flat `{base}/{file}` layout. */
    assert_eq!(
      candidate_urls(Some("/opt/cache"), false, &TEST_VARS),
      vec![
        format!("/opt/cache/v139.0.0/{}", TEST_VARS.file),
        format!("/opt/cache/{}", TEST_VARS.file),
      ]
    );
  }

  #[test]
  fn test_candidate_urls_template_mirror() {
    /* A template mirror is used verbatim after substitution; no tag/flat */
    /* variants are derived from it. */
    assert_eq!(
      candidate_urls(
        Some("https://mirror.example/artifacts/{version}/{file}"),
        false,
        &TEST_VARS
      ),
      vec![format!(
        "https://mirror.example/artifacts/140.0.0/{}",
        TEST_VARS.file
      )]
    );
  }

  #[test]
  fn test_archive_urls_file() {
    /* A non-directory value is used verbatim, whether it exists or not. */
    assert_eq!(
      archive_urls("/path/to/custom_archive.a.gz", "lib.a.gz", "lib.a"),
      vec!["/path/to/custom_archive.a.gz".to_string()]
    );
    assert_eq!(
      archive_urls("https://ex.com/lib.a.gz", "lib.a.gz", "lib.a"),
      vec!["https://ex.com/lib.a.gz".to_string()]
    );
  }

  #[test]
  fn test_archive_urls_directory() {
    /* A directory is searched for the expected artifact filename, gzipped */
    /* first, then plain. */
    let dir = env::temp_dir()
      .join(format!("rusty_v8_archive_dir_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(
      archive_urls(dir.to_str().unwrap(), "lib.a.gz", "lib.a"),
      vec![
        dir.join("lib.a.gz").to_string_lossy().into_owned(),
        dir.join("lib.a").to_string_lossy().into_owned(),
      ]
    );
    fs::remove_dir_all(&dir).unwrap();
  }

  #[test]
  fn test_resolve_binding_source_precedence() {
    /* RUSTY_V8_SRC_BINDING_PATH wins over RUSTY_V8_SRC_BINDING_URL. */
    assert_eq!(
      resolve_binding_source(
        Some("/gen/binding.rs".into()),
        Some("https://ex.com/binding.rs".into())
      ),
      BindingSource::Path("/gen/binding.rs".into())
    );
    assert_eq!(
      resolve_binding_source(None, Some("https://ex.com/binding.rs".into())),
      BindingSource::Url("https://ex.com/binding.rs".into())
    );
    assert_eq!(
      resolve_binding_source(None, None),
      BindingSource::Candidates
    );
    /* A set-but-empty URL counts as unset. */
    assert_eq!(
      resolve_binding_source(None, Some(String::new())),
      BindingSource::Candidates
    );
  }

  #[test]
  fn test_cache_key_derivation() {
    /* The cargo-home cache file name is the escaped `<tag>/<file>` key, so */
    /* it does not depend on which mirror served the bytes. */
    assert_eq!(
      replace_non_alphanumeric(
        "v139.0.0/librusty_v8_release_aarch64-apple-darwin.a.gz"
      ),
      "v139_0_0_librusty_v8_release_aarch64_apple_darwin_a_gz"
    );
  }

  #[test]
  fn test_sha256_vectors() {
    /* FIPS 180-4 test vectors. */
    assert_eq!(
      sha256_hex(b""),
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
      sha256_hex(b"abc"),
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
      sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
      "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert_eq!(
      sha256_hex(&[b'a'; 1_000_000]),
      "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
  }

  #[test]
  fn test_sha256_streaming_updates() {
    /* Odd-sized updates must produce the same digest as a single one: this */
    /* exercises the partial-block carry logic across block boundaries. */
    let data = [b'a'; 1_000_000];
    for chunk_size in [1, 37, 63, 64, 65, 999] {
      let mut hasher = Sha256::new();
      for chunk in data.chunks(chunk_size) {
        hasher.update(chunk);
      }
      assert_eq!(
        hasher.finish_hex(),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        "chunk size {chunk_size}"
      );
    }
  }

  #[test]
  fn test_verify_pinned_source() {
    let path = env::temp_dir()
      .join(format!("rusty_v8_pinned_source_{}", std::process::id()));
    fs::write(&path, b"abc").unwrap();
    let good =
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    /* No pin: everything passes. Matching pin (any case): passes. */
    assert!(verify_pinned_source(&path, None).is_ok());
    assert!(verify_pinned_source(&path, Some(good)).is_ok());
    assert!(
      verify_pinned_source(&path, Some(good.to_uppercase().as_str())).is_ok()
    );
    /* Wrong pin: fails. */
    assert!(verify_pinned_source(&path, Some(&"0".repeat(64))).is_err());
    fs::remove_file(&path).unwrap();
  }

  #[test]
  fn test_checksum_read_write() -> BuildResult<()> {
    let path = env::temp_dir()
      .join(format!("rusty_v8_checksum_{}.sum", std::process::id()));

    /* Legacy format: the bare source URL, as written by older builds. */
    fs::write(&path, "https://ex.com/lib.a.gz")?;
    let legacy = read_checksum(&path).ok_or("legacy checksum is missing")?;
    assert_eq!(legacy.url, "https://ex.com/lib.a.gz");
    assert_eq!(legacy.archive_sha256, None);
    assert_eq!(legacy.sha256, None);

    /* Current format round-trips. */
    write_checksum(
      &path,
      &ArtifactChecksum {
        url: "https://ex.com/lib.a.gz".to_string(),
        archive_sha256: Some("aa".repeat(32)),
        sha256: Some("bb".repeat(32)),
      },
    )?;
    let current = read_checksum(&path).ok_or("written checksum is missing")?;
    assert_eq!(current.url, "https://ex.com/lib.a.gz");
    assert_eq!(current.archive_sha256, Some("aa".repeat(32)));
    assert_eq!(current.sha256, Some("bb".repeat(32)));

    fs::remove_file(&path)?;
    Ok(())
  }

  #[test]
  fn test_verify_recorded_artifact() {
    let path =
      env::temp_dir().join(format!("rusty_v8_recorded_{}", std::process::id()));
    fs::write(&path, b"abc").unwrap();
    let abc_hash =
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert_recorded_artifact_reuse_cases(&path, abc_hash);

    fs::remove_file(&path).unwrap();
    let _ = fs::remove_file(static_checksum_path(&path));
  }

  fn assert_recorded_artifact_reuse_cases(path: &Path, abc_hash: &str) {
    let record = |sha256: Option<String>| ArtifactChecksum {
      url: "https://ex.com/lib.a.gz".to_string(),
      archive_sha256: None,
      sha256,
    };
    assert!(verify_recorded_artifact(
      path,
      &record(Some(abc_hash.to_string())),
      None
    ));
    assert!(!verify_recorded_artifact(
      path,
      &record(Some("00".repeat(32))),
      None
    ));

    /* A legacy record without a hash forces a re-fetch: the on-disk bytes */
    /* are not adopted as truth, so pre-existing corruption cannot be */
    /* grandfathered in. */
    assert!(!verify_recorded_artifact(path, &record(None), None));

    /* A pinned archive hash cannot be checked against a record without one. */
    assert!(!verify_recorded_artifact(
      path,
      &record(Some(abc_hash.to_string())),
      Some(&"0".repeat(64))
    ));
  }

  #[test]
  fn test_candidate_urls_mirror_fallback() {
    /* RUSTY_V8_MIRROR_FALLBACK=1 appends the upstream release after the */
    /* mirror entries. */
    assert_eq!(
      candidate_urls(Some("/opt/cache"), true, &TEST_VARS),
      vec![
        format!("/opt/cache/v139.0.0/{}", TEST_VARS.file),
        format!("/opt/cache/{}", TEST_VARS.file),
        format!("{DEFAULT_ARTIFACT_BASE}/v139.0.0/{}", TEST_VARS.file),
      ]
    );
    /* Without a mirror the flag changes nothing: upstream is the only */
    /* candidate either way. */
    assert_eq!(
      candidate_urls(None, true, &TEST_VARS),
      candidate_urls(None, false, &TEST_VARS)
    );
  }
}
