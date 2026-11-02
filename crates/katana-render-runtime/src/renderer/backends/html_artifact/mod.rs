mod artifact;
mod encoding;
mod filesystem;
mod key;
mod store;
mod types;

pub use artifact::HtmlFrameArtifact;
pub use store::HtmlArtifactStore;
pub use types::{HtmlArtifactError, HtmlArtifactProvenance};

use super::html_browser::{
    HtmlBrowserInput, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
};

impl HtmlFrameArtifact {
    pub fn resume_session(
        &self,
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
        expected: &HtmlArtifactProvenance,
    ) -> Result<HtmlBrowserSession, HtmlArtifactError> {
        self.validate()?;
        expected.validate()?;
        if expected.runtime_version != env!("CARGO_PKG_VERSION")
            || self.provenance != *expected
            || HtmlArtifactProvenance::source_sha256(&source) != expected.source_sha256
            || source.origin != expected.origin
            || viewport != expected.viewport
        {
            return Err(HtmlArtifactError::ProvenanceMismatch);
        }
        let mut session = HtmlRuntime
            .open(source, viewport)
            .map_err(|error| HtmlArtifactError::Runtime(error.to_string()))?;
        if self.frame.scroll_y > 0.0 {
            session
                .dispatch_input(HtmlBrowserInput::Scroll {
                    delta_x: 0.0,
                    delta_y: self.frame.scroll_y,
                })
                .map_err(|error| HtmlArtifactError::Runtime(error.to_string()))?;
        }
        Ok(session)
    }
}

#[cfg(test)]
mod tests;
