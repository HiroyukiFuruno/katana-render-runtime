use super::support::{LocalFixture, TestResult, assert_frame_contains, to_string, viewport};
use crate::renderer::backends::html_browser::HTML_BROWSER_MAX_SOURCE_BYTES;
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_runtime::StaticHtmlRuntime;
use crate::renderer::backends::html_subresources::HtmlSubresourceLoader;
use crate::renderer::backends::html_subresources::iframe::required_html_root;
use crate::renderer::backends::{HtmlBrowserSource, HtmlRuntime};
use markup5ever_rcdom::RcDom;
use std::net::TcpListener;

const SLIDE_SOURCE: &str = r#"<style>
html, body { margin: 0; }
#btn-next { width: 160px; height: 100px; background: #ef4444; }
</style>
<button id=btn-next>Waiting</button>
<script>
document.getElementById('btn-next').addEventListener('click', function () {
  this.textContent = 'Loaded';
  this.style.backgroundColor = '#35a853';
});
</script>"#;

const SLIDE_WRAPPER: &str = r#"<style>html, body, iframe { width: 100%; height: 100%; margin: 0; border: 0; }</style>
<iframe id=deck src=source.html><span>Fallback must be replaced</span></iframe>
<script>
document.getElementById('deck').addEventListener('load', function () {
  const requested = Number(new URLSearchParams(location.search).get('slide') || '1');
  const next = this.contentDocument.getElementById('btn-next');
  for (let index = 1; index < requested; index += 1) next.click();
});
</script>"#;

#[test]
fn cross_origin_network_iframe_sources_are_not_fetched() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(to_string)?;
    let address = listener.local_addr().map_err(to_string)?;
    listener.set_nonblocking(true).map_err(to_string)?;
    let source = HtmlBrowserSource::new(
        format!("<iframe src=http://{address}/frame.html></iframe><p>Visible</p>"),
        "https://example.test/site/index.html",
    )
    .map_err(to_string)?;

    let _session = HtmlRuntime.open(source, viewport()?).map_err(to_string)?;
    let error = match listener.accept() {
        Err(error) => error,
        Ok(_) => return Err("iframe must not be fetched".to_string()),
    };

    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    Ok(())
}

#[test]
fn same_root_local_iframe_joins_dom_css_javascript_and_load_event() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = slide_wrapper_source(&fixture)?;
    let rendered = load_rendered(&source)?;

    assert!(!rendered.contains("Fallback must be replaced"));
    let session = HtmlRuntime.open(source, viewport()?).map_err(to_string)?;
    let frame = session.latest_frame().ok_or("frame is missing")?;
    assert_frame_contains(&frame.pixels, [53, 168, 83]);
    assert!(
        !frame.pixels.as_chunks::<4>().0.iter().any(|pixel| {
            pixel[0] == 239 && pixel[1] == 68 && pixel[2] == 68 && pixel[3] == 255
        })
    );
    Ok(())
}

fn slide_wrapper_source(fixture: &LocalFixture) -> TestResult<HtmlBrowserSource> {
    std::fs::write(fixture.root.join("source.html"), SLIDE_SOURCE).map_err(to_string)?;
    let origin = format!("{}?slide=2", fixture.origin()?);
    HtmlBrowserSource::new(SLIDE_WRAPPER, origin).map_err(to_string)
}

#[test]
fn iframe_scripts_do_not_shift_parent_body_onload_parser_position() -> TestResult {
    let fixture = LocalFixture::new()?;
    std::fs::write(
        fixture.root.join("frame.html"),
        r#"<script>window.onload = () => { document.getElementById('status').textContent = 'frame'; };</script>"#,
    )
    .map_err(to_string)?;
    let source = fixture.source(
        r#"<p id=status>initial</p><iframe src=frame.html></iframe><script>window.onload = () => { document.getElementById('status').textContent = 'first'; };</script><body onload="document.getElementById('status').textContent = 'second'">"#,
    )?;
    let session = StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(to_string)?;
    let snapshot = session.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r#"<p id="status">second</p>"#),
        "{snapshot}"
    );
    Ok(())
}

