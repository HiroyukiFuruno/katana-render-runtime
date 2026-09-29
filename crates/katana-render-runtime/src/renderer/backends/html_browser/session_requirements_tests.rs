use super::HtmlBrowserSession;
use crate::system::ProcessService;
use crate::{HtmlBrowserFrame, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};
use std::process::Child;
use std::time::{Duration, Instant};

type TestResult<T = ()> = Result<T, String>;
const REQUIREMENTS_FIXTURE: &str =
    include_str!("../../../../tests/fixtures/html_browser/requirements_v1.html");
const REQUIREMENTS_ORIGIN: &str = "https://example.test/requirements-v1.html#s15";
const REQUIREMENTS_VIEWPORT_WIDTH: u32 = 1_280;
const REQUIREMENTS_VIEWPORT_HEIGHT: u32 = 900;
const DEVICE_SCALE_FACTOR: f32 = 1.0;
const RGB_CHANNEL_COUNT: usize = 3;
const STICKY_SIDEBAR_RGB: [u8; RGB_CHANNEL_COUNT] = [15, 23, 42];
const MINIMUM_STICKY_SIDEBAR_PIXELS: usize = 200_000;
const MAXIMUM_START_AND_CLOSE_DURATION: Duration = Duration::from_secs(60);
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);
const REQUIREMENTS_CHILD_ENV: &str = "KRR_HTML_REQUIREMENTS_CHILD";
const REQUIREMENTS_TEST_NAME: &str = "renderer::backends::html_browser::session::requirements_tests::requirements_fixture_returns_fragment_frame_with_sticky_sidebar_and_closes_within_budget";
const RGBA_CHANNEL_COUNT: usize = 4;
const RED_CHANNEL: usize = 0;
const GREEN_CHANNEL: usize = 1;
const BLUE_CHANNEL: usize = 2;
const ALPHA_CHANNEL: usize = 3;
const OPAQUE_ALPHA: u8 = 255;

#[test]
fn requirements_fixture_returns_fragment_frame_with_sticky_sidebar_and_closes_within_budget()
-> TestResult {
    if std::env::var_os(REQUIREMENTS_CHILD_ENV).is_some() {
        return run_requirements_fixture();
    }

    let mut child = start_requirements_fixture_child()?;
    wait_for_requirements_fixture_child(&mut child)
}

fn run_requirements_fixture() -> TestResult {
    let started = Instant::now();
    let mut session = open_requirements_fixture()?;
    assert_requirements_fragment_frame(&mut session)?;
    assert_requirements_close_within_budget(&mut session, started)
}

fn start_requirements_fixture_child() -> TestResult<Child> {
    ProcessService::create_command(std::env::current_exe().map_err(|error| error.to_string())?)
        .args(["--exact", REQUIREMENTS_TEST_NAME, "--nocapture"])
        .env(REQUIREMENTS_CHILD_ENV, "1")
        .spawn()
        .map_err(|error| error.to_string())
}

fn wait_for_requirements_fixture_child(child: &mut Child) -> TestResult {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return status.success().then_some(()).ok_or_else(|| {
                format!("requirements fixture child exited unsuccessfully: {status}")
            });
        }
        if started.elapsed() >= MAXIMUM_START_AND_CLOSE_DURATION {
            child.kill().map_err(|error| error.to_string())?;
            child.wait().map_err(|error| error.to_string())?;
            return Err(
                "requirements fixture exceeded the 60 second initial-frame and close budget"
                    .to_string(),
            );
        }
        std::thread::sleep(CHILD_POLL_INTERVAL);
    }
}

fn open_requirements_fixture() -> TestResult<HtmlBrowserSession> {
    let source = HtmlBrowserSource::new(REQUIREMENTS_FIXTURE, REQUIREMENTS_ORIGIN)
        .map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(
        REQUIREMENTS_VIEWPORT_WIDTH,
        REQUIREMENTS_VIEWPORT_HEIGHT,
        DEVICE_SCALE_FACTOR,
    )
    .map_err(|error| error.to_string())?;
    HtmlRuntime
        .open(source, viewport)
        .map_err(|error| error.to_string())
}

fn assert_requirements_fragment_frame(session: &mut HtmlBrowserSession) -> TestResult {
    let frame = session
        .take_frame_update()
        .ok_or_else(|| "requirements fixture must produce its first frame".to_string())?;
    assert_eq!(frame.origin.as_str(), REQUIREMENTS_ORIGIN);
    let sidebar_pixels = frame_matching_rgb_pixels(frame, STICKY_SIDEBAR_RGB);
    assert!(
        sidebar_pixels >= MINIMUM_STICKY_SIDEBAR_PIXELS,
        "sticky sidebar disappeared from the #s15 frame: {sidebar_pixels}"
    );
    Ok(())
}

fn assert_requirements_close_within_budget(
    session: &mut HtmlBrowserSession,
    started: Instant,
) -> TestResult {
    session.close().map_err(|error| error.to_string())?;
    assert!(
        started.elapsed() < MAXIMUM_START_AND_CLOSE_DURATION,
        "requirements fixture exceeded the 60 second initial-frame and close budget: {:?}",
        started.elapsed()
    );
    Ok(())
}

fn frame_matching_rgb_pixels(frame: &HtmlBrowserFrame, expected: [u8; RGB_CHANNEL_COUNT]) -> usize {
    frame
        .pixels
        .as_chunks::<RGBA_CHANNEL_COUNT>()
        .0
        .iter()
        .filter(|pixel| {
            pixel[RED_CHANNEL] == expected[RED_CHANNEL]
                && pixel[GREEN_CHANNEL] == expected[GREEN_CHANNEL]
                && pixel[BLUE_CHANNEL] == expected[BLUE_CHANNEL]
                && pixel[ALPHA_CHANNEL] == OPAQUE_ALPHA
        })
        .count()
}
