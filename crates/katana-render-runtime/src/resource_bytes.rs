use moka::sync::Cache;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::SystemTime,
};

const CACHE_BYTES: u64 = 16 * 1024 * 1024;
const ENTRY_OVERHEAD: usize = 512;
static CACHE: OnceLock<Cache<PathBuf, Arc<ResourceSnapshot>>> = OnceLock::new();

#[derive(Clone, PartialEq, Eq)]
struct Identity {
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    unix: (u64, u64, i64, i64),
}

impl Identity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            #[cfg(unix)]
            unix: (
                metadata.dev(),
                metadata.ino(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ),
        }
    }
}

pub(crate) struct ResourceSnapshot {
    content: ResourceContent,
    digest: OnceLock<String>,
    identity: Identity,
}

enum ResourceContent {
    Text(Arc<String>),
    Binary(Vec<u8>),
}

impl ResourceSnapshot {
    pub(crate) fn bytes(&self) -> &[u8] {
        match &self.content {
            ResourceContent::Text(text) => text.as_bytes(),
            ResourceContent::Binary(bytes) => bytes,
        }
    }
    pub(crate) fn digest(&self) -> &str {
        self.digest.get_or_init(|| {
            Sha256::digest(self.bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        })
    }
    #[cfg(test)]
    pub(crate) fn text(&self) -> std::io::Result<&str> {
        match &self.content {
            ResourceContent::Text(text) => Ok(text.as_str()),
            ResourceContent::Binary(_) => Err(Self::invalid_utf8()),
        }
    }

    pub(crate) fn text_handle(&self) -> std::io::Result<Arc<String>> {
        match &self.content {
            ResourceContent::Text(text) => Ok(Arc::clone(text)),
            ResourceContent::Binary(_) => Err(Self::invalid_utf8()),
        }
    }

    fn invalid_utf8() -> std::io::Error {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        )
    }
}

pub(crate) struct ResourceBytes;

impl ResourceBytes {
    pub(crate) fn snapshot(path: &Path) -> std::io::Result<Arc<ResourceSnapshot>> {
        let file = File::open(path)?;
        let before = Identity::from_metadata(&file.metadata()?);
        let cache = CACHE.get_or_init(Self::cache);
        if let Some(snapshot) = Self::cached(cache, path, &before) {
            return Ok(snapshot);
        }
        let snapshot = Self::read_snapshot(file, before)?;
        Self::remember(cache, path, &snapshot);
        Ok(snapshot)
    }

    fn cached(
        cache: &Cache<PathBuf, Arc<ResourceSnapshot>>,
        path: &Path,
        identity: &Identity,
    ) -> Option<Arc<ResourceSnapshot>> {
        identity.modified?;
        cache
            .get(path)
            .filter(|snapshot| snapshot.identity == *identity)
    }

    fn remember(
        cache: &Cache<PathBuf, Arc<ResourceSnapshot>>,
        path: &Path,
        snapshot: &Arc<ResourceSnapshot>,
    ) {
        let weight = snapshot
            .bytes()
            .len()
            .saturating_add(path.as_os_str().len())
            .saturating_add(ENTRY_OVERHEAD);
        if snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize {
            cache.insert(path.to_path_buf(), snapshot.clone());
            cache.run_pending_tasks();
        }
    }

    fn read_snapshot(mut file: File, before: Identity) -> std::io::Result<Arc<ResourceSnapshot>> {
        let bytes = Self::read_contents(&mut file)?;
        let after = Identity::from_metadata(&file.metadata()?);
        if before != after || before.len != bytes.len() as u64 {
            return Err(std::io::Error::other("resource changed while reading"));
        }
        let content = match String::from_utf8(bytes) {
            Ok(text) => ResourceContent::Text(Arc::new(text)),
            Err(error) => ResourceContent::Binary(error.into_bytes()),
        };
        Ok(Arc::new(ResourceSnapshot {
            content,
            digest: OnceLock::new(),
            identity: after,
        }))
    }

    fn cache() -> Cache<PathBuf, Arc<ResourceSnapshot>> {
        Cache::builder()
            .max_capacity(CACHE_BYTES)
            .weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {
                value
                    .bytes()
                    .len()
                    .saturating_add(path.as_os_str().len())
                    .saturating_add(ENTRY_OVERHEAD)
                    .try_into()
                    .unwrap_or(u32::MAX)
            })
            .build()
    }

    pub(crate) fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
        let mut file = File::open(path)?;
        Self::read_contents(&mut file)
    }

    fn read_contents(file: &mut File) -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    #[cfg(test)]
    pub(crate) fn read_text(path: &Path) -> std::io::Result<String> {
        Self::snapshot(path)?.text().map(str::to_owned)
    }
}

#[cfg(test)]
#[path = "resource_bytes_tests.rs"]
mod tests;
