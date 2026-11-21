use super::super::super::{
    HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserOrigin, HtmlBrowserOverflowPolicy,
    HtmlBrowserPixelFormat, HtmlBrowserScrollMetrics, HtmlBrowserSource, HtmlBrowserViewport,
};
use super::super::{HtmlBrowserSession, HtmlBrowserSessionState};

const FRAME_GENERATION: u64 = 7;
const FRAME_WIDTH: u32 = 100;
const FRAME_HEIGHT: u32 = 80;
const CONTENT_WIDTH: f32 = 220.0;
const CONTENT_HEIGHT: f32 = 180.0;
const SCROLL_X: f32 = 30.0;
const SCROLL_Y: f32 = 20.0;
const PAIR_REQUIRED: &str = "interactive frame and scroll metrics were not published as a pair";
const PAIR_MISMATCH: &str = "interactive frame and scroll metrics generation/viewport mismatch";

fn viewport() -> Result<HtmlBrowserViewport, HtmlBrowserError> {
    HtmlBrowserViewport::new(FRAME_WIDTH, FRAME_HEIGHT, 1.0)
}

fn frame() -> Result<HtmlBrowserFrame, HtmlBrowserError> {
    Ok(HtmlBrowserFrame::new(
        FRAME_GENERATION,
        HtmlBrowserOrigin::parse("https://example.test/sync.html")?,
        viewport()?,
        HtmlBrowserPixelFormat::Rgba8,
        vec![0; (FRAME_WIDTH * FRAME_HEIGHT * 4) as usize],
    )?
    .with_layout_metrics(SCROLL_Y, CONTENT_HEIGHT))
}

fn metrics() -> Result<HtmlBrowserScrollMetrics, HtmlBrowserError> {
    HtmlBrowserScrollMetrics::new(
        FRAME_GENERATION,
        viewport()?,
        (CONTENT_WIDTH, CONTENT_HEIGHT),
        (SCROLL_X, SCROLL_Y),
        (
            HtmlBrowserOverflowPolicy::Auto,
            HtmlBrowserOverflowPolicy::Auto,
        ),
    )
}

fn session_without_runtime() -> Result<HtmlBrowserSession, HtmlBrowserError> {
    Ok(HtmlBrowserSession {
        source: HtmlBrowserSource::new("<p>sync contract</p>", "https://example.test/sync.html")?,
        viewport: viewport()?,
        state: HtmlBrowserSessionState::Active,
        latest_frame: None,
        latest_scroll_metrics: None,
        frame_update_pending: false,
        pending_navigation: None,
        interactive: None,
    })
}

#[test]
fn optional_updates_require_a_complete_frame_metrics_pair() -> Result<(), HtmlBrowserError> {
    let frame = frame()?;
    let metrics = metrics()?;
    assert_eq!(
        HtmlBrowserSession::validate_optional_interactive_pair(None, None),
        Ok(())
    );
    assert_eq!(
        HtmlBrowserSession::validate_optional_interactive_pair(Some(&frame), Some(&metrics)),
        Ok(())
    );
    assert_eq!(
        HtmlBrowserSession::validate_optional_interactive_pair(Some(&frame), None),
        Err(HtmlBrowserError::RuntimeFailure {
            error: PAIR_REQUIRED.to_owned()
        })
    );
    assert_eq!(
        HtmlBrowserSession::validate_optional_interactive_pair(None, Some(&metrics)),
        Err(HtmlBrowserError::RuntimeFailure {
            error: PAIR_REQUIRED.to_owned()
        })
    );
    Ok(())
}

fn assert_pair_mismatch(frame: &HtmlBrowserFrame, metrics: &HtmlBrowserScrollMetrics) {
    assert_eq!(
        HtmlBrowserSession::validate_interactive_pair(frame, metrics),
        Err(HtmlBrowserError::RuntimeFailure {
            error: PAIR_MISMATCH.to_owned()
        })
    );
}

#[test]
fn frame_metrics_must_match_every_shared_layout_field() -> Result<(), HtmlBrowserError> {
    let frame = frame()?;
    let valid = metrics()?;
    let mismatches = [
        HtmlBrowserScrollMetrics {
            generation: valid.generation + 1,
            ..valid
        },
        HtmlBrowserScrollMetrics {
            viewport_width: valid.viewport_width + 1.0,
            ..valid
        },
        HtmlBrowserScrollMetrics {
            viewport_height: valid.viewport_height + 1.0,
            ..valid
        },
        HtmlBrowserScrollMetrics {
            scroll_y: valid.scroll_y + 1.0,
            ..valid
        },
        HtmlBrowserScrollMetrics {
            content_height: valid.content_height + 1.0,
            ..valid
        },
    ];
    for candidate in mismatches {
        assert_pair_mismatch(&frame, &candidate);
    }
    Ok(())
}

#[test]
fn syncing_rejects_either_half_without_its_counterpart() -> Result<(), HtmlBrowserError> {
    let mut session = session_without_runtime()?;
    assert_eq!(
        session.sync_interactive_frame_with_metrics(Some(frame()?), None),
        Err(HtmlBrowserError::RuntimeFailure {
            error: "interactive frame is missing scroll metrics".to_owned()
        })
    );
    assert_eq!(
        session.sync_interactive_frame_with_metrics(None, Some(metrics()?)),
        Err(HtmlBrowserError::RuntimeFailure {
            error: "interactive scroll metrics are missing their frame".to_owned()
        })
    );
    assert!(session.latest_frame.is_none());
    assert!(session.latest_scroll_metrics.is_none());
    Ok(())
}
