use super::super::html_browser::{HtmlBrowserOrigin, HtmlBrowserSource, HtmlBrowserViewport};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub(crate) const FORMAT_VERSION: u16 = 1;
pub(crate) const SHA256_HEX_LENGTH: usize = 64;
pub(crate) const MAX_ARTIFACT_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const RGBA_BYTES_PER_PIXEL: usize = 4;

#[derive(Debug, Error)]
pub enum HtmlArtifactError {
    #[error("HTML artifact persistence is disabled by the host")]
    Disabled,
    #[error("HTML artifact key is invalid")]
    InvalidKey,
    #[error("HTML artifact provenance is invalid: {0}")]
    InvalidProvenance(String),
    #[error("HTML artifact provenance does not match")]
    ProvenanceMismatch,
    #[error("HTML artifact resource and font fingerprints are required")]
    FingerprintMissing,
    #[error("HTML artifact exceeds the store capacity")]
    CapacityExceeded,
    #[error("HTML artifact exceeds the maximum encoded size")]
    ArtifactTooLarge,
    #[error("HTML artifact is corrupt")]
    Corrupt,
    #[error("HTML artifact format version is unsupported: {0}")]
    UnsupportedFormatVersion(u16),
    #[error("HTML artifact path is a symbolic link")]
    SymlinkPath,
    #[error("HTML artifact store is busy")]
    StoreBusy,
    #[error("HTML artifact store traversal limit exceeded")]
    StoreTraversalLimit,
    #[error("HTML artifact runtime failed: {0}")]
    Runtime(String),
    #[error("HTML artifact I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTML artifact serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlArtifactProvenance {
    pub source_sha256: String,
    pub origin: HtmlBrowserOrigin,
    pub runtime_version: String,
    pub viewport: HtmlBrowserViewport,
    pub render_config_fingerprint: String,
    pub resource_content_fingerprint: String,
    pub font_content_fingerprint: String,
}

impl HtmlArtifactProvenance {
    pub(crate) fn source_sha256(source: &HtmlBrowserSource) -> String {
        Sha256::digest(source.raw_html.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn for_source(
        source: &HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
        render: impl Into<String>,
        resources: impl Into<String>,
        fonts: impl Into<String>,
    ) -> Result<Self, HtmlArtifactError> {
        let result = Self {
            source_sha256: Self::source_sha256(source),
            origin: source.origin.clone(),
            runtime_version: env!("CARGO_PKG_VERSION").into(),
            viewport,
            render_config_fingerprint: render.into(),
            resource_content_fingerprint: resources.into(),
            font_content_fingerprint: fonts.into(),
        };
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), HtmlArtifactError> {
        self.viewport
            .validate()
            .map_err(|error| HtmlArtifactError::InvalidProvenance(error.to_string()))?;
        if self.runtime_version.trim().is_empty() {
            return Err(HtmlArtifactError::InvalidProvenance(
                "runtime version is empty".into(),
            ));
        }
        validate_fingerprint("source", &self.source_sha256)?;
        validate_fingerprint("render config", &self.render_config_fingerprint)?;
        validate_fingerprint("resource content", &self.resource_content_fingerprint)?;
        validate_fingerprint("font content", &self.font_content_fingerprint)
    }

    pub(crate) fn validate_current_runtime(&self) -> Result<(), HtmlArtifactError> {
        if self.runtime_version != env!("CARGO_PKG_VERSION") {
            return Err(HtmlArtifactError::InvalidProvenance(
                "runtime version does not match the current library".into(),
            ));
        }
        Ok(())
    }
}

fn validate_fingerprint(label: &str, value: &str) -> Result<(), HtmlArtifactError> {
    if value.len() != SHA256_HEX_LENGTH || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        if label == "resource content" || label == "font content" {
            return Err(HtmlArtifactError::FingerprintMissing);
        }
        return Err(HtmlArtifactError::InvalidProvenance(format!(
            "{label} fingerprint is not a SHA-256 hex string"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> Result<HtmlArtifactProvenance, Box<dyn std::error::Error>> {
        let origin = HtmlBrowserOrigin::parse("https://example.test/")?;
        Ok(HtmlArtifactProvenance {
            source_sha256: "a".repeat(SHA256_HEX_LENGTH),
            origin,
            runtime_version: env!("CARGO_PKG_VERSION").into(),
            viewport: HtmlBrowserViewport::new(1, 1, 1.0)?,
            render_config_fingerprint: "b".repeat(SHA256_HEX_LENGTH),
            resource_content_fingerprint: "c".repeat(SHA256_HEX_LENGTH),
            font_content_fingerprint: "d".repeat(SHA256_HEX_LENGTH),
        })
    }

    #[test]
    fn validation_rejects_empty_runtime_and_fingerprints() -> Result<(), Box<dyn std::error::Error>>
    {
        assert_invalid_runtime()?;
        assert_invalid_standard_fingerprint()?;
        assert_invalid_content_fingerprints()?;
        Ok(())
    }

    fn assert_invalid_runtime() -> Result<(), Box<dyn std::error::Error>> {
        let mut invalid = provenance()?;
        invalid.runtime_version.clear();
        assert!(matches!(
            invalid.validate(),
            Err(HtmlArtifactError::InvalidProvenance(_))
        ));

        Ok(())
    }

    fn assert_invalid_standard_fingerprint() -> Result<(), Box<dyn std::error::Error>> {
        let mut invalid = provenance()?;
        invalid.source_sha256 = "bad".into();
        assert!(matches!(
            invalid.validate(),
            Err(HtmlArtifactError::InvalidProvenance(_))
        ));
        invalid.source_sha256 = "a".repeat(SHA256_HEX_LENGTH);
        invalid.render_config_fingerprint = "bad".into();
        assert!(matches!(
            invalid.validate(),
            Err(HtmlArtifactError::InvalidProvenance(_))
        ));
        Ok(())
    }

    fn assert_invalid_content_fingerprints() -> Result<(), Box<dyn std::error::Error>> {
        let mut invalid = provenance()?;
        invalid.resource_content_fingerprint = "bad".into();
        assert!(matches!(
            invalid.validate(),
            Err(HtmlArtifactError::FingerprintMissing)
        ));
        invalid.resource_content_fingerprint = "c".repeat(SHA256_HEX_LENGTH);
        invalid.font_content_fingerprint = "bad".into();
        assert!(matches!(
            invalid.validate(),
            Err(HtmlArtifactError::FingerprintMissing)
        ));
        Ok(())
    }
}
