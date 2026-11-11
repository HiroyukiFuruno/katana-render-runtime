use image::RgbaImage;
use katana_render_runtime::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport,
    HtmlRuntime,
};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use url::Url;

#[cfg(test)]
#[path = "html_css_probe_tests.rs"]
mod html_css_probe_tests;

const VIEWPORT_WIDTH: u32 = 1230;
const VIEWPORT_HEIGHT: u32 = 867;
const SLIDE_COUNT: usize = 14;

struct ProbeConfig {
    source_path: PathBuf,
    output_directory: PathBuf,
    slide_count: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = ProbeConfig::from_arguments()?;
    fs::create_dir_all(&config.output_directory)?;
    let mut session = open_session(&config.source_path)?;
    save_slides(&mut session, &config.output_directory, config.slide_count)?;
    println!(
        "saved {} KRR frames at {}x{} CSS pixels",
        config.slide_count, VIEWPORT_WIDTH, VIEWPORT_HEIGHT,
    );
    Ok(())
}

impl ProbeConfig {
    fn from_arguments() -> Result<Self, Box<dyn Error>> {
        let mut arguments = std::env::args_os().skip(1);
        let source_path = arguments
            .next()
            .map(PathBuf::from)
            .ok_or("expected source HTML path")?;
        let output_directory = arguments
            .next()
            .map(PathBuf::from)
            .ok_or("expected output directory")?;
        let slide_count = arguments
            .next()
            .map(|value| value.to_string_lossy().parse::<usize>())
            .transpose()?
            .unwrap_or(SLIDE_COUNT)
            .clamp(1, SLIDE_COUNT);
        Ok(Self {
            source_path,
            output_directory,
            slide_count,
        })
    }
}

fn open_session(source_path: &Path) -> Result<HtmlBrowserSession, Box<dyn Error>> {
    let source = HtmlBrowserSource::new(
        fs::read_to_string(source_path)?,
        Url::from_file_path(source_path.canonicalize()?)
            .map_err(|()| "source path cannot be represented as a file URL")?,
    )?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)?;
    Ok(HtmlRuntime.open(source, viewport)?)
}

fn save_slides(
    session: &mut HtmlBrowserSession,
    output_directory: &Path,
    slide_count: usize,
) -> Result<(), Box<dyn Error>> {
    for slide_number in 1..=slide_count {
        save_frame(
            session.latest_frame().ok_or("missing slide frame")?,
            &output_directory.join(format!("krr-slide-{slide_number:02}.png")),
        )?;
        if slide_number < slide_count {
            session.dispatch_input(HtmlBrowserInput::KeyDown {
                key: "ArrowRight".to_string(),
            })?;
        }
    }
    Ok(())
}

fn save_frame(frame: &HtmlBrowserFrame, path: &Path) -> Result<(), Box<dyn Error>> {
    let image = RgbaImage::from_raw(
        frame.viewport.width,
        frame.viewport.height,
        frame.pixels.clone(),
    )
    .ok_or("invalid RGBA frame dimensions")?;
    image.save(path)?;
    Ok(())
}
