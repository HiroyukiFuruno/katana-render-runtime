use super::super::{HtmlBrowserSource, HtmlBrowserViewport};
use super::{HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserScrollMetrics, HtmlBrowserSession};
use crate::HtmlBrowserOrigin;

type TestResult<T = ()> = Result<T, String>;
type FramePair = (HtmlBrowserFrame, HtmlBrowserScrollMetrics);

const ORIGIN: &str = "https://example.test/sync.html";
const FRAGMENT_ORIGIN: &str = "https://example.test/sync.html#target";
const FOREIGN_ORIGIN: &str = "https://example.test/foreign.html";
const VIEWPORT_WIDTH: u32 = 80;
const VIEWPORT_HEIGHT: u32 = 60;
const HTML: &str =
    "<body style='margin:0'><div style='height:200px'></div><p id=target>target</p></body>";

fn open() -> TestResult<HtmlBrowserSession> {
    let source = HtmlBrowserSource::new(HTML, ORIGIN).map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    HtmlBrowserSession::new(source, viewport).map_err(|error| error.to_string())
}

fn current_pair(session: &HtmlBrowserSession) -> TestResult<FramePair> {
    session
        .latest_frame_with_metrics()
        .map(|(frame, metrics)| (frame.clone(), *metrics))
        .ok_or_else(|| "real runtime must publish a frame/metrics pair".to_owned())
}

fn stage_frame(session: &mut HtmlBrowserSession) -> TestResult<FramePair> {
    let interactive = session
        .interactive
        .as_mut()
        .ok_or_else(|| "real runtime must exist".to_owned())?;
    interactive
        .refresh_frame()
        .map_err(|error| error.to_string())?;
    let frame = interactive.latest_frame().cloned().ok_or("staged frame")?;
    let metrics = interactive
        .latest_scroll_metrics()
        .copied()
        .ok_or("staged metrics")?;
    Ok((frame, metrics))
}

fn sync_pair(session: &mut HtmlBrowserSession, pair: FramePair) -> TestResult {
    session
        .sync_interactive_frame_with_metrics(Some(pair.0), Some(pair.1))
        .map_err(|error| error.to_string())
}

#[test]
fn paired_helper_commits_real_frame_and_same_generation_metrics_without_republishing() -> TestResult
{
    let mut session = open()?;
    let pair = stage_frame(&mut session)?;
    sync_pair(&mut session, pair.clone())?;
    assert_eq!(current_pair(&session)?, pair);
    assert!(session.take_frame_update_with_metrics().is_some());
    sync_pair(&mut session, pair.clone())?;
    session
        .sync_interactive_frame_with_metrics(None, Some(pair.1))
        .map_err(|error| error.to_string())?;
    session
        .sync_interactive_frame_with_metrics(None, None)
        .map_err(|error| error.to_string())?;
    assert!(session.take_frame_update_with_metrics().is_none());
    assert_eq!(current_pair(&session)?, pair);
    session.close().map_err(|error| error.to_string())
}

#[test]
fn rejected_real_pair_preserves_public_snapshot_and_can_retry() -> TestResult {
    let mut session = open()?;
    let snapshot = current_pair(&session)?;
    let pair = stage_frame(&mut session)?;
    let mut invalid = pair.clone();
    invalid.1.generation += 1;
    assert!(sync_pair(&mut session, invalid).is_err());
    assert_eq!(current_pair(&session)?, snapshot);
    sync_pair(&mut session, pair.clone())?;
    assert_eq!(current_pair(&session)?, pair);
    assert_ne!(snapshot.0.generation, pair.0.generation);
    session.close().map_err(|error| error.to_string())
}

#[test]
fn same_generation_metrics_mismatch_keeps_the_committed_pair() -> TestResult {
    let mut session = open()?;
    let snapshot = current_pair(&session)?;
    let mut invalid = snapshot.1;
    invalid.viewport_width += 1.0;
    assert!(
        session
            .sync_interactive_frame_with_metrics(None, Some(invalid))
            .is_err()
    );
    assert_eq!(current_pair(&session)?, snapshot);
    session.close().map_err(|error| error.to_string())
}

#[test]
fn paired_fragment_acceptance_updates_origin_only_after_validation() -> TestResult {
    let mut session = open()?;
    let mut pair = stage_frame(&mut session)?;
    pair.0.origin = HtmlBrowserOrigin::parse(FRAGMENT_ORIGIN).map_err(|error| error.to_string())?;
    sync_pair(&mut session, pair.clone())?;
    assert_eq!(session.source().origin, pair.0.origin);
    assert_eq!(current_pair(&session)?, pair);
    session.close().map_err(|error| error.to_string())
}

#[test]
fn paired_fragment_rejection_restores_origin_and_closed_state() -> TestResult {
    let mut session = open()?;
    let mut pair = stage_frame(&mut session)?;
    pair.0.origin = HtmlBrowserOrigin::parse(FRAGMENT_ORIGIN).map_err(|error| error.to_string())?;
    session.close().map_err(|error| error.to_string())?;
    assert_eq!(
        session.sync_interactive_frame_with_metrics(Some(pair.0), Some(pair.1)),
        Err(HtmlBrowserError::SessionClosed)
    );
    assert_eq!(session.source().origin.as_str(), ORIGIN);
    assert!(session.latest_frame_with_metrics().is_none());
    Ok(())
}

#[test]
fn frame_only_helper_commits_real_frame_and_ignores_duplicate_generation() -> TestResult {
    let mut session = open()?;
    let (frame, _) = stage_frame(&mut session)?;
    session
        .sync_interactive_frame(Some(frame.clone()))
        .map_err(|error| error.to_string())?;
    assert_eq!(session.latest_frame(), Some(&frame));
    assert!(session.latest_scroll_metrics().is_none());
    assert!(session.take_frame_update().is_some());
    session
        .sync_interactive_frame(Some(frame))
        .map_err(|error| error.to_string())?;
    session
        .sync_interactive_frame(None)
        .map_err(|error| error.to_string())?;
    assert!(session.take_frame_update().is_none());
    session.close().map_err(|error| error.to_string())
}

#[test]
fn frame_only_rejects_fragment_after_close_and_preserves_origin() -> TestResult {
    let mut session = open()?;
    let (mut frame, _) = stage_frame(&mut session)?;
    frame.origin = HtmlBrowserOrigin::parse(FRAGMENT_ORIGIN).map_err(|error| error.to_string())?;
    session.close().map_err(|error| error.to_string())?;
    assert_eq!(
        session.sync_interactive_frame(Some(frame)),
        Err(HtmlBrowserError::SessionClosed)
    );
    assert_eq!(session.source().origin.as_str(), ORIGIN);
    Ok(())
}

#[test]
fn paired_foreign_origin_rejection_leaves_real_frame_and_metrics_unchanged() -> TestResult {
    let mut session = open()?;
    let snapshot = current_pair(&session)?;
    let mut pair = stage_frame(&mut session)?;
    pair.0.origin = HtmlBrowserOrigin::parse(FOREIGN_ORIGIN).map_err(|error| error.to_string())?;
    assert!(matches!(
        session.sync_interactive_frame_with_metrics(Some(pair.0), Some(pair.1)),
        Err(HtmlBrowserError::FrameOriginMismatch { .. })
    ));
    assert_eq!(current_pair(&session)?, snapshot);
    assert_eq!(session.source().origin.as_str(), ORIGIN);
    session.close().map_err(|error| error.to_string())
}
