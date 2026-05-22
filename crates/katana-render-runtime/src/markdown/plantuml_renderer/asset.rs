use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const PLANTUML_JAR_VERSION: &str = "1.2026.4";
pub const PLANTUML_JAR_CHECKSUM: &str =
    "1783d4569855f2f0a17e65bd192add377c7f2b5e3e1781b65dc94d084de98699";
pub const PLANTUML_DOWNLOAD_URL: &str = "https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/1.2026.4/plantuml-lgpl-1.2026.4.jar";

const DOWNLOAD_LIMIT_BYTES: u64 = 32 * 1024 * 1024;
const HEX_HIGH_NIBBLE_SHIFT: u8 = 4;
const HEX_LOW_NIBBLE_MASK: u8 = 0x0f;

pub(crate) struct PlantUmlJarAssetOps;

impl PlantUmlJarAssetOps {
    pub(crate) fn cache_path(cache_dir: Option<&Path>) -> PathBuf {
        Self::cache_root(cache_dir)
            .join(PLANTUML_JAR_VERSION)
            .join("plantuml.jar")
    }

    pub(crate) fn prepare_cache_jar(cache_dir: Option<&Path>) -> Result<PathBuf, String> {
        let path = Self::cache_path(cache_dir);
        if path.exists() {
            Self::verify_jar(&path)?;
            return Ok(path);
        }
        Self::download_to_cache(&path)?;
        Ok(path)
    }

    pub(crate) fn verify_jar(path: &Path) -> Result<(), String> {
        let digest = Self::digest_file(path)?;
        Self::verify_digest(&digest)
    }

    fn download_to_cache(path: &Path) -> Result<(), String> {
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
        let bytes = Self::download_jar()?;
        Self::verify_bytes(&bytes)?;
        let temp_path = Self::temp_path(path);
        std::fs::write(&temp_path, bytes).map_err(|error| {
            format!(
                "PlantUML cache file is not writable: {}: {error}",
                temp_path.display()
            )
        })?;
        Self::install_temp_file(&temp_path, path)
    }

    fn download_jar() -> Result<Vec<u8>, String> {
        let mut response = ureq::get(PLANTUML_DOWNLOAD_URL)
            .call()
            .map_err(Self::download_error)?;
        response
            .body_mut()
            .with_config()
            .limit(DOWNLOAD_LIMIT_BYTES)
            .read_to_vec()
            .map_err(Self::download_error)
    }

    fn download_error(error: ureq::Error) -> String {
        format!(
            "PlantUML JAR download failed from {PLANTUML_DOWNLOAD_URL}: {error}. network connection is required on first use when the cache is empty"
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

    fn cache_root(cache_dir: Option<&Path>) -> PathBuf {
        if let Some(path) = cache_dir {
            return path.to_path_buf();
        }
        if let Some(path) = Self::env_path("KDR_PLANTUML_CACHE_DIR") {
            return path;
        }
        Self::platform_cache_root()
    }

    #[cfg(target_os = "macos")]
    fn platform_cache_root() -> PathBuf {
        Self::home_dir()
            .map(|it| {
                it.join("Library")
                    .join("Caches")
                    .join("kdr")
                    .join("plantuml")
            })
            .unwrap_or_else(Self::temp_cache_root)
    }

    #[cfg(target_os = "windows")]
    fn platform_cache_root() -> PathBuf {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| Self::home_dir().map(|it| it.join("AppData").join("Local")))
            .map(|it| it.join("kdr").join("plantuml"))
            .unwrap_or_else(Self::temp_cache_root)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn platform_cache_root() -> PathBuf {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| Self::home_dir().map(|it| it.join(".cache")))
            .map(|it| it.join("kdr").join("plantuml"))
            .unwrap_or_else(Self::temp_cache_root)
    }

    fn temp_cache_root() -> PathBuf {
        std::env::temp_dir().join("kdr").join("plantuml")
    }

    fn env_path(name: &'static str) -> Option<PathBuf> {
        let value = std::env::var_os(name)?;
        (!value.is_empty()).then(|| PathBuf::from(value))
    }

    fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

#[cfg(test)]
#[path = "asset_tests.rs"]
mod tests;
