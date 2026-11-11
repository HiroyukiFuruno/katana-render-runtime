use crate::markdown::runtime_assets::RuntimeAsset;
use crate::markdown::{DiagramBlock, DiagramResult};
use crate::resource_bytes::{ResourceBytes, ResourceSnapshot};
#[cfg(test)]
use std::borrow::Cow;
use std::{path::Path, sync::Arc};

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
        let source = if let Some(kind) = bundled {
            ResourceSource::Static(Self::read_bundled(kind)?)
        } else {
            let snapshot = ResourceBytes::snapshot(path)?;
            let text = snapshot.text_handle()?;
            ResourceSource::File { snapshot, text }
        };
        Ok(LoadedResource { source })
    }

    #[cfg(test)]
    pub(crate) fn read_file(path: &Path) -> std::io::Result<String> {
        crate::resource_bytes::ResourceBytes::read_text(path)
    }

    #[cfg(test)]
    pub(crate) fn read(
        path: &Path,
        bundled: Option<BundledResource>,
    ) -> std::io::Result<Cow<'static, str>> {
        match bundled {
            Some(kind) => Self::read_bundled(kind).map(Cow::Borrowed),
            None => Self::read_file(path).map(Cow::Owned),
        }
    }

    fn read_bundled(bundled: BundledResource) -> std::io::Result<&'static str> {
        let source = match bundled {
            BundledResource::Mermaid => RuntimeAsset::mermaid()
                .bytes()
                .map_err(std::io::Error::other)?,
            BundledResource::Drawio => RuntimeAsset::drawio()
                .bytes()
                .map_err(std::io::Error::other)?,
            BundledResource::MathJax => {
                return Ok(include_str!(
                    "diagram_runtime/generated/mathjax-runtime.min.js"
                ));
            }
        };
        std::str::from_utf8(source)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }
}

#[cfg(test)]
#[path = "runtime_resource_loader_tests.rs"]
mod tests;
