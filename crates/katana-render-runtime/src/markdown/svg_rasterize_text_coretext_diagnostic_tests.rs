use super::{
    CASES, DIAGNOSTIC_FONT_SIZE, FontShape, HARFRUST_SCALE, Result, digest_hex, face_cache,
    harfrust_records, read_request, shape_font, write_fresh,
};
use core_graphics::base::kCGImageAlphaPremultipliedLast;
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::geometry::CGPoint;
use core_text::font::new_from_descriptor;
use core_text::font_manager::create_font_descriptor;
use image::ImageEncoder;
use resvg::usvg;
use serde_json::{Value, json};
use std::path::Path;

const CANVAS_WIDTH: u32 = 512;
const CANVAS_HEIGHT: u32 = 64;
const BASELINE_Y: f64 = 48.0;
const CORETEXT_POINT_SIZE: f64 = 16.0;
const BITS_PER_COMPONENT: usize = 8;
const RGBA_CHANNEL_COUNT: usize = 4;
type CaseFontData = (harfrust::Buffer, Vec<u8>, u32, String);

#[test]
#[ignore = "diagnostic only; requires macOS system Trebuchet and a fresh request JSON"]
fn records_coretext_tree_glyph_raster_and_krr_reference() -> Result<()> {
    let request = read_request()?;
    let input = std::fs::read(&request.input)?;
    if digest_hex(&input) != request.expected_source_sha256 {
        return Err("input SHA mismatch".into());
    }
    if request.output.exists() {
        return Err("diagnostic output must be fresh".into());
    }
    std::fs::create_dir(&request.output)?;
    let mut rows = Vec::new();
    for (index, (text, weight)) in CASES.into_iter().enumerate() {
        rows.push(coretext_case(&request.output, index, text, weight)?);
    }
    write_fresh(&request.output.join("input.html"), &input)?;
    let report = json!({
        "schema": "krr-tree-coretext-glyph-raster-diagnostic-v1",
        "input": request.input,
        "input_sha256": digest_hex(&input),
        "rows": rows,
        "diagnostic_only": true,
        "formal_acceptance": false,
    });
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(request.output.join("report.json"))?;
    serde_json::to_writer_pretty(file, &report)?;
    Ok(())
}

fn coretext_case(output: &Path, index: usize, text: &str, weight: u16) -> Result<Value> {
    let svg = case_svg(text, weight);
    let svg_path = output.join(format!("svg-{index:02}.svg"));
    write_fresh(&svg_path, svg.as_bytes())?;
    let svg_sha = digest_hex(svg.as_bytes());
    let options = crate::markdown::svg_rasterize::font::html_rasterizer_options(&svg);
    crate::markdown::svg_rasterize::text_shaping::with_validated_tree_parse(|| {
        let (tree, face_id, usvg_glyph_count) = shaped_case(&svg, &options)?;
        let shape = FontShape::new(DIAGNOSTIC_FONT_SIZE, weight, false);
        let (buffer, font_bytes, face_index, source) = case_font(&tree, face_id, text, shape)?;
        let coretext = coretext_png(output, index, &font_bytes, &buffer, text)?;
        let krr = krr_png(output, index, &svg, &options)?;
        Ok(json!({
            "text": text,
            "weight": weight,
            "font_source": source,
            "font_sha256": digest_hex(&font_bytes),
            "face_index": face_index,
            "glyphs": harfrust_records(&buffer),
            "usvg_glyph_count": usvg_glyph_count,
            "svg_path": svg_path,
            "svg_sha256": svg_sha,
            "coretext_point_size": CORETEXT_POINT_SIZE,
            "coretext": coretext,
            "krr_reference": krr,
            "font_smoothing": "enabled",
            "font_subpixel_positioning": "enabled",
        }))
    })
}

fn case_svg(text: &str, weight: u16) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS_WIDTH}" height="{CANVAS_HEIGHT}"><text id="run" x="0" y="{BASELINE_Y}" font-family="trebuchet ms,verdana,arial,sans-serif" font-size="16" font-weight="{weight}">{text}</text></svg>"#
    )
}

fn shaped_case(
    svg: &str,
    options: &usvg::Options,
) -> Result<(usvg::Tree, usvg::fontdb::ID, usize)> {
    let tree = usvg::Tree::from_str(svg, options)?;
    let span = super::first_text_span(&tree)?;
    let face_id = span
        .positioned_glyphs
        .first()
        .ok_or("diagnostic glyph missing")?
        .font;
    let glyph_count = span.positioned_glyphs.len();
    Ok((tree, face_id, glyph_count))
}

fn case_font(
    tree: &usvg::Tree,
    face_id: usvg::fontdb::ID,
    text: &str,
    shape: FontShape,
) -> Result<CaseFontData> {
    let face = face_cache::font_for_face(tree.fontdb(), face_id).ok_or("font unavailable")?;
    let buffer = shape_font(&face, text, shape, None).ok_or("shaping failed")?;
    let (font_bytes, face_index, source) = font_data(tree.fontdb(), face_id)?;
    if face_index != 0 {
        return Err(format!("unsupported font face index {face_index}").into());
    }
    Ok((buffer, font_bytes, face_index, source))
}

