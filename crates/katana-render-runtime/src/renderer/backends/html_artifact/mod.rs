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
        self.restore_vertical_scroll(&mut session)?;
        Ok(session)
    }

    fn restore_vertical_scroll(
        &self,
        session: &mut HtmlBrowserSession,
    ) -> Result<(), HtmlArtifactError> {
        let initial_scroll_y = session.latest_frame().map_or(0.0, |frame| frame.scroll_y);
        let delta_y =
            (self.frame.scroll_y - initial_scroll_y) * session.viewport().device_scale_factor;
        if delta_y != 0.0 {
            session
                .dispatch_input(HtmlBrowserInput::Scroll {
                    delta_x: 0.0,
                    delta_y,
                })
                .map_err(|error| HtmlArtifactError::Runtime(error.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
