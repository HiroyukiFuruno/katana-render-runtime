use katana_render_runtime::{HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};
use serde_json::{Value, json};
use std::{env, fs, path::PathBuf, time::Instant};

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;
const VIEWPORT_WIDTH: u32 = 1280;
const VIEWPORT_HEIGHT: u32 = 900;
const MILLIS_PER_SECOND: f64 = 1000.0;

fn main() {
    match run() {
        Ok(evidence) => println!("{evidence}"),
        Err(error) => {
            eprintln!("initial-frame probe failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run() -> ProbeResult<Value> {
    let input = input_path()?;
    let html = fs::read_to_string(&input)?;
    let origin = url::Url::from_file_path(&input).map_err(|()| "invalid file origin")?;
    let source = HtmlBrowserSource::new(html, origin.as_str())?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)?;
    let started = Instant::now();
    let mut session = HtmlRuntime.open(source, viewport)?;
    let first_frame_ms = started.elapsed().as_secs_f64() * MILLIS_PER_SECOND;
    let frame = session.latest_frame().map(frame_evidence);
    /* WHY: 入力固有の操作を仮定せず、初期描画後のclose自体を検証する。 */
    let close_started = Instant::now();
    session.close()?;
    let close_ms = close_started.elapsed().as_secs_f64() * MILLIS_PER_SECOND;
    Ok(json!({
        "first_frame_ms": first_frame_ms,
        "close_ms": close_ms,
        "closed": true,
        "frame": frame.ok_or("initial frame missing")?,
    }))
}

fn input_path() -> ProbeResult<PathBuf> {
    let mut args = env::args_os().skip(1);
    let input = args
        .next()
        .ok_or("usage: html_initial_frame_probe INPUT.html")?;
    if args.next().is_some() {
        return Err("exactly one input path is required".into());
    }
    Ok(fs::canonicalize(input)?)
}

fn frame_evidence(frame: &katana_render_runtime::HtmlBrowserFrame) -> Value {
    json!({
        "generation": frame.generation,
        "width": frame.viewport.width,
        "height": frame.viewport.height,
        "scale": frame.viewport.device_scale_factor,
        "pixel_format": frame.pixel_format,
        "pixel_bytes": frame.pixels.len(),
        "scroll_y": frame.scroll_y,
        "content_height": frame.content_height,
    })
}
