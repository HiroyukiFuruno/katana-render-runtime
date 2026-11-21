use super::{HtmlBrowserError, HtmlBrowserOverflowPolicy as Policy, HtmlBrowserScrollMetrics};
use crate::HtmlBrowserViewport;

const VIEWPORT_SIDE: u32 = 80;
const CONTENT_SIDE: f32 = 160.0;
const MAX_OFFSET: f32 = CONTENT_SIDE - VIEWPORT_SIDE as f32;

fn metrics(
    policy: Policy,
    offset: (f32, f32),
) -> Result<HtmlBrowserScrollMetrics, HtmlBrowserError> {
    HtmlBrowserScrollMetrics::new(
        1,
        HtmlBrowserViewport::new(VIEWPORT_SIDE, VIEWPORT_SIDE, 1.0)?,
        (CONTENT_SIDE, CONTENT_SIDE),
        offset,
        (policy, policy),
    )
}

#[test]
fn both_axes_preserve_overflow_policy_scroll_range_and_consumer_bar_visibility()
-> Result<(), HtmlBrowserError> {
    for (policy, range, visible) in [
        (Policy::Auto, MAX_OFFSET, true),
        (Policy::Scroll, MAX_OFFSET, true),
        (Policy::Hidden, MAX_OFFSET, false),
        (Policy::Clip, 0.0, false),
    ] {
        let metrics = metrics(policy, (0.0, 0.0))?;
        assert_eq!(metrics.max_scroll_x(), range);
        assert_eq!(metrics.max_scroll_y(), range);
        assert_eq!(metrics.horizontal_scrollbar_visible(), visible);
        assert_eq!(metrics.vertical_scrollbar_visible(), visible);
    }
    Ok(())
}

#[test]
fn non_finite_or_negative_offsets_are_rejected_before_scroll_bounds() {
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        for offset in [(value, 0.0), (0.0, value)] {
            assert_eq!(
                metrics(Policy::Auto, offset),
                Err(HtmlBrowserError::InvalidInputCoordinates)
            );
        }
    }
}

#[test]
fn hidden_allows_the_boundary_offset_but_clip_rejects_any_offset() {
    assert!(metrics(Policy::Hidden, (MAX_OFFSET, MAX_OFFSET)).is_ok());
    assert_eq!(
        metrics(Policy::Clip, (1.0, 0.0)),
        Err(HtmlBrowserError::InvalidInputCoordinates)
    );
    assert_eq!(
        metrics(Policy::Clip, (0.0, 1.0)),
        Err(HtmlBrowserError::InvalidInputCoordinates)
    );
}

#[test]
fn metrics_reject_invalid_viewport_before_computing_logical_dimensions() {
    for viewport in [
        HtmlBrowserViewport {
            width: 0,
            height: VIEWPORT_SIDE,
            device_scale_factor: 1.0,
        },
        HtmlBrowserViewport {
            width: VIEWPORT_SIDE,
            height: VIEWPORT_SIDE,
            device_scale_factor: 0.0,
        },
    ] {
        let expected = viewport.validate();
        assert!(expected.is_err());
        assert_eq!(
            HtmlBrowserScrollMetrics::new(
                1,
                viewport,
                (CONTENT_SIDE, CONTENT_SIDE),
                (0.0, 0.0),
                (Policy::Auto, Policy::Auto),
            )
            .map(|_| ()),
            expected
        );
    }
}