fn font_data(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
) -> Result<(Vec<u8>, u32, String)> {
    let face = database.face(face_id).ok_or("font face metadata missing")?;
    let source = match &face.source {
        usvg::fontdb::Source::Binary(_) => "binary".to_owned(),
        usvg::fontdb::Source::File(path) => path.display().to_string(),
        usvg::fontdb::Source::SharedFile(path, _) => path.display().to_string(),
    };
    database
        .with_face_data(face_id, |data, index| (data.to_vec(), index, source))
        .ok_or_else(|| "font face bytes missing".into())
}

fn coretext_png(
    output: &Path,
    index: usize,
    font_bytes: &[u8],
    buffer: &harfrust::Buffer,
    text: &str,
) -> Result<Value> {
    let width = CANVAS_WIDTH;
    let height = CANVAS_HEIGHT;
    let mut context = bitmap_context();
    let (glyphs, positions) = glyph_inputs(buffer);
    let rgba = render_coretext(&mut context, font_bytes, &glyphs, &positions)?;
    let path = output.join(format!("coretext-{index:02}.png"));
    let png_sha = write_png(&path, &rgba, width, height)?;
    Ok(json!({
        "path": path,
        "png_sha256": png_sha,
        "rgba_sha256": digest_hex(&rgba),
        "width": width,
        "height": height,
        "text": text,
    }))
}

fn glyph_inputs(buffer: &harfrust::Buffer) -> (Vec<u16>, Vec<CGPoint>) {
    let glyphs = buffer
        .glyph_infos()
        .iter()
        .map(|glyph| glyph.glyph_id as u16)
        .collect::<Vec<_>>();
    (glyphs, glyph_positions(buffer))
}

fn render_coretext(
    context: &mut CGContext,
    font_bytes: &[u8],
    glyphs: &[u16],
    positions: &[CGPoint],
) -> Result<Vec<u8>> {
    if CORETEXT_POINT_SIZE != f64::from(DIAGNOSTIC_FONT_SIZE) {
        return Err("CoreText diagnostic size mismatch".into());
    }
    let descriptor = create_font_descriptor(font_bytes)
        .map_err(|_| "CoreText font descriptor creation failed")?;
    let font = new_from_descriptor(&descriptor, CORETEXT_POINT_SIZE);
    font.draw_glyphs(glyphs, positions, context.clone());
    Ok(context.data().to_vec())
}

fn bitmap_context() -> CGContext {
    let color_space = CGColorSpace::create_device_rgb();
    let context = CGContext::create_bitmap_context(
        None,
        CANVAS_WIDTH as usize,
        CANVAS_HEIGHT as usize,
        BITS_PER_COMPONENT,
        CANVAS_WIDTH as usize * RGBA_CHANNEL_COUNT,
        &color_space,
        kCGImageAlphaPremultipliedLast,
    );
    context.set_allows_font_smoothing(true);
    context.set_should_smooth_fonts(true);
    context.set_allows_antialiasing(true);
    context.set_should_antialias(true);
    context.set_allows_font_subpixel_positioning(true);
    context.set_should_subpixel_position_fonts(true);
    context.set_rgb_fill_color(0.0, 0.0, 0.0, 1.0);
    /* WHY: CoreGraphicsはy-upなので、SVGの上原点baselineを反転して渡す。 */
    context
}

fn glyph_positions(buffer: &harfrust::Buffer) -> Vec<CGPoint> {
    let mut x = 0.0_f64;
    buffer
        .glyph_positions()
        .iter()
        .map(|position| {
            let point = CGPoint::new(
                x + f64::from(position.x_offset) / f64::from(HARFRUST_SCALE),
                f64::from(CANVAS_HEIGHT) - BASELINE_Y
                    + f64::from(position.y_offset) / f64::from(HARFRUST_SCALE),
            );
            x += f64::from(position.x_advance) / f64::from(HARFRUST_SCALE);
            point
        })
        .collect()
}

fn krr_png(output: &Path, index: usize, svg: &str, options: &usvg::Options) -> Result<Value> {
    let tree = usvg::Tree::from_str(svg, options)?;
    let mut pixmap =
        tiny_skia::Pixmap::new(CANVAS_WIDTH, CANVAS_HEIGHT).ok_or("pixmap unavailable")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap.data().to_vec();
    let path = output.join(format!("krr-{index:02}.png"));
    let png_sha = write_png(&path, &rgba, CANVAS_WIDTH, CANVAS_HEIGHT)?;
    Ok(json!({
        "path": path,
        "png_sha256": png_sha,
        "rgba_sha256": digest_hex(&rgba),
        "width": CANVAS_WIDTH,
        "height": CANVAS_HEIGHT,
    }))
}

fn write_png(path: &Path, rgba: &[u8], width: u32, height: u32) -> Result<String> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file)).write_image(
        rgba,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(digest_hex(&std::fs::read(path)?))
}
