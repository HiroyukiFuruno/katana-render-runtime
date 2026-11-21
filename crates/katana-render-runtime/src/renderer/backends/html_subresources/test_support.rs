use crate::renderer::backends::{HtmlBrowserSource, HtmlBrowserViewport};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use url::Url;

pub(in crate::renderer::backends::html_subresources) type TestResult<T = ()> = Result<T, String>;
type RequestLog = Arc<Mutex<Vec<String>>>;
pub(in crate::renderer::backends::html_subresources) type ResourceServer =
    std::thread::JoinHandle<std::io::Result<()>>;
type LocalResourceServer = (String, RequestLog, ResourceServer);

static NEXT_FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

pub(super) fn viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)
}

pub(super) fn assert_frame_contains(pixels: &[u8], expected: [u8; 3]) {
    assert!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[..3] == expected),
        "frame does not contain {expected:?}"
    );
}

pub(super) fn local_resource_server() -> TestResult<LocalResourceServer> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(to_string)?;
    let address = listener.local_addr().map_err(to_string)?;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);
    let server = std::thread::spawn(move || serve_resources(listener, &recorded));
    Ok((format!("http://{address}"), requests, server))
}

pub(in crate::renderer::backends::html_subresources) fn one_response_server(
    content_type: &'static str,
    body: &'static [u8],
) -> TestResult<(String, ResourceServer)> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(to_string)?;
    let address = listener.local_addr().map_err(to_string)?;
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
        let _path = request_path(&mut stream)?;
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(body)?;
        stream.flush()?;
        stream.shutdown(Shutdown::Write)
    });
    Ok((format!("http://{address}/image.svg"), server))
}

pub(super) fn delayed_dynamic_server() -> TestResult<(String, ResourceServer)> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(to_string)?;
    let address = listener.local_addr().map_err(to_string)?;
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
        let path = request_path(&mut stream)?;
        if path != "/state.txt" {
            return Err(std::io::Error::other("unexpected dynamic request"));
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        write_response(&mut stream, b"ready")
    });
    Ok((format!("http://{address}"), server))
}

fn serve_resources(listener: TcpListener, requests: &Mutex<Vec<String>>) -> std::io::Result<()> {
    for _ in 0..3 {
        let (mut stream, _) = listener.accept()?;
        let path = request_path(&mut stream)?;
        record_request(requests, path.clone())?;
        let body = resource_body(&path)?;
        write_response(&mut stream, body.as_bytes())?;
    }
    Ok(())
}

fn record_request(requests: &Mutex<Vec<String>>, path: String) -> std::io::Result<()> {
    requests
        .lock()
        .map_err(|_| std::io::Error::other("request log lock was poisoned"))?
        .push(path);
    Ok(())
}

fn resource_body(path: &str) -> std::io::Result<&'static str> {
    match path {
        "/style.css" => Ok("#styled { background: #10b981; width: 80px; height: 30px; }"),
        "/app.js" => Ok("document.getElementById('scripted').style.backgroundColor = '#ef4444';"),
        "/pixel.svg" => Ok(
            "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'><rect width='8' height='8' fill='#3182ce'/></svg>",
        ),
        _ => Err(std::io::Error::other("unexpected subresource request")),
    }
}

fn request_path(stream: &mut std::net::TcpStream) -> std::io::Result<String> {
    let mut request = [0_u8; 1024];
    let length = stream.read(&mut request)?;
    let request = std::str::from_utf8(&request[..length]).map_err(std::io::Error::other)?;
    request
        .split_ascii_whitespace()
        .nth(1)
        .map(str::to_string)
        .ok_or_else(missing_request_path)
}

fn missing_request_path() -> std::io::Error {
    std::io::Error::other("request path is missing")
}

fn write_response(stream: &mut std::net::TcpStream, body: &[u8]) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)
}

pub(in crate::renderer::backends::html_subresources) struct LocalFixture {
    pub(in crate::renderer::backends::html_subresources) root: PathBuf,
}

