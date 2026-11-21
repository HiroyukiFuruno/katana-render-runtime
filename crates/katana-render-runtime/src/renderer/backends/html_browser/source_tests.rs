use super::*;

#[test]
fn source_origin_and_size_validation_are_strict() {
    let file_url = url::Url::from_file_path(std::env::temp_dir().join("krr-source-origin.html"))
        .map_err(|()| "file URL conversion failed")
        .and_then(|url| HtmlBrowserOrigin::parse(url.to_string()).map_err(|_| "parse"));
    assert!(file_url.is_ok());
    assert!(HtmlBrowserOrigin::parse("http://example.test/").is_ok());
    assert!(HtmlBrowserOrigin::parse("https://example.test/").is_ok());
    assert!(matches!(
        HtmlBrowserOrigin::parse("not a url"),
        Err(HtmlBrowserError::InvalidOrigin { .. })
    ));
    assert!(matches!(
        HtmlBrowserOrigin::parse("data:text/html,hello"),
        Err(HtmlBrowserError::UnsupportedOriginScheme { .. })
    ));
    assert!(matches!(
        HtmlBrowserSource::new(
            "x".repeat(HTML_BROWSER_MAX_SOURCE_BYTES + 1),
            "https://example.test/"
        ),
        Err(HtmlBrowserError::SourceTooLarge { .. })
    ));
}

#[test]
fn serialized_origins_keep_url_validation() -> Result<(), String> {
    let origin: Result<HtmlBrowserOrigin, _> = serde_json::from_str("\"not a url\"");
    assert!(origin.is_err());
    assert_eq!(
        HtmlBrowserOrigin::parse("DATA:text/html,hello"),
        Err(HtmlBrowserError::UnsupportedOriginScheme {
            origin: "DATA:text/html,hello".to_owned(),
        }),
    );
    let mut source: HtmlBrowserSource =
        serde_json::from_str(r#"{"raw_html":"ok","origin":"DATA:text/html,hello"}"#)
            .map_err(|error| error.to_string())?;
    assert_eq!(
        source.validate(),
        Err(HtmlBrowserError::UnsupportedOriginScheme {
            origin: "data:text/html,hello".to_owned(),
        }),
    );
    source.raw_html = "x".repeat(HTML_BROWSER_MAX_SOURCE_BYTES + 1);
    assert_eq!(
        source.validate(),
        Err(HtmlBrowserError::SourceTooLarge {
            actual_bytes: HTML_BROWSER_MAX_SOURCE_BYTES + 1,
            max_bytes: HTML_BROWSER_MAX_SOURCE_BYTES,
        }),
    );
    Ok(())
}

#[test]
fn viewport_validation_rejects_invalid_numbers() {
    assert!(matches!(
        HtmlBrowserViewport::new(0, 1, 1.0),
        Err(HtmlBrowserError::InvalidViewport)
    ));
    assert!(matches!(
        HtmlBrowserViewport::new(1, 1, f32::NAN),
        Err(HtmlBrowserError::InvalidDeviceScaleFactor)
    ));
}

#[test]
fn pointer_and_scroll_input_validation_rejects_invalid_numbers() {
    assert!(matches!(
        HtmlBrowserInput::PointerMove {
            x: f32::INFINITY,
            y: 0.0
        }
        .validate(),
        Err(HtmlBrowserError::InvalidInputCoordinates)
    ));
    assert!(matches!(
        HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: f32::NEG_INFINITY
        }
        .validate(),
        Err(HtmlBrowserError::InvalidInputCoordinates)
    ));
}

#[test]
fn pointer_input_variants_accept_finite_coordinates() {
    assert!(
        HtmlBrowserInput::PointerDown {
            x: 1.0,
            y: 2.0,
            button: 0
        }
        .validate()
        .is_ok()
    );
    assert!(
        HtmlBrowserInput::PointerUp {
            x: 1.0,
            y: 2.0,
            button: 0
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn keyboard_and_text_input_variants_do_not_require_coordinates() {
    assert!(
        HtmlBrowserInput::KeyDown {
            key: "Enter".to_string()
        }
        .validate()
        .is_ok()
    );
    assert!(
        HtmlBrowserInput::KeyUp {
            key: "Enter".to_string()
        }
        .validate()
        .is_ok()
    );
    assert!(
        HtmlBrowserInput::Text {
            text: "ok".to_string()
        }
        .validate()
        .is_ok()
    );
    assert!(HtmlBrowserInput::Focus { focused: true }.validate().is_ok());
}

#[test]
fn navigation_validates_source_and_target_urls() -> Result<(), String> {
    let source = HtmlBrowserSource::new("<p>ok</p>", "https://example.test/")
        .map_err(|error| error.to_string())?;
    source.validate().map_err(|error| error.to_string())?;
    let navigation = HtmlBrowserNavigation::new(source).map_err(|error| error.to_string())?;
    assert_eq!(navigation.source.raw_html, "<p>ok</p>");
    assert!(matches!(
        HtmlBrowserNavigationEvent::new("mailto:user@example.test"),
        Err(HtmlBrowserError::UnsupportedOriginScheme { .. })
    ));
    let mut invalid = HtmlBrowserSource::new("<p>ok</p>", "https://example.test/")
        .map_err(|error| error.to_string())?;
    invalid.raw_html = "x".repeat(HTML_BROWSER_MAX_SOURCE_BYTES + 1);
    assert!(matches!(
        HtmlBrowserNavigation::new(invalid),
        Err(HtmlBrowserError::SourceTooLarge { .. })
    ));
    Ok(())
}
