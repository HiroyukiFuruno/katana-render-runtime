use super::support::{LocalFixture, TestResult, must_source, to_string, viewport};
use crate::renderer::backends::HtmlBrowserSource;
use crate::renderer::backends::html_subresources::{HtmlSubresourceLoader, HtmlSubresourcePolicy};
use url::Url;

#[test]
fn policy_allows_network_subresources_and_rejects_local_escape() -> TestResult {
    let fixture = LocalFixture::new()?;
    let policy = HtmlSubresourcePolicy::from_source(&fixture.source("<p>ok</p>")?);

    assert_direct_reference_policy(&policy);
    assert_navigation_policy(&policy);
    assert_rejects_non_local_file_navigation(&policy)?;
    assert_page_survives_blocked_resources(&fixture)?;
    Ok(())
}

#[test]
fn https_documents_reject_mixed_content_but_allow_cross_origin_https() -> TestResult {
    let source =
        HtmlBrowserSource::new("<p>ok</p>", "https://docs.example/page.html").map_err(to_string)?;
    let policy = HtmlSubresourcePolicy::from_source(&source);

    assert!(
        policy
            .resolve_subresource("https://cdn.example/style.css")
            .is_ok()
    );
    assert!(
        policy
            .resolve_subresource("http://cdn.example/style.css")
            .is_err()
    );
    assert!(
        policy
            .resolve_subresource("https://user@cdn.example/style.css")
            .is_err()
    );
    assert!(
        policy
            .resolve_subresource("https://user:pass@cdn.example/style.css")
            .is_err()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn policy_rejects_symlink_escape() -> TestResult {
    let fixture = LocalFixture::new()?;
    let outside = fixture
        .root
        .parent()
        .ok_or("fixture parent is missing")?
        .join("outside.css");
    std::fs::write(&outside, "#target { color: red; }").map_err(to_string)?;
    std::os::unix::fs::symlink(&outside, fixture.root.join("escape.css")).map_err(to_string)?;

    let policy = HtmlSubresourcePolicy::from_source(&fixture.source("<p>ok</p>")?);
    assert!(policy.resolve_subresource("escape.css").is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn policy_rejects_navigation_through_a_symlink_escape() -> TestResult {
    let fixture = LocalFixture::new()?;
    let outside = fixture.root.with_extension("outside");
    std::fs::create_dir_all(&outside).map_err(to_string)?;
    std::os::unix::fs::symlink(&outside, fixture.root.join("escape")).map_err(to_string)?;

    let policy = HtmlSubresourcePolicy::from_source(&fixture.source("<p>ok</p>")?);
    let rejected = policy.resolve_navigation("escape/next.html").is_err();
    std::fs::remove_dir_all(&outside).map_err(to_string)?;
    assert!(rejected);
    Ok(())
}

fn assert_rejects_non_local_file_navigation(policy: &HtmlSubresourcePolicy) -> TestResult {
    let url = Url::parse("file://example.test/outside.html").map_err(to_string)?;

    assert!(!policy.allows_local_navigation(&url));
    Ok(())
}

fn assert_direct_reference_policy(policy: &HtmlSubresourcePolicy) {
    assert!(policy.resolve_subresource("../outside.css").is_err());
    assert!(policy.resolve_subresource("/absolute.css").is_err());
    assert!(policy.resolve_subresource("http://[").is_err());
    assert!(
        policy
            .resolve_subresource("https://other.example/style.css")
            .is_ok()
    );
    assert!(
        policy
            .resolve_subresource("http://other.example/style.css")
            .is_ok()
    );
    assert!(
        policy
            .resolve_subresource("data:text/css,body%7B%7D")
            .is_ok()
    );
}

fn assert_navigation_policy(policy: &HtmlSubresourcePolicy) {
    assert!(
        policy
            .resolve_navigation("https://other.example/next.html")
            .is_err()
    );
    assert!(policy.resolve_navigation("next.html").is_ok());
    assert!(policy.resolve_navigation("guide/next.html").is_ok());
    assert!(policy.resolve_navigation("../outside.html").is_err());
}

#[test]
fn invalid_navigation_inputs_are_rejected() -> TestResult {
    let source =
        HtmlBrowserSource::new("<p>ok</p>", "https://docs.example/page.html").map_err(to_string)?;
    let policy = HtmlSubresourcePolicy::from_source(&source);

    assert!(policy.resolve_navigation("http://[").is_err());
    assert!(
        policy
            .resolve_subresource("https://docs.example.com/../")
            .is_ok()
    );
    assert!(policy.resolve_navigation("about:blank").is_err());
    Ok(())
}

#[test]
fn nonexistent_file_origin_allows_only_same_document_fragment() -> TestResult {
    let origin = "file:///__krr_missing_fragment_origin__/index.html";
    assert!(!std::path::Path::new("/__krr_missing_fragment_origin__").exists());
    let source = HtmlBrowserSource::new("<p>ok</p>", origin).map_err(to_string)?;
    let policy = HtmlSubresourcePolicy::from_source(&source);
    for fragment in ["#a", "#b", "#c"] {
        let url = policy.resolve_navigation(fragment).map_err(to_string)?;
        assert_eq!(url.as_str(), format!("{origin}{fragment}"));
    }
    for reference in [
        "index.html",
        "other.html#a",
        "../outside.html#a",
        "file:///__krr_missing_fragment_origin__/index.html#a",
        "https://other.example/index.html#a",
        "about:blank#a",
    ] {
        assert!(policy.resolve_navigation(reference).is_err(), "{reference}");
    }
    Ok(())
}

fn assert_page_survives_blocked_resources(fixture: &LocalFixture) -> TestResult {
    let source = must_source(
        fixture,
        "<link rel=stylesheet href=style.css>\
         <link rel=stylesheet href=../outside.css>\
         <script src=file:///outside.js></script>\
         <img src=ftp://example.test/image.png>\
         <iframe src=https://other.example/frame.html></iframe>\
         <div id=styled>Visible</div>",
    );
    let mut document =
        crate::renderer::backends::html_document::HtmlDocument::parse(&source.raw_html);
    let resources = HtmlSubresourceLoader::new(&source).load(&mut document);
    let frame = crate::renderer::backends::HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;

    assert_eq!(frame.viewport.width, 320);
    assert_eq!(frame.viewport.height, 240);
    assert_eq!(frame.pixels.len(), 320 * 240 * 4);
    assert!(resources.stylesheets.contains_key("style.css"));
    assert!(document.render().contains("Visible"));
    Ok(())
}
