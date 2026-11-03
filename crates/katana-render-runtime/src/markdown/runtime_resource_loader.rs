use crate::markdown::runtime_assets::RuntimeAsset;
use crate::markdown::{DiagramBlock, DiagramResult};
use crate::resource_bytes::{ResourceBytes, ResourceSnapshot};
use std::{borrow::Cow, path::Path, sync::Arc};

#[derive(Clone, Copy)]
pub(crate) enum BundledResource {
    Mermaid,
    Drawio,
    MathJax,
}

pub(crate) struct LoadedResource {
    source: ResourceSource,
}

enum ResourceSource {
    Static(&'static str),
    File {
        snapshot: Arc<ResourceSnapshot>,
        text: Arc<String>,
    },
}

impl LoadedResource {
    pub(crate) fn text(&self) -> &str {
        match &self.source {
            ResourceSource::Static(text) => text,
            ResourceSource::File { text, .. } => text.as_str(),
        }
    }

    pub(crate) fn digest(&self) -> Option<&str> {
        match &self.source {
            ResourceSource::Static(_) => None,
            ResourceSource::File { snapshot, .. } => Some(snapshot.digest()),
        }
    }

    pub(crate) fn into_digest(self) -> Option<String> {
        self.digest().map(str::to_owned)
    }
}

pub(crate) struct RuntimeResourceLoader;

impl RuntimeResourceLoader {
    pub(crate) fn diagram_error(
        block: &DiagramBlock,
        path: &Path,
        error: std::io::Error,
        kind: &str,
        download_url: &str,
        read_message: &str,
    ) -> DiagramResult {
        if error.kind() == std::io::ErrorKind::NotFound {
            return DiagramResult::NotInstalled {
                kind: kind.to_string(),
                download_url: download_url.to_string(),
                install_path: path.to_path_buf(),
            };
        }
        DiagramResult::Err {
            source: block.source.clone(),
            error: format!("{read_message}: {error}"),
        }
    }

    pub(crate) fn load(
        path: &Path,
        bundled: Option<BundledResource>,
    ) -> std::io::Result<LoadedResource> {
        let source = if bundled.is_some() {
            match Self::read(path, bundled)? {
                Cow::Borrowed(text) => ResourceSource::Static(text),
                Cow::Owned(_) => {
                    return Err(std::io::Error::other("bundled resource is not static"));
                }
            }
        } else {
            let snapshot = ResourceBytes::snapshot(path)?;
            let text = snapshot.text_handle()?;
            ResourceSource::File { snapshot, text }
        };
        Ok(LoadedResource { source })
    }

    pub(crate) fn read_file(path: &Path) -> std::io::Result<String> {
        crate::resource_bytes::ResourceBytes::read_text(path)
    }

    pub(crate) fn read(
        path: &Path,
        bundled: Option<BundledResource>,
    ) -> std::io::Result<Cow<'static, str>> {
        let source = match bundled {
            Some(BundledResource::Mermaid) => RuntimeAsset::mermaid()
                .bytes()
                .map_err(std::io::Error::other)?,
            Some(BundledResource::Drawio) => RuntimeAsset::drawio()
                .bytes()
                .map_err(std::io::Error::other)?,
            Some(BundledResource::MathJax) => {
                return Ok(Cow::Borrowed(include_str!(
                    "diagram_runtime/generated/mathjax-runtime.min.js"
                )));
            }
            None => return Self::read_file(path).map(Cow::Owned),
        };
        std::str::from_utf8(source)
            .map(Cow::Borrowed)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }
}

#[cfg(test)]
mod tests {
    use super::{BundledResource, RuntimeResourceLoader};

    #[test]
    fn bundled_resources_share_the_same_immutable_source() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("missing");
        for kind in [
            BundledResource::Mermaid,
            BundledResource::Drawio,
            BundledResource::MathJax,
        ] {
            let first = RuntimeResourceLoader::read(&path, Some(kind))?;
            let second = RuntimeResourceLoader::read(&path, Some(kind))?;
            assert_eq!(first.as_ptr(), second.as_ptr());
            assert_eq!(first, second);
            assert!(
                RuntimeResourceLoader::load(&path, Some(kind))?
                    .digest()
                    .is_none()
            );
            assert!(
                RuntimeResourceLoader::read(&path, None)
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            );
        }
        Ok(())
    }

    #[test]
    fn external_resources_reflect_replacement_removal_and_invalid_utf8() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        for extension in ["js", "svg", "xml"] {
            let path = dir.path().join(format!("resource.{extension}"));
            check_updates(&path, &dir.path().join("replacement"))?;
            check_read_failures(&path)?;
        }
        Ok(())
    }

    fn check_updates(path: &std::path::Path, replacement: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, "first")?;
        assert!(matches!(
            RuntimeResourceLoader::read_file(path).as_deref(),
            Ok("first")
        ));
        let first = RuntimeResourceLoader::load(path, None)?;
        let repeated = RuntimeResourceLoader::load(path, None)?;
        assert_eq!(first.digest(), repeated.digest());
        std::fs::write(path, "second")?;
        assert!(matches!(
            RuntimeResourceLoader::read_file(path).as_deref(),
            Ok("second")
        ));
        let changed = RuntimeResourceLoader::load(path, None)?;
        assert_ne!(first.digest(), changed.digest());
        std::fs::write(replacement, "replacement")?;
        #[cfg(windows)]
        std::fs::remove_file(path)?;
        std::fs::rename(replacement, path)?;
        assert!(matches!(
            RuntimeResourceLoader::read_file(path).as_deref(),
            Ok("replacement")
        ));
        check_parallel_reads(path);
        Ok(())
    }

    fn check_parallel_reads(path: &std::path::Path) {
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(move || {
                    assert!(matches!(
                        RuntimeResourceLoader::read_file(path).as_deref(),
                        Ok("replacement")
                    ));
                });
            }
        });
    }

    fn check_read_failures(path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, [0xff])?;
        assert!(
            RuntimeResourceLoader::read_file(path)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::InvalidData)
        );
        std::fs::remove_file(path)?;
        assert!(
            RuntimeResourceLoader::read_file(path)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        );
        Ok(())
    }
}
