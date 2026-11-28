use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};
use crate::system::ProcessService;
use crate::{HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};
use std::error::Error;

const CHILD_ENV: &str = "KRR_NATIVE_FONT_LIFETIME_TEST_CHILD";
const TEST_NAME: &str = "renderer::backends::html_browser::session::native_font_lifetime_tests::html_font_work_releases_temporary_coretext_objects_on_a_worker";
const COMPLETED: &str = "KRR_NATIVE_FONT_LIFETIME_COMPLETED";
const TEST_HTML: &str = "<!doctype html><meta charset=utf-8><p id=target>Previous 旧</p><script>document.querySelector('#target').textContent='Current 追加';</script>";
const VIEWPORT_WIDTH: u32 = 320;
const VIEWPORT_HEIGHT: u32 = 200;
const RGBA_CHANNELS: usize = 4;

#[test]
fn html_font_work_releases_temporary_coretext_objects_on_a_worker() -> Result<(), Box<dyn Error>> {
    if std::env::var_os(CHILD_ENV).is_some() {
        render_native_diagram_text()?;
        render_native_font_frames()?;
        println!("{COMPLETED}");
        return Ok(());
    }
    let output = ProcessService::create_command(std::env::current_exe()?)
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env(CHILD_ENV, "1")
        .env("OBJC_DEBUG_MISSING_POOLS", "YES")
        .output()?;
    assert!(
        output.status.success(),
        "font lifetime child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(COMPLETED));
    let warnings = String::from_utf8_lossy(&output.stderr);
    assert!(!warnings.contains("no pool in place"), "{warnings}");
    Ok(())
}

fn render_native_diagram_text() -> Result<(), Box<dyn Error>> {
    let script = DiagramRuntimeScript::borrowed(
        "diagram-font-lifetime.js",
        "globalThis.__katanaMeasureSvgTextMetrics('日本語 AV', 'Helvetica', 16, 400, false).advance",
    );
    let width: f64 = DiagramV8Runtime::render(&[script])?.parse()?;
    assert!(width > 0.0, "diagram text must have a positive advance");
    Ok(())
}

fn render_native_font_frames() -> Result<(), Box<dyn Error>> {
    let source = HtmlBrowserSource::new(TEST_HTML, "file:///native-font-lifetime.html")?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)?;
    let mut first = HtmlRuntime.open(source.clone(), viewport)?;
    let mut second = HtmlRuntime.open(source, viewport)?;
    let original = first.latest_frame().ok_or("first frame is missing")?;
    let concurrent = second.latest_frame().ok_or("second frame is missing")?;
    let (pixels, _) = original.pixels.as_chunks::<RGBA_CHANNELS>();
    assert!(pixels.iter().any(|pixel| pixel[0] < 128));
    assert!(original.pixels == concurrent.pixels);
    second.close()?;
    first.close()?;
    Ok(())
}