impl LocalFixture {
    pub(in crate::renderer::backends::html_subresources) fn new() -> TestResult<Self> {
        let root = temp_root();
        std::fs::create_dir_all(&root).map_err(to_string)?;
        std::fs::write(
            root.join("style.css"),
            "#styled { background: #10b981; width: 80px; height: 30px; }",
        )
        .map_err(to_string)?;
        std::fs::write(
            root.join("app.js"),
            "document.getElementById('scripted').style.backgroundColor = '#ef4444';",
        )
        .map_err(to_string)?;
        std::fs::write(root.join("pixel.svg"), "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'><rect width='8' height='8' fill='#3182ce'/></svg>")
            .map_err(to_string)?;
        std::fs::write(root.join("index.html"), "<!doctype html>").map_err(to_string)?;
        Ok(Self { root })
    }

    pub(super) fn origin(&self) -> TestResult<String> {
        Url::from_file_path(self.root.join("index.html"))
            .map(|url| url.to_string())
            .map_err(file_url_error)
    }

    pub(super) fn source(&self, raw_html: &str) -> TestResult<HtmlBrowserSource> {
        HtmlBrowserSource::new(raw_html, self.origin()?).map_err(to_string)
    }

    pub(super) fn html(&self) -> &'static str {
        "<link rel=stylesheet href=style.css><div id=styled>Styled</div><div id=scripted style='width:80px;height:30px'>Scripted</div><script src=app.js></script><img src=pixel.svg style='width:40px;height:40px'>"
    }
}

fn file_url_error(_: ()) -> String {
    "fixture path is not a file URL".to_string()
}

impl Drop for LocalFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn temp_root() -> PathBuf {
    let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("krr-html-resources-{}-{id}", std::process::id()))
}

pub(super) fn to_string(error: impl ToString) -> String {
    error.to_string()
}

pub(super) fn must_source(fixture: &LocalFixture, raw_html: &str) -> HtmlBrowserSource {
    must_result(fixture.source(raw_html))
}

pub(super) fn must_result<T, E>(result: Result<T, E>) -> T {
    assert!(result.is_ok());
    let mut values = result.into_iter().collect::<Vec<_>>();
    values.remove(0)
}

#[cfg(test)]
mod tests {
    use super::{
        delayed_dynamic_server, file_url_error, missing_request_path, record_request,
        resource_body, to_string,
    };
    use crate::renderer::backends::html_browser::HtmlBrowserError;
    use std::io::Write;
    use std::net::TcpStream;
    use std::sync::{Arc, Mutex};
    use url::Url;

    #[test]
    fn delayed_dynamic_server_rejects_unexpected_request_path() {
        let (origin, server) = super::must_result(delayed_dynamic_server());
        let mut stream =
            super::must_result(TcpStream::connect(origin.trim_start_matches("http://")));
        super::must_result(stream.write_all(
            b"GET /unexpected.txt HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        ));
        super::must_result(stream.shutdown(std::net::Shutdown::Write));
        let result = super::must_result(server.join());
        assert!(matches!(
            result,
            Err(error) if error.to_string() == "unexpected dynamic request"
        ));
    }

    #[test]
    fn support_helpers_report_unexpected_resources_and_stringify_errors() {
        assert!(resource_body("/missing.css").is_err());
        assert_eq!(to_string("fixture failure"), "fixture failure");
        assert_eq!(to_string("owned failure".to_string()), "owned failure");
        assert_eq!(to_string(std::io::Error::other("io failure")), "io failure");
        assert_eq!(
            to_string(HtmlBrowserError::InvalidViewport),
            "browser viewport dimensions must be non-zero"
        );
    }

    #[test]
    fn support_helpers_preserve_url_and_request_errors() {
        assert!(matches!(
            Url::parse("http://[").map_err(to_string),
            Err(message) if message == "invalid IPv6 address"
        ));
        assert_eq!(
            missing_request_path().to_string(),
            "request path is missing"
        );
        assert_eq!(file_url_error(()), "fixture path is not a file URL");
    }

    #[test]
    fn poisoned_request_log_is_reported_as_an_io_error() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let poisoned = Arc::clone(&requests);
        let thread = std::thread::spawn(move || {
            let _result = poisoned
                .lock()
                .map(|_guard| std::panic::resume_unwind(Box::new("poison request log")));
        });
        let _ = thread.join();

        assert!(matches!(
            record_request(&requests, "/style.css".to_string()),
            Err(error) if error.to_string() == "request log lock was poisoned"
        ));
    }
}
