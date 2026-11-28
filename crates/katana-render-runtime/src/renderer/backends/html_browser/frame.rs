pub use super::scroll_metrics::{HtmlBrowserOverflowPolicy, HtmlBrowserScrollMetrics};
use super::{HtmlBrowserError, HtmlBrowserOrigin, HtmlBrowserViewport};
use serde::{Deserialize, Serialize};

const RGBA8_BYTES_PER_PIXEL: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlBrowserPixelFormat {
    Rgba8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlBrowserFrame {
    pub generation: u64,
    pub origin: HtmlBrowserOrigin,
    pub viewport: HtmlBrowserViewport,
    #[serde(default)]
    pub scroll_y: f32,
    #[serde(default)]
    pub content_height: f32,
    pub pixel_format: HtmlBrowserPixelFormat,
    pub pixels: Vec<u8>,
}

impl HtmlBrowserFrame {
    pub fn new(
        generation: u64,
        origin: HtmlBrowserOrigin,
        viewport: HtmlBrowserViewport,
        pixel_format: HtmlBrowserPixelFormat,
        pixels: Vec<u8>,
    ) -> Result<Self, HtmlBrowserError> {
        viewport.validate()?;
        let expected_bytes = (viewport.width as usize)
            .checked_mul(viewport.height as usize)
            .and_then(|pixels| pixels.checked_mul(RGBA8_BYTES_PER_PIXEL))
            .ok_or(HtmlBrowserError::FrameDimensionsOverflow)?;
        if pixels.len() != expected_bytes {
            return Err(HtmlBrowserError::InvalidFrameBufferSize {
                actual_bytes: pixels.len(),
                expected_bytes,
            });
        }
        Ok(Self {
            generation,
            origin,
            viewport,
            scroll_y: 0.0,
            content_height: 0.0,
            pixel_format,
            pixels,
        })
    }

    pub(crate) fn with_layout_metrics(mut self, scroll_y: f32, content_height: f32) -> Self {
        self.scroll_y = scroll_y;
        self.content_height = content_height;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserOrigin, HtmlBrowserOverflowPolicy,
        HtmlBrowserPixelFormat, HtmlBrowserScrollMetrics, HtmlBrowserViewport,
    };
    #[test]
    fn frame_rejects_invalid_buffer_sizes() {
        let origin = origin();
        let viewport = must(HtmlBrowserViewport::new(2, 2, 1.0));

        let invalid = HtmlBrowserFrame::new(
            1,
            origin.clone(),
            viewport,
            HtmlBrowserPixelFormat::Rgba8,
            vec![0; 15],
        );
        assert_eq!(
            invalid,
            Err(HtmlBrowserError::InvalidFrameBufferSize {
                actual_bytes: 15,
                expected_bytes: 16
            })
        );
    }

    #[test]
    fn frame_rejects_dimension_overflow() {
        let origin = origin();
        let overflow_viewport = HtmlBrowserViewport {
            width: u32::MAX,
            height: u32::MAX,
            device_scale_factor: 1.0,
        };
        let overflow = HtmlBrowserFrame::new(
            1,
            origin,
            overflow_viewport,
            HtmlBrowserPixelFormat::Rgba8,
            Vec::new(),
        );
        assert_eq!(overflow, Err(HtmlBrowserError::FrameDimensionsOverflow));
    }

    #[test]
    fn frame_rejects_invalid_viewport() {
        let invalid_viewport = HtmlBrowserViewport {
            width: 0,
            height: 1,
            device_scale_factor: 1.0,
        };
        let frame = HtmlBrowserFrame::new(
            1,
            origin(),
            invalid_viewport,
            HtmlBrowserPixelFormat::Rgba8,
            Vec::new(),
        );

        assert_eq!(frame, Err(HtmlBrowserError::InvalidViewport));
    }

    #[test]
    fn frame_layout_metrics_default_and_update_without_changing_pixel_contract() {
        let viewport = must(HtmlBrowserViewport::new(1, 1, 1.0));
        let frame = must(HtmlBrowserFrame::new(
            1,
            origin(),
            viewport,
            HtmlBrowserPixelFormat::Rgba8,
            vec![255; 4],
        ));
        assert_eq!(frame.scroll_y, 0.0);
        assert_eq!(frame.content_height, 0.0);

        let frame = frame.with_layout_metrics(25.0, 400.0);
        assert_eq!(frame.scroll_y, 25.0);
        assert_eq!(frame.content_height, 400.0);
    }

    #[test]
    fn scroll_metrics_round_content_for_maximum_offsets_and_scrollbars() {
        let metrics = metrics(
            2.0,
            (80.6, 60.4),
            (10.0, 5.0),
            (
                HtmlBrowserOverflowPolicy::Auto,
                HtmlBrowserOverflowPolicy::Scroll,
            ),
        );

        assert_eq!(metrics.viewport_width, 50.0);
        assert_eq!(metrics.viewport_height, 40.0);
        assert_eq!(metrics.max_scroll_x(), 31.0);
        assert_eq!(metrics.max_scroll_y(), 20.0);
        assert!(metrics.horizontal_scrollbar_visible());
        assert!(metrics.vertical_scrollbar_visible());
    }

    #[test]
    fn clipped_axis_has_no_range_or_scrollbar() {
        let metrics = metrics(
            1.0,
            (200.0, 160.0),
            (0.0, 0.0),
            (
                HtmlBrowserOverflowPolicy::Clip,
                HtmlBrowserOverflowPolicy::Hidden,
            ),
        );

        assert_eq!(metrics.max_scroll_x(), 0.0);
        assert_eq!(metrics.max_scroll_y(), 80.0);
        assert!(!metrics.horizontal_scrollbar_visible());
        assert!(!metrics.vertical_scrollbar_visible());
    }

    #[test]
    fn scroll_metrics_reject_nonfinite_and_inconsistent_values() {
        let viewport = must(HtmlBrowserViewport::new(100, 80, 1.0));
        assert_eq!(
            HtmlBrowserScrollMetrics::new(
                1,
                viewport,
                (f32::NAN, 80.0),
                (0.0, 0.0),
                (
                    HtmlBrowserOverflowPolicy::Auto,
                    HtmlBrowserOverflowPolicy::Auto,
                ),
            ),
            Err(HtmlBrowserError::InvalidViewport)
        );
        assert_eq!(
            HtmlBrowserScrollMetrics::new(
                1,
                viewport,
                (100.0, 80.0),
                (101.0, 0.0),
                (
                    HtmlBrowserOverflowPolicy::Auto,
                    HtmlBrowserOverflowPolicy::Auto,
                ),
            ),
            Err(HtmlBrowserError::InvalidInputCoordinates)
        );
    }

    #[test]
    #[should_panic(
        expected = "unexpected test error: browser viewport dimensions must be non-zero"
    )]
    fn must_reports_unexpected_test_errors() {
        let _: HtmlBrowserViewport = must(HtmlBrowserViewport::new(0, 1, 1.0));
    }

    fn origin() -> HtmlBrowserOrigin {
        must(HtmlBrowserOrigin::parse("https://example.test/frame.html"))
    }

    fn must<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }

    fn metrics(
        device_scale_factor: f32,
        content: (f32, f32),
        offset: (f32, f32),
        overflow: (HtmlBrowserOverflowPolicy, HtmlBrowserOverflowPolicy),
    ) -> HtmlBrowserScrollMetrics {
        must(HtmlBrowserScrollMetrics::new(
            7,
            must(HtmlBrowserViewport::new(100, 80, device_scale_factor)),
            content,
            offset,
            overflow,
        ))
    }
}