#[test]
fn later_duplicate_body_onload_replays_after_foster_parented_iframe_scripts() -> TestResult {
    let fixture = LocalFixture::new()?;
    std::fs::write(
        fixture.root.join("frame.html"),
        "<script>document.body.removeAttribute('onload'); document.getElementById('status').textContent += 'frame|';</script>",
    )
    .map_err(to_string)?;
    let source = fixture.source(
        r#"<body onload="document.getElementById('status').textContent += 'first'"><p id=status></p><template><iframe src=ignored.html></iframe></template><table><iframe src=frame.html></iframe><body onload="document.getElementById('status').textContent += 'second'"><script>document.getElementById('status').textContent += 'parent|';</script></table>"#,
    )?;
    let snapshot = StaticHtmlRuntime
        .start_interactive(&source)
        .and_then(|session| session.snapshot())
        .map_err(to_string)?;

    assert!(
        snapshot.contains(r#"<p id="status">frame|parent|second</p>"#),
        "{snapshot}"
    );
    Ok(())
}

#[test]
fn iframe_after_table_select_transition_executes_before_parent_script_and_load() -> TestResult {
    let fixture = LocalFixture::new()?;
    std::fs::write(
        fixture.root.join("frame.html"),
        r#"<script>document.getElementById('status').textContent += 'frame|';</script>"#,
    )
    .map_err(to_string)?;
    let source = fixture.source(
        r#"<p id=status></p><table><select><tr><td><iframe id=real src=frame.html></iframe></td></tr></table><script>document.getElementById('status').textContent += 'after|';</script><body onload="document.getElementById('status').textContent += 'load|'">"#,
    )?;
    let mut document = HtmlDocument::parse(&source.raw_html);
    assert!(document.get_element_by_id("real").is_some());
    let resources = HtmlSubresourceLoader::new(&source).load(&mut document);
    assert!(
        resources
            .scripts
            .iter()
            .any(|script| script.source().contains("'frame|'"))
    );
    let session = StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(to_string)?;
    let snapshot = session.snapshot().map_err(to_string)?;

    assert!(
        snapshot.contains(r#"<p id="status">frame|after|load|</p>"#),
        "{snapshot}"
    );
    Ok(())
}

#[test]
fn local_iframe_escape_is_blocked_without_replacing_the_main_document() -> TestResult {
    let fixture = LocalFixture::new()?;
    let (outside, source) = escaped_iframe_source(&fixture)?;
    let rendered = load_rendered(&source)?;
    let session = HtmlRuntime.open(source, viewport()?).map_err(to_string)?;
    let frame = session.latest_frame().ok_or("frame is missing")?;
    std::fs::remove_file(outside).map_err(to_string)?;

    assert!(rendered.contains("data-krr-frame-error"));
    assert!(rendered.contains("Main"));
    assert!(!rendered.contains("Outside"));
    assert_eq!(frame.pixels.len(), 320 * 240 * 4);
    Ok(())
}

fn escaped_iframe_source(
    fixture: &LocalFixture,
) -> TestResult<(std::path::PathBuf, HtmlBrowserSource)> {
    let outside = fixture
        .root
        .parent()
        .ok_or("fixture parent is missing")?
        .join(format!("krr-outside-frame-{}.html", std::process::id()));
    std::fs::write(
        &outside,
        "<div style='width:100px;height:100px;background:#ef4444'>Outside</div>",
    )
    .map_err(to_string)?;
    let reference = format!(
        "../{}",
        outside
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("outside filename")?
    );
    let source = fixture.source(&format!(
        "<iframe src='{reference}'></iframe><main style='width:100px;height:100px;background:#35a853'>Main</main>"
    ))?;
    Ok((outside, source))
}

#[test]
fn local_iframe_cycle_is_bounded_and_duplicate_frames_remain_renderable() -> TestResult {
    let fixture = LocalFixture::new()?;
    write_cycle_fixtures(&fixture)?;
    let source = fixture.source(
        "<iframe src=cycle.html></iframe><iframe src=shared.html></iframe><iframe src=shared.html></iframe>",
    )?;
    let rendered = load_rendered(&source)?;
    let session = HtmlRuntime.open(source, viewport()?).map_err(to_string)?;
    let frame = session.latest_frame().ok_or("frame is missing")?;

    assert_eq!(rendered.matches("data-krr-local-frame").count(), 3);
    assert!(rendered.contains("data-krr-frame-error"));
    assert!(rendered.contains("Cycle"));
    assert!(rendered.contains("Shared"));
    assert_eq!(frame.pixels.len(), 320 * 240 * 4);
    Ok(())
}

fn write_cycle_fixtures(fixture: &LocalFixture) -> TestResult {
    std::fs::write(
        fixture.root.join("cycle.html"),
        "<iframe src=cycle.html></iframe><div style='width:40px;height:40px;background:#35a853'>Cycle</div>",
    )
    .map_err(to_string)?;
    std::fs::write(
        fixture.root.join("shared.html"),
        "<div style='width:40px;height:40px;background:#3182ce'>Shared</div>",
    )
    .map_err(to_string)?;
    Ok(())
}

#[test]
fn missing_local_iframe_renders_an_actionable_in_frame_diagnostic() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = fixture.source(
        "<iframe style='width:300px;height:180px' src='missing&amp;frame.html'></iframe>",
    )?;
    let mut document = HtmlDocument::parse(&source.raw_html);
    HtmlSubresourceLoader::new(&source).load(&mut document);
    let rendered = document.render();

    assert!(rendered.contains("data-krr-frame-error"));
    assert!(rendered.contains("HTML iframe could not be loaded."));
    assert!(rendered.contains("missing&amp;frame.html"));

    let session = HtmlRuntime.open(source, viewport()?).map_err(to_string)?;
    let frame = session.latest_frame().ok_or("frame is missing")?;
    assert_frame_contains(&frame.pixels, [220, 38, 38]);
    Ok(())
}

#[test]
fn source_less_iframe_keeps_fallback_content_without_a_diagnostic() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = fixture.source("<iframe><strong>Fallback content</strong></iframe>")?;
    let mut document = HtmlDocument::parse(&source.raw_html);
    HtmlSubresourceLoader::new(&source).load(&mut document);
    let rendered = document.render();

    assert!(rendered.contains("Fallback content"));
    assert!(!rendered.contains("data-krr-frame-error"));
    Ok(())
}

