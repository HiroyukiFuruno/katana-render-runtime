use super::frame_clone_tests::with_frame_session;
use std::time::Duration;

const FRAME_WIDTH: u32 = 80;
const FRAME_HEIGHT: u32 = 60;
const RGBA_CHANNELS: usize = 4;
const DELAY_MS: u64 = 25;
const RED: [u8; RGBA_CHANNELS] = [255, 0, 0, 255];
const BLUE: [u8; RGBA_CHANNELS] = [0, 0, 255, 255];
const SOURCE: &str = "<body style=\"margin:0;background:red\"><script>setTimeout(() => document.querySelector('body').setAttribute('style', 'margin:0;background:blue'), 25)</script></body>";
const FRAME_ORIGIN: &str = "https://example.test/frame-reuse.html";

#[test]
fn changed_frame_reuses_pixels_without_mutating_user_owned_snapshot() {
    with_frame_session(SOURCE, FRAME_ORIGIN, FRAME_WIDTH, FRAME_HEIGHT, |session| {
        assert!(session.take_frame_update().is_some());
        let pointer = session.latest_frame().map(|frame| frame.pixels.as_ptr());
        assert!(session.latest_frame().cloned().is_some_and(|initial| {
            assert!(solid_color(&initial.pixels, RED));
            assert!(
                session
                    .advance_time(Duration::from_millis(DELAY_MS))
                    .is_ok()
            );
            assert!(session.latest_frame().is_some_and(|frame| {
                frame.generation > initial.generation
                    && frame.origin == initial.origin
                    && frame.viewport == initial.viewport
                    && solid_color(&frame.pixels, BLUE)
            }));
            assert_eq!(
                session.latest_frame().map(|frame| frame.pixels.as_ptr()),
                pointer
            );
            assert!(solid_color(&initial.pixels, RED));
            assert!(session.take_frame_update().is_some());
            assert!(session.take_navigation().is_none());
            true
        }));
    });
}

fn solid_color(pixels: &[u8], expected: [u8; RGBA_CHANNELS]) -> bool {
    !pixels.is_empty()
        && pixels
            .chunks_exact(RGBA_CHANNELS)
            .all(|pixel| pixel == expected)
}
