use katana_render_runtime::{HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};
use url::Url;

#[path = "probe/frame.rs"]
mod probe_frame;
#[path = "probe/names.rs"]
mod probe_names;
use probe_names::ProbeNames;
#[cfg(test)]
#[path = "probe/html_fidelity_probe_tests.rs"]
mod html_fidelity_probe_tests;
const DEFAULT_WIDTH: u32 = 1_280;
const DEFAULT_HEIGHT: u32 = 900;
const DEFAULT_SCROLL_POSITIONS: [f32; 3] = [0.0, 600.0, 1_800.0];
const USAGE: &str = "usage: html_fidelity_probe --input INPUT.html --output DIR [--width PX] [--height PX] [--scale N] [--scroll-y PX ...]";

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

pub(crate) fn main() {
    if let Err(error) = run(env::args_os().skip(1)) {
        eprintln!("html fidelity probe failed: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> ProbeResult<()> {
    let options = ProbeOptions::parse(arguments)?;
    let html = fs::read_to_string(&options.input)?;
    let origin =
        Url::from_file_path(options.input.canonicalize()?).map_err(|()| "invalid file origin")?;
    let source = HtmlBrowserSource::new(html, origin.as_str())?;
    fs::create_dir_all(&options.output)?;
    write_frames(&options, &source)
}

fn write_frames(options: &ProbeOptions, source: &HtmlBrowserSource) -> ProbeResult<()> {
    let source_sha256 = Sha256::digest(source.raw_html.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!("source_sha256={source_sha256}");
    let viewport = HtmlBrowserViewport::new(options.width, options.height, options.scale)?;
    let mut session = HtmlRuntime.open(source.clone(), viewport)?;
    for &scroll_y in &options.scroll_y {
        probe_frame::write_frame(options, &mut session, scroll_y)?;
    }
    session.close()?;
    Ok(())
}

struct ProbeOptions {
    input: PathBuf,
    output: PathBuf,
    width: u32,
    height: u32,
    scale: f32,
    scroll_y: Vec<f32>,
}

struct RawProbeOptions {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    width: u32,
    height: u32,
    scale: f32,
    scroll_y: Vec<f32>,
}

impl ProbeOptions {
    fn parse(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> ProbeResult<Self> {
        let raw = parse_arguments(arguments)?;
        let input = raw.input.ok_or("--input is required")?;
        let output = raw.output.ok_or("--output is required")?;
        validate_options(&input, raw.width, raw.height, raw.scale, &raw.scroll_y)?;
        Ok(Self {
            input,
            output,
            width: raw.width,
            height: raw.height,
            scale: raw.scale,
            scroll_y: raw.scroll_y,
        })
    }
}

fn parse_arguments(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> ProbeResult<RawProbeOptions> {
    let mut raw = RawProbeOptions {
        input: None,
        output: None,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        scale: 1.0,
        scroll_y: Vec::new(),
    };
    let mut args = arguments
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned());
    while let Some(argument) = args.next() {
        apply_argument(argument, &mut args, &mut raw)?;
    }
    if raw.scroll_y.is_empty() {
        raw.scroll_y.extend(DEFAULT_SCROLL_POSITIONS);
    }
    Ok(raw)
}

fn apply_argument<I: Iterator<Item = String>>(
    argument: String,
    args: &mut I,
    raw: &mut RawProbeOptions,
) -> ProbeResult<()> {
    let mut value = || {
        args.next()
            .ok_or_else(|| format!("{argument} requires a value"))
    };
    match argument.as_str() {
        "--input" => raw.input = Some(PathBuf::from(value()?)),
        "--output" => raw.output = Some(PathBuf::from(value()?)),
        "--width" => raw.width = value()?.parse()?,
        "--height" => raw.height = value()?.parse()?,
        "--scale" => raw.scale = value()?.parse()?,
        "--scroll-y" => raw.scroll_y.push(value()?.parse()?),
        "--help" | "-h" => return Err(USAGE.into()),
        unknown => return Err(format!("unknown argument: {unknown}\n\n{USAGE}").into()),
    }
    Ok(())
}

fn validate_options(
    input: &std::path::Path,
    width: u32,
    height: u32,
    scale: f32,
    scroll_y: &[f32],
) -> ProbeResult<()> {
    if scroll_y.is_empty() {
        return Err("at least one scroll-y value is required".into());
    }
    if !input.is_file() {
        return Err(format!("input HTML is missing: {}", input.display()).into());
    }
    if width == 0 || height == 0 || !scale.is_finite() || scale <= 0.0 {
        return Err("width, height, and scale must be positive".into());
    }
    if scroll_y
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("scroll-y values must be finite and non-negative".into());
    }
    Ok(())
}
