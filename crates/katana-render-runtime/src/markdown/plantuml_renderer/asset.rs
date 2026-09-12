mod asset_path;

use asset_path::PlantUmlCachePathOps;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Mutex;

pub const PLANTUML_JAR_VERSION: &str = "1.2026.8";
pub const PLANTUML_JAR_CHECKSUM: &str =
    "1057dd8b346bed26a48ffebe6054e16fc785dda7c91f37f6c19030a4aab8a942";
pub const PLANTUML_DOWNLOAD_URL: &str = "https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/1.2026.8/plantuml-lgpl-1.2026.8.jar";

const DOWNLOAD_LIMIT_BYTES: u64 = 32 * 1024 * 1024;
const HEX_HIGH_NIBBLE_SHIFT: u8 = 4;
const HEX_LOW_NIBBLE_MASK: u8 = 0x0f;

type PlantUmlJarVerifier = fn(&[u8]) -> Result<(), String>;

#[cfg(test)]
pub(super) static PLANTUML_ENV_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct PlantUmlJarAssetOps;

impl PlantUmlJarAssetOps {
    pub(crate) fn cache_path(cache_dir: Option<&Path>) -> PathBuf {
        PlantUmlCachePathOps::cache_root(cache_dir)
            .join(PLANTUML_JAR_VERSION)
            .join("plantuml.jar")
    }

    pub(crate) fn prepare_cache_jar(cache_dir: Option<&Path>) -> Result<PathBuf, String> {
        Self::prepare_cache_jar_from(cache_dir, PLANTUML_DOWNLOAD_URL)
    }

    fn prepare_cache_jar_from(
        cache_dir: Option<&Path>,
        download_url: &str,
    ) -> Result<PathBuf, String> {
        let mut download = || Self::download_from_url(download_url);
        Self::prepare_cache_path(
            Self::cache_path(cache_dir),
            Self::verify_bytes,
            &mut download,
        )
    }

    fn prepare_cache_path(
        path: PathBuf,
        verify: PlantUmlJarVerifier,
        download: &mut dyn FnMut() -> Result<Vec<u8>, String>,
    ) -> Result<PathBuf, String> {
        if path.exists() {
            let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
            verify(&bytes)?;
            return Ok(path);
        }
        Self::download_to_cache_with(&path, verify, download)?;
        Ok(path)
    }

    pub(crate) fn verify_jar(path: &Path) -> Result<(), String> {
        let digest = Self::digest_file(path)?;
        Self::verify_digest(&digest)
    }

    fn download_to_cache_with(
        path: &Path,
        verify: PlantUmlJarVerifier,
        download: &mut dyn FnMut() -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        let parent = path.parent().ok_or_else(|| {
            format!(
                "PlantUML cache path has no parent directory: {}",
                path.display()
            )
        })?;
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "PlantUML cache directory is not writable: {}: {error}",
                parent.display()
            )
        })?;
        let bytes = download()?;
        verify(&bytes)?;
        let temp_path = Self::temp_path(path);
        std::fs::write(&temp_path, bytes).map_err(|error| {
            format!(
                "PlantUML cache file is not writable: {}: {error}",
                temp_path.display()
            )
        })?;
        Self::install_temp_file(&temp_path, path)
    }

    fn download_from_url(url: &str) -> Result<Vec<u8>, String> {
        let mut response = ureq::get(url)
            .call()
            .map_err(|error| Self::download_error(url, error))?;
        response
            .body_mut()
            .with_config()
            .limit(DOWNLOAD_LIMIT_BYTES)
            .read_to_vec()
            .map_err(|error| Self::download_error(url, error))
    }

    fn download_error(url: &str, error: ureq::Error) -> String {
        format!(
            "PlantUML JAR download failed from {url}: {error}. network connection is required on first use when the cache is empty"
        )
    }

    fn install_temp_file(temp_path: &Path, path: &Path) -> Result<(), String> {
        match std::fs::rename(temp_path, path) {
            Ok(()) => Ok(()),
            Err(_) if path.exists() => {
                let _ = std::fs::remove_file(temp_path);
                Self::verify_jar(path).map_err(|checksum_error| {
                    format!(
                        "PlantUML cache install raced and existing file is invalid: {checksum_error}"
                    )
                })
            }
            Err(error) => Err(format!(
                "PlantUML cache file could not be installed: {} -> {}: {error}",
                temp_path.display(),
                path.display()
            )),
        }
    }

    fn verify_bytes(bytes: &[u8]) -> Result<(), String> {
        Self::verify_digest(&Self::digest_bytes(bytes))
    }

    fn verify_digest(digest: &str) -> Result<(), String> {
        if digest == PLANTUML_JAR_CHECKSUM {
            return Ok(());
        }
        Err(format!(
            "plantuml.jar checksum mismatch: expected {PLANTUML_JAR_CHECKSUM}, actual {digest}"
        ))
    }

    fn digest_file(path: &Path) -> Result<String, String> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        Ok(Self::digest_bytes(&bytes))
    }

    fn digest_bytes(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        Self::hex_lower(&digest)
    }

    fn hex_lower(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let value = *byte;
            output.push(HEX[(value >> HEX_HIGH_NIBBLE_SHIFT) as usize] as char);
            output.push(HEX[(value & HEX_LOW_NIBBLE_MASK) as usize] as char);
        }
        output
    }

    fn temp_path(path: &Path) -> PathBuf {
        let file_name = path
            .file_name()
            .and_then(|it| it.to_str())
            .unwrap_or("plantuml.jar");
        path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()))
    }
}

#[cfg(test)]
#[path = "asset_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "asset_env_tests.rs"]
mod env_tests;

#[cfg(test)]
#[path = "asset_download_tests.rs"]
mod download_tests;
