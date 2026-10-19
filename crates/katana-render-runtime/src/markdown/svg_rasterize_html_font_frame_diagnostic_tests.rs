use image::ImageEncoder;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_skia_tree_c4_diagnostic_tests.rs"]
mod skia_tree_c4_diagnostic;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    input: PathBuf,
    output: PathBuf,
    width: u32,
    height: u32,
    expected_source_sha256: String,
}

struct SvgCapture {
    render_input_sha: String,
    render_input_paint_call: u64,
    resolved_svg_sha: String,
}

#[test]
#[ignore = "diagnostic only; requires a fresh request and output directory"]
fn ignored_html_frame_and_font_audit_are_saved_together() -> Result<(), String> {
    let (request, html, source_sha, origin) = read_request()?;
    let (session, audit) =
        super::open_html_with_audit(&html, &origin, request.width, request.height)?;
    let frame = session
        .latest_frame()
        .ok_or("HTML session produced no frame")?;
    super::assert_valid_paint_evidence(&audit)?;
    if (frame.viewport.width, frame.viewport.height) != (request.width, request.height)
        || frame.viewport.device_scale_factor != 1.0
    {
        return Err("actual frame viewport differs from request".to_owned());
    }
    save_capture(&request, &source_sha, &origin, frame, audit)
}

fn read_request() -> Result<(Request, String, String, String), String> {
    let request = load_request()?;
    let (html, source_sha, origin) = read_input(&request)?;
    if request.output.is_relative() {
        return Err("output directory must be absolute".to_owned());
    }
    std::fs::create_dir(&request.output)
        .map_err(|error| format!("create fresh output: {error}"))?;
    Ok((request, html, source_sha, origin))
}

fn load_request() -> Result<Request, String> {
    let request_path = std::env::var_os("KRR_HTML_FONT_AUDIT_REQUEST")
        .map(PathBuf::from)
        .ok_or("KRR_HTML_FONT_AUDIT_REQUEST is required")?;
    if request_path.is_relative() {
        return Err("KRR_HTML_FONT_AUDIT_REQUEST must be absolute".to_owned());
    }
    serde_json::from_slice(
        &std::fs::read(&request_path).map_err(|error| format!("read request: {error}"))?,
    )
    .map_err(|error| format!("parse request: {error}"))
}

fn read_input(request: &Request) -> Result<(String, String, String), String> {
    let input = request
        .input
        .canonicalize()
        .map_err(|error| format!("canonicalize input: {error}"))?;
    let html = std::fs::read_to_string(&input).map_err(|error| format!("read input: {error}"))?;
    let source_sha = super::super::output::sha256(html.as_bytes());
    if source_sha != request.expected_source_sha256 {
        return Err("input source SHA256 does not match request".to_owned());
    }
    let origin = url::Url::from_file_path(&input)
        .map_err(|()| "input path cannot form file origin".to_owned())?
        .to_string();
    Ok((html, source_sha, origin))
}

fn save_capture(
    request: &Request,
    source_sha: &str,
    origin: &str,
    frame: &crate::HtmlBrowserFrame,
    audit: serde_json::Value,
) -> Result<(), String> {
    let binary = std::fs::read(std::env::current_exe().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let png = encode_png(&frame.pixels, frame.viewport.width, frame.viewport.height)?;
    write_fresh(&request.output.join("frame.png"), &png)?;
    let svg = save_svg_capture(request, &audit)?;
    let report = serde_json::json!({
        "sourceSha256": source_sha,
        "renderInputSvgSha256": svg.render_input_sha,
        "renderInputPaintCall": svg.render_input_paint_call,
        "resolvedSvgPath": request.output.join("resolved-svg.svg"),
        "resolvedSvgSha256": svg.resolved_svg_sha,
        "origin": origin,
        "viewport": {"width": frame.viewport.width, "height": frame.viewport.height, "scale": frame.viewport.device_scale_factor, "scrollY": frame.scroll_y},
        "actualBinarySha256": super::super::output::sha256(&binary),
        "rgbaSha256": super::super::output::sha256(&frame.pixels),
        "formalAcceptance": false,
        "audit": audit,
    });
    write_fresh(
        &request.output.join("report.json"),
        &serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
}

fn save_svg_capture(request: &Request, audit: &serde_json::Value) -> Result<SvgCapture, String> {
    let (render_input, render_input_sha, render_input_paint_call) = root_render_input(audit)?;
    let (resolved_svg, resolved_svg_sha) = root_resolved_svg(audit)?;
    write_fresh(
        &request.output.join("render-input.svg"),
        render_input.as_bytes(),
    )?;
    write_fresh(
        &request.output.join("resolved-svg.svg"),
        resolved_svg.as_bytes(),
    )?;
    Ok(SvgCapture {
        render_input_sha,
        render_input_paint_call,
        resolved_svg_sha,
    })
}

fn root_resolved_svg(audit: &serde_json::Value) -> Result<(String, String), String> {
    let root = root_tree(audit)?;
    root_svg_field(
        root,
        "resolvedSvg",
        "resolvedSvgSha256",
        "root resolved SVG",
    )
}

fn root_render_input(audit: &serde_json::Value) -> Result<(String, String, u64), String> {
    let root = root_tree(audit)?;
    let (source, source_sha) = root_svg_field(
        root,
        "sourceSvg",
        "sourceSvgSha256",
        "root render input source",
    )?;
    let paint_call = root["paintCall"]
        .as_u64()
        .ok_or("root render input paint call is absent")?;
    Ok((source, source_sha, paint_call))
}

fn root_tree(audit: &serde_json::Value) -> Result<&serde_json::Value, String> {
    audit["finalGlyphTrees"]
        .as_array()
        .ok_or("final glyph tree evidence is absent")?
        .iter()
        .rev()
        .find(|tree| tree["treePath"] == "root")
        .ok_or_else(|| "root final glyph tree evidence is absent".to_owned())
}

fn root_svg_field(
    root: &serde_json::Value,
    svg_field: &str,
    sha_field: &str,
    label: &str,
) -> Result<(String, String), String> {
    let svg = root[svg_field]
        .as_str()
        .ok_or_else(|| format!("{} SVG is absent", label))?;
    let sha = super::super::output::sha256(svg.as_bytes());
    if root[sha_field] != sha {
        return Err(format!("{} SVG SHA differs from audit", label));
    }
    Ok((svg.to_owned(), sha))
}

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let expected =
        width as usize * height as usize * image::ExtendedColorType::Rgba8.channel_count() as usize;
    if rgba.len() != expected {
        return Err("RGBA frame dimensions do not match request".to_owned());
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| error.to_string())?;
    Ok(png)
}

fn write_fresh(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create {}: {error}", path.display()))?;
    file.write_all(bytes).map_err(|error| error.to_string())
}
