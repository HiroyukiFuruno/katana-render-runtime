use super::{TestResult, capture_frame_pixels, find_svg, session_frame_pixels};
use crate::{HtmlBrowserError, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport};
use xmltree::Element;

const VIEWPORT_SIZE: u32 = 16;

#[test]
fn invalid_viewport_is_reported() -> TestResult {
    assert_capture_error(
        "https://example.test/invalid.svg",
        0,
        HtmlBrowserError::InvalidViewport,
    )
}

#[test]
fn invalid_source_errors_are_preserved() -> TestResult {
    let invalid = "not a URL";
    let unsupported = "data:text/plain,svg";
    for (url, expected) in [
        (
            invalid,
            HtmlBrowserError::InvalidOrigin {
                origin: invalid.into(),
            },
        ),
        (
            unsupported,
            HtmlBrowserError::UnsupportedOriginScheme {
                origin: unsupported.into(),
            },
        ),
    ] {
        assert_capture_error(url, VIEWPORT_SIZE, expected)?;
    }
    Ok(())
}

fn assert_capture_error(url: &str, width: u32, expected: HtmlBrowserError) -> TestResult {
    let actual = capture_frame_pixels("<svg/>", url, width, 1)
        .err()
        .ok_or("invalid capture was accepted")?;
    assert_eq!(actual.downcast_ref::<HtmlBrowserError>(), Some(&expected));
    assert_eq!(actual.to_string(), expected.to_string());
    Ok(())
}

#[test]
fn svg_search_skips_text_comments_and_non_svg_subtrees() -> TestResult {
    for (xml, expected) in [
        (
            "<root>text<!-- comment --><section>text</section></root>",
            None,
        ),
        (
            "<root>text<!-- comment --><section><svg id=\"nested\"/></section><svg id=\"later\"/></root>",
            Some("nested"),
        ),
    ] {
        let root = Element::parse(xml.as_bytes())?;
        let found = find_svg(&root);
        assert_eq!(found.map(|svg| svg.name.as_str()), expected.map(|_| "svg"));
        assert_eq!(
            found
                .and_then(|svg| svg.attributes.get("id"))
                .map(String::as_str),
            expected
        );
    }
    Ok(())
}

#[test]
fn closed_session_has_no_frame() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\"/>",
        "https://example.test/closed.svg",
    )?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_SIZE, VIEWPORT_SIZE, 1.0)?;
    let mut session = HtmlBrowserSession::new(source, viewport)?;
    session.close()?;

    let error = session_frame_pixels(&session)
        .err()
        .ok_or("closed session retained a frame")?;
    assert_eq!(error.to_string(), "missing SVG frame");
    Ok(())
}
