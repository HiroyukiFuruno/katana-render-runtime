use super::super::RasterTarget;
use super::super::font::{
    current_process_rss_kib, html_font_memory_snapshot, html_rasterizer_options,
};
use resvg::usvg;

#[test]
fn cold_html_font_loading_keeps_system_font_bytes_out_of_the_process_cache()
-> Result<(), Box<dyn std::error::Error>> {
    /* WHY: process全体のRSSへ他の並列testの割当てを混ぜず、cacheがcoldな状態で測る。 */
    if std::env::var_os("KRR_TEST_COLD_HTML_FONT_CHILD").is_none() {
        return run_cold_html_font_memory_in_child();
    }
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="192" height="48"><rect width="192" height="48" fill="#fff"/><text x="4" y="36" font-family="Helvetica Neue, Arial, sans-serif" font-size="28" fill="#14532d">日本🙂</text></svg>"##;
    let rss_before = current_process_rss_kib();
    let memory = render_cold_html_svg(svg)?;
    let rss_after = current_process_rss_kib();

    assert_cold_html_font_memory(&memory);
    if let (Some(before), Some(after)) = (rss_before, rss_after) {
        assert_cold_html_rss(before, after, memory.owned_font_bytes);
    }
    Ok(())
}

fn run_cold_html_font_memory_in_child() -> Result<(), Box<dyn std::error::Error>> {
    let output = crate::system::ProcessService::create_command(std::env::current_exe()?)
        .args([
            "markdown::svg_rasterize::tests::font_memory_tests::cold_html_font_loading_keeps_system_font_bytes_out_of_the_process_cache",
            "--exact", "--nocapture", "--test-threads=1",
        ])
        .env("KRR_TEST_COLD_HTML_FONT_CHILD", "1")
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cold font child failed: {stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("test result: ok. 1 passed;"),
        "cold font child did not run exactly one test: {stdout}"
    );
    Ok(())
}

fn render_cold_html_svg(svg: &str) -> Result<super::super::font::HtmlFontMemorySnapshot, String> {
    let options = html_rasterizer_options(svg);
    let memory = html_font_memory_snapshot(&options.fontdb);
    let tree = usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
    let image = RasterTarget::new(tree.size(), 1.0)
        .render(&tree)
        .map_err(|error| error.to_string())?;
    drop(tree);
    drop(options);
    drop(image);
    Ok(memory)
}

fn assert_cold_html_font_memory(memory: &super::super::font::HtmlFontMemorySnapshot) {
    assert!(
        memory.owned_font_bytes <= 2 * 1024 * 1024,
        "owned font bytes: {}",
        memory.owned_font_bytes
    );
    assert!(
        memory.system_font_file_faces > 0,
        "CJK/emoji fallback must be registered as file-backed sources"
    );
    assert!(
        memory.total_faces <= 64,
        "lazy HTML database unexpectedly contains {} faces",
        memory.total_faces
    );
}

fn assert_cold_html_rss(before: usize, after: usize, owned_font_bytes: usize) {
    let delta = after.saturating_sub(before);
    eprintln!(
        "cold HTML font cache: owned={} B, rss_before={before} KiB, rss_after={after} KiB, delta={delta} KiB",
        owned_font_bytes
    );
    assert!(
        delta < 64 * 1024,
        "cold HTML RSS increased by {delta} KiB after file-backed font loading"
    );
}