#[test]
fn nested_directory_iframe_is_rejected_with_an_in_frame_diagnostic() -> TestResult {
    let fixture = LocalFixture::new()?;
    let nested = fixture.root.join("nested");
    std::fs::create_dir(&nested).map_err(to_string)?;
    std::fs::write(nested.join("frame.html"), "<p>Nested</p>").map_err(to_string)?;
    let source = fixture.source("<iframe src=nested/frame.html></iframe>")?;
    let mut document = HtmlDocument::parse(&source.raw_html);
    HtmlSubresourceLoader::new(&source).load(&mut document);
    let rendered = document.render();

    assert!(rendered.contains("data-krr-frame-error"));
    assert!(rendered.contains("must be in the document directory"));
    Ok(())
}

#[test]
fn local_iframe_depth_and_document_limits_render_diagnostics() -> TestResult {
    let fixture = LocalFixture::new()?;
    for depth in 0..8 {
        std::fs::write(
            fixture.root.join(format!("depth-{depth}.html")),
            format!("<iframe src=depth-{}.html></iframe>", depth + 1),
        )
        .map_err(to_string)?;
    }
    std::fs::write(fixture.root.join("shared.html"), "<p>Shared</p>").map_err(to_string)?;

    let repeated = (0..17)
        .map(|_| "<iframe src=shared.html></iframe>")
        .collect::<String>();
    let source = fixture.source(&format!("<iframe src=depth-0.html></iframe>{repeated}"))?;
    let mut document = HtmlDocument::parse(&source.raw_html);
    HtmlSubresourceLoader::new(&source).load(&mut document);
    let rendered = document.render();

    assert!(rendered.contains("iframe nesting exceeds 8"));
    assert!(rendered.contains("iframe count exceeds 16"));
    Ok(())
}

#[test]
fn iframe_root_and_source_errors_are_rendered_as_diagnostics() -> TestResult {
    let empty = RcDom::default();
    assert!(matches!(
        required_html_root(&empty.document, "empty iframe"),
        Err(message) if message == "empty iframe has no html root"
    ));

    let fixture = LocalFixture::new()?;
    std::fs::write(
        fixture.root.join("oversized.html"),
        vec![b'x'; HTML_BROWSER_MAX_SOURCE_BYTES + 1],
    )
    .map_err(to_string)?;
    let source = fixture.source("<iframe src=oversized.html></iframe>")?;
    let rendered = load_rendered(&source)?;
    assert!(rendered.contains("data-krr-frame-error"));
    assert!(rendered.contains("browser source exceeds"));
    Ok(())
}

#[test]
fn invalid_iframe_reference_is_reported_without_runtime_failure() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = fixture.source("<iframe src='http://['></iframe>")?;
    let rendered = load_rendered(&source)?;

    assert!(rendered.contains("data-krr-frame-error"));
    assert!(rendered.contains("resource URL is invalid"));
    Ok(())
}

fn load_rendered(source: &HtmlBrowserSource) -> TestResult<String> {
    let mut document = HtmlDocument::parse(&source.raw_html);
    HtmlSubresourceLoader::new(source).load(&mut document);
    Ok(document.render())
}
