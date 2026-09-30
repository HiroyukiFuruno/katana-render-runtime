use super::{
    MAX_BASE64_DATA_URL_BYTES, MAX_PERCENT_ENCODED_BASE64_DATA_URL_BYTES, MAX_SUBRESOURCE_BYTES,
    check_data_url_size, decode_data_url, decode_forgiving_base64, load_image_data_url, load_text,
    normalize_forgiving_base64,
};
use base64::Engine as _;
use url::Url;

#[test]
fn text_data_urls_support_percent_and_base64_payloads() {
    with_url("data:text/plain,hello%20world", |percent| {
        assert_eq!(load_text(percent).ok().as_deref(), Some("hello world"));
    });
    with_url("data:text/plain,%EF%BB%BF%7B%22ready%22%3Atrue%7D", |bom| {
        assert_eq!(load_text(bom).ok().as_deref(), Some("{\"ready\":true}"));
    });
    with_url("data:text/plain;base64,aGVsbG8=", |base64| {
        assert_eq!(load_text(base64).ok().as_deref(), Some("hello"));
    });
}

#[test]
fn image_data_urls_require_a_decodable_image_payload() {
    with_url(
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
        |image| {
            assert_eq!(
                load_image_data_url(image).ok().as_deref(),
                Some(image.as_str())
            );
        },
    );
    with_url("data:image/png;base64,!", |url| {
        assert!(load_image_data_url(url).is_err());
    });
    with_url("data:image/png;base64,AAAA", |url| {
        assert!(load_image_data_url(url).is_err());
    });
    with_url("data:image/jpeg;base64,/9j/2Q==", |url| {
        assert!(load_image_data_url(url).is_err());
    });
    with_url("data:text/plain;base64,aGVsbG8=", |url| {
        assert!(load_image_data_url(url).is_err());
    });
}

#[test]
fn unpadded_base64_image_data_urls_are_decoded_and_validated() {
    let source = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII";
    with_url(source, |url| {
        assert_eq!(load_image_data_url(url).ok().as_deref(), Some(source));
    });
    with_url("data:image/png;base64,AAAA", |url| {
        assert!(load_image_data_url(url).is_err());
    });
    with_url(
        "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4",
        |url| {
            assert!(load_image_data_url(url).is_ok());
        },
    );
}

#[test]
fn base64_padding_is_supplemented_only_when_the_payload_is_unpadded() {
    assert_eq!(decode_forgiving_base64(b"AA"), Ok(vec![0]));
    assert_eq!(decode_forgiving_base64(b"AAA"), Ok(vec![0, 0]));
    assert_eq!(decode_forgiving_base64(b"AA=="), Ok(vec![0]));
    assert!(decode_forgiving_base64(b"AA=").is_err());
    assert!(decode_forgiving_base64(b"AAAA=").is_err());
}

#[test]
fn forgiving_base64_size_is_measured_after_ascii_whitespace_is_removed() {
    let payload = b" A\tA\nA\r";
    assert_eq!(normalize_forgiving_base64(payload), b"AAA");
    assert_eq!(decode_forgiving_base64(payload), Ok(vec![0, 0]));
}

#[test]
fn percent_encoded_base64_image_data_urls_preserve_the_source_and_decode() {
    let source = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII%3D";
    with_url(source, |url| {
        assert_eq!(load_image_data_url(url).ok().as_deref(), Some(source));
    });
}

#[test]
fn percent_decoded_ascii_whitespace_is_ignored_in_base64_payloads() {
    let encoded = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let source = format!(
        "data:image/png;base64,{}%20{}%09{}%0A{}%0C{}%0D{}",
        &encoded[..8],
        &encoded[8..16],
        &encoded[16..24],
        &encoded[24..32],
        &encoded[32..40],
        &encoded[40..],
    );
    with_url(&source, |url| {
        assert_eq!(
            load_image_data_url(url).ok().as_deref(),
            Some(source.as_str())
        );
    });
}

#[test]
fn noncanonical_base64_trailing_bits_are_ignored_for_images() {
    let source = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYIJ=";
    with_url(source, |url| {
        assert_eq!(load_image_data_url(url).ok().as_deref(), Some(source));
    });
}

