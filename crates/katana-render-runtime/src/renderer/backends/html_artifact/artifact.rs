use super::super::html_browser::{HtmlBrowserFrame, HtmlBrowserPixelFormat};
use super::types::{
    FORMAT_VERSION, HtmlArtifactError, HtmlArtifactProvenance, RGBA_BYTES_PER_PIXEL,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const FRAME_DIGEST_DOMAIN: &[u8] = b"KRR-HTML-FRAME-SHA256\0v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlFrameArtifact {
    pub format_version: u16,
    pub provenance: HtmlArtifactProvenance,
    pub frame: HtmlBrowserFrame,
    pub frame_sha256: String,
}

impl HtmlFrameArtifact {
    pub fn new(
        provenance: HtmlArtifactProvenance,
        frame: HtmlBrowserFrame,
    ) -> Result<Self, HtmlArtifactError> {
        let artifact = Self {
            format_version: FORMAT_VERSION,
            provenance,
            frame_sha256: frame_digest(&frame),
            frame,
        };
        artifact.validate()?;
        Ok(artifact)
    }
    pub fn validate(&self) -> Result<(), HtmlArtifactError> {
        if self.format_version != FORMAT_VERSION {
            return Err(HtmlArtifactError::UnsupportedFormatVersion(
                self.format_version,
            ));
        }
        self.provenance.validate()?;
        if self.frame_sha256 != frame_digest(&self.frame) {
            return Err(HtmlArtifactError::Corrupt);
        }
        if self.frame.origin != self.provenance.origin
            || self.frame.viewport != self.provenance.viewport
        {
            return Err(HtmlArtifactError::ProvenanceMismatch);
        }
        self.validate_frame_metadata()?;
        Ok(())
    }

    fn validate_frame_metadata(&self) -> Result<(), HtmlArtifactError> {
        if self.frame.generation == 0
            || !self.frame.scroll_y.is_finite()
            || self.frame.scroll_y < 0.0
            || !self.frame.content_height.is_finite()
            || self.frame.content_height < 0.0
        {
            return Err(HtmlArtifactError::InvalidProvenance(
                "frame metadata is invalid".into(),
            ));
        }
        let expected = (self.frame.viewport.width as usize)
            .checked_mul(self.frame.viewport.height as usize)
            .and_then(|pixels| pixels.checked_mul(RGBA_BYTES_PER_PIXEL))
            .ok_or_else(|| {
                HtmlArtifactError::InvalidProvenance("frame dimensions overflow".into())
            })?;
        if self.frame.pixels.len() != expected {
            return Err(HtmlArtifactError::InvalidProvenance(
                "frame buffer size is invalid".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn encode_with_limit(&self, limit: u64) -> Result<Vec<u8>, HtmlArtifactError> {
        self.validate()?;
        super::encoding::to_vec_bounded(self, limit)
    }
    pub(crate) fn decode_with_limit(bytes: &[u8], limit: u64) -> Result<Self, HtmlArtifactError> {
        if bytes.len() as u64 > limit {
            return Err(HtmlArtifactError::ArtifactTooLarge);
        }
        let artifact: Self =
            serde_json::from_slice(bytes).map_err(|_| HtmlArtifactError::Corrupt)?;
        artifact.validate()?;
        Ok(artifact)
    }
}

fn frame_digest(frame: &HtmlBrowserFrame) -> String {
    let mut hasher = Sha256::new();
    hasher.update(FRAME_DIGEST_DOMAIN);
    hasher.update(frame.generation.to_le_bytes());
    hasher.update(frame.viewport.width.to_le_bytes());
    hasher.update(frame.viewport.height.to_le_bytes());
    hasher.update(frame.viewport.device_scale_factor.to_bits().to_le_bytes());
    hasher.update(frame.scroll_y.to_bits().to_le_bytes());
    hasher.update(frame.content_height.to_bits().to_le_bytes());
    update_bytes(&mut hasher, frame.origin.as_str().as_bytes());
    hasher.update([match frame.pixel_format {
        HtmlBrowserPixelFormat::Rgba8 => 0,
    }]);
    update_bytes(&mut hasher, &frame.pixels);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn update_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
#[path = "artifact_tests.rs"]
mod tests;
