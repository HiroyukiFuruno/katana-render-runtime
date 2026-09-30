use super::HtmlBrowserSession;
use crate::system::ProcessService;
use crate::{HtmlBrowserFrame, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};
use std::process::Child;
use std::time::{Duration, Instant};

type TestResult<T = ()> = Result<T, String>;
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
const REQUIREMENTS_DOCUMENT_NODE_COUNT: usize = 1_744;
const REQUIREMENTS_FIXED_NODE_COUNT: usize = 53;
const REQUIREMENTS_GENERATED_NODE_COUNT: usize =
    REQUIREMENTS_DOCUMENT_NODE_COUNT - REQUIREMENTS_FIXED_NODE_COUNT;
const REQUIREMENTS_GENERATED_NODE_CAPACITY_BYTES: usize = 48;
const REQUIREMENTS_FIXED_DOCUMENT: &str = r#"<style>
    html, body { margin: 0; }
    .app { display: flex; align-items: flex-start; }
    .sidebar { width: 224px; flex-shrink: 0; height: 100vh; background: #0f172a; color: #ffffff; position: sticky; top: 0; }
    .sidebar h1 { margin: 0; padding: 32px 24px; font-size: 24px; }
    main { flex: 1; padding: 48px; }
    section { min-height: 180px; border-bottom: 1px solid #cbd5e1; }
    #s15 { min-height: 360px; background: #e8c7ff; }
</style>
<div class="app">
    <aside class="sidebar"><h1>目次</h1></aside>
    <main>
    <section id="s01"><h2>1. 目的</h2><p>要件を記録します。</p></section>
    <section id="s02"><h2>2. 背景</h2><p>利用者の作業を支援します。</p></section>
    <section id="s03"><h2>3. 範囲</h2><p>HTML 文書を表示します。</p></section>
    <section id="s04"><h2>4. 用語</h2><p>文書、画面、項目を定義します。</p></section>
    <section id="s05"><h2>5. 利用者</h2><p>編集者と閲覧者を対象にします。</p></section>
    <section id="s06"><h2>6. 入力</h2><p>入力値を検証します。</p></section>
    <section id="s07"><h2>7. 出力</h2><p>結果を画面に表示します。</p></section>
    <section id="s08"><h2>8. 権限</h2><p>権限に応じて操作を制御します。</p></section>
    <section id="s09"><h2>9. 通知</h2><p>必要な通知を送信します。</p></section>
    <section id="s10"><h2>10. 監査</h2><p>操作履歴を保存します。</p></section>
    <section id="s11"><h2>11. 可用性</h2><p>継続して利用できるようにします。</p></section>
    <section id="s12"><h2>12. 性能</h2><p>画面を速やかに表示します。</p></section>
    <section id="s13"><h2>13. 保守</h2><p>更新を安全に実施します。</p></section>
    <section id="s14"><h2>14. 移行</h2><p>既存データを移行します。</p></section>
    <section id="s15"><h2>15. 受入基準</h2><p>fragment 表示と固定目次を検証します。</p></section>
    <section id="s16"><h2>16. 付録</h2><p>補足情報を記録します。</p></section>"#;

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
    let fixture = requirements_fixture();
    let source =
        HtmlBrowserSource::new(&fixture, REQUIREMENTS_ORIGIN).map_err(|error| error.to_string())?;
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

fn requirements_fixture() -> String {
    let mut fixture = String::from(REQUIREMENTS_FIXED_DOCUMENT);
    fixture.reserve(REQUIREMENTS_GENERATED_NODE_COUNT * REQUIREMENTS_GENERATED_NODE_CAPACITY_BYTES);
    for index in 0..REQUIREMENTS_GENERATED_NODE_COUNT {
        fixture.push_str("<p class=\"detail\">性能回帰フィクスチャ ");
        fixture.push_str(&index.to_string());
        fixture.push_str("</p>");
    }
    fixture.push_str("</main></div>");
    fixture
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