#[test]
fn base64_metadata_must_be_the_final_flag_for_image_data_urls() {
    let source = "data:image/svg+xml;base64;charset=utf-8,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%2F%3E";
    with_url(source, |url| {
        assert_eq!(load_image_data_url(url).ok().as_deref(), Some(source));
    });
}

#[test]
fn image_data_url_fragment_is_excluded_from_the_decoded_payload() {
    let source = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=#view";
    with_url(source, |url| {
        assert_eq!(load_image_data_url(url).ok().as_deref(), Some(source));
    });
}

#[test]
fn malformed_text_data_urls_are_rejected() {
    with_url("data:text/plain", |url| assert!(load_text(url).is_err()));
    with_url("data:text/plain;base64,!", |url| {
        assert!(load_text(url).is_err());
    });
    with_url("data:application/octet-stream;base64,/w==", |url| {
        assert!(load_text(url).is_err());
    });
}

#[test]
fn oversized_data_url_payloads_are_rejected_before_decoding() {
    let oversized_payload = "A".repeat(MAX_PERCENT_ENCODED_BASE64_DATA_URL_BYTES as usize + 1);
    let source = format!("data:image/png;base64,{oversized_payload}");
    with_url(&source, |url| {
        assert!(matches!(
            decode_data_url(url),
            Err(error) if error.contains("encoded payload exceeds")
        ));
    });
}

#[test]
fn percent_decoded_base64_payloads_are_limited_before_base64_decoding() {
    let oversized_payload = "A".repeat(MAX_BASE64_DATA_URL_BYTES as usize + 1);
    let source = format!("data:text/plain;base64,{oversized_payload}");
    with_url(&source, |url| {
        assert!(matches!(
            decode_data_url(url),
            Err(error) if error.contains("base64 encoded payload exceeds")
        ));
    });
}

#[test]
fn percent_encoded_payloads_are_limited_after_decoding() {
    let payload = "%41".repeat(MAX_SUBRESOURCE_BYTES as usize);
    let source = format!("data:text/plain,{payload}");
    with_url(&source, |url| {
        assert_eq!(
            decode_data_url(url).ok().as_deref().map(<[u8]>::len),
            Some(MAX_SUBRESOURCE_BYTES as usize)
        );
    });
}

#[test]
fn decoded_data_url_size_uses_the_subresource_limit() {
    assert!(matches!(
        check_data_url_size(MAX_SUBRESOURCE_BYTES as usize + 1, MAX_SUBRESOURCE_BYTES, "decoded"),
        Err(error) if error.contains("decoded payload exceeds")
    ));
    assert!(
        check_data_url_size(
            MAX_SUBRESOURCE_BYTES as usize,
            MAX_SUBRESOURCE_BYTES,
            "decoded"
        )
        .is_ok()
    );
}

#[test]
fn encoded_data_url_size_allows_base64_expansion_at_the_decoded_limit() {
    assert!(
        check_data_url_size(
            MAX_BASE64_DATA_URL_BYTES as usize,
            MAX_BASE64_DATA_URL_BYTES,
            "encoded"
        )
        .is_ok()
    );
    assert!(matches!(
        check_data_url_size(
            MAX_BASE64_DATA_URL_BYTES as usize + 1,
            MAX_BASE64_DATA_URL_BYTES,
            "encoded"
        ),
        Err(error) if error.contains("encoded payload exceeds")
    ));
}

#[test]
fn base64_data_url_at_the_decoded_limit_is_accepted() {
    let bytes = vec![0_u8; MAX_SUBRESOURCE_BYTES as usize];
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let source = format!("data:application/octet-stream;base64,{encoded}");
    with_url(&source, |url| {
        assert_eq!(
            decode_data_url(url).ok().as_deref().map(<[u8]>::len),
            Some(MAX_SUBRESOURCE_BYTES as usize)
        );
    });
}

fn with_url(source: &str, assertion: impl FnMut(&Url)) {
    let parsed = Url::parse(source);
    assert!(parsed.is_ok(), "fixture URL must be valid: {source}");
    parsed.iter().for_each(assertion);
}
