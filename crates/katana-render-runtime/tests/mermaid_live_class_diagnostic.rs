use image::ImageEncoder;
use katana_render_runtime::{
    DiagramKind, MermaidRenderer, RenderConfig, RenderContext, RenderInput, RenderPolicy,
    RenderThemeSnapshot, Renderer, RuntimePathResolver,
};
use sha2::Digest;
use std::io::Write;
use std::path::{Path, PathBuf};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "diagnostic only; requires explicitly supplied public Live source/config and fresh TMP output path"]
fn renders_live_class_ja_with_public_render_input_theme_snapshot() -> TestResult<()> {
    let source_path = required_path("KRR_MERMAID_LIVE_SOURCE_PATH")?;
    let config_path = required_path("KRR_MERMAID_LIVE_CONFIG_PATH")?;
    let output_path = required_path("KRR_MERMAID_LIVE_OUTPUT_SVG_PATH")?;
    let (source, source_sha) = read_verified_public_source(&source_path)?;
    let (theme, config_sha) = read_verified_theme_snapshot(&config_path)?;
    prepare_output_path(&output_path)?;
    let (svg, runtime_sha) = render_verified_mermaid_svg(source, theme)?;
    write_new_svg(&output_path, &svg)?;
    log_render_evidence(&source_sha, &config_sha, &runtime_sha, &svg, &output_path);
    if let Some(png_path) = optional_path("KRR_MERMAID_LIVE_OUTPUT_PNG_PATH") {
        let (png_sha, width, height) = write_new_rasterized_png(&png_path, &svg)?;
        eprintln!(
            "live-class-ja rasterized_png_sha256={png_sha} width={width} height={height} output={}",
            png_path.display()
        );
    }
    Ok(())
}

fn read_verified_public_source(source_path: &Path) -> TestResult<(String, String)> {
    let source_bytes = std::fs::read(source_path)?;
    let source_sha = sha256(&source_bytes);
    assert_eq!(
        source_sha,
        "bce4f26020c0d23d5a3941e0c8da992b5fa32da30b8d0826e5c4f2638eb1499c"
    );
    Ok((String::from_utf8(source_bytes)?, source_sha))
}

fn read_verified_theme_snapshot(config_path: &Path) -> TestResult<(RenderThemeSnapshot, String)> {
    let config_bytes = std::fs::read(config_path)?;
    let config_sha = sha256(&config_bytes);
    assert_eq!(
        config_sha,
        "9e40e6c30aed11d02d784a6787cda0ea45131ffa13391101facc2c09f9c78ff9"
    );
    let config: serde_json::Value = serde_json::from_slice(&config_bytes)?;
    let theme = serde_json::from_value(config["krrRuntimeThemeSnapshotCandidate"].clone())?;
    Ok((theme, config_sha))
}

fn prepare_output_path(path: &Path) -> TestResult<()> {
    assert!(path.is_absolute(), "output path must be absolute");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn render_verified_mermaid_svg(
    source: String,
    theme: RenderThemeSnapshot,
) -> TestResult<(String, String)> {
    let runtime_path = RuntimePathResolver::resolve(DiagramKind::Mermaid, None)?;
    let runtime_bytes = std::fs::read(&runtime_path)?;
    let runtime_sha = sha256(&runtime_bytes);
    assert_eq!(
        runtime_sha,
        katana_render_runtime::markdown::mermaid_renderer::MERMAID_JS_CHECKSUM
    );
    let renderer = MermaidRenderer::with_runtime_path(runtime_path);
    let input = RenderInput {
        kind: DiagramKind::Mermaid,
        source,
        config: RenderConfig::default(),
        policy: RenderPolicy::default(),
        context: RenderContext {
            theme_fingerprint: None,
            document_id: Some("live-class-ja-diagnostic".to_string()),
            theme: Some(theme),
        },
    };
    let rendered = renderer.render(&input)?;
    assert!(rendered.svg.contains("<svg"));
    Ok((rendered.svg, runtime_sha))
}

fn write_new_svg(path: &Path, svg: &str) -> TestResult<()> {
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    output.write_all(svg.as_bytes())?;
    Ok(())
}

fn write_new_rasterized_png(path: &Path, svg: &str) -> TestResult<(String, u32, u32)> {
    assert!(path.is_absolute(), "PNG output path must be absolute");
    let raster =
        katana_render_runtime::markdown::svg_rasterize::SvgRasterizeOps::rasterize_svg(svg, 1.0)?;
    let png = encode_rasterized_png(&raster)?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    output.write_all(&png)?;
    Ok((sha256(&png), raster.width, raster.height))
}

fn encode_rasterized_png(
    raster: &katana_render_runtime::markdown::svg_rasterize::RasterizedSvg,
) -> TestResult<Vec<u8>> {
    let expected = usize::try_from(raster.width)
        .ok()
        .and_then(|width| {
            usize::try_from(raster.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4));
    assert_eq!(
        Some(raster.rgba.len()),
        expected,
        "raster RGBA size mismatch"
    );
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        &raster.rgba,
        raster.width,
        raster.height,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(png)
}

fn optional_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn log_render_evidence(
    source_sha: &str,
    config_sha: &str,
    runtime_sha: &str,
    svg: &str,
    output_path: &Path,
) {
    eprintln!(
        "live-class-ja source_sha256={source_sha} config_sha256={config_sha} runtime_sha256={runtime_sha} svg_sha256={} svg_bytes={} output={}",
        sha256(svg.as_bytes()),
        svg.len(),
        output_path.display(),
    );
}

fn required_path(name: &str) -> TestResult<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing required environment variable {name}").into())
}

fn sha256(bytes: &[u8]) -> String {
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
