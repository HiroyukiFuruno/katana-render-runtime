use katana_render_runtime::{
    HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
};
use std::{
    env,
    error::Error,
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};
use url::Url;

const DEFAULT_WIDTH: u32 = 1_280;
const DEFAULT_HEIGHT: u32 = 900;
const USAGE: &str = "usage: html_open_memory_probe INPUT.html [WIDTH] [HEIGHT] [SCALE]";

type ProbeResult<T> = Result<T, Box<dyn Error>>;

fn main() {
    if let Err(error) = run(env::args_os().skip(1)) {
        eprintln!("HTML open memory probe failed: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> ProbeResult<()> {
    run_with_io(arguments, &mut io::stdin().lock(), &mut io::stdout().lock())
}

fn run_with_io(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> ProbeResult<()> {
    let options = ProbeOptions::parse(arguments)?;
    let source = source_from_input(&options.input)?;
    let viewport = HtmlBrowserViewport::new(options.width, options.height, options.scale)?;

    synchronize(input, output)?;
    let mut session = HtmlRuntime.open(source, viewport)?;
    close_after_acknowledgement(&mut session, synchronize(input, output))
}

fn source_from_input(input: &Path) -> ProbeResult<HtmlBrowserSource> {
    let input = input.canonicalize()?;
    let origin = Url::from_file_path(&input)
        .map_err(|()| "input path cannot be represented as a file URL")?;
    Ok(HtmlBrowserSource::new(
        fs::read_to_string(input)?,
        origin.as_str(),
    )?)
}

fn close_after_acknowledgement(
    session: &mut HtmlBrowserSession,
    acknowledgement: ProbeResult<()>,
) -> ProbeResult<()> {
    combine_results(acknowledgement, session.close().map_err(Into::into))
}

fn combine_results(acknowledgement: ProbeResult<()>, closed: ProbeResult<()>) -> ProbeResult<()> {
    match (acknowledgement, closed) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(ack), Err(close)) => Err(format!("ACK failed: {ack}; close failed: {close}").into()),
    }
}

fn synchronize(input: &mut impl BufRead, output: &mut impl Write) -> ProbeResult<()> {
    output.write_all(b"READY\n")?;
    output.flush()?;
    let mut acknowledgement = String::new();
    input.read_line(&mut acknowledgement)?;
    if !is_acknowledgement(&acknowledgement) {
        return Err("expected ACK on standard input".into());
    }
    Ok(())
}

fn is_acknowledgement(line: &str) -> bool {
    line.trim_end_matches(['\r', '\n']) == "ACK"
}

struct ProbeOptions {
    input: PathBuf,
    width: u32,
    height: u32,
    scale: f32,
}

impl ProbeOptions {
    fn parse(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> ProbeResult<Self> {
        let mut arguments = arguments
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned());
        let input = arguments.next().map(PathBuf::from).ok_or(USAGE)?;
        let width = arguments
            .next()
            .map_or(Ok(DEFAULT_WIDTH), |value| value.parse())?;
        let height = arguments
            .next()
            .map_or(Ok(DEFAULT_HEIGHT), |value| value.parse())?;
        let scale = arguments.next().map_or(Ok(1.0), |value| value.parse())?;
        if arguments.next().is_some() {
            return Err(USAGE.into());
        }
        validate_options(&input, width, height, scale)?;
        Ok(Self {
            input,
            width,
            height,
            scale,
        })
    }
}

fn validate_options(input: &PathBuf, width: u32, height: u32, scale: f32) -> ProbeResult<()> {
    if !input.is_file() {
        return Err(format!("input HTML is missing: {}", input.display()).into());
    }
    if width == 0 || height == 0 || !scale.is_finite() || scale <= 0.0 {
        return Err("width, height, and scale must be positive".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ProbeOptions, close_after_acknowledgement, combine_results, run_with_io, source_from_input,
        synchronize,
    };
    use katana_render_runtime::{
        HtmlBrowserSessionState, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
    };
    use std::{ffi::OsString, io::Cursor};
    use tempfile::TempDir;

    const HTML: &str = "<!doctype html><p>open-memory-probe</p>";

    #[test]
    fn reports_both_acknowledgement_and_close_failures() {
        let result = combine_results(Err("bad ACK".into()), Err("bad close".into()));
        assert_eq!(
            result.unwrap_err().to_string(),
            "ACK failed: bad ACK; close failed: bad close"
        );
        assert_eq!(
            combine_results(Ok(()), Err("bad close".into()))
                .unwrap_err()
                .to_string(),
            "bad close"
        );
    }

    fn fixture() -> (TempDir, std::path::PathBuf) {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("fixture.html");
        std::fs::write(&path, HTML).unwrap();
        (directory, path)
    }

    fn arguments(path: &std::path::Path) -> Vec<OsString> {
        vec![
            path.as_os_str().to_owned(),
            "320".into(),
            "200".into(),
            "1.0".into(),
        ]
    }

    #[test]
    fn synchronization_accepts_acknowledgement_and_emits_ready() {
        let mut output = Vec::new();
        synchronize(&mut Cursor::new("ACK\n"), &mut output).unwrap();
        assert_eq!(output, b"READY\n");
    }

    #[test]
    fn synchronization_rejects_invalid_acknowledgement_after_emitting_ready() {
        let mut output = Vec::new();
        let error = synchronize(&mut Cursor::new("not-ack\n"), &mut output).unwrap_err();
        assert_eq!(error.to_string(), "expected ACK on standard input");
        assert_eq!(output, b"READY\n");
    }

    #[test]
    fn run_opens_the_fixture_between_two_acknowledgements() {
        let (_directory, path) = fixture();
        let mut input = Cursor::new("ACK\nACK\n");
        let mut output = Vec::new();
        run_with_io(arguments(&path), &mut input, &mut output).unwrap();
        assert_eq!(output, b"READY\nREADY\n");
    }

    #[test]
    fn source_and_options_reject_missing_or_invalid_input() {
        let (_directory, path) = fixture();
        assert_eq!(source_from_input(&path).unwrap().raw_html, HTML);
        assert!(ProbeOptions::parse([OsString::from("missing.html")]).is_err());
        assert!(ProbeOptions::parse([path.clone().into_os_string(), "0".into()]).is_err());
        assert!(
            ProbeOptions::parse([path.into_os_string(), "1".into(), "1".into(), "NaN".into(),])
                .is_err()
        );
    }

    #[test]
    fn close_runs_when_the_post_open_acknowledgement_fails() {
        let source = HtmlBrowserSource::new(HTML, "https://example.test/fixture.html").unwrap();
        let viewport = HtmlBrowserViewport::new(320, 200, 1.0).unwrap();
        let mut session = HtmlRuntime.open(source, viewport).unwrap();
        let result = close_after_acknowledgement(&mut session, Err("invalid ACK".into()));
        assert_eq!(result.unwrap_err().to_string(), "invalid ACK");
        assert_eq!(session.state(), HtmlBrowserSessionState::Closed);
    }
}
