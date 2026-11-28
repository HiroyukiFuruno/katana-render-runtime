use super::super::HtmlRuntime;
use super::*;

const FRAME_WIDTH: u32 = 80;
const FRAME_HEIGHT: u32 = 60;
const WIDER_FRAME_WIDTH: u32 = FRAME_WIDTH * 2;
const NO_SCROLL: f32 = 0.0;
const NEGATIVE_VIEWPORT_WIDTH: u32 = 0;
const SOURCE: &str = "<body style=\"margin:0;background:red\"></body>";
const FRAME_ORIGIN: &str = "https://example.test/frame.html";

fn assert_synchronized_inner_frame_released(session: &HtmlBrowserSession) {
    assert!(
        session
            .interactive
            .as_ref()
            .is_some_and(|interactive| interactive.latest_frame().is_none())
    );
}

#[test]
fn unchanged_generation_keeps_public_frame_and_pending_update_state() {
    with_frame_session(SOURCE, FRAME_ORIGIN, FRAME_WIDTH, FRAME_HEIGHT, |session| {
        assert_synchronized_inner_frame_released(session);
        assert!(session.take_frame_update().is_some());
        let pointer = session.latest_frame().map(|frame| frame.pixels.as_ptr());
        assert!(session.latest_frame().cloned().is_some_and(|initial| {
            assert!(
                session
                    .dispatch_input(HtmlBrowserInput::Scroll {
                        delta_x: NO_SCROLL,
                        delta_y: NO_SCROLL,
                    })
                    .is_ok()
            );
            assert!(
                session
                    .latest_frame()
                    .is_some_and(|frame| frame == &initial)
            );
            assert_eq!(
                session.latest_frame().map(|frame| frame.pixels.as_ptr()),
                pointer
            );
            assert_synchronized_inner_frame_released(session);
            assert!(session.take_frame_update().is_none());
            assert!(session.take_navigation().is_none());
            true
        }));
    });
}

#[test]
fn refreshed_frame_and_public_error_preserve_owned_prior_snapshot() {
    with_frame_session(SOURCE, FRAME_ORIGIN, FRAME_WIDTH, FRAME_HEIGHT, |session| {
        assert_synchronized_inner_frame_released(session);
        let pointer = session.latest_frame().map(|frame| frame.pixels.as_ptr());
        assert!(session.latest_frame().cloned().is_some_and(|initial| {
            let saved_pixels = initial.pixels.clone();
            assert!(session.refresh_frame().is_ok());
            assert_synchronized_inner_frame_released(session);
            assert_eq!(
                session.latest_frame().map(|frame| frame.pixels.as_ptr()),
                pointer
            );
            assert!(session.latest_frame().is_some_and(|frame| {
                assert!(frame.generation > initial.generation);
                assert!(frame.pixels == saved_pixels);
                true
            }));
            assert!(initial.pixels == saved_pixels);
            assert!(session.take_frame_update().is_some());
            assert_resize_preserves_owned_snapshot(session, &initial);
            assert_synchronized_inner_frame_released(session);
            assert_public_error_keeps_current_frame(session);
            true
        }));
    });
}

fn assert_resize_preserves_owned_snapshot(
    session: &mut HtmlBrowserSession,
    initial: &HtmlBrowserFrame,
) {
    let saved_pixels = initial.pixels.clone();
    assert!(
        HtmlBrowserViewport::new(WIDER_FRAME_WIDTH, FRAME_HEIGHT, 1.0).is_ok_and(|viewport| {
            assert!(session.resize(viewport).is_ok());
            assert!(
                session
                    .latest_frame()
                    .is_some_and(|frame| frame.viewport.width == WIDER_FRAME_WIDTH)
            );
            assert_eq!(initial.viewport.width, FRAME_WIDTH);
            assert!(initial.pixels == saved_pixels);
            assert!(session.take_frame_update().is_some());
            true
        })
    );
}

fn assert_public_error_keeps_current_frame(session: &mut HtmlBrowserSession) {
    assert!(session.latest_frame().cloned().is_some_and(|accepted| {
        let invalid = HtmlBrowserViewport {
            width: NEGATIVE_VIEWPORT_WIDTH,
            height: FRAME_HEIGHT,
            device_scale_factor: 1.0,
        };
        assert_eq!(
            session.resize(invalid),
            Err(HtmlBrowserError::InvalidViewport)
        );
        assert!(
            session
                .latest_frame()
                .is_some_and(|frame| frame == &accepted)
        );
        assert!(session.take_frame_update().is_none());
        true
    }));
}

pub(super) fn with_frame_session(
    source_text: &str,
    origin: &str,
    width: u32,
    height: u32,
    action: impl FnOnce(&mut HtmlBrowserSession),
) {
    assert!(
        HtmlBrowserSource::new(source_text, origin).is_ok_and(|source| {
            assert!(
                HtmlBrowserViewport::new(width, height, 1.0).is_ok_and(|viewport| {
                    assert!(HtmlRuntime.open(source, viewport).is_ok_and(|mut session| {
                        action(&mut session);
                        assert!(session.close().is_ok());
                        true
                    }));
                    true
                })
            );
            true
        })
    );
}
