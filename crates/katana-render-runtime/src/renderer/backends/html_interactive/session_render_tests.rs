use super::super::tests::test_error;
use super::HtmlInteractiveSession;
use crate::renderer::backends::html_browser::{HtmlBrowserSource, HtmlBrowserViewport};

#[test]
fn opening_a_fragment_propagates_a_failure_during_the_first_layout() -> Result<(), String> {
    let html = r#"<div id="target">observer target</div><script>
        new IntersectionObserver(() => {
            throw new Error("first layout observer failed");
        }).observe(document.getElementById("target"));
        </script>"#;
    let source = HtmlBrowserSource::new(html, "https://example.test/initial.html#target")
        .map_err(test_error)?;
    let viewport = HtmlBrowserViewport::new(320, 200, 1.0).map_err(test_error)?;
    let error = HtmlInteractiveSession::start(source, viewport)
        .err()
        .ok_or("expected the first layout to fail")?;
    assert!(error.to_string().contains("first layout observer failed"));
    Ok(())
}
