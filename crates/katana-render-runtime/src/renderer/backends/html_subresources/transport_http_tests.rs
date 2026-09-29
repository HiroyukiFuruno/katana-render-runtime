use super::super::HtmlSubresourceLoader;
use super::{image_data_url_is_decodable, load_bytes, load_image_data_url};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::thread;
use std::time::Duration;
use url::Url;

#[test]
fn unsupported_schemes_and_image_media_types_are_explicit() {
    with_url("ftp://example.test/image.png", |ftp| {
        assert!(load_bytes(ftp).is_err());
    });
    assert!(!image_data_url_is_decodable("not a URL"));
    assert!(!HtmlSubresourceLoader::image_data_url_is_decodable(
        "not a URL",
    ));
    with_http_response(None, b"not an image", |url| {
        assert_eq!(
            load_image_data_url(url),
            Err("subresource is not a supported image".to_string())
        );
    });
}

#[test]
fn extensionless_http_images_use_response_type_or_byte_sniffing() {
    const GIF: &[u8] =
        b"GIF89a\x01\0\x01\0\x80\0\0\0\0\0\xff\xff\xff,\0\0\0\0\x01\0\x01\0\0\x02\x01L\0;";
    with_http_response(Some("image/gif"), GIF, |url| {
        assert!(
            load_image_data_url(url).is_ok_and(|value| value.starts_with("data:image/gif;base64,"))
        );
    });
    with_http_response(None, GIF, |url| {
        assert!(
            load_image_data_url(url).is_ok_and(|value| value.starts_with("data:image/gif;base64,"))
        );
    });
}

#[test]
fn http_image_fixtures_close_the_connection_after_the_declared_body() {
    let response = http_response_headers(Some("image/gif"), 43);
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("Content-Length: 43\r\n"));
    assert!(response.contains("Connection: close\r\n"));
    assert!(response.contains("Content-Type: image/gif\r\n"));
    assert!(response.ends_with("\r\n\r\n"));
}

#[test]
fn local_and_http_read_errors_are_reported() {
    with_url("file://example.test/unsupported", |url| {
        assert!(load_bytes(url).is_err());
    });
    with_file_url(&missing_file_path(), |url| {
        assert!(load_bytes(url).is_err());
        assert!(load_image_data_url(url).is_err());
    });
    with_url("http://127.0.0.1:0/unreachable", |url| {
        assert!(load_bytes(url).is_err());
    });
    with_truncated_http_response(|url| assert!(load_bytes(url).is_err()));
}

fn with_url(source: &str, assertion: impl FnMut(&Url)) {
    let parsed = Url::parse(source);
    assert!(parsed.is_ok(), "fixture URL must be valid: {source}");
    parsed.iter().for_each(assertion);
}

fn with_file_url(path: &Path, assertion: impl FnMut(&Url)) {
    let parsed = Url::from_file_path(path);
    assert!(parsed.is_ok(), "fixture file URL must be valid: {path:?}");
    parsed.iter().for_each(assertion);
}

fn missing_file_path() -> std::path::PathBuf {
    std::env::temp_dir().join("krr-html-subresource-missing-file")
}

fn with_truncated_http_response(mut assertion: impl FnMut(&Url)) {
    let listener = TcpListener::bind("127.0.0.1:0");
    assert!(listener.is_ok());
    listener.into_iter().for_each(|listener| {
        let address = listener.local_addr();
        assert!(address.is_ok());
        address.into_iter().for_each(|address| {
            thread::scope(|scope| {
                scope.spawn(|| {
                    let accepted = listener.accept();
                    assert!(accepted.is_ok());
                    accepted.into_iter().for_each(|(mut stream, _)| {
                        assert!(
                            stream
                                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nx")
                                .is_ok()
                        );
                    });
                });
                let url = Url::parse(&format!("http://{address}/truncated"));
                assert!(url.is_ok());
                url.iter().for_each(&mut assertion);
            });
        });
    });
}

fn with_http_response(content_type: Option<&str>, body: &[u8], mut assertion: impl FnMut(&Url)) {
    let listener = TcpListener::bind("127.0.0.1:0");
    assert!(listener.is_ok());
    let Some(listener) = listener.ok() else {
        return;
    };
    let address = listener.local_addr();
    assert!(address.is_ok());
    let Some(address) = address.ok() else {
        return;
    };
    thread::scope(|scope| {
        scope.spawn(|| write_http_response(listener, content_type, body));
        let url = Url::parse(&format!("http://{address}/avatar?id=1"));
        assert!(url.is_ok());
        url.iter().for_each(&mut assertion);
    });
}

fn write_http_response(listener: TcpListener, content_type: Option<&str>, body: &[u8]) {
    let accepted = listener.accept();
    assert!(accepted.is_ok());
    accepted.into_iter().for_each(|(mut stream, _)| {
        read_http_request_headers(&mut stream);
        let response = http_response_headers(content_type, body.len());
        assert!(stream.write_all(response.as_bytes()).is_ok());
        assert!(stream.write_all(body).is_ok());
        assert!(stream.flush().is_ok());
        assert!(stream.shutdown(Shutdown::Write).is_ok());
    });
}

fn read_http_request_headers(stream: &mut TcpStream) {
    const MAX_REQUEST_HEADER_BYTES: usize = 16 * 1024;
    const REQUEST_READ_BUFFER_BYTES: usize = 1024;
    const HTTP_HEADER_TERMINATOR_BYTES: usize = 4;
    assert!(
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .is_ok()
    );
    let mut request = Vec::new();
    let mut buffer = [0_u8; REQUEST_READ_BUFFER_BYTES];
    loop {
        let read = stream.read(&mut buffer);
        assert!(read.is_ok());
        let read = read.unwrap_or(0);
        assert!(read > 0, "HTTP client closed before the request headers");
        request.extend_from_slice(&buffer[..read]);
        assert!(
            request.len() <= MAX_REQUEST_HEADER_BYTES,
            "HTTP request headers exceed the fixture limit"
        );
        if request
            .windows(HTTP_HEADER_TERMINATOR_BYTES)
            .any(|window| window == b"\r\n\r\n")
        {
            return;
        }
    }
}

fn http_response_headers(content_type: Option<&str>, body_length: usize) -> String {
    let mut response =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {body_length}\r\nConnection: close\r\n");
    if let Some(content_type) = content_type {
        response.push_str(&format!("Content-Type: {content_type}\r\n"));
    }
    response.push_str("\r\n");
    response
}
