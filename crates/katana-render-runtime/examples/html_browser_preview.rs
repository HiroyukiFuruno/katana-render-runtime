#[path = "html_browser_preview/args.rs"]
mod args;
#[path = "html_browser_preview/capture.rs"]
mod capture;
#[path = "html_browser_preview/interaction.rs"]
mod interaction;
#[path = "html_browser_preview/paths.rs"]
mod paths;
#[cfg(test)]
#[path = "html_browser_preview/preview_tests.rs"]
mod preview_tests;
#[path = "html_browser_preview/report.rs"]
mod report;

use args::PreviewArgs;
use katana_render_runtime::{HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};
use paths::AppResult;
use report::{PreviewCapture, PreviewReport};
use std::{env, fs};

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: impl IntoIterator<Item = String>) -> AppResult<()> {
    let args = PreviewArgs::parse(arguments)?;
    args.validate()?;

    let raw_html = fs::read_to_string(&args.input)
        .map_err(|error| format!("failed to read {}: {error}", args.input.display()))?;
    let origin = args.origin()?;
    let source =
        HtmlBrowserSource::new(raw_html, origin.as_str()).map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(args.width, args.height, args.device_scale_factor)
        .map_err(|error| error.to_string())?;
    let mut session = HtmlRuntime
        .open(source, viewport)
        .map_err(|error| error.to_string())?;
    let mut captures = Vec::new();
    capture::PreviewCaptureWriter::capture_latest(
        &mut captures,
        "Initial render",
        &args.output,
        &session,
    )?;
    interaction::PreviewInteraction::run_interaction_scenario(&args, &mut session, &mut captures)?;
    if let Err(error) = session.close() {
        eprintln!("warning: failed to close browser session cleanly: {error}");
    }
    let navigation =
        interaction::PreviewInteraction::run_link_navigation_probe(origin.as_str(), viewport)?;
    write_report_and_print(&args, origin.as_str(), &captures, &navigation)
}

fn write_report_and_print(
    args: &PreviewArgs,
    origin: &str,
    captures: &[PreviewCapture],
    navigation: &str,
) -> AppResult<()> {
    PreviewReport::write(args, origin, captures, Some(navigation))?;
    println!("png={}", args.output.display());
    println!("report={}", args.report.display());
    for capture in captures {
        println!("capture:{}={}", capture.label, capture.path.display());
    }
    println!("link_navigation={navigation}");
    Ok(())
}
