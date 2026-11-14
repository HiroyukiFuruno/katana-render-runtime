use super::{SvgRasterizeOps, font::observation};
use crate::{HtmlBrowserFrame, HtmlBrowserSource};
use sha2::{Digest, Sha256};

impl SvgRasterizeOps {
    pub(crate) fn font_work_enabled() -> bool {
        observation::enabled()
    }

    pub(crate) fn font_work_frame_scope() -> observation::FrameScope {
        observation::FrameScope::enter()
    }

    pub(crate) fn observe_font_work(domain: &str, value: serde_json::Value) {
        observation::work_value(domain, value);
    }

    pub(crate) fn observe_font_frame(frame: Option<&HtmlBrowserFrame>, source: &HtmlBrowserSource) {
        if !observation::enabled() {
            return;
        }
        if let Some(frame) = frame {
            observation::work_value(
                "frame",
                serde_json::json!({
                    "generation": frame.generation, "origin": frame.origin.as_str(),
                    "viewport": frame.viewport, "scrollYBits": frame.scroll_y.to_bits(),
                    "contentHeightBits": frame.content_height.to_bits(), "rgbaBytes": frame.pixels.len(),
                    "pixelSha256": Sha256::digest(&frame.pixels).to_vec(),
                    "sourceSha256": Sha256::digest(source.raw_html.as_bytes()).to_vec(),
                }),
            );
        }
    }
}
